use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState, anti_abuse,
    auth::AuthUser,
    error::AppError,
    notifications,
    reconsiderations::resolve_cleared_reconsiderations,
    review_actions::{
        ensure_archive_voting_unlocked, ensure_review_unlocked, ensure_voting_unlocked,
    },
};

#[derive(Debug, Deserialize)]
pub struct CastSentimentVoteRequest {
    pub vote_value: String,
}

#[derive(Debug, Deserialize)]
pub struct CastMergeVoteRequest {
    pub target_proposal_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct CastVoteResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub user_id: Uuid,
    pub sentiment_vote: Option<String>,
    pub merge_vote_present: bool,
    pub merge_target_proposal_id: Option<Uuid>,
}

struct VoteableProposalContext {
    board_code: String,
    primary_state: String,
}

pub async fn cast_sentiment_vote_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Path(proposal_id): Path<Uuid>,
    Json(payload): Json<CastSentimentVoteRequest>,
) -> Result<(StatusCode, Json<CastVoteResponse>), AppError> {
    auth_user.require_verified()?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let vote_value = payload.vote_value.trim().to_lowercase();

    if vote_value != "support"
        && vote_value != "not_a_fit"
        && vote_value != "unclear"
        && vote_value != "unsafe"
    {
        return Err(AppError::BadRequest(
            "vote_value must be one of: support, not_a_fit, unclear, unsafe.".to_string(),
        ));
    }

    let proposal = get_voteable_proposal_context(&state.db, proposal_id).await?;
    if proposal.primary_state == "archived" {
        ensure_archive_voting_unlocked(&state.db, auth_user.user_id, Some(&proposal.board_code))
            .await?;
    } else {
        ensure_voting_unlocked(&state.db, auth_user.user_id, Some(&proposal.board_code)).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO proposal_sentiment_votes (proposal_id, user_id, vote_value)
        VALUES ($1, $2, $3)
        ON CONFLICT (proposal_id, user_id)
        DO UPDATE SET
            vote_value = EXCLUDED.vote_value,
            updated_at = NOW()
        "#,
    )
    .bind(proposal_id)
    .bind(auth_user.user_id)
    .bind(&vote_value)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error upserting sentiment vote: {}", err);
        AppError::Internal("Failed to save vote.".to_string())
    })?;

    refresh_proposal_vote_counts(&state.db, proposal_id).await?;
    anti_abuse::record_user_activity(
        &state.db,
        auth_user.user_id,
        "sentiment_vote",
        Some(proposal_id),
        None,
        &headers,
        json!({
            "vote_value": vote_value,
            "board_code": proposal.board_code,
            "proposal_state": proposal.primary_state
        }),
    )
    .await?;

    let merge_vote_present = has_merge_vote(&state.db, proposal_id, auth_user.user_id).await?;
    let merge_target_proposal_id =
        get_merge_vote_target(&state.db, proposal_id, auth_user.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(CastVoteResponse {
            ok: true,
            proposal_id,
            user_id: auth_user.user_id,
            sentiment_vote: Some(vote_value),
            merge_vote_present,
            merge_target_proposal_id,
        }),
    ))
}

pub async fn cast_merge_vote_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Path(proposal_id): Path<Uuid>,
    payload: Option<Json<CastMergeVoteRequest>>,
) -> Result<(StatusCode, Json<CastVoteResponse>), AppError> {
    auth_user.require_verified()?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let target_proposal_id = payload
        .and_then(|Json(payload)| payload.target_proposal_id)
        .ok_or_else(|| {
            AppError::BadRequest("target_proposal_id is required for merge signaling.".to_string())
        })?;

    if target_proposal_id == proposal_id {
        return Err(AppError::BadRequest(
            "target_proposal_id must be different from proposal_id.".to_string(),
        ));
    }

    let proposal = get_voteable_proposal_context(&state.db, proposal_id).await?;
    if proposal.primary_state == "archived" {
        return Err(AppError::BadRequest(
            "Merge signaling is only available for active proposals.".to_string(),
        ));
    }
    ensure_review_unlocked(&state.db, auth_user.user_id, Some(&proposal.board_code)).await?;

    validate_merge_target(&state.db, proposal_id, target_proposal_id).await?;

    sqlx::query(
        r#"
        INSERT INTO proposal_merge_votes (proposal_id, user_id, target_proposal_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (proposal_id, user_id)
        DO UPDATE SET
            target_proposal_id = COALESCE(
                EXCLUDED.target_proposal_id,
                proposal_merge_votes.target_proposal_id
            ),
            updated_at = NOW()
        "#,
    )
    .bind(proposal_id)
    .bind(auth_user.user_id)
    .bind(target_proposal_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error inserting merge vote: {}", err);
        AppError::Internal("Failed to save merge vote.".to_string())
    })?;

    upsert_merge_relationship(
        &state.db,
        proposal_id,
        target_proposal_id,
        auth_user.user_id,
    )
    .await?;
    deactivate_stale_merge_relationships(&state.db, proposal_id, target_proposal_id).await?;

    refresh_proposal_vote_counts(&state.db, proposal_id).await?;
    notifications::record_merge_watch_notifications(&state.db, proposal_id, target_proposal_id)
        .await?;
    anti_abuse::record_user_activity(
        &state.db,
        auth_user.user_id,
        "merge_vote",
        Some(proposal_id),
        Some(target_proposal_id),
        &headers,
        json!({
            "board_code": proposal.board_code
        }),
    )
    .await?;

    let sentiment_vote = get_sentiment_vote(&state.db, proposal_id, auth_user.user_id).await?;
    let merge_target_proposal_id =
        get_merge_vote_target(&state.db, proposal_id, auth_user.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(CastVoteResponse {
            ok: true,
            proposal_id,
            user_id: auth_user.user_id,
            sentiment_vote,
            merge_vote_present: true,
            merge_target_proposal_id,
        }),
    ))
}

async fn get_voteable_proposal_context(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
) -> Result<VoteableProposalContext, AppError> {
    let proposal = sqlx::query(
        r#"
        SELECT
            b.code AS board_code,
            p.primary_state
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = 'world'
          AND b.code IN ('issue', 'solution')
          AND p.primary_state IN ('active', 'archived')
          AND (
                p.primary_state = 'archived'
                OR (
                    c.is_active = TRUE
                    AND
                    NOT EXISTS (
                        SELECT 1
                        FROM proposal_watch_flags wf
                        WHERE wf.proposal_id = p.id
                          AND wf.flag_code = 'frozen_for_review'
                          AND wf.cleared_at IS NULL
                    )
                    AND NOT EXISTS (
                        SELECT 1
                        FROM reconsideration_windows rw
                        WHERE rw.proposal_id = p.id
                          AND rw.status = 'open'
                          AND rw.ends_at <= NOW()
                          AND (
                            p.unsafe_count >= 8
                            OR (
                                (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                                AND p.unsafe_count::numeric
                                    / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.35
                            )
                          )
                    )
                )
              )
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error checking proposal existence: {}", err);
        AppError::Internal("Failed to process vote.".to_string())
    })?;

    let Some(proposal) = proposal else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    Ok(VoteableProposalContext {
        board_code: proposal.try_get("board_code").map_err(internal_db_err)?,
        primary_state: proposal.try_get("primary_state").map_err(internal_db_err)?,
    })
}

async fn validate_merge_target(
    db: &sqlx::PgPool,
    source_proposal_id: Uuid,
    target_proposal_id: Uuid,
) -> Result<(), AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            sb.code AS source_board_code,
            tb.code AS target_board_code,
            sp.cycle_id AS source_cycle_id,
            tp.cycle_id AS target_cycle_id,
            sp.locale_id AS source_locale_id,
            tp.locale_id AS target_locale_id,
            tp.primary_state AS target_primary_state
        FROM proposals sp
        JOIN proposals tp ON tp.id = $2
        JOIN boards sb ON sb.id = sp.board_id
        JOIN boards tb ON tb.id = tp.board_id
        WHERE sp.id = $1
        LIMIT 1
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error validating merge target: {}", err);
        AppError::Internal("Failed to validate merge target.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Merge target proposal not found.".to_string(),
        ));
    };

    let source_board_code: String = row.try_get("source_board_code").map_err(internal_db_err)?;
    let target_board_code: String = row.try_get("target_board_code").map_err(internal_db_err)?;
    let source_cycle_id: Uuid = row.try_get("source_cycle_id").map_err(internal_db_err)?;
    let target_cycle_id: Uuid = row.try_get("target_cycle_id").map_err(internal_db_err)?;
    let source_locale_id: Uuid = row.try_get("source_locale_id").map_err(internal_db_err)?;
    let target_locale_id: Uuid = row.try_get("target_locale_id").map_err(internal_db_err)?;
    let target_primary_state: String = row
        .try_get("target_primary_state")
        .map_err(internal_db_err)?;

    if source_board_code != target_board_code
        || source_cycle_id != target_cycle_id
        || source_locale_id != target_locale_id
    {
        return Err(AppError::BadRequest(
            "Merge target must be in the same board, cycle, and locale.".to_string(),
        ));
    }

    if target_primary_state != "active" {
        return Err(AppError::BadRequest(
            "Merge target must be an active proposal.".to_string(),
        ));
    }

    Ok(())
}

async fn upsert_merge_relationship(
    db: &sqlx::PgPool,
    source_proposal_id: Uuid,
    target_proposal_id: Uuid,
    created_by_user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO proposal_merge_relationships (
            source_proposal_id,
            target_proposal_id,
            created_by_user_id
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (source_proposal_id, target_proposal_id)
        DO UPDATE SET
            status = 'active',
            updated_at = NOW()
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .bind(created_by_user_id)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error upserting merge relationship: {}", err);
        AppError::Internal("Failed to save merge relationship.".to_string())
    })?;

    Ok(())
}

async fn deactivate_stale_merge_relationships(
    db: &sqlx::PgPool,
    source_proposal_id: Uuid,
    current_target_proposal_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE proposal_merge_relationships r
        SET status = 'inactive',
            updated_at = NOW()
        WHERE r.source_proposal_id = $1
          AND r.target_proposal_id <> $2
          AND r.status = 'active'
          AND NOT EXISTS (
              SELECT 1
              FROM proposal_merge_votes mv
              WHERE mv.proposal_id = r.source_proposal_id
                AND mv.target_proposal_id = r.target_proposal_id
          )
        "#,
    )
    .bind(source_proposal_id)
    .bind(current_target_proposal_id)
    .execute(db)
    .await
    .map_err(|err| {
        error!(
            "database error deactivating stale merge relationships: {}",
            err
        );
        AppError::Internal("Failed to update merge relationships.".to_string())
    })?;

    Ok(())
}

pub(crate) async fn refresh_proposal_vote_counts(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        WITH counts AS (
            SELECT
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'support'
                ) AS support_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'not_a_fit'
                ) AS not_a_fit_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'unclear'
                ) AS unclear_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_sentiment_votes
                    WHERE proposal_id = $1
                      AND vote_value = 'unsafe'
                ) AS unsafe_count,
                (
                    SELECT COUNT(*)::int
                    FROM proposal_merge_votes mv
                    WHERE mv.proposal_id = $1
                      AND mv.target_proposal_id IS NOT NULL
                      AND EXISTS (
                        SELECT 1
                        FROM proposals target
                        WHERE target.id = mv.target_proposal_id
                          AND target.primary_state = 'active'
                      )
                      AND EXISTS (
                        SELECT 1
                        FROM proposal_merge_relationships r
                        WHERE r.source_proposal_id = mv.proposal_id
                          AND r.target_proposal_id = mv.target_proposal_id
                          AND r.status = 'active'
                      )
                ) AS merge_count
        )
        UPDATE proposals p
        SET
            support_count = counts.support_count,
            not_a_fit_count = counts.not_a_fit_count,
            unclear_count = counts.unclear_count,
            unsafe_count = counts.unsafe_count,
            merge_count = counts.merge_count,
            high_moderation_watch_started_at = CASE
                WHEN counts.unsafe_count >= 8
                  OR (
                    (counts.support_count + counts.not_a_fit_count + counts.unclear_count + counts.unsafe_count + counts.merge_count) > 0
                    AND counts.unsafe_count::numeric
                        / (counts.support_count + counts.not_a_fit_count + counts.unclear_count + counts.unsafe_count + counts.merge_count)::numeric >= 0.35
                  )
                THEN COALESCE(p.high_moderation_watch_started_at, NOW())
                ELSE NULL
            END
        FROM counts
        WHERE p.id = $1
        "#,
    )
    .bind(proposal_id)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error refreshing proposal counts: {}", err);
        AppError::Internal("Failed to refresh vote counts.".to_string())
    })?;

    Ok(())
}

async fn has_merge_vote(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    user_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM proposal_merge_votes
            WHERE proposal_id = $1
              AND user_id = $2
        ) AS exists_flag
        "#,
    )
    .bind(proposal_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking merge vote: {}", err);
        AppError::Internal("Failed to read vote state.".to_string())
    })?;

    row.try_get("exists_flag").map_err(|err| {
        error!("row decode error: {}", err);
        AppError::Internal("Failed to read vote state.".to_string())
    })
}

async fn get_merge_vote_target(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT target_proposal_id
        FROM proposal_merge_votes
        WHERE proposal_id = $1
          AND user_id = $2
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error reading merge vote target: {}", err);
        AppError::Internal("Failed to read vote state.".to_string())
    })?;

    match row {
        Some(row) => row.try_get("target_proposal_id").map_err(|err| {
            error!("row decode error: {}", err);
            AppError::Internal("Failed to read vote state.".to_string())
        }),
        None => Ok(None),
    }
}

async fn get_sentiment_vote(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    user_id: Uuid,
) -> Result<Option<String>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT vote_value
        FROM proposal_sentiment_votes
        WHERE proposal_id = $1
          AND user_id = $2
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error reading sentiment vote: {}", err);
        AppError::Internal("Failed to read vote state.".to_string())
    })?;

    match row {
        Some(row) => row.try_get("vote_value").map(Some).map_err(|err| {
            error!("row decode error: {}", err);
            AppError::Internal("Failed to read vote state.".to_string())
        }),
        None => Ok(None),
    }
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read vote state.".to_string())
}

use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState, anti_abuse, auth::AuthUser, error::AppError, locale,
    reconsiderations::resolve_cleared_reconsiderations, votes::refresh_proposal_vote_counts,
};

#[derive(Debug, Deserialize)]
pub struct SubmitReviewActionRequest {
    pub proposal_id: Uuid,
    pub vote_value: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitReviewActionResponse {
    pub ok: bool,
    pub proposal_id: Uuid,
    pub credited: bool,
    pub completed_review_actions: i64,
    pub required_review_actions: i64,
    pub review_unlocked: bool,
    pub cycle_phase: String,
    pub submission_open: bool,
    pub voting_open: bool,
    pub submit_unlocked: bool,
    pub voting_unlocked: bool,
    pub archive_voting_unlocked: bool,
    pub sentiment_vote: String,
}

#[derive(Debug, Deserialize)]
pub struct UnlockStatusQuery {
    pub board_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UnlockStatusResponse {
    pub ok: bool,
    pub board_code: Option<String>,
    pub locale_name: String,
    pub completed_review_actions: i64,
    pub required_review_actions: i64,
    pub review_unlocked: bool,
    pub cycle_phase: String,
    pub submission_open: bool,
    pub voting_open: bool,
    pub starts_at: DateTime<Utc>,
    pub submission_ends_at: DateTime<Utc>,
    pub voting_ends_at: DateTime<Utc>,
    pub submit_unlocked: bool,
    pub voting_unlocked: bool,
    pub archive_voting_unlocked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnlockState {
    pub locale_name: String,
    pub completed_review_actions: i64,
    pub required_review_actions: i64,
    pub review_unlocked: bool,
    pub cycle_phase: String,
    pub submission_open: bool,
    pub voting_open: bool,
    pub starts_at: DateTime<Utc>,
    pub submission_ends_at: DateTime<Utc>,
    pub voting_ends_at: DateTime<Utc>,
    pub submit_unlocked: bool,
    pub voting_unlocked: bool,
    pub archive_voting_unlocked: bool,
}

pub async fn submit_review_action_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Json(payload): Json<SubmitReviewActionRequest>,
) -> Result<(StatusCode, Json<SubmitReviewActionResponse>), AppError> {
    auth_user.require_verified()?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let vote_value = normalize_review_vote(&payload.vote_value)?;
    let active_cycle = get_active_cycle(&state.db).await?;

    let proposal = sqlx::query(
        r#"
        SELECT
            p.id,
            p.author_user_id,
            b.code AS board_code,
            p.primary_state,
            p.cycle_id,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            EXISTS (
                SELECT 1
                FROM proposal_watch_flags wf
                WHERE wf.proposal_id = p.id
                  AND wf.flag_code = 'frozen_for_review'
                  AND wf.cleared_at IS NULL
            ) AS frozen_for_review,
            EXISTS (
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
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
            ) AS reconsideration_moderation_due
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        WHERE p.id = $1
        LIMIT 1
        "#,
    )
    .bind(payload.proposal_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading proposal for review action: {}", err);
        AppError::Internal("Failed to submit review action.".to_string())
    })?;

    let Some(proposal) = proposal else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let proposal_cycle_id: Uuid = proposal.try_get("cycle_id").map_err(internal_db_err)?;
    let proposal_state: String = proposal.try_get("primary_state").map_err(internal_db_err)?;
    let proposal_board_code: String = proposal.try_get("board_code").map_err(internal_db_err)?;
    let proposal_author_user_id: Uuid = proposal
        .try_get("author_user_id")
        .map_err(internal_db_err)?;

    if proposal_cycle_id != active_cycle.cycle_id {
        return Err(AppError::BadRequest(
            "Proposal is not in the active cycle.".to_string(),
        ));
    }

    if proposal_author_user_id == auth_user.user_id {
        return Err(AppError::BadRequest(
            "Your own proposal cannot count for required review credit.".to_string(),
        ));
    }

    if proposal_state != "active" {
        return Err(AppError::BadRequest(
            "Only active proposals can count for review actions.".to_string(),
        ));
    }

    let frozen_for_review: bool = proposal
        .try_get("frozen_for_review")
        .map_err(internal_db_err)?;
    if frozen_for_review {
        return Err(AppError::BadRequest(
            "Frozen proposals cannot count for review actions.".to_string(),
        ));
    }

    let reconsideration_moderation_due: bool = proposal
        .try_get("reconsideration_moderation_due")
        .map_err(internal_db_err)?;
    if reconsideration_moderation_due {
        return Err(AppError::BadRequest(
            "This proposal has returned to moderator review after reconsideration.".to_string(),
        ));
    }

    if proposal_board_code != "issue" && proposal_board_code != "solution" {
        return Err(AppError::BadRequest(
            "Only issue or solution proposals can count for review actions.".to_string(),
        ));
    }

    let support_count: i32 = proposal.try_get("support_count").map_err(internal_db_err)?;
    let not_a_fit_count: i32 = proposal
        .try_get("not_a_fit_count")
        .map_err(internal_db_err)?;
    let unclear_count: i32 = proposal.try_get("unclear_count").map_err(internal_db_err)?;
    let unsafe_count: i32 = proposal.try_get("unsafe_count").map_err(internal_db_err)?;
    let merge_count: i32 = proposal.try_get("merge_count").map_err(internal_db_err)?;

    if excluded_from_review_credit(
        support_count,
        not_a_fit_count,
        unclear_count,
        unsafe_count,
        merge_count,
    ) {
        return Err(AppError::BadRequest(
            "This proposal is not eligible for review-unlock credit.".to_string(),
        ));
    }

    let current_unlock_state =
        compute_unlock_state(&state.db, auth_user.user_id, Some(&proposal_board_code)).await?;
    if current_unlock_state.review_unlocked {
        return Err(AppError::BadRequest(
            "Required reviews are already complete for this board.".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(|err| {
        error!("database error starting review action transaction: {}", err);
        AppError::Internal("Failed to submit review action.".to_string())
    })?;

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
    .bind(payload.proposal_id)
    .bind(auth_user.user_id)
    .bind(&vote_value)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error saving review sentiment vote: {}", err);
        AppError::Internal("Failed to submit review action.".to_string())
    })?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO review_actions (user_id, proposal_id, cycle_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, proposal_id, cycle_id)
        DO NOTHING
        RETURNING id
        "#,
    )
    .bind(auth_user.user_id)
    .bind(payload.proposal_id)
    .bind(active_cycle.cycle_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error inserting review action: {}", err);
        AppError::Internal("Failed to submit review action.".to_string())
    })?;

    let credited = insert_result.is_some();

    tx.commit().await.map_err(|err| {
        error!(
            "database error committing review action transaction: {}",
            err
        );
        AppError::Internal("Failed to submit review action.".to_string())
    })?;

    refresh_proposal_vote_counts(&state.db, payload.proposal_id).await?;
    anti_abuse::record_user_activity(
        &state.db,
        auth_user.user_id,
        "review_action",
        Some(payload.proposal_id),
        None,
        &headers,
        serde_json::json!({
            "vote_value": vote_value,
            "credited": credited,
            "board_code": proposal_board_code
        }),
    )
    .await?;

    let unlock_state =
        compute_unlock_state(&state.db, auth_user.user_id, Some(&proposal_board_code)).await?;

    Ok((
        StatusCode::OK,
        Json(SubmitReviewActionResponse {
            ok: true,
            proposal_id: payload.proposal_id,
            credited,
            completed_review_actions: unlock_state.completed_review_actions,
            required_review_actions: unlock_state.required_review_actions,
            review_unlocked: unlock_state.review_unlocked,
            cycle_phase: unlock_state.cycle_phase,
            submission_open: unlock_state.submission_open,
            voting_open: unlock_state.voting_open,
            submit_unlocked: unlock_state.submit_unlocked,
            voting_unlocked: unlock_state.voting_unlocked,
            archive_voting_unlocked: unlock_state.archive_voting_unlocked,
            sentiment_vote: vote_value,
        }),
    ))
}

fn normalize_review_vote(value: &str) -> Result<String, AppError> {
    let vote_value = value.trim().to_lowercase();

    if vote_value != "support"
        && vote_value != "not_a_fit"
        && vote_value != "unclear"
        && vote_value != "unsafe"
    {
        return Err(AppError::BadRequest(
            "vote_value must be one of: support, not_a_fit, unclear, unsafe.".to_string(),
        ));
    }

    Ok(vote_value)
}

pub async fn unlock_status_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<UnlockStatusQuery>,
) -> Result<Json<UnlockStatusResponse>, AppError> {
    resolve_cleared_reconsiderations(&state.db).await?;

    let board_code_filter = query.board_code.map(|v| v.trim().to_lowercase());

    if let Some(board_code) = board_code_filter.as_deref() {
        if board_code != "issue" && board_code != "solution" {
            return Err(AppError::BadRequest(
                "board_code must be 'issue' or 'solution'.".to_string(),
            ));
        }
    }

    let unlock_state =
        compute_unlock_state(&state.db, auth_user.user_id, board_code_filter.as_deref()).await?;

    Ok(Json(UnlockStatusResponse {
        ok: true,
        board_code: board_code_filter,
        locale_name: unlock_state.locale_name,
        completed_review_actions: unlock_state.completed_review_actions,
        required_review_actions: unlock_state.required_review_actions,
        review_unlocked: unlock_state.review_unlocked,
        cycle_phase: unlock_state.cycle_phase,
        submission_open: unlock_state.submission_open,
        voting_open: unlock_state.voting_open,
        starts_at: unlock_state.starts_at,
        submission_ends_at: unlock_state.submission_ends_at,
        voting_ends_at: unlock_state.voting_ends_at,
        submit_unlocked: unlock_state.submit_unlocked,
        voting_unlocked: unlock_state.voting_unlocked,
        archive_voting_unlocked: unlock_state.archive_voting_unlocked,
    }))
}

pub async fn ensure_submit_unlocked(
    db: &sqlx::PgPool,
    user_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<(), AppError> {
    let unlock_state = compute_unlock_state(db, user_id, board_code_filter).await?;

    if !unlock_state.submission_open {
        return Err(AppError::BadRequest(format!(
            "Submission is closed for the current cycle: {}.",
            unlock_state.cycle_phase
        )));
    }

    if !unlock_state.review_unlocked {
        return Err(AppError::BadRequest(format!(
            "Submission is locked. Complete {}/{} required review actions first.",
            unlock_state.completed_review_actions, unlock_state.required_review_actions
        )));
    }

    Ok(())
}

pub async fn ensure_voting_unlocked(
    db: &sqlx::PgPool,
    user_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<(), AppError> {
    let unlock_state = compute_unlock_state(db, user_id, board_code_filter).await?;

    if !unlock_state.voting_open {
        return Err(AppError::BadRequest(format!(
            "Voting is closed for the current cycle: {}.",
            unlock_state.cycle_phase
        )));
    }

    if !unlock_state.review_unlocked {
        return Err(AppError::BadRequest(format!(
            "Voting is locked. Complete {}/{} required review actions first.",
            unlock_state.completed_review_actions, unlock_state.required_review_actions
        )));
    }

    Ok(())
}

pub async fn ensure_review_unlocked(
    db: &sqlx::PgPool,
    user_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<(), AppError> {
    let unlock_state = compute_unlock_state(db, user_id, board_code_filter).await?;

    if !unlock_state.review_unlocked {
        return Err(AppError::BadRequest(format!(
            "Merge signaling is locked. Complete {}/{} required review actions first.",
            unlock_state.completed_review_actions, unlock_state.required_review_actions
        )));
    }

    Ok(())
}

pub async fn ensure_archive_voting_unlocked(
    db: &sqlx::PgPool,
    user_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<(), AppError> {
    let unlock_state = compute_unlock_state(db, user_id, board_code_filter).await?;

    if !unlock_state.archive_voting_unlocked {
        return Err(AppError::BadRequest(format!(
            "Archive voting is locked. Complete {}/{} required review actions first.",
            unlock_state.completed_review_actions, unlock_state.required_review_actions
        )));
    }

    Ok(())
}

pub async fn compute_unlock_state(
    db: &sqlx::PgPool,
    user_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<UnlockState, AppError> {
    let active_cycle = get_active_cycle(db).await?;

    let completed_review_actions =
        get_completed_review_actions(db, user_id, active_cycle.cycle_id, board_code_filter).await?;

    let required_review_actions =
        get_required_review_actions(db, user_id, active_cycle.cycle_id, board_code_filter).await?;

    let available_or_completed = completed_review_actions + required_review_actions;
    let target_review_actions = available_or_completed.clamp(0, 4);
    let dynamically_unlocked = completed_review_actions >= target_review_actions;
    let persisted_unlocked =
        has_persisted_review_unlock(db, user_id, active_cycle.cycle_id, board_code_filter).await?;

    if dynamically_unlocked && !persisted_unlocked {
        persist_review_unlock(
            db,
            user_id,
            active_cycle.cycle_id,
            board_code_filter,
            completed_review_actions,
            target_review_actions,
        )
        .await?;
    }

    let review_unlocked = persisted_unlocked || dynamically_unlocked;
    let now = Utc::now();
    let cycle_open = now >= active_cycle.starts_at && now < active_cycle.voting_ends_at;
    let submission_open = cycle_open;
    let voting_open = cycle_open;
    let cycle_phase = if now < active_cycle.starts_at {
        "pending"
    } else if cycle_open {
        "active"
    } else {
        "closed"
    }
    .to_string();

    Ok(UnlockState {
        locale_name: active_cycle.locale_name,
        completed_review_actions,
        required_review_actions: target_review_actions,
        review_unlocked,
        cycle_phase,
        submission_open,
        voting_open,
        starts_at: active_cycle.starts_at,
        submission_ends_at: active_cycle.submission_ends_at,
        voting_ends_at: active_cycle.voting_ends_at,
        submit_unlocked: review_unlocked && submission_open,
        voting_unlocked: review_unlocked && voting_open,
        archive_voting_unlocked: review_unlocked,
    })
}

async fn has_persisted_review_unlock(
    db: &sqlx::PgPool,
    user_id: Uuid,
    cycle_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<bool, AppError> {
    let Some(board_code) = board_code_filter else {
        return Ok(false);
    };

    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM review_unlocks
            WHERE user_id = $1
              AND cycle_id = $2
              AND board_code = $3
        ) AS unlocked
        "#,
    )
    .bind(user_id)
    .bind(cycle_id)
    .bind(board_code)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error loading persisted review unlock: {}", err);
        AppError::Internal("Failed to load review progress.".to_string())
    })?;

    row.try_get("unlocked").map_err(internal_db_err)
}

async fn persist_review_unlock(
    db: &sqlx::PgPool,
    user_id: Uuid,
    cycle_id: Uuid,
    board_code_filter: Option<&str>,
    completed_review_actions: i64,
    required_review_actions: i64,
) -> Result<(), AppError> {
    let Some(board_code) = board_code_filter else {
        return Ok(());
    };

    if board_code != "issue" && board_code != "solution" {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO review_unlocks (
            user_id,
            cycle_id,
            board_code,
            completed_review_actions,
            required_review_actions
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id, cycle_id, board_code)
        DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(cycle_id)
    .bind(board_code)
    .bind(completed_review_actions as i32)
    .bind(required_review_actions as i32)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error persisting review unlock: {}", err);
        AppError::Internal("Failed to save review progress.".to_string())
    })?;

    Ok(())
}

struct ActiveCycle {
    cycle_id: Uuid,
    locale_name: String,
    starts_at: DateTime<Utc>,
    submission_ends_at: DateTime<Utc>,
    voting_ends_at: DateTime<Utc>,
}

async fn get_active_cycle(db: &sqlx::PgPool) -> Result<ActiveCycle, AppError> {
    let locale_slug = locale::configured_locale_slug();
    let row = sqlx::query(
        r#"
        SELECT
            c.id AS cycle_id,
            l.name AS locale_name,
            c.starts_at,
            c.submission_ends_at,
            c.voting_ends_at
        FROM cycles c
        JOIN locales l ON l.id = c.locale_id
        WHERE l.slug = $1
          AND c.is_active = TRUE
        ORDER BY c.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&locale_slug)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error loading active cycle: {}", err);
        AppError::Internal("Failed to load active cycle.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::Internal("No active cycle exists.".to_string()));
    };

    Ok(ActiveCycle {
        cycle_id: row.try_get("cycle_id").map_err(internal_db_err)?,
        locale_name: row.try_get("locale_name").map_err(internal_db_err)?,
        starts_at: row.try_get("starts_at").map_err(internal_db_err)?,
        submission_ends_at: row.try_get("submission_ends_at").map_err(internal_db_err)?,
        voting_ends_at: row.try_get("voting_ends_at").map_err(internal_db_err)?,
    })
}

async fn get_completed_review_actions(
    db: &sqlx::PgPool,
    user_id: Uuid,
    cycle_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<i64, AppError> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS review_count
        FROM review_actions ra
        JOIN proposals p ON p.id = ra.proposal_id
        JOIN boards b ON b.id = p.board_id
        WHERE ra.user_id = $1
          AND ra.cycle_id = $2
          AND p.author_user_id <> $1
          AND ($3::text IS NULL OR b.code = $3)
        "#,
    )
    .bind(user_id)
    .bind(cycle_id)
    .bind(board_code_filter)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error counting review actions: {}", err);
        AppError::Internal("Failed to load review progress.".to_string())
    })?;

    row.try_get("review_count").map_err(internal_db_err)
}

async fn get_required_review_actions(
    db: &sqlx::PgPool,
    user_id: Uuid,
    cycle_id: Uuid,
    board_code_filter: Option<&str>,
) -> Result<i64, AppError> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS eligible_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        LEFT JOIN review_actions ra
            ON ra.proposal_id = p.id
           AND ra.user_id = $2
           AND ra.cycle_id = p.cycle_id
        LEFT JOIN proposal_sentiment_votes sv
            ON sv.proposal_id = p.id
           AND sv.user_id = $2
        LEFT JOIN proposal_merge_votes mv
            ON mv.proposal_id = p.id
           AND mv.user_id = $2
        WHERE p.cycle_id = $1
          AND p.primary_state = 'active'
          AND p.author_user_id <> $2
          AND b.code IN ('issue', 'solution')
          AND ($3::text IS NULL OR b.code = $3)
          AND ra.id IS NULL
          AND sv.id IS NULL
          AND mv.id IS NULL
          AND NOT EXISTS (
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
                            / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                    )
                  )
              )
          AND (
                p.not_a_fit_count + p.unclear_count + p.unsafe_count
              ) <= 8 * GREATEST(p.support_count, 1)
          AND p.unsafe_count < 8
          AND (
                (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) = 0
                OR (
                    p.unsafe_count::numeric
                    / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric
                ) < 0.50
              )
        "#,
    )
    .bind(cycle_id)
    .bind(user_id)
    .bind(board_code_filter)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error counting eligible review actions: {}", err);
        AppError::Internal("Failed to load unlock requirement.".to_string())
    })?;

    let eligible_count: i64 = row.try_get("eligible_count").map_err(internal_db_err)?;

    Ok(eligible_count.clamp(0, 4))
}

fn excluded_from_review_credit(
    support_count: i32,
    not_a_fit_count: i32,
    unclear_count: i32,
    unsafe_count: i32,
    merge_count: i32,
) -> bool {
    let negative_count = not_a_fit_count + unclear_count + unsafe_count;
    let total_count = support_count + negative_count + merge_count;

    negative_count > 8 * support_count.max(1)
        || unsafe_count >= 8
        || fraction_at_least(unsafe_count, total_count, 0.50)
}

fn fraction_at_least(part: i32, total: i32, threshold: f64) -> bool {
    total > 0 && (part as f64 / total as f64) >= threshold
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read review action data.".to_string())
}

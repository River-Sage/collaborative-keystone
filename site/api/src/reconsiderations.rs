use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{AppState, auth::AuthUser, error::AppError, locale};

const MAX_NOTE_CHARS: usize = 2000;

#[derive(Debug, Deserialize)]
pub struct StartReconsiderationRequest {
    pub start_reason: String,
    pub start_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StartReconsiderationResponse {
    pub ok: bool,
    pub reconsideration_id: Uuid,
    pub proposal_id: Uuid,
    pub status: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReconsiderationQueueResponse {
    pub ok: bool,
    pub reconsiderations: Vec<ReconsiderationQueueItem>,
}

#[derive(Debug, Serialize)]
pub struct ReconsiderationQueueItem {
    pub reconsideration_id: Uuid,
    pub proposal_id: Uuid,
    pub proposal_title: String,
    pub primary_state: String,
    pub start_reason: String,
    pub start_note: Option<String>,
    pub previous_archived_reason: Option<String>,
    pub status: String,
    pub review_due: bool,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveReconsiderationRequest {
    pub outcome: String,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResolveReconsiderationResponse {
    pub ok: bool,
    pub reconsideration_id: Uuid,
    pub proposal_id: Uuid,
    pub outcome: String,
    pub proposal_primary_state: String,
}

pub async fn start_reconsideration_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(proposal_id): Path<Uuid>,
    Json(payload): Json<StartReconsiderationRequest>,
) -> Result<(StatusCode, Json<StartReconsiderationResponse>), AppError> {
    require_moderator(&auth_user)?;

    let start_reason = payload.start_reason.trim().to_string();
    if start_reason.is_empty() {
        return Err(AppError::BadRequest(
            "start_reason is required.".to_string(),
        ));
    }

    if start_reason.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::BadRequest(
            "start_reason is too long.".to_string(),
        ));
    }

    let start_note = payload
        .start_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if start_note
        .as_ref()
        .map(|value| value.chars().count() > MAX_NOTE_CHARS)
        .unwrap_or(false)
    {
        return Err(AppError::BadRequest("start_note is too long.".to_string()));
    }

    let proposal = sqlx::query(
        r#"
        SELECT
            p.id,
            p.cycle_id,
            p.primary_state,
            p.archived_reason,
            p.moderation_note,
            (
                SELECT ma.moderator_user_id
                FROM moderator_actions ma
                WHERE ma.proposal_id = p.id
                  AND ma.action_type = 'archive'
                ORDER BY ma.created_at DESC
                LIMIT 1
            ) AS last_archive_moderator_user_id
        FROM proposals p
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = $2
          AND c.is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading reconsideration proposal: {}", err);
        AppError::Internal("Failed to start reconsideration.".to_string())
    })?;

    let Some(proposal) = proposal else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let primary_state: String = proposal.try_get("primary_state").map_err(internal_db_err)?;
    if primary_state != "archived" {
        return Err(AppError::BadRequest(
            "Only archived proposals can enter reconsideration.".to_string(),
        ));
    }

    let last_archive_moderator_user_id: Option<Uuid> = proposal
        .try_get("last_archive_moderator_user_id")
        .map_err(internal_db_err)?;
    let alternate_moderator_available =
        another_verified_moderator_available(&state.db, auth_user.user_id).await?;
    if last_archive_moderator_user_id == Some(auth_user.user_id) && alternate_moderator_available {
        return Err(AppError::Forbidden(
            "A different moderator must start reconsideration when another moderator is available."
                .to_string(),
        ));
    }

    let cycle_id: Uuid = proposal.try_get("cycle_id").map_err(internal_db_err)?;
    let previous_archived_reason: Option<String> = proposal
        .try_get("archived_reason")
        .map_err(internal_db_err)?;
    if previous_archived_reason.as_deref() == Some("cycle_closed") {
        return Err(AppError::BadRequest(
            "Cycle-closed proposals must be re-submitted as new proposals.".to_string(),
        ));
    }
    if previous_archived_reason.as_deref() == Some("merged") {
        return Err(AppError::BadRequest(
            "Merged proposals require a future merge reversal flow and cannot enter reconsideration in v1.".to_string(),
        ));
    }
    let previous_moderation_note: Option<String> = proposal
        .try_get("moderation_note")
        .map_err(internal_db_err)?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO reconsideration_windows (
            proposal_id,
            cycle_id,
            started_by_moderator_user_id,
            start_reason,
            start_note,
            previous_archived_reason,
            previous_moderation_note
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, status, starts_at, ends_at
        "#,
    )
    .bind(proposal_id)
    .bind(cycle_id)
    .bind(auth_user.user_id)
    .bind(&start_reason)
    .bind(&start_note)
    .bind(&previous_archived_reason)
    .bind(&previous_moderation_note)
    .fetch_one(&state.db)
    .await;

    let row = match insert_result {
        Ok(row) => row,
        Err(sqlx::Error::Database(db_err)) => {
            if db_err.constraint() == Some("reconsideration_windows_unique_proposal_cycle") {
                return Err(AppError::Conflict(
                    "This proposal has already entered reconsideration this cycle.".to_string(),
                ));
            }

            error!(
                "database error inserting reconsideration window: {}",
                db_err
            );
            return Err(AppError::Internal(
                "Failed to start reconsideration.".to_string(),
            ));
        }
        Err(err) => {
            error!("database error inserting reconsideration window: {}", err);
            return Err(AppError::Internal(
                "Failed to start reconsideration.".to_string(),
            ));
        }
    };

    let reconsideration_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
    let status: String = row.try_get("status").map_err(internal_db_err)?;
    let starts_at: DateTime<Utc> = row.try_get("starts_at").map_err(internal_db_err)?;
    let ends_at: DateTime<Utc> = row.try_get("ends_at").map_err(internal_db_err)?;

    sqlx::query(
        r#"
        UPDATE proposals
        SET
            primary_state = 'active',
            archived_reason = NULL,
            moderation_note = NULL,
            merged_into_proposal_id = NULL
        WHERE id = $1
          AND primary_state = 'archived'
        "#,
    )
    .bind(proposal_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error starting reconsideration: {}", err);
        AppError::Internal("Failed to start reconsideration.".to_string())
    })?;

    insert_moderator_action(
        &state.db,
        "reconsideration_start",
        proposal_id,
        None,
        Some(auth_user.user_id),
        Some(&start_reason),
        start_note.as_deref(),
        None,
        json!({
            "reconsideration_id": reconsideration_id,
            "previous_state": primary_state,
            "previous_archived_reason": previous_archived_reason,
            "previous_moderation_note_present": previous_moderation_note.is_some(),
            "starts_at": starts_at,
            "ends_at": ends_at
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(StartReconsiderationResponse {
            ok: true,
            reconsideration_id,
            proposal_id,
            status,
            starts_at,
            ends_at,
        }),
    ))
}

pub async fn reconsideration_review_queue_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ReconsiderationQueueResponse>, AppError> {
    require_moderator(&auth_user)?;
    resolve_cleared_reconsiderations(&state.db).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            r.id AS reconsideration_id,
            r.proposal_id,
            p.title AS proposal_title,
            p.primary_state,
            r.start_reason,
            r.start_note,
            r.previous_archived_reason,
            r.status,
            (r.ends_at <= NOW()) AS review_due,
            r.starts_at,
            r.ends_at
        FROM reconsideration_windows r
        JOIN proposals p ON p.id = r.proposal_id
        JOIN cycles c ON c.id = r.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE r.status = 'open'
          AND c.is_active = TRUE
          AND l.slug = $1
          AND r.ends_at <= NOW()
          AND (
            p.unsafe_count >= 8
            OR (
                (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                AND p.unsafe_count::numeric
                    / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
            )
          )
        ORDER BY review_due DESC, r.ends_at ASC
        "#,
    )
    .bind(&state.locale.slug)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading reconsideration queue: {}", err);
        AppError::Internal("Failed to load reconsideration queue.".to_string())
    })?;

    let reconsiderations = rows
        .into_iter()
        .map(map_reconsideration_queue_row)
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(ReconsiderationQueueResponse {
        ok: true,
        reconsiderations,
    }))
}

pub async fn resolve_reconsideration_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(reconsideration_id): Path<Uuid>,
    Json(payload): Json<ResolveReconsiderationRequest>,
) -> Result<(StatusCode, Json<ResolveReconsiderationResponse>), AppError> {
    require_moderator(&auth_user)?;

    let outcome = payload.outcome.trim().to_lowercase();
    if outcome != "restore_active" && outcome != "return_archive" && outcome != "freeze" {
        return Err(AppError::BadRequest(
            "outcome must be one of: restore_active, return_archive, freeze.".to_string(),
        ));
    }

    let resolution_note = payload
        .resolution_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if resolution_note
        .as_ref()
        .map(|value| value.chars().count() > MAX_NOTE_CHARS)
        .unwrap_or(false)
    {
        return Err(AppError::BadRequest(
            "resolution_note is too long.".to_string(),
        ));
    }

    let reconsideration = sqlx::query(
        r#"
        SELECT
            r.id,
            r.proposal_id,
            r.status,
            r.start_reason,
            r.previous_archived_reason,
            r.previous_moderation_note,
            r.ends_at,
            (r.ends_at <= NOW()) AS review_due,
            (
                p.unsafe_count >= 8
                OR (
                    (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                    AND p.unsafe_count::numeric
                        / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
                )
            ) AS still_high_moderation_watch,
            p.primary_state
        FROM reconsideration_windows r
        JOIN proposals p ON p.id = r.proposal_id
        JOIN locales l ON l.id = p.locale_id
        WHERE r.id = $1
          AND l.slug = $2
        LIMIT 1
        "#,
    )
    .bind(reconsideration_id)
    .bind(&state.locale.slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!(
            "database error loading reconsideration for resolution: {}",
            err
        );
        AppError::Internal("Failed to resolve reconsideration.".to_string())
    })?;

    let Some(reconsideration) = reconsideration else {
        return Err(AppError::BadRequest(
            "Reconsideration window not found.".to_string(),
        ));
    };

    let status: String = reconsideration.try_get("status").map_err(internal_db_err)?;
    if status != "open" {
        return Err(AppError::BadRequest(
            "Only open reconsideration windows can be resolved.".to_string(),
        ));
    }

    let review_due: bool = reconsideration
        .try_get("review_due")
        .map_err(internal_db_err)?;
    if !review_due {
        return Err(AppError::BadRequest(
            "The reconsideration window is still open for voting.".to_string(),
        ));
    }

    let still_high_moderation_watch: bool = reconsideration
        .try_get("still_high_moderation_watch")
        .map_err(internal_db_err)?;
    if !still_high_moderation_watch {
        return Err(AppError::BadRequest(
            "This proposal no longer meets the moderation threshold after reconsideration."
                .to_string(),
        ));
    }

    let proposal_id: Uuid = reconsideration
        .try_get("proposal_id")
        .map_err(internal_db_err)?;
    let previous_state: String = reconsideration
        .try_get("primary_state")
        .map_err(internal_db_err)?;
    let previous_archived_reason: Option<String> = reconsideration
        .try_get("previous_archived_reason")
        .map_err(internal_db_err)?;
    let previous_moderation_note: Option<String> = reconsideration
        .try_get("previous_moderation_note")
        .map_err(internal_db_err)?;
    let start_reason: String = reconsideration
        .try_get("start_reason")
        .map_err(internal_db_err)?;
    let ends_at: DateTime<Utc> = reconsideration
        .try_get("ends_at")
        .map_err(internal_db_err)?;

    if outcome == "restore_active" || outcome == "freeze" {
        if previous_archived_reason.as_deref() == Some("merged") {
            return Err(AppError::BadRequest(
                "Merged proposals require a future merge reversal flow and cannot be restored through reconsideration in v1.".to_string(),
            ));
        }
        if previous_archived_reason.as_deref() == Some("cycle_closed") {
            return Err(AppError::BadRequest(
                "Cycle-closed proposals must be re-submitted as new proposals.".to_string(),
            ));
        }
    }

    let proposal_primary_state = if outcome == "restore_active" {
        sqlx::query(
            r#"
            UPDATE proposals
            SET
                primary_state = 'active',
                archived_reason = NULL,
                moderation_note = NULL,
                merged_into_proposal_id = NULL
            WHERE id = $1
            "#,
        )
        .bind(proposal_id)
        .execute(&state.db)
        .await
        .map_err(|err| {
            error!("database error restoring reconsideration proposal: {}", err);
            AppError::Internal("Failed to resolve reconsideration.".to_string())
        })?;

        "active".to_string()
    } else if outcome == "return_archive" {
        let archived_reason = previous_archived_reason
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("manual_archive");
        let moderation_note = resolution_note
            .as_ref()
            .or(previous_moderation_note.as_ref());

        sqlx::query(
            r#"
            UPDATE proposals
            SET
                primary_state = 'archived',
                archived_reason = $2,
                moderation_note = $3,
                merged_into_proposal_id = NULL
            WHERE id = $1
            "#,
        )
        .bind(proposal_id)
        .bind(archived_reason)
        .bind(moderation_note)
        .execute(&state.db)
        .await
        .map_err(|err| {
            error!(
                "database error returning reconsideration proposal to archive: {}",
                err
            );
            AppError::Internal("Failed to resolve reconsideration.".to_string())
        })?;

        "archived".to_string()
    } else {
        let moderation_note = resolution_note
            .as_ref()
            .or(previous_moderation_note.as_ref());

        sqlx::query(
            r#"
            UPDATE proposals
            SET
                primary_state = 'active',
                archived_reason = NULL,
                moderation_note = $2,
                merged_into_proposal_id = NULL
            WHERE id = $1
            "#,
        )
        .bind(proposal_id)
        .bind(moderation_note)
        .execute(&state.db)
        .await
        .map_err(|err| {
            error!("database error freezing reconsideration proposal: {}", err);
            AppError::Internal("Failed to resolve reconsideration.".to_string())
        })?;

        insert_frozen_for_review_flag(
            &state.db,
            proposal_id,
            Some(auth_user.user_id),
            Some("reconsideration_needs_review"),
        )
        .await?;

        "active".to_string()
    };

    sqlx::query(
        r#"
        UPDATE reconsideration_windows
        SET
            status = 'resolved',
            outcome = $2,
            resolved_by_moderator_user_id = $3,
            resolution_note = $4,
            resolved_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(reconsideration_id)
    .bind(&outcome)
    .bind(auth_user.user_id)
    .bind(&resolution_note)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!(
            "database error updating reconsideration resolution: {}",
            err
        );
        AppError::Internal("Failed to resolve reconsideration.".to_string())
    })?;

    insert_moderator_action(
        &state.db,
        "reconsideration_end",
        proposal_id,
        None,
        Some(auth_user.user_id),
        Some(&outcome),
        resolution_note.as_deref(),
        None,
        json!({
            "reconsideration_id": reconsideration_id,
            "start_reason": start_reason,
            "previous_state": previous_state,
            "result_state": proposal_primary_state,
            "window_ended_at": ends_at
        }),
    )
    .await?;

    if outcome == "restore_active" {
        insert_moderator_action(
            &state.db,
            "unarchive",
            proposal_id,
            None,
            Some(auth_user.user_id),
            Some("reconsideration_restored"),
            resolution_note.as_deref(),
            None,
            json!({
                "reconsideration_id": reconsideration_id,
                "previous_archived_reason": previous_archived_reason
            }),
        )
        .await?;
    } else if outcome == "freeze" {
        insert_moderator_action(
            &state.db,
            "freeze",
            proposal_id,
            None,
            Some(auth_user.user_id),
            Some("reconsideration_needs_review"),
            resolution_note.as_deref(),
            None,
            json!({
                "reconsideration_id": reconsideration_id,
                "previous_archived_reason": previous_archived_reason
            }),
        )
        .await?;
    }

    Ok((
        StatusCode::OK,
        Json(ResolveReconsiderationResponse {
            ok: true,
            reconsideration_id,
            proposal_id,
            outcome,
            proposal_primary_state,
        }),
    ))
}

pub async fn resolve_cleared_reconsiderations(db: &sqlx::PgPool) -> Result<(), AppError> {
    let locale_slug = locale::configured_locale_slug();
    let rows = sqlx::query(
        r#"
        SELECT
            r.id AS reconsideration_id,
            r.proposal_id,
            r.start_reason,
            r.ends_at,
            p.primary_state,
            r.previous_archived_reason,
            r.previous_moderation_note
        FROM reconsideration_windows r
        JOIN proposals p ON p.id = r.proposal_id
        JOIN cycles c ON c.id = r.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE r.status = 'open'
          AND r.ends_at <= NOW()
          AND c.is_active = TRUE
          AND l.slug = $1
          AND COALESCE(r.previous_archived_reason, '') NOT IN ('merged', 'cycle_closed')
          AND NOT (
            p.unsafe_count >= 8
            OR (
                (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count) > 0
                AND p.unsafe_count::numeric
                    / (p.support_count + p.not_a_fit_count + p.unclear_count + p.unsafe_count + p.merge_count)::numeric >= 0.50
            )
          )
        "#,
    )
    .bind(&locale_slug)
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading cleared reconsideration windows: {}",
            err
        );
        AppError::Internal("Failed to resolve cleared reconsiderations.".to_string())
    })?;

    for row in rows {
        let reconsideration_id: Uuid =
            row.try_get("reconsideration_id").map_err(internal_db_err)?;
        let proposal_id: Uuid = row.try_get("proposal_id").map_err(internal_db_err)?;
        let start_reason: String = row.try_get("start_reason").map_err(internal_db_err)?;
        let previous_state: String = row.try_get("primary_state").map_err(internal_db_err)?;
        let previous_archived_reason: Option<String> = row
            .try_get("previous_archived_reason")
            .map_err(internal_db_err)?;
        let previous_moderation_note: Option<String> = row
            .try_get("previous_moderation_note")
            .map_err(internal_db_err)?;
        let ends_at: DateTime<Utc> = row.try_get("ends_at").map_err(internal_db_err)?;

        let update_result = sqlx::query(
            r#"
            UPDATE reconsideration_windows
            SET
                status = 'resolved',
                outcome = 'restore_active',
                resolved_by_moderator_user_id = NULL,
                resolution_note = 'Automatically restored because the reconsideration threshold cleared.',
                resolved_at = NOW()
            WHERE id = $1
              AND status = 'open'
            "#,
        )
        .bind(reconsideration_id)
        .execute(db)
        .await
        .map_err(|err| {
            error!(
                "database error auto-resolving reconsideration window: {}",
                err
            );
            AppError::Internal("Failed to resolve cleared reconsideration.".to_string())
        })?;

        if update_result.rows_affected() == 0 {
            continue;
        }

        sqlx::query(
            r#"
            UPDATE proposals
            SET
                primary_state = 'active',
                archived_reason = NULL,
                moderation_note = NULL,
                merged_into_proposal_id = NULL
            WHERE id = $1
            "#,
        )
        .bind(proposal_id)
        .execute(db)
        .await
        .map_err(|err| {
            error!(
                "database error restoring cleared reconsideration proposal: {}",
                err
            );
            AppError::Internal("Failed to restore cleared reconsideration.".to_string())
        })?;

        let state_snapshot = json!({
            "reconsideration_id": reconsideration_id,
            "start_reason": start_reason,
            "previous_state": previous_state,
            "previous_archived_reason": previous_archived_reason,
            "previous_moderation_note_present": previous_moderation_note.is_some(),
            "result_state": "active",
            "window_ended_at": ends_at,
            "auto_resolved": true,
            "threshold_cleared": true
        });

        insert_moderator_action(
            db,
            "reconsideration_end",
            proposal_id,
            None,
            None,
            Some("auto_restored_threshold_cleared"),
            Some("Automatically restored because the reconsideration threshold cleared."),
            None,
            state_snapshot.clone(),
        )
        .await?;

        insert_moderator_action(
            db,
            "unarchive",
            proposal_id,
            None,
            None,
            Some("reconsideration_threshold_cleared"),
            Some("Automatically restored after reconsideration."),
            None,
            state_snapshot,
        )
        .await?;
    }

    Ok(())
}

fn map_reconsideration_queue_row(
    row: sqlx::postgres::PgRow,
) -> Result<ReconsiderationQueueItem, AppError> {
    Ok(ReconsiderationQueueItem {
        reconsideration_id: row.try_get("reconsideration_id").map_err(internal_db_err)?,
        proposal_id: row.try_get("proposal_id").map_err(internal_db_err)?,
        proposal_title: row.try_get("proposal_title").map_err(internal_db_err)?,
        primary_state: row.try_get("primary_state").map_err(internal_db_err)?,
        start_reason: row.try_get("start_reason").map_err(internal_db_err)?,
        start_note: row.try_get("start_note").map_err(internal_db_err)?,
        previous_archived_reason: row
            .try_get("previous_archived_reason")
            .map_err(internal_db_err)?,
        status: row.try_get("status").map_err(internal_db_err)?,
        review_due: row.try_get("review_due").map_err(internal_db_err)?,
        starts_at: row.try_get("starts_at").map_err(internal_db_err)?,
        ends_at: row.try_get("ends_at").map_err(internal_db_err)?,
    })
}

async fn insert_frozen_for_review_flag(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
    moderator_user_id: Option<Uuid>,
    reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO proposal_watch_flags (
            proposal_id,
            flag_code,
            created_by_moderator_user_id,
            reason
        )
        VALUES ($1, 'frozen_for_review', $2, $3)
        ON CONFLICT (proposal_id, flag_code)
        WHERE cleared_at IS NULL
        DO UPDATE SET
            created_by_moderator_user_id = EXCLUDED.created_by_moderator_user_id,
            reason = EXCLUDED.reason
        "#,
    )
    .bind(proposal_id)
    .bind(moderator_user_id)
    .bind(reason)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting frozen watch flag: {}", err);
        AppError::Internal("Failed to freeze proposal.".to_string())
    })?;

    Ok(())
}

fn require_moderator(auth_user: &AuthUser) -> Result<(), AppError> {
    if auth_user.can_moderate() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Moderator privileges are required for this action.".to_string(),
        ))
    }
}

async fn another_verified_moderator_available(
    db: &sqlx::PgPool,
    current_user_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM users
            WHERE id <> $1
              AND role_code = 'moderator'
              AND email_verified = TRUE
        ) AS available
        "#,
    )
    .bind(current_user_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking alternate moderator: {}", err);
        AppError::Internal("Failed to start reconsideration.".to_string())
    })?;

    row.try_get("available").map_err(internal_db_err)
}

async fn insert_moderator_action(
    db: &sqlx::PgPool,
    action_type: &str,
    proposal_id: Uuid,
    related_proposal_id: Option<Uuid>,
    moderator_user_id: Option<Uuid>,
    action_reason: Option<&str>,
    public_note: Option<&str>,
    internal_note: Option<&str>,
    state_snapshot: Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO moderator_actions (
            action_type,
            proposal_id,
            related_proposal_id,
            moderator_user_id,
            action_reason,
            public_note,
            internal_note,
            state_snapshot
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(action_type)
    .bind(proposal_id)
    .bind(related_proposal_id)
    .bind(moderator_user_id)
    .bind(action_reason)
    .bind(public_note)
    .bind(internal_note)
    .bind(state_snapshot)
    .execute(db)
    .await
    .map_err(|err| {
        error!(
            "database error inserting reconsideration audit action: {}",
            err
        );
        AppError::Internal("Failed to log reconsideration action.".to_string())
    })?;

    Ok(())
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read reconsideration data.".to_string())
}

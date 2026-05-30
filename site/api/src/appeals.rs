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

use crate::{AppState, auth::AuthUser, error::AppError};

const MAX_NOTE_CHARS: usize = 2000;

#[derive(Debug, Deserialize)]
pub struct SubmitAppealRequest {
    pub appeal_reason: String,
    pub clarification_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitAppealResponse {
    pub ok: bool,
    pub appeal_id: Uuid,
    pub proposal_id: Uuid,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AppealQueueResponse {
    pub ok: bool,
    pub appeals: Vec<AppealQueueItem>,
}

#[derive(Debug, Serialize)]
pub struct AppealQueueItem {
    pub appeal_id: Uuid,
    pub proposal_id: Uuid,
    pub proposal_title: String,
    pub author_user_id: Uuid,
    pub appeal_reason: String,
    pub clarification_note: Option<String>,
    pub status: String,
    pub archived_reason: Option<String>,
    pub last_archive_moderator_user_id: Option<Uuid>,
    pub current_moderator_must_recuse: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveAppealRequest {
    pub outcome: String,
    pub moderator_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResolveAppealResponse {
    pub ok: bool,
    pub appeal_id: Uuid,
    pub proposal_id: Uuid,
    pub outcome: String,
    pub appeal_status: String,
    pub proposal_restored: bool,
}

pub async fn submit_appeal_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(proposal_id): Path<Uuid>,
    Json(payload): Json<SubmitAppealRequest>,
) -> Result<(StatusCode, Json<SubmitAppealResponse>), AppError> {
    auth_user.require_verified()?;

    let appeal_reason = payload.appeal_reason.trim().to_string();

    if appeal_reason.is_empty() {
        return Err(AppError::BadRequest(
            "appeal_reason is required.".to_string(),
        ));
    }

    if appeal_reason.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::BadRequest(
            "appeal_reason is too long.".to_string(),
        ));
    }

    let clarification_note = payload
        .clarification_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if clarification_note
        .as_ref()
        .map(|value| value.chars().count() > MAX_NOTE_CHARS)
        .unwrap_or(false)
    {
        return Err(AppError::BadRequest(
            "clarification_note is too long.".to_string(),
        ));
    }

    let proposal = sqlx::query(
        r#"
        SELECT p.id, p.author_user_id, p.primary_state, p.archived_reason, p.cycle_id
        FROM proposals p
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = 'world'
          AND c.is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading appeal proposal: {}", err);
        AppError::Internal("Failed to submit appeal.".to_string())
    })?;

    let Some(proposal) = proposal else {
        return Err(AppError::BadRequest("Proposal not found.".to_string()));
    };

    let author_user_id: Uuid = proposal
        .try_get("author_user_id")
        .map_err(internal_db_err)?;
    if author_user_id != auth_user.user_id {
        return Err(AppError::Forbidden(
            "Only the proposal author can submit an appeal.".to_string(),
        ));
    }

    let primary_state: String = proposal.try_get("primary_state").map_err(internal_db_err)?;
    if primary_state != "archived" {
        return Err(AppError::BadRequest(
            "Only archived proposals can be appealed.".to_string(),
        ));
    }

    let cycle_id: Uuid = proposal.try_get("cycle_id").map_err(internal_db_err)?;
    let archived_reason: Option<String> = proposal
        .try_get("archived_reason")
        .map_err(internal_db_err)?;
    if archived_reason.as_deref() == Some("merged") {
        return Err(AppError::BadRequest(
            "Merged proposals require a future merge reversal flow and cannot use archive appeals in v1.".to_string(),
        ));
    }
    if archived_reason.as_deref() == Some("cycle_closed") {
        return Err(AppError::BadRequest(
            "Cycle-closed proposals must be re-submitted as new proposals.".to_string(),
        ));
    }

    let insert_result = sqlx::query(
        r#"
        INSERT INTO appeals (
            proposal_id,
            author_user_id,
            cycle_id,
            appeal_reason,
            clarification_note
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, status
        "#,
    )
    .bind(proposal_id)
    .bind(auth_user.user_id)
    .bind(cycle_id)
    .bind(&appeal_reason)
    .bind(&clarification_note)
    .fetch_one(&state.db)
    .await;

    let row = match insert_result {
        Ok(row) => row,
        Err(sqlx::Error::Database(db_err)) => {
            if db_err.constraint() == Some("appeals_unique_author_proposal_cycle") {
                return Err(AppError::Conflict(
                    "An appeal has already been submitted for this proposal.".to_string(),
                ));
            }

            error!("database error submitting appeal: {}", db_err);
            return Err(AppError::Internal("Failed to submit appeal.".to_string()));
        }
        Err(err) => {
            error!("database error submitting appeal: {}", err);
            return Err(AppError::Internal("Failed to submit appeal.".to_string()));
        }
    };

    let appeal_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
    let status: String = row.try_get("status").map_err(internal_db_err)?;

    insert_moderator_action(
        &state.db,
        "appeal_submission",
        proposal_id,
        None,
        None,
        Some("appeal_submitted"),
        Some(&appeal_reason),
        clarification_note.as_deref(),
        json!({
            "appeal_id": appeal_id,
            "author_user_id": auth_user.user_id,
            "archived_reason": archived_reason,
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(SubmitAppealResponse {
            ok: true,
            appeal_id,
            proposal_id,
            status,
        }),
    ))
}

pub async fn appeal_review_queue_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<AppealQueueResponse>, AppError> {
    require_moderator(&auth_user)?;
    let alternate_moderator_available =
        another_verified_moderator_available(&state.db, auth_user.user_id).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            a.id AS appeal_id,
            a.proposal_id,
            p.title AS proposal_title,
            a.author_user_id,
            a.appeal_reason,
            a.clarification_note,
            a.status,
            p.archived_reason,
            (
                SELECT ma.moderator_user_id
                FROM moderator_actions ma
                WHERE ma.proposal_id = p.id
                  AND ma.action_type IN ('archive', 'merge')
                ORDER BY ma.created_at DESC
                LIMIT 1
            ) AS last_archive_moderator_user_id,
            a.created_at
        FROM appeals a
        JOIN proposals p ON p.id = a.proposal_id
        JOIN cycles c ON c.id = a.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE a.status = 'pending'
          AND c.is_active = TRUE
          AND l.slug = 'world'
        ORDER BY a.created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading appeal queue: {}", err);
        AppError::Internal("Failed to load appeal queue.".to_string())
    })?;

    let appeals = rows
        .into_iter()
        .map(|row| map_appeal_queue_row(row, auth_user.user_id, alternate_moderator_available))
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(AppealQueueResponse { ok: true, appeals }))
}

pub async fn resolve_appeal_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(appeal_id): Path<Uuid>,
    Json(payload): Json<ResolveAppealRequest>,
) -> Result<(StatusCode, Json<ResolveAppealResponse>), AppError> {
    require_moderator(&auth_user)?;

    let outcome = payload.outcome.trim().to_lowercase();
    if outcome != "restore" && outcome != "uphold_archive" {
        return Err(AppError::BadRequest(
            "outcome must be either 'restore' or 'uphold_archive'.".to_string(),
        ));
    }

    let moderator_note = payload
        .moderator_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("moderator_note is required.".to_string()))?;

    if moderator_note.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::BadRequest(
            "moderator_note is too long.".to_string(),
        ));
    }

    let appeal = sqlx::query(
        r#"
        SELECT
            a.id,
            a.proposal_id,
            a.status,
            a.appeal_reason,
            p.primary_state,
            p.archived_reason,
            (
                SELECT ma.moderator_user_id
                FROM moderator_actions ma
                WHERE ma.proposal_id = p.id
                  AND ma.action_type IN ('archive', 'merge')
                ORDER BY ma.created_at DESC
                LIMIT 1
            ) AS last_archive_moderator_user_id
        FROM appeals a
        JOIN proposals p ON p.id = a.proposal_id
        WHERE a.id = $1
        LIMIT 1
        "#,
    )
    .bind(appeal_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading appeal for resolution: {}", err);
        AppError::Internal("Failed to resolve appeal.".to_string())
    })?;

    let Some(appeal) = appeal else {
        return Err(AppError::BadRequest("Appeal not found.".to_string()));
    };

    let status: String = appeal.try_get("status").map_err(internal_db_err)?;
    if status != "pending" {
        return Err(AppError::BadRequest(
            "Only pending appeals can be resolved.".to_string(),
        ));
    }

    let proposal_id: Uuid = appeal.try_get("proposal_id").map_err(internal_db_err)?;
    let previous_state: String = appeal.try_get("primary_state").map_err(internal_db_err)?;
    let archived_reason: Option<String> =
        appeal.try_get("archived_reason").map_err(internal_db_err)?;
    let appeal_reason: String = appeal.try_get("appeal_reason").map_err(internal_db_err)?;
    let last_archive_moderator_user_id: Option<Uuid> = appeal
        .try_get("last_archive_moderator_user_id")
        .map_err(internal_db_err)?;
    let alternate_moderator_available =
        another_verified_moderator_available(&state.db, auth_user.user_id).await?;

    if last_archive_moderator_user_id == Some(auth_user.user_id) && alternate_moderator_available {
        return Err(AppError::Forbidden(
            "A different moderator must resolve this appeal when another moderator is available."
                .to_string(),
        ));
    }

    let (appeal_status, proposal_restored) = if outcome == "restore" {
        if previous_state != "archived" {
            return Err(AppError::BadRequest(
                "Only archived proposals can be restored through appeal.".to_string(),
            ));
        }
        if archived_reason.as_deref() == Some("merged") {
            return Err(AppError::BadRequest(
                "Merged proposals require a future merge reversal flow and cannot be restored through archive appeals in v1.".to_string(),
            ));
        }
        if archived_reason.as_deref() == Some("cycle_closed") {
            return Err(AppError::BadRequest(
                "Cycle-closed proposals must be re-submitted as new proposals.".to_string(),
            ));
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
              AND primary_state = 'archived'
            "#,
        )
        .bind(proposal_id)
        .execute(&state.db)
        .await
        .map_err(|err| {
            error!("database error restoring appeal proposal: {}", err);
            AppError::Internal("Failed to resolve appeal.".to_string())
        })?;

        ("accepted".to_string(), true)
    } else {
        ("rejected".to_string(), false)
    };

    sqlx::query(
        r#"
        UPDATE appeals
        SET
            status = $2,
            outcome = $3,
            moderator_user_id = $4,
            moderator_note = $5,
            resolved_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(appeal_id)
    .bind(&appeal_status)
    .bind(&outcome)
    .bind(auth_user.user_id)
    .bind(&moderator_note)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error updating appeal resolution: {}", err);
        AppError::Internal("Failed to resolve appeal.".to_string())
    })?;

    insert_moderator_action(
        &state.db,
        "appeal_outcome",
        proposal_id,
        None,
        Some(auth_user.user_id),
        Some(&outcome),
        Some(&moderator_note),
        None,
        json!({
            "appeal_id": appeal_id,
            "appeal_reason": appeal_reason,
            "appeal_status": appeal_status,
            "proposal_restored": proposal_restored,
            "previous_state": previous_state.clone(),
            "archived_reason": archived_reason.clone(),
            "last_archive_moderator_user_id": last_archive_moderator_user_id,
            "alternate_moderator_available": alternate_moderator_available,
        }),
    )
    .await?;

    if proposal_restored {
        insert_moderator_action(
            &state.db,
            "unarchive",
            proposal_id,
            None,
            Some(auth_user.user_id),
            Some("appeal_restore"),
            Some(&moderator_note),
            None,
            json!({
                "appeal_id": appeal_id,
                "previous_state": previous_state,
                "archived_reason": archived_reason,
                "last_archive_moderator_user_id": last_archive_moderator_user_id,
            }),
        )
        .await?;
    }

    Ok((
        StatusCode::OK,
        Json(ResolveAppealResponse {
            ok: true,
            appeal_id,
            proposal_id,
            outcome,
            appeal_status,
            proposal_restored,
        }),
    ))
}

fn map_appeal_queue_row(
    row: sqlx::postgres::PgRow,
    current_moderator_user_id: Uuid,
    alternate_moderator_available: bool,
) -> Result<AppealQueueItem, AppError> {
    let last_archive_moderator_user_id: Option<Uuid> = row
        .try_get("last_archive_moderator_user_id")
        .map_err(internal_db_err)?;
    let current_moderator_must_recuse = last_archive_moderator_user_id
        == Some(current_moderator_user_id)
        && alternate_moderator_available;

    Ok(AppealQueueItem {
        appeal_id: row.try_get("appeal_id").map_err(internal_db_err)?,
        proposal_id: row.try_get("proposal_id").map_err(internal_db_err)?,
        proposal_title: row.try_get("proposal_title").map_err(internal_db_err)?,
        author_user_id: row.try_get("author_user_id").map_err(internal_db_err)?,
        appeal_reason: row.try_get("appeal_reason").map_err(internal_db_err)?,
        clarification_note: row.try_get("clarification_note").map_err(internal_db_err)?,
        status: row.try_get("status").map_err(internal_db_err)?,
        archived_reason: row.try_get("archived_reason").map_err(internal_db_err)?,
        last_archive_moderator_user_id,
        current_moderator_must_recuse,
        created_at: row.try_get("created_at").map_err(internal_db_err)?,
    })
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
        AppError::Internal("Failed to resolve appeal.".to_string())
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
        error!("database error inserting appeal audit action: {}", err);
        AppError::Internal("Failed to log appeal action.".to_string())
    })?;

    Ok(())
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read appeal data.".to_string())
}

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{AppState, auth::AuthUser, error::AppError};

const MAX_NOTE_CHARS: usize = 2000;

#[derive(Debug, Deserialize)]
pub struct UpsertMergeDistinctionNoteRequest {
    pub target_proposal_id: Uuid,
    pub difference_type: Option<String>,
    pub note_text: String,
}

#[derive(Debug, Serialize)]
pub struct MergeDistinctionNoteResponse {
    pub ok: bool,
    pub source_proposal_id: Uuid,
    pub target_proposal_id: Uuid,
    pub author_user_id: Uuid,
    pub difference_type: String,
    pub note_text: String,
}

#[derive(Debug, Serialize)]
pub struct MergeRelationshipResponse {
    pub ok: bool,
    pub source_proposal_id: Uuid,
    pub target_proposal_id: Uuid,
    pub note: Option<MergeRelationshipNote>,
}

#[derive(Debug, Serialize)]
pub struct MergeRelationshipNote {
    pub author_user_id: Uuid,
    pub difference_type: String,
    pub note_text: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn upsert_merge_distinction_note_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(source_proposal_id): Path<Uuid>,
    Json(payload): Json<UpsertMergeDistinctionNoteRequest>,
) -> Result<(StatusCode, Json<MergeDistinctionNoteResponse>), AppError> {
    auth_user.require_verified()?;

    if source_proposal_id == payload.target_proposal_id {
        return Err(AppError::BadRequest(
            "source and target proposals must be different.".to_string(),
        ));
    }

    let note_text = payload.note_text.trim().to_string();

    if note_text.is_empty() {
        return Err(AppError::BadRequest("note_text is required.".to_string()));
    }

    if note_text.chars().count() > MAX_NOTE_CHARS {
        return Err(AppError::BadRequest("note_text is too long.".to_string()));
    }

    let difference_type = payload
        .difference_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("other")
        .to_lowercase();

    if !is_valid_difference_type(&difference_type) {
        return Err(AppError::BadRequest(
            "difference_type is invalid.".to_string(),
        ));
    }

    let source_row = sqlx::query(
        r#"
        SELECT
            p.author_user_id,
            p.primary_state,
            p.cycle_id,
            p.locale_id,
            b.code AS board_code,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        WHERE p.id = $1
        LIMIT 1
        "#,
    )
    .bind(source_proposal_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading source proposal: {}", err);
        AppError::Internal("Failed to save distinction note.".to_string())
    })?;

    let Some(source_row) = source_row else {
        return Err(AppError::BadRequest(
            "Source proposal not found.".to_string(),
        ));
    };

    let source_author_user_id: Uuid = source_row
        .try_get("author_user_id")
        .map_err(internal_db_err)?;
    let source_cycle_id: Uuid = source_row.try_get("cycle_id").map_err(internal_db_err)?;
    let source_locale_id: Uuid = source_row.try_get("locale_id").map_err(internal_db_err)?;
    let source_board_code: String = source_row.try_get("board_code").map_err(internal_db_err)?;
    let source_primary_state: String = source_row
        .try_get("primary_state")
        .map_err(internal_db_err)?;

    if source_author_user_id != auth_user.user_id {
        return Err(AppError::BadRequest(
            "Only the source proposal author can edit its distinction note.".to_string(),
        ));
    }

    if source_primary_state != "active" {
        return Err(AppError::BadRequest(
            "Source proposal must be active.".to_string(),
        ));
    }

    let source_support_count: i32 = source_row
        .try_get("support_count")
        .map_err(internal_db_err)?;
    let source_not_a_fit_count: i32 = source_row
        .try_get("not_a_fit_count")
        .map_err(internal_db_err)?;
    let source_unclear_count: i32 = source_row
        .try_get("unclear_count")
        .map_err(internal_db_err)?;
    let source_unsafe_count: i32 = source_row
        .try_get("unsafe_count")
        .map_err(internal_db_err)?;
    let source_merge_count: i32 = source_row.try_get("merge_count").map_err(internal_db_err)?;

    if !proposal_is_merge_watch(
        source_support_count,
        source_not_a_fit_count,
        source_unclear_count,
        source_unsafe_count,
        source_merge_count,
    ) {
        return Err(AppError::BadRequest(
            "Distinction notes are available only after the source proposal receives enough duplicate signals."
                .to_string(),
        ));
    }

    let target_row = sqlx::query(
        r#"
        SELECT p.cycle_id, p.locale_id, p.primary_state, b.code AS board_code
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        WHERE p.id = $1
        LIMIT 1
        "#,
    )
    .bind(payload.target_proposal_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading target proposal: {}", err);
        AppError::Internal("Failed to save distinction note.".to_string())
    })?;

    let Some(target_row) = target_row else {
        return Err(AppError::BadRequest(
            "Target proposal not found.".to_string(),
        ));
    };

    let target_cycle_id: Uuid = target_row.try_get("cycle_id").map_err(internal_db_err)?;
    let target_locale_id: Uuid = target_row.try_get("locale_id").map_err(internal_db_err)?;
    let target_board_code: String = target_row.try_get("board_code").map_err(internal_db_err)?;
    let target_primary_state: String = target_row
        .try_get("primary_state")
        .map_err(internal_db_err)?;

    if source_cycle_id != target_cycle_id
        || source_locale_id != target_locale_id
        || source_board_code != target_board_code
    {
        return Err(AppError::BadRequest(
            "Target proposal must be in the same board, cycle, and locale.".to_string(),
        ));
    }

    if target_primary_state != "active" {
        return Err(AppError::BadRequest(
            "Target proposal must be active.".to_string(),
        ));
    }

    if !active_merge_relationship_exists(&state.db, source_proposal_id, payload.target_proposal_id)
        .await?
    {
        return Err(AppError::BadRequest(
            "Distinction notes require an active merge relationship.".to_string(),
        ));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO merge_distinction_notes (
            source_proposal_id,
            target_proposal_id,
            author_user_id,
            difference_type,
            note_text
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (source_proposal_id, target_proposal_id)
        DO UPDATE SET
            difference_type = EXCLUDED.difference_type,
            note_text = EXCLUDED.note_text,
            updated_at = NOW()
        RETURNING
            source_proposal_id,
            target_proposal_id,
            author_user_id,
            difference_type,
            note_text
        "#,
    )
    .bind(source_proposal_id)
    .bind(payload.target_proposal_id)
    .bind(auth_user.user_id)
    .bind(&difference_type)
    .bind(&note_text)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        error!("database error upserting distinction note: {}", err);
        AppError::Internal("Failed to save distinction note.".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(MergeDistinctionNoteResponse {
            ok: true,
            source_proposal_id: row.try_get("source_proposal_id").map_err(internal_db_err)?,
            target_proposal_id: row.try_get("target_proposal_id").map_err(internal_db_err)?,
            author_user_id: row.try_get("author_user_id").map_err(internal_db_err)?,
            difference_type: row.try_get("difference_type").map_err(internal_db_err)?,
            note_text: row.try_get("note_text").map_err(internal_db_err)?,
        }),
    ))
}

pub async fn get_merge_relationship_handler(
    State(state): State<Arc<AppState>>,
    Path((source_proposal_id, target_proposal_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<MergeRelationshipResponse>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            n.author_user_id,
            n.difference_type,
            n.note_text,
            n.created_at,
            n.updated_at
        FROM proposal_merge_relationships r
        LEFT JOIN merge_distinction_notes n
            ON n.source_proposal_id = r.source_proposal_id
           AND n.target_proposal_id = r.target_proposal_id
        WHERE r.source_proposal_id = $1
          AND r.target_proposal_id = $2
          AND r.status = 'active'
        LIMIT 1
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading distinction note: {}", err);
        AppError::Internal("Failed to load merge relationship.".to_string())
    })?;

    let note = match row {
        Some(row) => map_note(row)?,
        None => None,
    };

    Ok(Json(MergeRelationshipResponse {
        ok: true,
        source_proposal_id,
        target_proposal_id,
        note,
    }))
}

async fn active_merge_relationship_exists(
    db: &sqlx::PgPool,
    source_proposal_id: Uuid,
    target_proposal_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM proposal_merge_relationships
            WHERE source_proposal_id = $1
              AND target_proposal_id = $2
              AND status = 'active'
        ) AS exists_flag
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking merge relationship: {}", err);
        AppError::Internal("Failed to save distinction note.".to_string())
    })?;

    row.try_get("exists_flag").map_err(internal_db_err)
}

fn map_note(row: sqlx::postgres::PgRow) -> Result<Option<MergeRelationshipNote>, AppError> {
    let note_text: Option<String> = row.try_get("note_text").map_err(internal_db_err)?;

    let Some(note_text) = note_text else {
        return Ok(None);
    };

    Ok(Some(MergeRelationshipNote {
        author_user_id: row.try_get("author_user_id").map_err(internal_db_err)?,
        difference_type: row.try_get("difference_type").map_err(internal_db_err)?,
        note_text,
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .map_err(internal_db_err)?
            .to_rfc3339(),
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
            .map_err(internal_db_err)?
            .to_rfc3339(),
    }))
}

fn is_valid_difference_type(value: &str) -> bool {
    matches!(
        value,
        "different_scope"
            | "different_cause"
            | "different_affected_group"
            | "different_implementation"
            | "different_completion_criteria"
            | "other"
    )
}

fn proposal_is_merge_watch(
    support_count: i32,
    not_a_fit_count: i32,
    unclear_count: i32,
    unsafe_count: i32,
    merge_count: i32,
) -> bool {
    let total_count = support_count + not_a_fit_count + unclear_count + unsafe_count + merge_count;

    total_count >= 10 && fraction_at_least(merge_count, total_count, 0.20)
}

fn fraction_at_least(part: i32, total: i32, threshold: f64) -> bool {
    total > 0 && (part as f64 / total as f64) >= threshold
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read distinction note data.".to_string())
}

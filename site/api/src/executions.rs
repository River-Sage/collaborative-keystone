use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{AppState, auth::AuthUser, error::AppError};

const MAX_REQUIRED_RESOURCE_CATEGORIES: usize = 8;
const MAX_COMPLETION_CRITERIA: usize = 8;
const MAX_RESOURCE_REQUIREMENTS: usize = 64;
const MAX_COMPLETION_CRITERION_CHARS: usize = 240;
const MAX_RESOURCE_AMOUNT_CHARS: usize = 64;
const MAX_RESOURCE_UNIT_CHARS: usize = 64;
const MAX_RESOURCE_TARGET_CHARS: usize = 140;
const MAX_NOTE_CHARS: usize = 2000;
const MAX_LINK_CHARS: usize = 2048;
const MAX_TIMESTAMP_CHARS: usize = 64;

#[derive(Debug, serde::Deserialize)]
pub struct UpdateExecutionRecordRequest {
    pub status: Option<String>,
    pub completion_criteria: Option<Value>,
    pub execution_tracking_entries: Option<Value>,
    pub update_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateExecutionRecordResponse {
    pub ok: bool,
    pub execution_record: ExecutionRecordDetail,
}

#[derive(Debug, Serialize)]
pub struct ExecutionRecordListResponse {
    pub ok: bool,
    pub execution_records: Vec<ExecutionRecordSummary>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionRecordDetailResponse {
    pub ok: bool,
    pub execution_record: ExecutionRecordDetail,
}

#[derive(Debug, Serialize)]
pub struct UpdateExecutionRecordResponse {
    pub ok: bool,
    pub execution_record: ExecutionRecordDetail,
}

#[derive(Debug, Serialize)]
pub struct ExecutionRecordSummary {
    pub id: Uuid,
    pub solution_proposal_id: Uuid,
    pub parent_issue_proposal_id: Uuid,
    pub title: String,
    pub parent_issue_title: String,
    pub action_description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionRecordDetail {
    #[serde(flatten)]
    pub summary: ExecutionRecordSummary,
    pub required_resource_categories: Value,
    pub completion_criteria: Value,
    pub execution_tracking_entries: Value,
}

pub async fn create_execution_record_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(solution_proposal_id): Path<Uuid>,
) -> Result<(StatusCode, Json<CreateExecutionRecordResponse>), AppError> {
    require_moderator(&auth_user)?;

    let execution_record = create_execution_record_from_solution(
        &state.db,
        auth_user.user_id,
        solution_proposal_id,
        false,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateExecutionRecordResponse {
            ok: true,
            execution_record,
        }),
    ))
}

pub async fn create_execution_record_from_solution(
    db: &sqlx::PgPool,
    moderator_user_id: Uuid,
    solution_proposal_id: Uuid,
    return_existing_on_conflict: bool,
) -> Result<ExecutionRecordDetail, AppError> {
    let solution = sqlx::query(
        r#"
        SELECT
            p.id,
            p.parent_issue_proposal_id,
            p.cycle_id,
            p.locale_id,
            p.title,
            p.primary_state,
            p.action_description,
            p.required_resource_categories,
            p.completion_criteria,
            p.execution_tracking_entries,
            p.support_count,
            p.not_a_fit_count,
            p.unclear_count,
            p.unsafe_count,
            p.merge_count,
            b.code AS board_code
        FROM proposals p
        JOIN boards b ON b.id = p.board_id
        JOIN cycles c ON c.id = p.cycle_id
        JOIN locales l ON l.id = p.locale_id
        WHERE p.id = $1
          AND l.slug = 'world'
          AND c.is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(solution_proposal_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error loading winning solution proposal: {}", err);
        AppError::Internal("Failed to create execution record.".to_string())
    })?;

    let Some(solution) = solution else {
        return Err(AppError::BadRequest(
            "Solution proposal not found.".to_string(),
        ));
    };

    let board_code: String = solution.try_get("board_code").map_err(internal_db_err)?;
    if board_code != "solution" {
        return Err(AppError::BadRequest(
            "Only solution proposals can become execution records.".to_string(),
        ));
    }

    let primary_state: String = solution.try_get("primary_state").map_err(internal_db_err)?;
    if primary_state != "active" && primary_state != "ranked" {
        return Err(AppError::BadRequest(
            "Only active solution proposals can become execution records.".to_string(),
        ));
    }

    let parent_issue_proposal_id: Option<Uuid> = solution
        .try_get("parent_issue_proposal_id")
        .map_err(internal_db_err)?;
    let Some(parent_issue_proposal_id) = parent_issue_proposal_id else {
        return Err(AppError::BadRequest(
            "Solution proposal is missing a parent issue.".to_string(),
        ));
    };

    let action_description: Option<String> = solution
        .try_get("action_description")
        .map_err(internal_db_err)?;
    let action_description = action_description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest("Solution proposal is missing an action description.".to_string())
        })?;

    let required_resource_categories = validate_required_resource_categories_value(
        nonempty_json_array(&solution, "required_resource_categories")?,
    )?;
    let completion_criteria =
        validate_completion_criteria_value(nonempty_json_array(&solution, "completion_criteria")?)?;
    let execution_tracking_entries = validate_execution_tracking_entries_value(
        nonempty_json_array(&solution, "execution_tracking_entries")?,
    )?;

    let cycle_id: Uuid = solution.try_get("cycle_id").map_err(internal_db_err)?;
    let locale_id: Uuid = solution.try_get("locale_id").map_err(internal_db_err)?;
    let title: String = solution.try_get("title").map_err(internal_db_err)?;

    let vote_snapshot = json!({
        "support_count": solution.try_get::<i32, _>("support_count").map_err(internal_db_err)?,
        "not_a_fit_count": solution.try_get::<i32, _>("not_a_fit_count").map_err(internal_db_err)?,
        "unclear_count": solution.try_get::<i32, _>("unclear_count").map_err(internal_db_err)?,
        "unsafe_count": solution.try_get::<i32, _>("unsafe_count").map_err(internal_db_err)?,
        "merge_count": solution.try_get::<i32, _>("merge_count").map_err(internal_db_err)?,
    });

    let insert_result = sqlx::query(
        r#"
        INSERT INTO execution_records (
            solution_proposal_id,
            parent_issue_proposal_id,
            cycle_id,
            locale_id,
            created_by_moderator_user_id,
            title,
            action_description,
            required_resource_categories,
            completion_criteria,
            execution_tracking_entries,
            proposal_vote_snapshot
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#,
    )
    .bind(solution_proposal_id)
    .bind(parent_issue_proposal_id)
    .bind(cycle_id)
    .bind(locale_id)
    .bind(moderator_user_id)
    .bind(&title)
    .bind(&action_description)
    .bind(&required_resource_categories)
    .bind(&completion_criteria)
    .bind(&execution_tracking_entries)
    .bind(&vote_snapshot)
    .fetch_one(db)
    .await;

    let execution_record_id = match insert_result {
        Ok(row) => row.try_get("id").map_err(internal_db_err)?,
        Err(sqlx::Error::Database(db_err)) => {
            let constraint = db_err.constraint().unwrap_or_default();
            if constraint == "execution_records_solution_unique"
                || constraint == "execution_records_one_solution_per_issue_cycle"
            {
                if return_existing_on_conflict {
                    return load_execution_record_by_solution(db, solution_proposal_id).await;
                }

                return Err(AppError::Conflict(
                    "An execution record already exists for this winning solution scope."
                        .to_string(),
                ));
            }

            error!("database error creating execution record: {}", db_err);
            return Err(AppError::Internal(
                "Failed to create execution record.".to_string(),
            ));
        }
        Err(err) => {
            error!("database error creating execution record: {}", err);
            return Err(AppError::Internal(
                "Failed to create execution record.".to_string(),
            ));
        }
    };

    insert_moderator_action(
        db,
        solution_proposal_id,
        parent_issue_proposal_id,
        moderator_user_id,
        json!({
            "execution_record_id": execution_record_id,
            "previous_solution_state": primary_state,
            "vote_snapshot": vote_snapshot
        }),
    )
    .await?;

    load_execution_record(db, execution_record_id).await
}

pub async fn list_execution_records_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ExecutionRecordListResponse>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            er.id,
            er.solution_proposal_id,
            er.parent_issue_proposal_id,
            er.title,
            pi.title AS parent_issue_title,
            er.action_description,
            er.status,
            er.created_at,
            er.updated_at
        FROM execution_records er
        JOIN proposals pi ON pi.id = er.parent_issue_proposal_id
        JOIN cycles c ON c.id = er.cycle_id
        JOIN locales l ON l.id = er.locale_id
        WHERE l.slug = 'world'
        ORDER BY er.created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading execution records: {}", err);
        AppError::Internal("Failed to load execution records.".to_string())
    })?;

    let execution_records = rows
        .into_iter()
        .map(map_execution_summary_row)
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(ExecutionRecordListResponse {
        ok: true,
        execution_records,
    }))
}

pub async fn get_execution_record_handler(
    State(state): State<Arc<AppState>>,
    Path(execution_record_id): Path<Uuid>,
) -> Result<Json<ExecutionRecordDetailResponse>, AppError> {
    let execution_record = load_execution_record(&state.db, execution_record_id).await?;

    Ok(Json(ExecutionRecordDetailResponse {
        ok: true,
        execution_record,
    }))
}

pub async fn update_execution_record_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(execution_record_id): Path<Uuid>,
    Json(payload): Json<UpdateExecutionRecordRequest>,
) -> Result<Json<UpdateExecutionRecordResponse>, AppError> {
    require_moderator(&auth_user)?;

    let existing = sqlx::query(
        r#"
        SELECT
            id,
            solution_proposal_id,
            parent_issue_proposal_id,
            status,
            completion_criteria,
            execution_tracking_entries
        FROM execution_records
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(execution_record_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!(
            "database error loading execution record for update: {}",
            err
        );
        AppError::Internal("Failed to update execution record.".to_string())
    })?;

    let Some(existing) = existing else {
        return Err(AppError::BadRequest(
            "Execution record not found.".to_string(),
        ));
    };

    let previous_status: String = existing.try_get("status").map_err(internal_db_err)?;
    let solution_proposal_id: Uuid = existing
        .try_get("solution_proposal_id")
        .map_err(internal_db_err)?;
    let parent_issue_proposal_id: Uuid = existing
        .try_get("parent_issue_proposal_id")
        .map_err(internal_db_err)?;
    let previous_completion_criteria: Value = existing
        .try_get("completion_criteria")
        .map_err(internal_db_err)?;
    let previous_execution_tracking_entries: Value = existing
        .try_get("execution_tracking_entries")
        .map_err(internal_db_err)?;

    let status = match payload.status.as_deref().map(str::trim) {
        Some("") | None => previous_status.clone(),
        Some(value) if is_valid_execution_status(value) => value.to_string(),
        Some(_) => {
            return Err(AppError::BadRequest(
                "status must be one of: active, paused, completed, cancelled.".to_string(),
            ));
        }
    };

    let completion_criteria = match payload.completion_criteria {
        Some(value) => validate_completion_criteria_value(value)?,
        None => previous_completion_criteria.clone(),
    };

    let execution_tracking_entries = match payload.execution_tracking_entries {
        Some(value) => validate_execution_tracking_entries_value(value)?,
        None => previous_execution_tracking_entries.clone(),
    };

    let update_note = payload
        .update_note
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(note) = update_note.as_ref() {
        validate_text_max_chars(note, "update_note", MAX_NOTE_CHARS)?;
    }

    sqlx::query(
        r#"
        UPDATE execution_records
        SET
            status = $2,
            completion_criteria = $3,
            execution_tracking_entries = $4,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(execution_record_id)
    .bind(&status)
    .bind(&completion_criteria)
    .bind(&execution_tracking_entries)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error updating execution record: {}", err);
        AppError::Internal("Failed to update execution record.".to_string())
    })?;

    insert_execution_update_action(
        &state.db,
        solution_proposal_id,
        parent_issue_proposal_id,
        auth_user.user_id,
        update_note.as_deref(),
        json!({
            "execution_record_id": execution_record_id,
            "previous_status": previous_status,
            "status": status,
            "completion_criteria_changed": completion_criteria != previous_completion_criteria,
            "execution_tracking_entries_changed": execution_tracking_entries != previous_execution_tracking_entries
        }),
    )
    .await?;

    let execution_record = load_execution_record(&state.db, execution_record_id).await?;

    Ok(Json(UpdateExecutionRecordResponse {
        ok: true,
        execution_record,
    }))
}

async fn load_execution_record(
    db: &sqlx::PgPool,
    execution_record_id: Uuid,
) -> Result<ExecutionRecordDetail, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            er.id,
            er.solution_proposal_id,
            er.parent_issue_proposal_id,
            er.title,
            pi.title AS parent_issue_title,
            er.action_description,
            er.required_resource_categories,
            er.completion_criteria,
            er.execution_tracking_entries,
            er.status,
            er.created_at,
            er.updated_at
        FROM execution_records er
        JOIN proposals pi ON pi.id = er.parent_issue_proposal_id
        JOIN locales l ON l.id = er.locale_id
        WHERE er.id = $1
          AND l.slug = 'world'
        LIMIT 1
        "#,
    )
    .bind(execution_record_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!("database error loading execution record detail: {}", err);
        AppError::Internal("Failed to load execution record.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Execution record not found.".to_string(),
        ));
    };

    map_execution_detail_row(row)
}

async fn load_execution_record_by_solution(
    db: &sqlx::PgPool,
    solution_proposal_id: Uuid,
) -> Result<ExecutionRecordDetail, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id
        FROM execution_records
        WHERE solution_proposal_id = $1
        LIMIT 1
        "#,
    )
    .bind(solution_proposal_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading execution record by solution: {}",
            err
        );
        AppError::Internal("Failed to load execution record.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::Internal(
            "Execution record conflict could not be resolved.".to_string(),
        ));
    };

    let execution_record_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
    load_execution_record(db, execution_record_id).await
}

fn map_execution_summary_row(
    row: sqlx::postgres::PgRow,
) -> Result<ExecutionRecordSummary, AppError> {
    Ok(ExecutionRecordSummary {
        id: row.try_get("id").map_err(internal_db_err)?,
        solution_proposal_id: row
            .try_get("solution_proposal_id")
            .map_err(internal_db_err)?,
        parent_issue_proposal_id: row
            .try_get("parent_issue_proposal_id")
            .map_err(internal_db_err)?,
        title: row.try_get("title").map_err(internal_db_err)?,
        parent_issue_title: row.try_get("parent_issue_title").map_err(internal_db_err)?,
        action_description: row.try_get("action_description").map_err(internal_db_err)?,
        status: row.try_get("status").map_err(internal_db_err)?,
        created_at: row.try_get("created_at").map_err(internal_db_err)?,
        updated_at: row.try_get("updated_at").map_err(internal_db_err)?,
    })
}

fn map_execution_detail_row(row: sqlx::postgres::PgRow) -> Result<ExecutionRecordDetail, AppError> {
    let summary = ExecutionRecordSummary {
        id: row.try_get("id").map_err(internal_db_err)?,
        solution_proposal_id: row
            .try_get("solution_proposal_id")
            .map_err(internal_db_err)?,
        parent_issue_proposal_id: row
            .try_get("parent_issue_proposal_id")
            .map_err(internal_db_err)?,
        title: row.try_get("title").map_err(internal_db_err)?,
        parent_issue_title: row.try_get("parent_issue_title").map_err(internal_db_err)?,
        action_description: row.try_get("action_description").map_err(internal_db_err)?,
        status: row.try_get("status").map_err(internal_db_err)?,
        created_at: row.try_get("created_at").map_err(internal_db_err)?,
        updated_at: row.try_get("updated_at").map_err(internal_db_err)?,
    };

    Ok(ExecutionRecordDetail {
        summary,
        required_resource_categories: row
            .try_get("required_resource_categories")
            .map_err(internal_db_err)?,
        completion_criteria: row
            .try_get("completion_criteria")
            .map_err(internal_db_err)?,
        execution_tracking_entries: row
            .try_get("execution_tracking_entries")
            .map_err(internal_db_err)?,
    })
}

fn nonempty_json_array(row: &sqlx::postgres::PgRow, field_name: &str) -> Result<Value, AppError> {
    let value: Option<Value> = row.try_get(field_name).map_err(internal_db_err)?;
    let value = value.unwrap_or_else(|| json!([]));

    if value
        .as_array()
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        return Err(AppError::BadRequest(format!(
            "{field_name} must be a non-empty array."
        )));
    }

    Ok(value)
}

fn is_valid_execution_status(value: &str) -> bool {
    matches!(value, "active" | "paused" | "completed" | "cancelled")
}

fn validate_required_resource_categories_value(value: Value) -> Result<Value, AppError> {
    let items = require_nonempty_value_array(&value, "required_resource_categories")?;

    if items.len() > MAX_REQUIRED_RESOURCE_CATEGORIES {
        return Err(AppError::BadRequest(format!(
            "required_resource_categories is capped at {MAX_REQUIRED_RESOURCE_CATEGORIES} items."
        )));
    }

    for (index, item) in items.iter().enumerate() {
        let category = item.as_str().ok_or_else(|| {
            AppError::BadRequest(format!(
                "required_resource_categories[{index}] must be a text category."
            ))
        })?;

        validate_resource_category(category, &format!("required_resource_categories[{index}]"))?;
    }

    Ok(value)
}

fn validate_completion_criteria_value(value: Value) -> Result<Value, AppError> {
    let items = require_nonempty_value_array(&value, "completion_criteria")?;

    if items.len() > MAX_COMPLETION_CRITERIA {
        return Err(AppError::BadRequest(format!(
            "completion_criteria is capped at {MAX_COMPLETION_CRITERIA} items."
        )));
    }

    for (index, item) in items.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            AppError::BadRequest(format!(
                "completion_criteria[{index}] must be a structured object."
            ))
        })?;

        require_object_text(
            object,
            "completion_criteria",
            index,
            "criterion_description",
        )
        .and_then(|value| {
            validate_text_max_chars(
                value,
                &format!("completion_criteria[{index}].criterion_description"),
                MAX_COMPLETION_CRITERION_CHARS,
            )
        })?;
        let status =
            require_object_text(object, "completion_criteria", index, "completion_status")?;
        if !is_valid_completion_item_status(status) {
            return Err(AppError::BadRequest(format!(
                "completion_criteria[{index}].completion_status must be one of: not_started, in_progress, completed, blocked."
            )));
        }
        validate_object_text_or_empty_max(
            object,
            "completion_criteria",
            index,
            "evidence_note",
            MAX_NOTE_CHARS,
        )?;
        validate_object_null_or_text_max(
            object,
            "completion_criteria",
            index,
            "updated_at",
            MAX_TIMESTAMP_CHARS,
        )?;
    }

    Ok(value)
}

fn validate_execution_tracking_entries_value(value: Value) -> Result<Value, AppError> {
    let items = require_nonempty_value_array(&value, "execution_tracking_entries")?;

    if items.len() > MAX_RESOURCE_REQUIREMENTS {
        return Err(AppError::BadRequest(format!(
            "execution_tracking_entries is capped at {MAX_RESOURCE_REQUIREMENTS} items."
        )));
    }

    for (index, item) in items.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            AppError::BadRequest(format!(
                "execution_tracking_entries[{index}] must be a structured object."
            ))
        })?;

        let category = require_object_text(
            object,
            "execution_tracking_entries",
            index,
            "resource_category",
        )?;
        validate_resource_category(
            category,
            &format!("execution_tracking_entries[{index}].resource_category"),
        )?;
        let target_needed =
            require_object_text(object, "execution_tracking_entries", index, "target_needed")?;
        validate_text_max_chars(
            target_needed,
            &format!("execution_tracking_entries[{index}].target_needed"),
            MAX_RESOURCE_TARGET_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "target_amount",
            MAX_RESOURCE_AMOUNT_CHARS,
        )?;
        validate_optional_object_text_max(
            object,
            "execution_tracking_entries",
            index,
            "target_unit",
            MAX_RESOURCE_UNIT_CHARS,
        )?;
        validate_object_text_or_empty_max(
            object,
            "execution_tracking_entries",
            index,
            "current_acquired_amount",
            MAX_RESOURCE_AMOUNT_CHARS,
        )?;
        validate_object_text_or_empty_max(
            object,
            "execution_tracking_entries",
            index,
            "external_coordination_link",
            MAX_LINK_CHARS,
        )?;
        validate_object_text_or_empty_max(
            object,
            "execution_tracking_entries",
            index,
            "status_proof_note",
            MAX_NOTE_CHARS,
        )?;
    }

    Ok(value)
}

fn require_nonempty_value_array<'a>(
    value: &'a Value,
    field_name: &str,
) -> Result<&'a Vec<Value>, AppError> {
    match value.as_array() {
        Some(items) if !items.is_empty() => Ok(items),
        _ => Err(AppError::BadRequest(format!(
            "{field_name} must be a non-empty array."
        ))),
    }
}

fn require_object_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
) -> Result<&'a str, AppError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest(format!("{field_name}[{index}].{key} is required.")))
}

fn validate_text_max_chars(
    value: &str,
    field_name: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    if value.chars().count() > max_chars {
        return Err(AppError::BadRequest(format!(
            "{field_name} is too long. Keep it to {max_chars} characters or fewer."
        )));
    }

    Ok(())
}

fn validate_object_text_or_empty_max(
    object: &serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    match object.get(key) {
        Some(Value::String(value)) => validate_text_max_chars(
            value.trim(),
            &format!("{field_name}[{index}].{key}"),
            max_chars,
        ),
        _ => Err(AppError::BadRequest(format!(
            "{field_name}[{index}].{key} must be a text field."
        ))),
    }
}

fn validate_optional_object_text_max(
    object: &serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    match object.get(key) {
        Some(Value::String(value)) => validate_text_max_chars(
            value.trim(),
            &format!("{field_name}[{index}].{key}"),
            max_chars,
        ),
        Some(Value::Null) | None => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "{field_name}[{index}].{key} must be a text field."
        ))),
    }
}

fn validate_object_null_or_text_max(
    object: &serde_json::Map<String, Value>,
    field_name: &str,
    index: usize,
    key: &str,
    max_chars: usize,
) -> Result<(), AppError> {
    match object.get(key) {
        Some(Value::Null) => Ok(()),
        Some(Value::String(value)) => validate_text_max_chars(
            value.trim(),
            &format!("{field_name}[{index}].{key}"),
            max_chars,
        ),
        _ => Err(AppError::BadRequest(format!(
            "{field_name}[{index}].{key} must be null or text."
        ))),
    }
}

fn validate_resource_category(value: &str, field_name: &str) -> Result<(), AppError> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(AppError::BadRequest(format!("{field_name} is required.")));
    }

    if is_supported_resource_category(&normalized) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "{field_name} must be one of: money, labor / manpower, skills / trades, materials, equipment, logistics / transport, organizational support, other."
        )))
    }
}

fn is_supported_resource_category(value: &str) -> bool {
    matches!(
        value,
        "money"
            | "labor"
            | "manpower"
            | "labor / manpower"
            | "skills"
            | "trades"
            | "skills / trades"
            | "materials"
            | "equipment"
            | "logistics"
            | "transport"
            | "logistics / transport"
            | "organizational support"
            | "other"
    )
}

fn is_valid_completion_item_status(value: &str) -> bool {
    matches!(
        value,
        "not_started" | "in_progress" | "completed" | "blocked"
    )
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

async fn insert_moderator_action(
    db: &sqlx::PgPool,
    solution_proposal_id: Uuid,
    parent_issue_proposal_id: Uuid,
    moderator_user_id: Uuid,
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
        VALUES (
            'execution_record_created',
            $1,
            $2,
            $3,
            'winning_solution_transition',
            NULL,
            NULL,
            $4
        )
        "#,
    )
    .bind(solution_proposal_id)
    .bind(parent_issue_proposal_id)
    .bind(moderator_user_id)
    .bind(state_snapshot)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting execution audit action: {}", err);
        AppError::Internal("Failed to log execution record action.".to_string())
    })?;

    Ok(())
}

async fn insert_execution_update_action(
    db: &sqlx::PgPool,
    solution_proposal_id: Uuid,
    parent_issue_proposal_id: Uuid,
    moderator_user_id: Uuid,
    public_note: Option<&str>,
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
        VALUES (
            'execution_record_updated',
            $1,
            $2,
            $3,
            'execution_status_update',
            $4,
            NULL,
            $5
        )
        "#,
    )
    .bind(solution_proposal_id)
    .bind(parent_issue_proposal_id)
    .bind(moderator_user_id)
    .bind(public_note)
    .bind(state_snapshot)
    .execute(db)
    .await
    .map_err(|err| {
        error!(
            "database error inserting execution update audit action: {}",
            err
        );
        AppError::Internal("Failed to log execution update.".to_string())
    })?;

    Ok(())
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read execution record data.".to_string())
}

use std::{env, sync::Arc};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Acquire, Row};
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{hash_password, normalize_email, validate_password},
    error::AppError,
};

const BOOTSTRAP_TOKEN_ENV: &str = "CK_BOOTSTRAP_MODERATOR_TOKEN";
const BOOTSTRAP_RATE_LIMIT_MAX: usize = 8;
const BOOTSTRAP_RATE_LIMIT_WINDOW_MINUTES: i64 = 15;
const MIN_BOOTSTRAP_TOKEN_CHARS: usize = 32;

#[derive(Debug, Deserialize)]
pub struct FirstModeratorBootstrapRequest {
    pub email: String,
    pub password: String,
    pub bootstrap_token: String,
}

#[derive(Debug, Serialize)]
pub struct FirstModeratorBootstrapResponse {
    pub ok: bool,
    pub email: String,
    pub role_code: String,
    pub bootstrap_complete: bool,
}

pub async fn first_moderator_bootstrap_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<FirstModeratorBootstrapRequest>,
) -> Result<(StatusCode, Json<FirstModeratorBootstrapResponse>), AppError> {
    enforce_bootstrap_rate_limit(&state, &headers).await?;

    let configured_token = configured_bootstrap_token()?;
    let provided_token = payload.bootstrap_token.trim();
    if !constant_time_eq(configured_token.as_bytes(), provided_token.as_bytes()) {
        return Err(AppError::Forbidden(
            "Bootstrap token is invalid.".to_string(),
        ));
    }

    let email = normalize_email(&payload.email)?;
    validate_password(&payload.password)?;
    let password_hash = hash_password(&payload.password)?;

    let mut tx = state.db.begin().await.map_err(|err| {
        error!("database error starting bootstrap transaction: {}", err);
        AppError::Internal("Failed to bootstrap first moderator.".to_string())
    })?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('ck_first_moderator_bootstrap'))")
        .execute(tx.acquire().await.map_err(internal_db_err)?)
        .await
        .map_err(|err| {
            error!("database error locking bootstrap transaction: {}", err);
            AppError::Internal("Failed to bootstrap first moderator.".to_string())
        })?;

    if first_moderator_bootstrap_complete_in_executor(tx.acquire().await.map_err(internal_db_err)?)
        .await?
    {
        return Err(AppError::Conflict(
            "First moderator bootstrap is already complete.".to_string(),
        ));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, email_verified, role_code)
        VALUES ($1, $2, TRUE, 'moderator')
        ON CONFLICT (email)
        DO UPDATE SET
            password_hash = EXCLUDED.password_hash,
            email_verified = TRUE,
            role_code = 'moderator'
        RETURNING id, email, role_code
        "#,
    )
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(tx.acquire().await.map_err(internal_db_err)?)
    .await
    .map_err(|err| {
        error!("database error creating first moderator: {}", err);
        AppError::Internal("Failed to bootstrap first moderator.".to_string())
    })?;

    let user_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
    let returned_email: String = row.try_get("email").map_err(internal_db_err)?;
    let role_code: String = row.try_get("role_code").map_err(internal_db_err)?;

    sqlx::query(
        r#"
        INSERT INTO deployment_audit_events (event_type, actor_user_id, metadata)
        VALUES ('first_moderator_bootstrap', $1, $2)
        "#,
    )
    .bind(user_id)
    .bind(json!({
        "role_code": role_code,
        "bootstrap_token_env": BOOTSTRAP_TOKEN_ENV
    }))
    .execute(tx.acquire().await.map_err(internal_db_err)?)
    .await
    .map_err(|err| {
        error!("database error auditing first moderator bootstrap: {}", err);
        AppError::Internal("Failed to audit first moderator bootstrap.".to_string())
    })?;

    tx.commit().await.map_err(|err| {
        error!("database error committing bootstrap transaction: {}", err);
        AppError::Internal("Failed to bootstrap first moderator.".to_string())
    })?;

    Ok((
        StatusCode::CREATED,
        Json(FirstModeratorBootstrapResponse {
            ok: true,
            email: returned_email,
            role_code,
            bootstrap_complete: true,
        }),
    ))
}

pub async fn first_moderator_bootstrap_complete(db: &sqlx::PgPool) -> bool {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM users
            WHERE role_code = 'moderator'
              AND email_verified = TRUE
        ) AS complete
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    row.and_then(|row| row.try_get("complete").ok())
        .unwrap_or(false)
}

fn configured_bootstrap_token() -> Result<String, AppError> {
    let token = env::var(BOOTSTRAP_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Forbidden("First moderator bootstrap is not configured.".to_string())
        })?;

    if token.chars().count() < MIN_BOOTSTRAP_TOKEN_CHARS {
        return Err(AppError::Forbidden(
            "First moderator bootstrap token is not configured with a safe value.".to_string(),
        ));
    }

    Ok(token)
}

async fn enforce_bootstrap_rate_limit(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<(), AppError> {
    let key = format!(
        "bootstrap:first-moderator:{}",
        client_rate_limit_identity(headers)
    );
    state
        .rate_limiter
        .check(
            key,
            BOOTSTRAP_RATE_LIMIT_MAX,
            Duration::minutes(BOOTSTRAP_RATE_LIMIT_WINDOW_MINUTES),
        )
        .await
}

async fn first_moderator_bootstrap_complete_in_executor<'c, E>(
    executor: E,
) -> Result<bool, AppError>
where
    E: sqlx::Executor<'c, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM users
            WHERE role_code = 'moderator'
              AND email_verified = TRUE
        ) AS complete
        "#,
    )
    .fetch_one(executor)
    .await
    .map_err(|err| {
        error!("database error checking first moderator bootstrap: {}", err);
        AppError::Internal("Failed to check first moderator bootstrap.".to_string())
    })?;

    row.try_get("complete").map_err(internal_db_err)
}

fn client_rate_limit_identity(headers: &HeaderMap) -> String {
    for header_name in ["cf-connecting-ip", "true-client-ip", "x-real-ip"] {
        if let Some(value) = normalized_header_value(headers, header_name) {
            return format!("{header_name}:{value}");
        }
    }

    if let Some(value) = normalized_header_value(headers, "x-forwarded-for") {
        if let Some(first_ip) = value
            .split(',')
            .map(str::trim)
            .find(|part| !part.is_empty())
        {
            return format!("x-forwarded-for:{first_ip}");
        }
    }

    "unknown-client".to_string()
}

fn normalized_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0;
    for (left_byte, right_byte) in left.iter().zip(right.iter()) {
        diff |= left_byte ^ right_byte;
    }

    diff == 0
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read bootstrap data.".to_string())
}

use std::sync::Arc;

use argon2::{
    Argon2, PasswordHasher,
    password_hash::{
        PasswordHash, PasswordHashString, PasswordVerifier, SaltString, rand_core::OsRng,
    },
};
use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{COOKIE, SET_COOKIE},
        request::Parts,
    },
};
use rand::{Rng, distributions::Alphanumeric};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tracing::error;
use uuid::Uuid;

use crate::{AppState, anti_abuse, csrf::CSRF_COOKIE_NAME, error::AppError};

pub const SESSION_COOKIE_NAME: &str = "ck_session";
const SESSION_DURATION_HOURS: i64 = 24 * 30;
const AUTH_RATE_LIMIT_WINDOW_MINUTES: i64 = 15;
const LOGIN_RATE_LIMIT_MAX: usize = 10;
const REGISTER_RATE_LIMIT_MAX: usize = 5;
const EMAIL_TOKEN_RATE_LIMIT_MAX: usize = 3;
const VERIFY_EMAIL_RATE_LIMIT_MAX: usize = 10;
const PASSWORD_RESET_REQUEST_RATE_LIMIT_MAX: usize = 5;
const PASSWORD_RESET_CONFIRM_RATE_LIMIT_MAX: usize = 10;
const PASSWORD_RESET_DURATION_HOURS: i64 = 1;
const TURNSTILE_SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const DEV_AUTH_TOKEN_ENV: &str = "CK_EXPOSE_DEV_AUTH_TOKENS";
const LEGACY_DEV_EMAIL_TOKEN_ENV: &str = "CK_EXPOSE_DEV_EMAIL_TOKENS";
#[cfg(debug_assertions)]
const DEV_SEED_ACCOUNTS_ENV: &str = "CK_SEED_DEV_ACCOUNTS";
#[cfg(debug_assertions)]
const DEV_ACCOUNT_PASSWORD: &str = "SuperSecurePass123";
#[cfg(debug_assertions)]
const DEV_ACCOUNTS: [(&str, &str); 3] = [
    ("user@example.com", "registered_user"),
    ("moderator@example.com", "moderator"),
    ("test2@example.com", "registered_user"),
];

fn env_is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn is_production_environment() -> bool {
    for key in ["APP_ENV", "RUST_ENV"] {
        if let Ok(value) = std::env::var(key) {
            if value.trim().eq_ignore_ascii_case("production") {
                return true;
            }
        }
    }

    false
}

fn expose_dev_auth_tokens() -> bool {
    if is_production_environment() {
        return false;
    }

    std::env::var(DEV_AUTH_TOKEN_ENV)
        .or_else(|_| std::env::var(LEGACY_DEV_EMAIL_TOKEN_ENV))
        .map(|value| env_is_truthy(&value))
        .unwrap_or(cfg!(debug_assertions))
}

#[cfg(debug_assertions)]
fn seed_development_accounts_enabled() -> bool {
    if is_production_environment() {
        return false;
    }

    std::env::var(DEV_SEED_ACCOUNTS_ENV)
        .map(|value| env_is_truthy(&value))
        .unwrap_or(true)
}

#[cfg(debug_assertions)]
pub async fn seed_development_accounts(db: &PgPool) {
    if !seed_development_accounts_enabled() {
        return;
    }

    for (email, role_code) in DEV_ACCOUNTS {
        let password_hash = match hash_password(DEV_ACCOUNT_PASSWORD) {
            Ok(hash) => hash,
            Err(err) => {
                error!("failed to hash dev account password: {:?}", err);
                return;
            }
        };

        let result = sqlx::query(
            r#"
            INSERT INTO users (email, password_hash, email_verified, role_code)
            VALUES ($1, $2, TRUE, $3)
            ON CONFLICT (email) DO NOTHING
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .bind(role_code)
        .execute(db)
        .await;

        match result {
            Ok(query_result) if query_result.rows_affected() > 0 => {
                tracing::info!("seeded development account {} as {}", email, role_code);
            }
            Ok(_) => {}
            Err(err) => {
                error!("failed to seed development account {}: {}", email, err);
            }
        }
    }
}

#[cfg(not(debug_assertions))]
pub async fn seed_development_accounts(_db: &PgPool) {}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub turnstile_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub ok: bool,
    pub email: String,
    pub email_verified: bool,
    pub verification_required: bool,
    pub verification_email_sent: bool,
    pub dev_verification_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub ok: bool,
    pub email: String,
    pub email_verified: bool,
    pub role_code: String,
    pub onboarding_required: bool,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub ok: bool,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyEmailResponse {
    pub ok: bool,
    pub email_verified: bool,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailLinkRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct EmailVerificationTokenResponse {
    pub ok: bool,
    pub email_verified: bool,
    pub verification_email_sent: bool,
    pub dev_verification_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
    pub turnstile_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PasswordResetRequestResponse {
    pub ok: bool,
    pub dev_reset_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetConfirmRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct PasswordResetConfirmResponse {
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub ok: bool,
    pub email: String,
    pub email_verified: bool,
    pub role_code: String,
    pub onboarding_required: bool,
}

#[derive(Debug, Deserialize)]
struct TurnstileSiteverifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub email: String,
    pub email_verified: bool,
    pub role_code: String,
    pub onboarding_required: bool,
}

impl AuthUser {
    pub fn can_moderate(&self) -> bool {
        self.role_code == "moderator"
    }

    pub fn require_verified(&self) -> Result<(), AppError> {
        if self.email_verified {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "Email verification is required for this action.".to_string(),
            ))
        }
    }
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let cookie_header = parts
            .headers
            .get(COOKIE)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Not authenticated.".to_string()))?;

        let session_token = extract_cookie_value(cookie_header, SESSION_COOKIE_NAME)
            .ok_or_else(|| AppError::Unauthorized("Not authenticated.".to_string()))?;

        let row = sqlx::query(
            r#"
            SELECT
                s.id AS session_id,
                u.id AS user_id,
                u.email,
                u.email_verified,
                u.role_code,
                u.last_login_at IS NULL AS onboarding_required
            FROM sessions s
            JOIN users u ON u.id = s.user_id
            WHERE s.session_token = encode(digest($1, 'sha256'), 'hex')
              AND s.revoked_at IS NULL
              AND s.expires_at > NOW()
            "#,
        )
        .bind(session_token)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            error!("database error loading session user: {}", err);
            AppError::Internal("Failed to load authenticated user.".to_string())
        })?;

        let Some(row) = row else {
            return Err(AppError::Unauthorized("Not authenticated.".to_string()));
        };

        Ok(AuthUser {
            session_id: row.try_get("session_id").map_err(internal_db_err)?,
            user_id: row.try_get("user_id").map_err(internal_db_err)?,
            email: row.try_get("email").map_err(internal_db_err)?,
            email_verified: row.try_get("email_verified").map_err(internal_db_err)?,
            role_code: row.try_get("role_code").map_err(internal_db_err)?,
            onboarding_required: row
                .try_get("onboarding_required")
                .map_err(internal_db_err)?,
        })
    }
}

pub async fn register_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    let email = normalize_email(&payload.email)?;
    enforce_auth_rate_limit(
        &state,
        &headers,
        "register",
        &email,
        REGISTER_RATE_LIMIT_MAX,
    )
    .await?;
    enforce_turnstile(&headers, payload.turnstile_token.as_deref()).await?;
    validate_password(&payload.password)?;
    let password_hash = hash_password(&payload.password)?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, email_verified, role_code)
        VALUES ($1, $2, FALSE, 'registered_user')
        RETURNING id, email, email_verified
        "#,
    )
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await;

    match insert_result {
        Ok(row) => {
            let user_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
            let verification_token = create_email_verification_token(&state.db, user_id).await?;
            let email: String = row.try_get("email").map_err(internal_db_err)?;
            let verification_email_sent =
                send_verification_email(&state, &email, &verification_token).await;
            let dev_verification_token = expose_dev_auth_tokens().then_some(verification_token);
            let response = RegisterResponse {
                ok: true,
                email,
                email_verified: row.try_get("email_verified").map_err(internal_db_err)?,
                verification_required: true,
                verification_email_sent,
                dev_verification_token,
            };

            Ok(Json(response))
        }
        Err(sqlx::Error::Database(db_err)) => {
            if let Some(constraint) = db_err.constraint() {
                if constraint == "users_email_key" {
                    return Err(AppError::Conflict(
                        "An account with that email already exists.".to_string(),
                    ));
                }
            }

            error!("database error during register: {}", db_err);
            Err(AppError::Internal("Failed to create account.".to_string()))
        }
        Err(err) => {
            error!("unexpected sqlx error during register: {}", err);
            Err(AppError::Internal("Failed to create account.".to_string()))
        }
    }
}

pub async fn verify_email_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
    Json(payload): Json<VerifyEmailRequest>,
) -> Result<Json<VerifyEmailResponse>, AppError> {
    let token = payload.token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "Verification token is required.".to_string(),
        ));
    }

    enforce_auth_rate_limit(
        &state,
        &headers,
        "verify-email",
        &auth_user.user_id.to_string(),
        VERIFY_EMAIL_RATE_LIMIT_MAX,
    )
    .await?;

    let token_row = sqlx::query(
        r#"
        SELECT id
        FROM email_verification_tokens
        WHERE user_id = $1
          AND token = encode(digest($2, 'sha256'), 'hex')
          AND consumed_at IS NULL
          AND expires_at > NOW()
        LIMIT 1
        "#,
    )
    .bind(auth_user.user_id)
    .bind(token)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading email verification token: {}", err);
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    let Some(token_row) = token_row else {
        return Err(AppError::BadRequest(
            "Verification token is invalid or expired.".to_string(),
        ));
    };

    let token_id: Uuid = token_row.try_get("id").map_err(internal_db_err)?;

    sqlx::query(
        r#"
        UPDATE users
        SET email_verified = TRUE
        WHERE id = $1
        "#,
    )
    .bind(auth_user.user_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error marking email verified: {}", err);
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE email_verification_tokens
        SET consumed_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error consuming email verification token: {}", err);
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    Ok(Json(VerifyEmailResponse {
        ok: true,
        email_verified: true,
    }))
}

pub async fn verify_email_link_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifyEmailLinkRequest>,
) -> Result<(StatusCode, HeaderMap, Json<LoginResponse>), AppError> {
    let token = payload.token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "Verification link is missing its code.".to_string(),
        ));
    }

    enforce_auth_rate_limit(
        &state,
        &headers,
        "verify-email-link",
        "link",
        VERIFY_EMAIL_RATE_LIMIT_MAX,
    )
    .await?;

    let mut tx = state.db.begin().await.map_err(|err| {
        error!(
            "database error starting email verification link transaction: {}",
            err
        );
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    let token_row = sqlx::query(
        r#"
        SELECT
            evt.id AS token_id,
            u.id AS user_id,
            u.email,
            u.role_code,
            u.last_login_at IS NULL AS onboarding_required
        FROM email_verification_tokens evt
        JOIN users u ON u.id = evt.user_id
        WHERE evt.token = encode(digest($1, 'sha256'), 'hex')
          AND evt.consumed_at IS NULL
          AND evt.expires_at > NOW()
        LIMIT 1
        FOR UPDATE OF evt, u
        "#,
    )
    .bind(token)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| {
        error!(
            "database error loading email verification link token: {}",
            err
        );
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    let Some(token_row) = token_row else {
        return Err(AppError::BadRequest(
            "Verification link is invalid or expired.".to_string(),
        ));
    };

    let user_id: Uuid = token_row.try_get("user_id").map_err(internal_db_err)?;
    let email: String = token_row.try_get("email").map_err(internal_db_err)?;
    let role_code: String = token_row.try_get("role_code").map_err(internal_db_err)?;
    let onboarding_required: bool = token_row
        .try_get("onboarding_required")
        .map_err(internal_db_err)?;

    sqlx::query(
        r#"
        UPDATE users
        SET email_verified = TRUE,
            last_login_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error marking email verified from link: {}", err);
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE email_verification_tokens
        SET consumed_at = NOW()
        WHERE user_id = $1
          AND consumed_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!(
            "database error consuming email verification tokens from link: {}",
            err
        );
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    let session_token = generate_session_token();
    let csrf_token = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(SESSION_DURATION_HOURS);

    sqlx::query(
        r#"
        INSERT INTO sessions (user_id, session_token, expires_at)
        VALUES ($1, encode(digest($2, 'sha256'), 'hex'), $3)
        "#,
    )
    .bind(user_id)
    .bind(&session_token)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!(
            "database error creating session after email verification link: {}",
            err
        );
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    tx.commit().await.map_err(|err| {
        error!("database error committing email verification link: {}", err);
        AppError::Internal("Failed to verify email.".to_string())
    })?;

    let response_headers = build_session_response_headers(&session_token, &csrf_token)?;
    let response = LoginResponse {
        ok: true,
        email,
        email_verified: true,
        role_code,
        onboarding_required,
    };

    Ok((StatusCode::OK, response_headers, Json(response)))
}

pub async fn email_verification_token_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    headers: HeaderMap,
) -> Result<Json<EmailVerificationTokenResponse>, AppError> {
    if auth_user.email_verified {
        return Ok(Json(EmailVerificationTokenResponse {
            ok: true,
            email_verified: true,
            verification_email_sent: false,
            dev_verification_token: None,
        }));
    }

    enforce_auth_rate_limit(
        &state,
        &headers,
        "email-token",
        &auth_user.user_id.to_string(),
        EMAIL_TOKEN_RATE_LIMIT_MAX,
    )
    .await?;

    let verification_token = create_email_verification_token(&state.db, auth_user.user_id).await?;
    let verification_email_sent =
        send_verification_email(&state, &auth_user.email, &verification_token).await;

    Ok(Json(EmailVerificationTokenResponse {
        ok: true,
        email_verified: false,
        verification_email_sent,
        dev_verification_token: expose_dev_auth_tokens().then_some(verification_token),
    }))
}

pub async fn password_reset_request_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<PasswordResetRequest>,
) -> Result<Json<PasswordResetRequestResponse>, AppError> {
    let email = normalize_email(&payload.email)?;
    enforce_auth_rate_limit(
        &state,
        &headers,
        "password-reset-request",
        &email,
        PASSWORD_RESET_REQUEST_RATE_LIMIT_MAX,
    )
    .await?;
    enforce_turnstile(&headers, payload.turnstile_token.as_deref()).await?;

    let user_row = sqlx::query(
        r#"
        SELECT id, email
        FROM users
        WHERE email = $1
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error during password reset lookup: {}", err);
        AppError::Internal("Failed to request password reset.".to_string())
    })?;

    let Some(user_row) = user_row else {
        return Ok(Json(PasswordResetRequestResponse {
            ok: true,
            dev_reset_token: None,
        }));
    };

    let user_id: Uuid = user_row.try_get("id").map_err(internal_db_err)?;
    let email: String = user_row.try_get("email").map_err(internal_db_err)?;
    let reset_token = create_password_reset_token(&state.db, user_id).await?;
    let _reset_email_sent = send_password_reset_email(&state, &email, &reset_token).await;

    Ok(Json(PasswordResetRequestResponse {
        ok: true,
        dev_reset_token: expose_dev_auth_tokens().then_some(reset_token),
    }))
}

pub async fn password_reset_confirm_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<PasswordResetConfirmRequest>,
) -> Result<Json<PasswordResetConfirmResponse>, AppError> {
    let token = payload.token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest("Reset token is required.".to_string()));
    }

    enforce_auth_rate_limit(
        &state,
        &headers,
        "password-reset-confirm",
        "all",
        PASSWORD_RESET_CONFIRM_RATE_LIMIT_MAX,
    )
    .await?;
    validate_password(&payload.new_password)?;
    let password_hash = hash_password(&payload.new_password)?;

    let mut tx = state.db.begin().await.map_err(|err| {
        error!(
            "database error starting password reset transaction: {}",
            err
        );
        AppError::Internal("Failed to reset password.".to_string())
    })?;

    let token_row = sqlx::query(
        r#"
        SELECT user_id
        FROM password_reset_tokens
        WHERE token = encode(digest($1, 'sha256'), 'hex')
          AND consumed_at IS NULL
          AND expires_at > NOW()
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(token)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error loading password reset token: {}", err);
        AppError::Internal("Failed to reset password.".to_string())
    })?;

    let Some(token_row) = token_row else {
        return Err(AppError::BadRequest(
            "Reset token is invalid or expired.".to_string(),
        ));
    };

    let user_id: Uuid = token_row.try_get("user_id").map_err(internal_db_err)?;

    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = $2,
            email_verified = TRUE
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error updating password: {}", err);
        AppError::Internal("Failed to reset password.".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET consumed_at = NOW()
        WHERE user_id = $1
          AND consumed_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("database error consuming password reset tokens: {}", err);
        AppError::Internal("Failed to reset password.".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE sessions
        SET revoked_at = NOW()
        WHERE user_id = $1
          AND revoked_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!(
            "database error revoking sessions after password reset: {}",
            err
        );
        AppError::Internal("Failed to reset password.".to_string())
    })?;

    tx.commit().await.map_err(|err| {
        error!("database error committing password reset: {}", err);
        AppError::Internal("Failed to reset password.".to_string())
    })?;

    Ok(Json(PasswordResetConfirmResponse { ok: true }))
}

pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    request_headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<(StatusCode, HeaderMap, Json<LoginResponse>), AppError> {
    let email = normalize_email(&payload.email)?;
    enforce_auth_rate_limit(
        &state,
        &request_headers,
        "login",
        &email,
        LOGIN_RATE_LIMIT_MAX,
    )
    .await?;

    if payload.password.is_empty() {
        return Err(AppError::BadRequest("Password is required.".to_string()));
    }

    let row = sqlx::query(
        r#"
        SELECT
            id,
            email,
            password_hash,
            email_verified,
            role_code,
            last_login_at IS NULL AS onboarding_required
        FROM users
        WHERE email = $1
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error during login lookup: {}", err);
        AppError::Internal("Failed to process login.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Invalid email or password.".to_string(),
        ));
    };

    let password_hash: String = row.try_get("password_hash").map_err(internal_db_err)?;
    verify_password(&payload.password, &password_hash)?;

    let user_id: Uuid = row.try_get("id").map_err(internal_db_err)?;
    let email: String = row.try_get("email").map_err(internal_db_err)?;
    let email_verified: bool = row.try_get("email_verified").map_err(internal_db_err)?;
    let role_code: String = row.try_get("role_code").map_err(internal_db_err)?;
    let onboarding_required: bool = row
        .try_get("onboarding_required")
        .map_err(internal_db_err)?;

    let session_token = generate_session_token();
    let csrf_token = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(SESSION_DURATION_HOURS);

    let session_row = sqlx::query(
        r#"
        INSERT INTO sessions (user_id, session_token, expires_at)
        VALUES ($1, encode(digest($2, 'sha256'), 'hex'), $3)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&session_token)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        error!("database error during session creation: {}", err);
        AppError::Internal("Failed to create session.".to_string())
    })?;

    let _session_id: Uuid = session_row.try_get("id").map_err(internal_db_err)?;

    anti_abuse::record_user_activity(
        &state.db,
        user_id,
        "login",
        None,
        None,
        &request_headers,
        serde_json::json!({
            "email_verified": email_verified,
            "role_code": role_code
        }),
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE users
        SET last_login_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error updating last login: {}", err);
        AppError::Internal("Failed to process login.".to_string())
    })?;

    let response_headers = build_session_response_headers(&session_token, &csrf_token)?;

    let response = LoginResponse {
        ok: true,
        email,
        email_verified,
        role_code,
        onboarding_required,
    };

    Ok((StatusCode::OK, response_headers, Json(response)))
}

pub async fn logout_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<(StatusCode, HeaderMap, Json<LogoutResponse>), AppError> {
    sqlx::query(
        r#"
        UPDATE sessions
        SET revoked_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(auth_user.session_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        error!("database error during logout: {}", err);
        AppError::Internal("Failed to log out.".to_string())
    })?;

    let mut headers = HeaderMap::new();
    headers.append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_session_cookie()?)
            .map_err(|_| AppError::Internal("Failed to clear session cookie.".to_string()))?,
    );
    headers.append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_csrf_cookie()?)
            .map_err(|_| AppError::Internal("Failed to clear CSRF cookie.".to_string()))?,
    );

    Ok((StatusCode::OK, headers, Json(LogoutResponse { ok: true })))
}

pub async fn me_handler(auth_user: AuthUser) -> Result<Json<MeResponse>, AppError> {
    Ok(Json(MeResponse {
        ok: true,
        email: auth_user.email,
        email_verified: auth_user.email_verified,
        role_code: auth_user.role_code,
        onboarding_required: auth_user.onboarding_required,
    }))
}

pub(crate) fn normalize_email(email: &str) -> Result<String, AppError> {
    let normalized = email.trim().to_lowercase();

    if normalized.is_empty() {
        return Err(AppError::BadRequest("Email is required.".to_string()));
    }

    if normalized.len() > 320 {
        return Err(AppError::BadRequest("Email is too long.".to_string()));
    }

    if !normalized.contains('@') {
        return Err(AppError::BadRequest("Email must be valid.".to_string()));
    }

    Ok(normalized)
}

pub(crate) fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 12 {
        return Err(AppError::BadRequest(
            "Password must be at least 12 characters.".to_string(),
        ));
    }

    if password.len() > 256 {
        return Err(AppError::BadRequest("Password is too long.".to_string()));
    }

    Ok(())
}

pub(crate) fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| {
            error!("password hashing failed: {}", err);
            AppError::Internal("Failed to process password.".to_string())
        })?;

    Ok(PasswordHashString::from(hash).to_string())
}

fn verify_password(password: &str, stored_hash: &str) -> Result<(), AppError> {
    let parsed_hash = PasswordHash::new(stored_hash).map_err(|err| {
        error!("stored password hash could not be parsed: {}", err);
        AppError::Internal("Failed to process login.".to_string())
    })?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::BadRequest("Invalid email or password.".to_string()))
}

fn generate_session_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

fn generate_email_verification_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn generate_password_reset_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

async fn create_email_verification_token(
    db: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<String, AppError> {
    sqlx::query(
        r#"
        UPDATE email_verification_tokens
        SET consumed_at = NOW()
        WHERE user_id = $1
          AND consumed_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error expiring verification tokens: {}", err);
        AppError::Internal("Failed to create verification token.".to_string())
    })?;

    let token = generate_email_verification_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    sqlx::query(
        r#"
        INSERT INTO email_verification_tokens (user_id, token, expires_at)
        VALUES ($1, encode(digest($2, 'sha256'), 'hex'), $3)
        "#,
    )
    .bind(user_id)
    .bind(&token)
    .bind(expires_at)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting verification token: {}", err);
        AppError::Internal("Failed to create verification token.".to_string())
    })?;

    Ok(token)
}

async fn create_password_reset_token(db: &sqlx::PgPool, user_id: Uuid) -> Result<String, AppError> {
    sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET consumed_at = NOW()
        WHERE user_id = $1
          AND consumed_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error expiring password reset tokens: {}", err);
        AppError::Internal("Failed to create password reset token.".to_string())
    })?;

    let token = generate_password_reset_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(PASSWORD_RESET_DURATION_HOURS);

    sqlx::query(
        r#"
        INSERT INTO password_reset_tokens (user_id, token, expires_at)
        VALUES ($1, encode(digest($2, 'sha256'), 'hex'), $3)
        "#,
    )
    .bind(user_id)
    .bind(&token)
    .bind(expires_at)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting password reset token: {}", err);
        AppError::Internal("Failed to create password reset token.".to_string())
    })?;

    Ok(token)
}

fn build_session_cookie(token: &str) -> Result<String, AppError> {
    Ok(format!(
        "{name}={token}; HttpOnly; Path=/; Max-Age={max_age}; SameSite=Lax{secure}",
        name = SESSION_COOKIE_NAME,
        max_age = SESSION_DURATION_HOURS * 60 * 60,
        secure = session_cookie_secure_suffix()
    ))
}

fn build_csrf_cookie(token: &str) -> Result<String, AppError> {
    Ok(format!(
        "{name}={token}; Path=/; Max-Age={max_age}; SameSite=Lax{secure}",
        name = CSRF_COOKIE_NAME,
        max_age = SESSION_DURATION_HOURS * 60 * 60,
        secure = session_cookie_secure_suffix()
    ))
}

fn build_session_response_headers(
    session_token: &str,
    csrf_token: &str,
) -> Result<HeaderMap, AppError> {
    let mut response_headers = HeaderMap::new();
    response_headers.append(
        SET_COOKIE,
        HeaderValue::from_str(&build_session_cookie(session_token)?)
            .map_err(|_| AppError::Internal("Failed to set session cookie.".to_string()))?,
    );
    response_headers.append(
        SET_COOKIE,
        HeaderValue::from_str(&build_csrf_cookie(csrf_token)?)
            .map_err(|_| AppError::Internal("Failed to set CSRF cookie.".to_string()))?,
    );

    Ok(response_headers)
}

fn clear_session_cookie() -> Result<String, AppError> {
    Ok(format!(
        "{name}=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax{secure}",
        name = SESSION_COOKIE_NAME,
        secure = session_cookie_secure_suffix()
    ))
}

fn clear_csrf_cookie() -> Result<String, AppError> {
    Ok(format!(
        "{name}=; Path=/; Max-Age=0; SameSite=Lax{secure}",
        name = CSRF_COOKIE_NAME,
        secure = session_cookie_secure_suffix()
    ))
}

fn session_cookie_secure_suffix() -> &'static str {
    if is_production_environment() {
        "; Secure"
    } else {
        ""
    }
}

async fn send_verification_email(state: &Arc<AppState>, email: &str, token: &str) -> bool {
    match state.mailer.send_verification_email(email, token).await {
        Ok(()) => true,
        Err(err) => {
            error!("failed to send verification email to {}: {}", email, err);
            false
        }
    }
}

async fn send_password_reset_email(state: &Arc<AppState>, email: &str, token: &str) -> bool {
    match state.mailer.send_password_reset_email(email, token).await {
        Ok(()) => true,
        Err(err) => {
            error!("failed to send password reset email to {}: {}", email, err);
            false
        }
    }
}

async fn enforce_turnstile(
    headers: &HeaderMap,
    turnstile_token: Option<&str>,
) -> Result<(), AppError> {
    let Some(secret) = turnstile_secret_key() else {
        return Ok(());
    };

    let token = turnstile_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest("Complete the human check and try again.".to_string())
        })?;

    let mut form = vec![
        ("secret".to_string(), secret),
        ("response".to_string(), token.to_string()),
    ];

    if let Some(remote_ip) = turnstile_remote_ip(headers) {
        form.push(("remoteip".to_string(), remote_ip));
    }

    let response = reqwest::Client::new()
        .post(TURNSTILE_SITEVERIFY_URL)
        .form(&form)
        .send()
        .await
        .map_err(|err| {
            error!("turnstile verification request failed: {}", err);
            AppError::Internal("Failed to verify human check.".to_string())
        })?;

    if !response.status().is_success() {
        error!(
            "turnstile verification returned HTTP status {}",
            response.status()
        );
        return Err(AppError::BadRequest(
            "Human check failed. Refresh and try again.".to_string(),
        ));
    }

    let payload = response
        .json::<TurnstileSiteverifyResponse>()
        .await
        .map_err(|err| {
            error!(
                "turnstile verification response could not be decoded: {}",
                err
            );
            AppError::Internal("Failed to verify human check.".to_string())
        })?;

    if payload.success {
        return Ok(());
    }

    error!(
        "turnstile verification failed with codes: {:?}",
        payload.error_codes
    );
    Err(AppError::BadRequest(
        "Human check failed. Refresh and try again.".to_string(),
    ))
}

async fn enforce_auth_rate_limit(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    action: &str,
    subject: &str,
    max_attempts: usize,
) -> Result<(), AppError> {
    let key = format!(
        "auth:{action}:{}:{}",
        client_rate_limit_identity(headers),
        subject.trim().to_ascii_lowercase()
    );

    state
        .rate_limiter
        .check(
            key,
            max_attempts,
            chrono::Duration::minutes(AUTH_RATE_LIMIT_WINDOW_MINUTES),
        )
        .await
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

fn turnstile_secret_key() -> Option<String> {
    std::env::var("CF_TURNSTILE_SECRET_KEY")
        .or_else(|_| std::env::var("CK_TURNSTILE_SECRET_KEY"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn turnstile_remote_ip(headers: &HeaderMap) -> Option<String> {
    for header_name in ["cf-connecting-ip", "true-client-ip", "x-real-ip"] {
        if let Some(value) = normalized_header_value(headers, header_name) {
            return Some(value);
        }
    }

    normalized_header_value(headers, "x-forwarded-for").and_then(|value| {
        value
            .split(',')
            .map(str::trim)
            .find(|part| !part.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn normalized_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header.split(';').find_map(|part| {
        let trimmed = part.trim();
        let (name, value) = trimmed.split_once('=')?;
        if name == cookie_name {
            Some(value)
        } else {
            None
        }
    })
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read user data.".to_string())
}

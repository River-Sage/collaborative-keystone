use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::{AppState, auth::AuthUser, error::AppError};

const NEW_ACCOUNT_HOURS: i64 = 24;
const NEW_ACCOUNT_ACTIVITY_THRESHOLD: i64 = 5;
const RAPID_REVIEW_THRESHOLD: i64 = 4;
const RAPID_REVIEW_MINUTES: i64 = 2;
const RAPID_VOTE_THRESHOLD: i64 = 12;
const RAPID_VOTE_MINUTES: i64 = 10;
const SHARED_CLIENT_USER_THRESHOLD: i64 = 5;
const SHARED_DEVICE_USER_THRESHOLD: i64 = 4;
const MERGE_CLUSTER_USER_THRESHOLD: i64 = 3;

#[derive(Debug, Serialize)]
pub struct AntiAbuseQueueResponse {
    pub ok: bool,
    pub flags: Vec<AntiAbuseFlagSummary>,
}

#[derive(Debug, Serialize)]
pub struct AntiAbuseResolveResponse {
    pub ok: bool,
    pub flag_id: Uuid,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AntiAbuseFlagSummary {
    pub id: Uuid,
    pub user_public_id: Option<String>,
    pub flag_code: String,
    pub severity: String,
    pub status: String,
    pub proposal_id: Option<Uuid>,
    pub proposal_title: Option<String>,
    pub related_proposal_id: Option<Uuid>,
    pub related_proposal_title: Option<String>,
    pub client_ip_hint: Option<String>,
    pub user_agent_hash: Option<String>,
    pub details: Value,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by_moderator_user_id: Option<Uuid>,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveAntiAbuseFlagRequest {
    pub outcome: String,
    pub resolution_note: String,
}

#[derive(Clone)]
struct ClientSignals {
    client_ip_hint: Option<String>,
    user_agent_hash: Option<String>,
}

pub async fn record_user_activity(
    db: &sqlx::PgPool,
    user_id: Uuid,
    event_type: &str,
    proposal_id: Option<Uuid>,
    related_proposal_id: Option<Uuid>,
    headers: &HeaderMap,
    metadata: Value,
) -> Result<(), AppError> {
    let signals = capture_client_signals(headers);

    sqlx::query(
        r#"
        INSERT INTO user_activity_events (
            user_id,
            event_type,
            proposal_id,
            related_proposal_id,
            client_ip_hint,
            user_agent_hash,
            metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(user_id)
    .bind(event_type)
    .bind(proposal_id)
    .bind(related_proposal_id)
    .bind(&signals.client_ip_hint)
    .bind(&signals.user_agent_hash)
    .bind(metadata)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error recording user activity: {}", err);
        AppError::Internal("Failed to record activity signal.".to_string())
    })?;

    detect_anomalies(
        db,
        user_id,
        event_type,
        proposal_id,
        related_proposal_id,
        &signals,
    )
    .await
}

pub async fn anti_abuse_review_queue_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<AntiAbuseQueueResponse>, AppError> {
    require_moderator(&auth_user)?;

    let rows = sqlx::query(
        r#"
        SELECT
            f.id,
            CASE
                WHEN f.user_id IS NULL THEN NULL
                ELSE 'user-' || LEFT(REPLACE(f.user_id::text, '-', ''), 10)
            END AS user_public_id,
            f.flag_code,
            f.severity,
            f.status,
            f.proposal_id,
            p.title AS proposal_title,
            f.related_proposal_id,
            rp.title AS related_proposal_title,
            f.client_ip_hint,
            f.user_agent_hash,
            f.details,
            f.created_at,
            f.reviewed_at,
            f.reviewed_by_moderator_user_id,
            f.resolution_note
        FROM anti_abuse_flags f
        LEFT JOIN proposals p ON p.id = f.proposal_id
        LEFT JOIN proposals rp ON rp.id = f.related_proposal_id
        WHERE f.status = 'open'
        ORDER BY
            CASE f.severity
                WHEN 'high' THEN 1
                WHEN 'medium' THEN 2
                ELSE 3
            END,
            f.created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        error!("database error loading anti-abuse queue: {}", err);
        AppError::Internal("Failed to load trust review queue.".to_string())
    })?;

    let flags = rows
        .into_iter()
        .map(map_flag_row)
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(AntiAbuseQueueResponse { ok: true, flags }))
}

pub async fn resolve_anti_abuse_flag_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(flag_id): Path<Uuid>,
    Json(payload): Json<ResolveAntiAbuseFlagRequest>,
) -> Result<Json<AntiAbuseResolveResponse>, AppError> {
    require_moderator(&auth_user)?;

    let outcome = payload.outcome.trim().to_lowercase();
    if outcome != "acknowledged" && outcome != "dismissed" {
        return Err(AppError::BadRequest(
            "outcome must be either 'acknowledged' or 'dismissed'.".to_string(),
        ));
    }

    let resolution_note = payload.resolution_note.trim().to_string();
    if resolution_note.is_empty() {
        return Err(AppError::BadRequest(
            "A resolution note is required.".to_string(),
        ));
    }

    let row = sqlx::query(
        r#"
        UPDATE anti_abuse_flags
        SET
            status = $2,
            reviewed_at = NOW(),
            reviewed_by_moderator_user_id = $3,
            resolution_note = $4
        WHERE id = $1
          AND status = 'open'
        RETURNING id, status
        "#,
    )
    .bind(flag_id)
    .bind(&outcome)
    .bind(auth_user.user_id)
    .bind(&resolution_note)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        error!("database error resolving anti-abuse flag: {}", err);
        AppError::Internal("Failed to resolve trust review flag.".to_string())
    })?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "Open trust review flag not found.".to_string(),
        ));
    };

    Ok(Json(AntiAbuseResolveResponse {
        ok: true,
        flag_id: row.try_get("id").map_err(internal_db_err)?,
        status: row.try_get("status").map_err(internal_db_err)?,
    }))
}

async fn detect_anomalies(
    db: &sqlx::PgPool,
    user_id: Uuid,
    event_type: &str,
    proposal_id: Option<Uuid>,
    related_proposal_id: Option<Uuid>,
    signals: &ClientSignals,
) -> Result<(), AppError> {
    let new_account_activity = sqlx::query(
        r#"
        SELECT
            (u.created_at > NOW() - ($2::text::interval)) AS is_new_account,
            (
                SELECT COUNT(*)::bigint
                FROM user_activity_events e
                WHERE e.user_id = u.id
                  AND e.created_at > NOW() - INTERVAL '1 hour'
                  AND e.event_type IN (
                    'proposal_created',
                    'review_action',
                    'sentiment_vote',
                    'merge_vote'
                  )
            ) AS recent_activity_count
        FROM users u
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .bind(format!("{NEW_ACCOUNT_HOURS} hours"))
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking account age anomaly: {}", err);
        AppError::Internal("Failed to evaluate activity signal.".to_string())
    })?;

    let is_new_account: bool = new_account_activity
        .try_get("is_new_account")
        .map_err(internal_db_err)?;
    let recent_activity_count: i64 = new_account_activity
        .try_get("recent_activity_count")
        .map_err(internal_db_err)?;
    if is_new_account && recent_activity_count >= NEW_ACCOUNT_ACTIVITY_THRESHOLD {
        insert_flag_once(
            db,
            Some(user_id),
            "new_account_activity",
            "medium",
            None,
            None,
            signals,
            json!({
                "summary": "New account has accumulated several participation actions in its first day.",
                "recent_activity_count": recent_activity_count,
                "account_age_threshold_hours": NEW_ACCOUNT_HOURS
            }),
        )
        .await?;
    }

    if event_type == "review_action" {
        let review_count =
            count_user_events(db, user_id, "review_action", RAPID_REVIEW_MINUTES).await?;
        if review_count >= RAPID_REVIEW_THRESHOLD {
            insert_flag_once(
                db,
                Some(user_id),
                "rapid_review_activity",
                "low",
                proposal_id,
                None,
                signals,
                json!({
                    "summary": "User completed required-review actions unusually quickly.",
                    "review_count": review_count,
                    "window_minutes": RAPID_REVIEW_MINUTES
                }),
            )
            .await?;
        }
    }

    let vote_like_count = count_vote_like_events(db, user_id, RAPID_VOTE_MINUTES).await?;
    if vote_like_count >= RAPID_VOTE_THRESHOLD {
        insert_flag_once(
            db,
            Some(user_id),
            "rapid_vote_activity",
            "medium",
            proposal_id,
            related_proposal_id,
            signals,
            json!({
                "summary": "User generated many review/vote actions in a short window.",
                "activity_count": vote_like_count,
                "window_minutes": RAPID_VOTE_MINUTES
            }),
        )
        .await?;
    }

    detect_client_cluster(db, user_id, signals).await?;

    if event_type == "merge_vote" {
        detect_merge_signal_cluster(db, user_id, proposal_id, related_proposal_id, signals).await?;
    }

    Ok(())
}

async fn detect_client_cluster(
    db: &sqlx::PgPool,
    user_id: Uuid,
    signals: &ClientSignals,
) -> Result<(), AppError> {
    let Some(client_ip_hint) = signals.client_ip_hint.as_deref() else {
        return Ok(());
    };

    if is_low_value_client_hint(client_ip_hint) {
        return Ok(());
    }

    let shared_client_count = sqlx::query(
        r#"
        SELECT COUNT(DISTINCT user_id)::bigint AS user_count
        FROM user_activity_events
        WHERE client_ip_hint = $1
          AND created_at > NOW() - INTERVAL '24 hours'
        "#,
    )
    .bind(client_ip_hint)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking shared client anomaly: {}", err);
        AppError::Internal("Failed to evaluate activity signal.".to_string())
    })?
    .try_get::<i64, _>("user_count")
    .map_err(internal_db_err)?;

    if shared_client_count >= SHARED_CLIENT_USER_THRESHOLD {
        insert_flag_once(
            db,
            Some(user_id),
            "shared_client_identity",
            "low",
            None,
            None,
            signals,
            json!({
                "summary": "Multiple accounts recently acted from the same network identity.",
                "distinct_user_count": shared_client_count,
                "window_hours": 24
            }),
        )
        .await?;
    }

    let Some(user_agent_hash) = signals.user_agent_hash.as_deref() else {
        return Ok(());
    };

    let shared_device_count = sqlx::query(
        r#"
        SELECT COUNT(DISTINCT user_id)::bigint AS user_count
        FROM user_activity_events
        WHERE client_ip_hint = $1
          AND user_agent_hash = $2
          AND created_at > NOW() - INTERVAL '24 hours'
        "#,
    )
    .bind(client_ip_hint)
    .bind(user_agent_hash)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking shared device anomaly: {}", err);
        AppError::Internal("Failed to evaluate activity signal.".to_string())
    })?
    .try_get::<i64, _>("user_count")
    .map_err(internal_db_err)?;

    if shared_device_count >= SHARED_DEVICE_USER_THRESHOLD {
        insert_flag_once(
            db,
            Some(user_id),
            "shared_device_browser_cluster",
            "medium",
            None,
            None,
            signals,
            json!({
                "summary": "Multiple accounts recently acted from the same network and browser signature.",
                "distinct_user_count": shared_device_count,
                "window_hours": 24
            }),
        )
        .await?;
    }

    Ok(())
}

async fn detect_merge_signal_cluster(
    db: &sqlx::PgPool,
    user_id: Uuid,
    proposal_id: Option<Uuid>,
    related_proposal_id: Option<Uuid>,
    signals: &ClientSignals,
) -> Result<(), AppError> {
    let (Some(source_id), Some(target_id), Some(client_ip_hint)) = (
        proposal_id,
        related_proposal_id,
        signals.client_ip_hint.as_deref(),
    ) else {
        return Ok(());
    };

    if is_low_value_client_hint(client_ip_hint) {
        return Ok(());
    }

    let cluster_count = sqlx::query(
        r#"
        SELECT COUNT(DISTINCT user_id)::bigint AS user_count
        FROM user_activity_events
        WHERE event_type = 'merge_vote'
          AND proposal_id = $1
          AND related_proposal_id = $2
          AND client_ip_hint = $3
          AND created_at > NOW() - INTERVAL '24 hours'
        "#,
    )
    .bind(source_id)
    .bind(target_id)
    .bind(client_ip_hint)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking merge signal cluster: {}", err);
        AppError::Internal("Failed to evaluate activity signal.".to_string())
    })?
    .try_get::<i64, _>("user_count")
    .map_err(internal_db_err)?;

    if cluster_count >= MERGE_CLUSTER_USER_THRESHOLD {
        insert_flag_once(
            db,
            Some(user_id),
            "merge_signal_cluster",
            "high",
            Some(source_id),
            Some(target_id),
            signals,
            json!({
                "summary": "Several accounts from the same network identity signaled the same duplicate relationship.",
                "distinct_user_count": cluster_count,
                "window_hours": 24
            }),
        )
        .await?;
    }

    Ok(())
}

async fn count_user_events(
    db: &sqlx::PgPool,
    user_id: Uuid,
    event_type: &str,
    window_minutes: i64,
) -> Result<i64, AppError> {
    sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS event_count
        FROM user_activity_events
        WHERE user_id = $1
          AND event_type = $2
          AND created_at > NOW() - ($3::text::interval)
        "#,
    )
    .bind(user_id)
    .bind(event_type)
    .bind(format!("{window_minutes} minutes"))
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error counting user events: {}", err);
        AppError::Internal("Failed to evaluate activity signal.".to_string())
    })?
    .try_get("event_count")
    .map_err(internal_db_err)
}

async fn count_vote_like_events(
    db: &sqlx::PgPool,
    user_id: Uuid,
    window_minutes: i64,
) -> Result<i64, AppError> {
    sqlx::query(
        r#"
        SELECT COUNT(*)::bigint AS event_count
        FROM user_activity_events
        WHERE user_id = $1
          AND event_type IN ('review_action', 'sentiment_vote', 'merge_vote')
          AND created_at > NOW() - ($2::text::interval)
        "#,
    )
    .bind(user_id)
    .bind(format!("{window_minutes} minutes"))
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error counting vote-like events: {}", err);
        AppError::Internal("Failed to evaluate activity signal.".to_string())
    })?
    .try_get("event_count")
    .map_err(internal_db_err)
}

async fn insert_flag_once(
    db: &sqlx::PgPool,
    user_id: Option<Uuid>,
    flag_code: &str,
    severity: &str,
    proposal_id: Option<Uuid>,
    related_proposal_id: Option<Uuid>,
    signals: &ClientSignals,
    details: Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO anti_abuse_flags (
            user_id,
            flag_code,
            severity,
            proposal_id,
            related_proposal_id,
            client_ip_hint,
            user_agent_hash,
            details
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(flag_code)
    .bind(severity)
    .bind(proposal_id)
    .bind(related_proposal_id)
    .bind(&signals.client_ip_hint)
    .bind(&signals.user_agent_hash)
    .bind(details)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting anti-abuse flag: {}", err);
        AppError::Internal("Failed to record trust review flag.".to_string())
    })?;

    Ok(())
}

fn capture_client_signals(headers: &HeaderMap) -> ClientSignals {
    ClientSignals {
        client_ip_hint: client_rate_limit_identity(headers),
        user_agent_hash: normalized_header_value(headers, "user-agent")
            .map(|value| stable_hash(&value)),
    }
}

fn client_rate_limit_identity(headers: &HeaderMap) -> Option<String> {
    for header_name in ["cf-connecting-ip", "true-client-ip", "x-real-ip"] {
        if let Some(value) = normalized_header_value(headers, header_name) {
            return Some(format!("{header_name}:{value}"));
        }
    }

    if let Some(value) = normalized_header_value(headers, "x-forwarded-for") {
        if let Some(first_ip) = value
            .split(',')
            .map(str::trim)
            .find(|part| !part.is_empty())
        {
            return Some(format!("x-forwarded-for:{first_ip}"));
        }
    }

    None
}

fn normalized_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn is_low_value_client_hint(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("unknown-client")
        || lowered.ends_with(":127.0.0.1")
        || lowered.ends_with(":localhost")
        || lowered.ends_with(":::1")
        || lowered.ends_with(":[::1]")
}

fn map_flag_row(row: sqlx::postgres::PgRow) -> Result<AntiAbuseFlagSummary, AppError> {
    Ok(AntiAbuseFlagSummary {
        id: row.try_get("id").map_err(internal_db_err)?,
        user_public_id: row.try_get("user_public_id").map_err(internal_db_err)?,
        flag_code: row.try_get("flag_code").map_err(internal_db_err)?,
        severity: row.try_get("severity").map_err(internal_db_err)?,
        status: row.try_get("status").map_err(internal_db_err)?,
        proposal_id: row.try_get("proposal_id").map_err(internal_db_err)?,
        proposal_title: row.try_get("proposal_title").map_err(internal_db_err)?,
        related_proposal_id: row
            .try_get("related_proposal_id")
            .map_err(internal_db_err)?,
        related_proposal_title: row
            .try_get("related_proposal_title")
            .map_err(internal_db_err)?,
        client_ip_hint: row.try_get("client_ip_hint").map_err(internal_db_err)?,
        user_agent_hash: row.try_get("user_agent_hash").map_err(internal_db_err)?,
        details: row.try_get("details").map_err(internal_db_err)?,
        created_at: row.try_get("created_at").map_err(internal_db_err)?,
        reviewed_at: row.try_get("reviewed_at").map_err(internal_db_err)?,
        reviewed_by_moderator_user_id: row
            .try_get("reviewed_by_moderator_user_id")
            .map_err(internal_db_err)?,
        resolution_note: row.try_get("resolution_note").map_err(internal_db_err)?,
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

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read trust review data.".to_string())
}

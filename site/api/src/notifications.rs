use serde_json::json;
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::error::AppError;

pub async fn record_merge_watch_notifications(
    db: &sqlx::PgPool,
    source_proposal_id: Uuid,
    target_proposal_id: Uuid,
) -> Result<(), AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            sp.author_user_id,
            sp.title AS source_title,
            tp.title AS target_title,
            (
                sp.support_count + sp.not_a_fit_count + sp.unclear_count
                + sp.unsafe_count + sp.merge_count
            ) AS source_total_count,
            sp.merge_count AS source_merge_count
        FROM proposals sp
        JOIN proposals tp ON tp.id = $2
        WHERE sp.id = $1
          AND sp.primary_state = 'active'
          AND tp.primary_state = 'active'
        LIMIT 1
        "#,
    )
    .bind(source_proposal_id)
    .bind(target_proposal_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading merge-watch notification context: {}",
            err
        );
        AppError::Internal("Failed to record merge notification.".to_string())
    })?;

    let Some(row) = row else {
        return Ok(());
    };

    let source_total_count: i32 = row.try_get("source_total_count").map_err(internal_db_err)?;
    let source_merge_count: i32 = row.try_get("source_merge_count").map_err(internal_db_err)?;

    if source_total_count < 10
        || source_total_count == 0
        || (source_merge_count as f64 / source_total_count as f64) < 0.20
    {
        return Ok(());
    }

    let author_user_id: Uuid = row.try_get("author_user_id").map_err(internal_db_err)?;
    let source_title: String = row.try_get("source_title").map_err(internal_db_err)?;
    let target_title: String = row.try_get("target_title").map_err(internal_db_err)?;
    let payload = json!({
        "summary": "Duplicate signals have reached the author distinction-note threshold.",
        "source_title": source_title,
        "target_title": target_title,
        "source_total_count": source_total_count,
        "source_merge_count": source_merge_count
    });

    insert_notification_once(
        db,
        author_user_id,
        "merge_watch_author",
        source_proposal_id,
        target_proposal_id,
        payload.clone(),
    )
    .await?;

    let moderator_rows = sqlx::query(
        r#"
        SELECT id
        FROM users
        WHERE role_code = 'moderator'
          AND email_verified = TRUE
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(
            "database error loading moderators for notification: {}",
            err
        );
        AppError::Internal("Failed to record moderator notification.".to_string())
    })?;

    for moderator in moderator_rows {
        let moderator_user_id: Uuid = moderator.try_get("id").map_err(internal_db_err)?;
        insert_notification_once(
            db,
            moderator_user_id,
            "merge_watch_moderator",
            source_proposal_id,
            target_proposal_id,
            payload.clone(),
        )
        .await?;
    }

    Ok(())
}

pub async fn merge_watch_author_notified(
    db: &sqlx::PgPool,
    proposal_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM notification_events
            WHERE notification_type = 'merge_watch_author'
              AND proposal_id = $1
        ) AS exists_flag
        "#,
    )
    .bind(proposal_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        error!("database error checking merge notification: {}", err);
        AppError::Internal("Failed to read notification state.".to_string())
    })?;

    row.try_get("exists_flag").map_err(internal_db_err)
}

async fn insert_notification_once(
    db: &sqlx::PgPool,
    recipient_user_id: Uuid,
    notification_type: &str,
    proposal_id: Uuid,
    related_proposal_id: Uuid,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO notification_events (
            recipient_user_id,
            notification_type,
            proposal_id,
            related_proposal_id,
            payload
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(recipient_user_id)
    .bind(notification_type)
    .bind(proposal_id)
    .bind(related_proposal_id)
    .bind(payload)
    .execute(db)
    .await
    .map_err(|err| {
        error!("database error inserting notification: {}", err);
        AppError::Internal("Failed to record notification.".to_string())
    })?;

    Ok(())
}

fn internal_db_err(err: sqlx::Error) -> AppError {
    error!("row decode error: {}", err);
    AppError::Internal("Failed to read notification data.".to_string())
}

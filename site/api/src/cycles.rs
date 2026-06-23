use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

const CYCLE_DAYS: i64 = 30;

pub async fn ensure_active_world_cycle(db: &PgPool) -> Result<Option<Uuid>, sqlx::Error> {
    let active_exists: bool = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM cycles c
            JOIN locales l ON l.id = c.locale_id
            WHERE l.slug = 'world'
              AND c.is_active = TRUE
        ) AS active_exists
        "#,
    )
    .fetch_one(db)
    .await?
    .try_get("active_exists")?;

    if active_exists {
        return Ok(None);
    }

    let mut tx = db.begin().await?;

    let locale_id: Uuid = sqlx::query(
        r#"
        SELECT id
        FROM locales
        WHERE slug = 'world'
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .fetch_one(&mut *tx)
    .await?
    .try_get("id")?;

    let active_exists_in_tx: bool = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM cycles
            WHERE locale_id = $1
              AND is_active = TRUE
        ) AS active_exists
        "#,
    )
    .bind(locale_id)
    .fetch_one(&mut *tx)
    .await?
    .try_get("active_exists")?;

    if active_exists_in_tx {
        tx.commit().await?;
        return Ok(None);
    }

    let max_cycle_number: Option<i32> = sqlx::query(
        r#"
        SELECT MAX(cycle_number) AS max_cycle_number
        FROM cycles
        WHERE locale_id = $1
        "#,
    )
    .bind(locale_id)
    .fetch_one(&mut *tx)
    .await?
    .try_get("max_cycle_number")?;

    let starts_at = Utc::now();
    let new_cycle_id = insert_cycle(
        &mut tx,
        locale_id,
        max_cycle_number.unwrap_or(0) + 1,
        starts_at,
    )
    .await?;

    tx.commit().await?;
    info!("opened initial world cycle {}", new_cycle_id);

    Ok(Some(new_cycle_id))
}

pub async fn open_next_world_cycle_after_resolution(
    db: &PgPool,
    resolved_cycle_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let mut tx = db.begin().await?;

    let current = sqlx::query(
        r#"
        SELECT
            c.locale_id,
            c.cycle_number,
            c.voting_ends_at
        FROM cycles c
        JOIN locales l ON l.id = c.locale_id
        WHERE c.id = $1
          AND l.slug = 'world'
          AND c.is_active = TRUE
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(resolved_cycle_id)
    .fetch_one(&mut *tx)
    .await?;

    let locale_id: Uuid = current.try_get("locale_id")?;
    let current_cycle_number: i32 = current.try_get("cycle_number")?;
    let previous_voting_ends_at: DateTime<Utc> = current.try_get("voting_ends_at")?;
    let starts_at = previous_voting_ends_at.max(Utc::now());
    let next_cycle_number = current_cycle_number + 1;

    sqlx::query(
        r#"
        UPDATE cycles
        SET is_active = FALSE
        WHERE locale_id = $1
          AND is_active = TRUE
        "#,
    )
    .bind(locale_id)
    .execute(&mut *tx)
    .await?;

    let row = sqlx::query(
        r#"
        INSERT INTO cycles (
            locale_id,
            cycle_number,
            starts_at,
            submission_ends_at,
            voting_ends_at,
            is_active
        )
        VALUES ($1, $2, $3, $4, $5, TRUE)
        ON CONFLICT (locale_id, cycle_number)
        DO UPDATE SET is_active = TRUE
        RETURNING id
        "#,
    )
    .bind(locale_id)
    .bind(next_cycle_number)
    .bind(starts_at)
    .bind(starts_at + Duration::days(CYCLE_DAYS))
    .bind(starts_at + Duration::days(CYCLE_DAYS))
    .fetch_one(&mut *tx)
    .await?;

    let next_cycle_id: Uuid = row.try_get("id")?;
    tx.commit().await?;
    info!(
        "opened world cycle {} after resolving cycle {}",
        next_cycle_id, resolved_cycle_id
    );

    Ok(next_cycle_id)
}

async fn insert_cycle(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    locale_id: Uuid,
    cycle_number: i32,
    starts_at: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cycles (
            locale_id,
            cycle_number,
            starts_at,
            submission_ends_at,
            voting_ends_at,
            is_active
        )
        VALUES ($1, $2, $3, $4, $5, TRUE)
        RETURNING id
        "#,
    )
    .bind(locale_id)
    .bind(cycle_number)
    .bind(starts_at)
    .bind(starts_at + Duration::days(CYCLE_DAYS))
    .bind(starts_at + Duration::days(CYCLE_DAYS))
    .fetch_one(&mut **tx)
    .await?
    .try_get("id")
}

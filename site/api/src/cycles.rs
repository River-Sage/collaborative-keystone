use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

use crate::locale::{self, LocaleConfig};

pub async fn ensure_active_locale_cycle(
    db: &PgPool,
    locale_config: &LocaleConfig,
) -> Result<Option<Uuid>, sqlx::Error> {
    let locale_id = locale::ensure_configured_locale(db, locale_config).await?;

    let active_exists: bool = sqlx::query(
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
    .fetch_one(db)
    .await?
    .try_get("active_exists")?;

    if active_exists {
        return Ok(None);
    }

    let mut tx = db.begin().await?;

    sqlx::query("SELECT id FROM locales WHERE id = $1 FOR UPDATE")
        .bind(locale_id)
        .fetch_one(&mut *tx)
        .await?;

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

    let starts_at = calendar_month_start(Utc::now());
    let new_cycle_id = insert_cycle(
        &mut tx,
        locale_id,
        max_cycle_number.unwrap_or(0) + 1,
        starts_at,
    )
    .await?;

    tx.commit().await?;
    info!(
        "opened initial {} cycle {}",
        locale_config.slug, new_cycle_id
    );

    Ok(Some(new_cycle_id))
}

pub async fn open_next_locale_cycle_after_resolution(
    db: &PgPool,
    resolved_cycle_id: Uuid,
    locale_slug: &str,
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
          AND l.slug = $2
          AND c.is_active = TRUE
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(resolved_cycle_id)
    .bind(locale_slug)
    .fetch_one(&mut *tx)
    .await?;

    let locale_id: Uuid = current.try_get("locale_id")?;
    let current_cycle_number: i32 = current.try_get("cycle_number")?;
    let previous_voting_ends_at: DateTime<Utc> = current.try_get("voting_ends_at")?;
    let starts_at = next_cycle_start_after(previous_voting_ends_at);
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
    .bind(next_calendar_month_start(starts_at))
    .bind(next_calendar_month_start(starts_at))
    .fetch_one(&mut *tx)
    .await?;

    let next_cycle_id: Uuid = row.try_get("id")?;
    tx.commit().await?;
    info!(
        "opened {} cycle {} after resolving cycle {}",
        locale_slug, next_cycle_id, resolved_cycle_id
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
    .bind(next_calendar_month_start(starts_at))
    .bind(next_calendar_month_start(starts_at))
    .fetch_one(&mut **tx)
    .await?
    .try_get("id")
}

fn next_cycle_start_after(previous_voting_ends_at: DateTime<Utc>) -> DateTime<Utc> {
    if is_calendar_month_boundary(previous_voting_ends_at) {
        previous_voting_ends_at
    } else {
        next_calendar_month_start(previous_voting_ends_at)
    }
}

fn calendar_month_start(value: DateTime<Utc>) -> DateTime<Utc> {
    utc_ymd(value.year(), value.month(), 1)
}

fn next_calendar_month_start(value: DateTime<Utc>) -> DateTime<Utc> {
    let (year, month) = if value.month() == 12 {
        (value.year() + 1, 1)
    } else {
        (value.year(), value.month() + 1)
    };
    utc_ymd(year, month, 1)
}

fn is_calendar_month_boundary(value: DateTime<Utc>) -> bool {
    value.day() == 1
        && value.hour() == 0
        && value.minute() == 0
        && value.second() == 0
        && value.nanosecond() == 0
}

fn utc_ymd(year: i32, month: u32, day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, 0, 0, 0)
        .single()
        .expect("valid UTC calendar month boundary")
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{calendar_month_start, next_calendar_month_start, next_cycle_start_after};

    #[test]
    fn initial_cycle_starts_at_current_calendar_month_boundary() {
        let now = Utc.with_ymd_and_hms(2026, 8, 28, 18, 37, 22).unwrap();

        assert_eq!(
            calendar_month_start(now),
            Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn next_calendar_month_start_handles_december() {
        let value = Utc.with_ymd_and_hms(2026, 12, 15, 12, 0, 0).unwrap();

        assert_eq!(
            next_calendar_month_start(value),
            Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn resolved_cycle_continues_from_clean_month_boundary() {
        let previous_end = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();

        assert_eq!(next_cycle_start_after(previous_end), previous_end);
    }

    #[test]
    fn old_rolling_cycle_end_snaps_forward_without_overlap() {
        let previous_end = Utc.with_ymd_and_hms(2026, 9, 26, 18, 37, 22).unwrap();

        assert_eq!(
            next_cycle_start_after(previous_end),
            Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap()
        );
    }
}

//! Shared Korean holiday dates. Missing/stale coverage never means a working day.
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use time::{Date, Duration, Month, OffsetDateTime};
use utoipa::ToSchema;

use crate::{Database, StorageError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HolidayStatus {
    WorkingDay,
    Holiday { name: String },
    Unavailable,
}

impl Database {
    /// Whether complete month coverage was refreshed within the last day.
    ///
    /// # Errors
    /// Returns a classified persistence failure; callers retry instead of replacing data.
    pub async fn holiday_month_is_fresh(
        &self,
        year: i32,
        month: u8,
        now: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        let fetched = sqlx::query_scalar::<_, OffsetDateTime>(
            "SELECT fetched_at FROM holiday_calendar_months WHERE year=$1 AND month=$2",
        )
        .bind(year)
        .bind(i32::from(month))
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        Ok(fetched
            .is_some_and(|at| at >= now - Duration::days(1) && at <= now + Duration::minutes(5)))
    }
    /// Atomically replaces one fully fetched month, including an empty month.
    ///
    /// # Errors
    /// Rejects out-of-month or malformed dates; leaves previous coverage intact.
    pub async fn replace_holiday_month(
        &self,
        year: i32,
        month: u8,
        dates: &[(Date, String)],
        fetched_at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        let month_value = Month::try_from(month).map_err(|_| StorageError::InvalidConfiguration)?;
        if !(2000..=2200).contains(&year)
            || dates.len() > 31
            || dates.iter().any(|(date, name)| {
                date.year() != year
                    || date.month() != month_value
                    || name.trim().is_empty()
                    || name.chars().count() > 200
                    || name.chars().any(char::is_control)
            })
            || dates
                .iter()
                .enumerate()
                .any(|(i, (date, _))| dates[..i].iter().any(|(d, _)| d == date))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let start = Date::from_calendar_date(year, month_value, 1)
            .map_err(|_| StorageError::InvalidConfiguration)?;
        let end = if month == 12 {
            Date::from_calendar_date(year + 1, Month::January, 1)
        } else {
            Date::from_calendar_date(year, month_value.next(), 1)
        }
        .map_err(|_| StorageError::InvalidConfiguration)?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        // Serialize concurrent month refreshes before deleting/replacing dates.
        sqlx::query("INSERT INTO holiday_calendar_months(year,month,fetched_at) VALUES($1,$2,$3) ON CONFLICT(year,month) DO UPDATE SET fetched_at=EXCLUDED.fetched_at")
            .bind(year).bind(i32::from(month)).bind(fetched_at).execute(&mut *tx).await.map_err(classify)?;
        sqlx::query("DELETE FROM holiday_calendar_dates WHERE date >= $1 AND date < $2")
            .bind(start)
            .bind(end)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        for (date, name) in dates {
            sqlx::query("INSERT INTO holiday_calendar_dates(date,name) VALUES($1,$2)")
                .bind(date)
                .bind(name.trim())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
        }
        tx.commit().await.map_err(classify)
    }
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

pub(crate) async fn holiday_status(
    tx: &mut Transaction<'_, Postgres>,
    date: Date,
    now: OffsetDateTime,
) -> Result<HolidayStatus, StorageError> {
    let fetched = sqlx::query_scalar::<_, OffsetDateTime>(
        "SELECT fetched_at FROM holiday_calendar_months WHERE year=$1 AND month=$2",
    )
    .bind(date.year())
    .bind(i32::from(u8::from(date.month())))
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    if fetched.is_none_or(|at| at < now - Duration::days(7) || at > now + Duration::minutes(5)) {
        return Ok(HolidayStatus::Unavailable);
    }
    let name =
        sqlx::query_scalar::<_, String>("SELECT name FROM holiday_calendar_dates WHERE date=$1")
            .bind(date)
            .fetch_optional(&mut **tx)
            .await
            .map_err(classify)?;
    Ok(
        name.map_or(HolidayStatus::WorkingDay, |name| HolidayStatus::Holiday {
            name,
        }),
    )
}

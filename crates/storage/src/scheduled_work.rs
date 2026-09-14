//! Recurring project reminders with fresh data, durable slots and delivery receipts.

mod execution;

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use time::{Duration, OffsetDateTime, Time, UtcOffset};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{Database, StorageError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkScope {
    Today,
    Tomorrow,
    TodayTomorrow,
    Overdue,
    AllOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkDestination {
    GoogleChat,
    InApp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScheduledWorkDefinition {
    pub title: String,
    pub workspace_id: Uuid,
    pub project_id: Option<Uuid>,
    pub webhook_id: Option<Uuid>,
    /// ISO weekday numbers: Monday = 1, Sunday = 7.
    pub weekdays: Vec<u8>,
    /// Local HH:MM. v1 supports Korea time explicitly.
    pub time: String,
    pub follow_up_time: Option<String>,
    pub time_zone: String,
    pub task_scope: WorkScope,
    pub include_overdue: bool,
    pub assignee_names: Vec<String>,
    pub destination: WorkDestination,
    pub mention_assignees: bool,
    pub mention_names: Vec<String>,
    pub include_schedules: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledWork {
    pub id: Uuid,
    #[sqlx(json)]
    pub definition: ScheduledWorkDefinition,
    pub enabled: bool,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String)]
    pub next_run_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledWorkRun {
    pub id: Uuid,
    pub scheduled_work_id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String)]
    pub scheduled_for: OffsetDateTime,
    pub kind: String,
    pub status: String,
    pub reason: Option<String>,
    pub task_ids: Vec<Uuid>,
    #[sqlx(json)]
    pub messages: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String)]
    pub started_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledWorkPreview {
    pub task_ids: Vec<Uuid>,
    pub messages: Vec<String>,
    pub warnings: Vec<String>,
    pub schedule_count: usize,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String)]
    pub next_run_at: OffsetDateTime,
}

pub(super) fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

pub(super) fn korea_offset() -> UtcOffset {
    UtcOffset::from_hms(9, 0, 0).expect("Korea offset is valid")
}

fn parse_clock(value: &str) -> Result<Time, StorageError> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || ![bytes[0], bytes[1], bytes[3], bytes[4]]
            .iter()
            .all(u8::is_ascii_digit)
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Time::from_hms(
        (bytes[0] - b'0') * 10 + bytes[1] - b'0',
        (bytes[3] - b'0') * 10 + bytes[4] - b'0',
        0,
    )
    .map_err(|_| StorageError::InvalidConfiguration)
}

impl ScheduledWorkDefinition {
    /// Checks the complete recurrence and destination contract before persistence.
    ///
    /// # Errors
    /// Returns invalid configuration for malformed or ambiguous rules.
    pub fn validate(&self) -> Result<(), StorageError> {
        let primary = parse_clock(&self.time)?;
        let names_valid = |values: &[String]| {
            values.len() <= 50
                && values.iter().all(|value| {
                    !value.trim().is_empty()
                        && value.trim() == value
                        && value.chars().count() <= 80
                        && !value
                            .chars()
                            .any(|c| c.is_control() || matches!(c, '@' | '<' | '>'))
                })
        };
        if self.title.trim().is_empty()
            || self.title.chars().count() > 120
            || self.title.chars().any(char::is_control)
            || self.workspace_id.get_version_num() != 7
            || self.project_id.is_some_and(|id| id.get_version_num() != 7)
            || self.webhook_id.is_some_and(|id| id.get_version_num() != 7)
            || self.weekdays.is_empty()
            || self.weekdays.len() > 7
            || self.weekdays.iter().any(|day| !(1..=7).contains(day))
            || self
                .weekdays
                .iter()
                .enumerate()
                .any(|(i, day)| self.weekdays[..i].contains(day))
            || self.time_zone != "Asia/Seoul"
            || !names_valid(&self.assignee_names)
            || !names_valid(&self.mention_names)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        if let Some(followup) = &self.follow_up_time
            && (parse_clock(followup)? <= primary || self.include_schedules)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        match self.destination {
            WorkDestination::GoogleChat
                if self.project_id.is_none()
                    || self.webhook_id.is_none()
                    || self.include_schedules =>
            {
                Err(StorageError::InvalidConfiguration)
            }
            WorkDestination::InApp
                if self.webhook_id.is_some()
                    || self.mention_assignees
                    || !self.mention_names.is_empty() =>
            {
                Err(StorageError::InvalidConfiguration)
            }
            _ => Ok(()),
        }
    }

    /// Finds the first future slot without replaying historic occurrences.
    ///
    /// # Errors
    /// Returns invalid configuration for malformed recurrence.
    pub fn next_slot(&self, after: OffsetDateTime) -> Result<OffsetDateTime, StorageError> {
        self.validate()?;
        let local = after.to_offset(korea_offset());
        for offset in 0..=7 {
            let date = local.date() + Duration::days(offset);
            if !self.weekdays.contains(&date.weekday().number_from_monday()) {
                continue;
            }
            for clock in std::iter::once(&self.time).chain(self.follow_up_time.iter()) {
                let candidate = date
                    .with_time(parse_clock(clock)?)
                    .assume_offset(korea_offset());
                if candidate > after {
                    return Ok(candidate);
                }
            }
        }
        Err(StorageError::InvalidConfiguration)
    }

    fn slot_kind(&self, at: OffsetDateTime) -> &'static str {
        if self
            .follow_up_time
            .as_deref()
            .and_then(|clock| parse_clock(clock).ok())
            == Some(at.to_offset(korea_offset()).time())
        {
            "followup"
        } else {
            "primary"
        }
    }
}

impl Database {
    /// Pauses even when the connected project or webhook is no longer available.
    ///
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn pause_scheduled_work(
        &self,
        user: Uuid,
        id: Uuid,
        version: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let updated=sqlx::query("UPDATE scheduled_work SET enabled=FALSE,version=version+1,updated_at=NOW() WHERE id=$1 AND user_id=$2 AND version=$3").bind(id).bind(user).bind(version).execute(&mut *tx).await.map_err(classify)?.rows_affected();
        if updated == 1 {
            cancel_pending(&mut tx, id).await?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(updated == 1)
    }

    /// Lists only the authenticated owner's recurring work.
    ///
    /// # Errors
    /// Returns storage errors without exposing rule contents.
    pub async fn scheduled_work_for_user(
        &self,
        user: Uuid,
    ) -> Result<Vec<ScheduledWork>, StorageError> {
        sqlx::query_as("SELECT id, definition, enabled, next_run_at, version FROM scheduled_work WHERE user_id=$1 ORDER BY enabled DESC, next_run_at, id")
            .bind(user).fetch_all(self.pool()).await.map_err(classify)
    }

    /// Creates or replaces a version-matched schedule and cancels unsent old work.
    ///
    /// # Errors
    /// Invalid scope, input, or persistence failure is classified.
    pub async fn save_scheduled_work(
        &self,
        user: Uuid,
        id: Uuid,
        definition: &ScheduledWorkDefinition,
        enabled: bool,
        expected: Option<i64>,
        now: OffsetDateTime,
    ) -> Result<Option<ScheduledWork>, StorageError> {
        if user.get_version_num() != 7
            || id.get_version_num() != 7
            || expected.is_some_and(|v| v <= 0)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        definition.validate()?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let result =
            save_in_transaction(&mut tx, user, id, definition, enabled, expected, now).await?;
        tx.commit().await.map_err(classify)?;
        Ok(result)
    }

    /// Returns durable receipts; oldest receipts remain accessible by cursor.
    ///
    /// # Errors
    /// Returns a classified read failure.
    pub async fn scheduled_work_runs(
        &self,
        user: Uuid,
        id: Option<Uuid>,
        before: Option<Uuid>,
    ) -> Result<Vec<ScheduledWorkRun>, StorageError> {
        sqlx::query_as("SELECT id, scheduled_work_id, scheduled_for, kind, status, reason, task_ids, messages, started_at FROM scheduled_work_runs WHERE user_id=$1 AND ($2::UUID IS NULL OR scheduled_work_id=$2) AND ($3::UUID IS NULL OR id < $3) ORDER BY id DESC LIMIT 50")
            .bind(user).bind(id).bind(before).fetch_all(self.pool()).await.map_err(classify)
    }

    /// Deletes one owned, version-matched rule after cancelling queued sends.
    ///
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn delete_scheduled_work(
        &self,
        user: Uuid,
        id: Uuid,
        version: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let existing = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM scheduled_work WHERE user_id=$1 AND id=$2 AND version=$3 FOR UPDATE",
        )
        .bind(user)
        .bind(id)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if existing.is_none() {
            return Ok(false);
        }
        cancel_pending(&mut tx, id).await?;
        sqlx::query("DELETE FROM scheduled_work WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(true)
    }
}

pub(crate) async fn save_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: Uuid,
    id: Uuid,
    definition: &ScheduledWorkDefinition,
    enabled: bool,
    expected: Option<i64>,
    now: OffsetDateTime,
) -> Result<Option<ScheduledWork>, StorageError> {
    definition.validate()?;
    validate_scope(tx, user, definition).await?;
    if enabled
        && !execution::preview_in_transaction(tx, user, definition, now, None)
            .await?
            .warnings
            .is_empty()
    {
        return Err(StorageError::InvalidConfiguration);
    }
    let next = definition.next_slot(now)?;
    let result = if let Some(version) = expected {
        let current = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM scheduled_work WHERE user_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(user)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(classify)?;
        if current != Some(version) {
            return Ok(None);
        }
        cancel_pending(tx, id).await?;
        sqlx::query_as("UPDATE scheduled_work SET workspace_id=$3, project_id=$4, definition=$5, enabled=$6, next_run_at=$7, version=version+1, updated_at=NOW() WHERE id=$1 AND user_id=$2 RETURNING id, definition, enabled, next_run_at, version")
            .bind(id).bind(user).bind(definition.workspace_id).bind(definition.project_id).bind(Json(definition)).bind(enabled).bind(next).fetch_optional(&mut **tx).await.map_err(classify)?
    } else {
        sqlx::query_as("INSERT INTO scheduled_work (id,user_id,workspace_id,project_id,definition,enabled,next_run_at) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(id) DO NOTHING RETURNING id,definition,enabled,next_run_at,version")
            .bind(id).bind(user).bind(definition.workspace_id).bind(definition.project_id).bind(Json(definition)).bind(enabled).bind(next).fetch_optional(&mut **tx).await.map_err(classify)?
    };
    Ok(result)
}

async fn validate_scope(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: Uuid,
    definition: &ScheduledWorkDefinition,
) -> Result<(), StorageError> {
    definition.validate()?;
    let scoped = sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM workspaces w JOIN users u ON u.id=w.user_id AND u.status='active' WHERE w.id=$1 AND w.user_id=$2 AND ($3::UUID IS NULL OR EXISTS (SELECT 1 FROM projects p WHERE p.id=$3 AND p.user_id=$2 AND p.workspace_id=w.id AND p.status='active')) AND ($4::UUID IS NULL OR EXISTS (SELECT 1 FROM project_webhooks h WHERE h.id=$4 AND h.user_id=$2 AND h.project_id=$3 AND h.provider='google_chat' AND h.enabled)) AND (NOT $5 OR w.scope='personal'))")
        .bind(definition.workspace_id).bind(user).bind(definition.project_id).bind(definition.webhook_id).bind(definition.include_schedules).fetch_one(&mut **tx).await.map_err(classify)?;
    if scoped {
        Ok(())
    } else {
        Err(StorageError::InvalidConfiguration)
    }
}

async fn cancel_pending(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<(), StorageError> {
    sqlx::query("UPDATE webhook_deliveries d SET status='failed', last_error_code='schedule.cancelled', next_attempt_at=NULL WHERE d.id IN (SELECT l.delivery_id FROM scheduled_work_run_deliveries l JOIN scheduled_work_runs r ON r.id=l.run_id WHERE r.scheduled_work_id=$1) AND d.status IN ('queued','retry_wait')")
        .bind(id).execute(&mut **tx).await.map_err(classify)?;
    sqlx::query("UPDATE scheduled_work_runs r SET status='skipped', reason='cancelled', finished_at=NOW() WHERE r.scheduled_work_id=$1 AND r.status='delivering' AND NOT EXISTS (SELECT 1 FROM scheduled_work_run_deliveries l JOIN webhook_deliveries d ON d.id=l.delivery_id WHERE l.run_id=r.id AND d.status IN ('sending','delivered'))")
        .bind(id).execute(&mut **tx).await.map_err(classify)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> ScheduledWorkDefinition {
        ScheduledWorkDefinition {
            title: "마감 안내".into(),
            workspace_id: Uuid::now_v7(),
            project_id: None,
            webhook_id: None,
            weekdays: vec![1, 2, 3, 4, 5],
            time: "09:00".into(),
            follow_up_time: Some("16:00".into()),
            time_zone: "Asia/Seoul".into(),
            task_scope: WorkScope::Today,
            include_overdue: true,
            assignee_names: vec![],
            destination: WorkDestination::InApp,
            mention_assignees: false,
            mention_names: vec![],
            include_schedules: false,
        }
    }
    #[test]
    fn next_slot_handles_friday_weekend_and_exact_slot() {
        let parse =
            |s| OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).unwrap();
        let rule = rule();
        assert_eq!(
            rule.next_slot(parse("2026-09-18T16:00:00+09:00")).unwrap(),
            parse("2026-09-21T09:00:00+09:00")
        );
        assert_eq!(
            rule.next_slot(parse("2026-09-14T09:00:00+09:00")).unwrap(),
            parse("2026-09-14T16:00:00+09:00")
        );
        assert_eq!(
            rule.next_slot(parse("2026-09-13T23:59:00+09:00")).unwrap(),
            parse("2026-09-14T09:00:00+09:00")
        );
    }
    #[test]
    fn rejects_ambiguous_recurrence_and_destinations() {
        for value in ["9:00", "24:00", "12:60", "ab:cd", "09:00:00"] {
            let mut r = rule();
            r.time = value.into();
            assert!(r.validate().is_err());
        }
        let mut r = rule();
        r.follow_up_time = Some("08:00".into());
        assert!(r.validate().is_err());
        r = rule();
        r.weekdays = vec![1, 1];
        assert!(r.validate().is_err());
        r = rule();
        r.destination = WorkDestination::GoogleChat;
        assert!(r.validate().is_err());
        r = rule();
        r.time_zone = "UTC".into();
        assert!(r.validate().is_err());
    }
}

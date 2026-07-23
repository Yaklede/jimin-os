//! Server-owned schedule and task persistence used before any external
//! calendar provider is linked.

use sqlx::{Postgres, Transaction};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::append_change,
    calendar::PrimaryCalendarMutationTarget,
    calendar_mutation::{
        ScheduleCalendarMutationOperation, ScheduleCalendarMutationPayload,
        attach_schedule_and_queue_create, queue_linked_schedule_mutation,
    },
    webhook::{project_event_payload, queue_project_event_in_transaction},
};

const MAX_TITLE_CHARS: usize = 200;
const MAX_NOTES_CHARS: usize = 10_000;

/// Validated manual schedule input. Provider-originated entries will use a
/// separate adapter path so clients cannot spoof a provider source.
pub struct NewScheduleEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub notes: Option<String>,
    pub starts_at: OffsetDateTime,
    pub ends_at: OffsetDateTime,
    pub time_zone: String,
}

/// A version-checked replacement of one owned manual schedule entry.
pub struct ScheduleEntryUpdate {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub notes: Option<String>,
    pub starts_at: OffsetDateTime,
    pub ends_at: OffsetDateTime,
    pub time_zone: String,
    pub expected_version: i64,
}

impl NewScheduleEntry {
    /// Validates a bounded personal schedule entry before it reaches SQL.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid client data.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !self
                .notes
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_NOTES_CHARS, true))
            || !valid_time_zone(&self.time_zone)
            || self.ends_at <= self.starts_at
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl ScheduleEntryUpdate {
    /// Validates an editable schedule entry before database access.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs,
    /// text, time ranges, time zones, or optimistic version values.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !self
                .notes
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_NOTES_CHARS, true))
            || !valid_time_zone(&self.time_zone)
            || self.ends_at <= self.starts_at
            || self.expected_version <= 0
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Validated personal task input.
pub struct NewTask {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
}

/// A version-checked replacement of the mutable task fields.
pub struct TaskUpdate {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub status: TaskStatus,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub expected_version: i64,
}

impl NewTask {
    /// Validates a bounded task before it reaches SQL.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid client data.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !self.project_id.is_none_or(is_v7)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !self
                .notes
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_NOTES_CHARS, true))
            || !(0..=3).contains(&self.priority)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl TaskUpdate {
    /// Validates all mutable task fields before database access.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs,
    /// text, priority, or optimistic version values.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !self.project_id.is_none_or(is_v7)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !self
                .notes
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_NOTES_CHARS, true))
            || !(0..=3).contains(&self.priority)
            || self.expected_version <= 0
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// A personal schedule entry that is safe to return to its owning device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleEntry {
    pub id: Uuid,
    pub title: String,
    pub notes: Option<String>,
    pub starts_at: OffsetDateTime,
    pub ends_at: OffsetDateTime,
    pub time_zone: String,
    pub status: ScheduleStatus,
    pub source: ScheduleSource,
    pub editable: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleStatus {
    Confirmed,
    Cancelled,
}

/// Origin used to explain whether an item belongs to Jimin OS directly or a
/// connected read-only Google Calendar source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleSource {
    Manual,
    GoogleCalendar,
}

/// A personal task that is safe to return to its owning device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub assignee_name: Option<String>,
    pub status: TaskStatus,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    Completed,
    Cancelled,
}

/// Result of an idempotent task deletion request. Tasks are soft deleted so
/// audit and sync history remain available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteTaskOutcome {
    Deleted,
    AlreadyDeleted,
    AlreadyAbsent,
    VersionConflict,
}

#[derive(sqlx::FromRow)]
struct ScheduleRow {
    id: Uuid,
    title: String,
    notes: Option<String>,
    starts_at: OffsetDateTime,
    ends_at: OffsetDateTime,
    time_zone: String,
    status: String,
    source: String,
    editable: bool,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    project_id: Option<Uuid>,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    status: String,
    priority: i16,
    due_at: Option<OffsetDateTime>,
    completed_at: Option<OffsetDateTime>,
    version: i64,
}

impl TryFrom<ScheduleRow> for ScheduleEntry {
    type Error = StorageError;

    fn try_from(row: ScheduleRow) -> Result<Self, Self::Error> {
        let status = match row.status.as_str() {
            "confirmed" => ScheduleStatus::Confirmed,
            "cancelled" => ScheduleStatus::Cancelled,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        let source = match row.source.as_str() {
            "manual" => ScheduleSource::Manual,
            "google_calendar" => ScheduleSource::GoogleCalendar,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            title: row.title,
            notes: row.notes,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            time_zone: row.time_zone,
            status,
            source,
            editable: row.editable,
            version: row.version,
        })
    }
}

impl TryFrom<TaskRow> for Task {
    type Error = StorageError;

    fn try_from(row: TaskRow) -> Result<Self, Self::Error> {
        let status = match row.status.as_str() {
            "open" => TaskStatus::Open,
            "completed" => TaskStatus::Completed,
            "cancelled" => TaskStatus::Cancelled,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            project_id: row.project_id,
            title: row.title,
            notes: row.notes,
            assignee_name: row.assignee_name,
            status,
            priority: row.priority,
            due_at: row.due_at,
            completed_at: row.completed_at,
            version: row.version,
        })
    }
}

impl Database {
    /// Creates a manual schedule entry using its client-generated ID as the
    /// idempotency key. An exact retry returns the existing entry without
    /// appending another sync change; reusing the ID for another owner or
    /// payload is rejected.
    ///
    /// # Errors
    ///
    /// Returns an identity-conflict error when the idempotency key has already
    /// been used with different schedule data, and a classified persistence
    /// error when the atomic write cannot commit.
    pub async fn create_schedule_entry(
        &self,
        entry: &NewScheduleEntry,
    ) -> Result<ScheduleEntry, StorageError> {
        entry.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ScheduleRow>(
            "\
            INSERT INTO schedule_entries (
                id, user_id, title, notes, starts_at, ends_at, time_zone, source, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual', 'confirmed')
            ON CONFLICT (id) DO NOTHING
            RETURNING id, title, notes, starts_at, ends_at, time_zone, status, source,
                TRUE AS editable, version",
        )
        .bind(entry.id)
        .bind(entry.user_id)
        .bind(entry.title.trim())
        .bind(
            entry
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(entry.starts_at)
        .bind(entry.ends_at)
        .bind(entry.time_zone.trim())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            let existing = sqlx::query_as::<_, ScheduleRow>(
                "\
                SELECT id, title, notes, starts_at, ends_at, time_zone, status, source,
                    TRUE AS editable, version
                FROM schedule_entries
                WHERE id = $1
                  AND user_id = $2
                  AND source = 'manual'
                  AND status = 'confirmed'",
            )
            .bind(entry.id)
            .bind(entry.user_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?;
            let existing = existing
                .map(ScheduleEntry::try_from)
                .transpose()?
                .filter(|existing| schedule_matches_new(existing, entry))
                .ok_or(StorageError::IdentityConflict)?;
            transaction.commit().await.map_err(classify)?;
            return Ok(existing);
        };
        let created = ScheduleEntry::try_from(row)?;
        append_change(
            &mut transaction,
            entry.user_id,
            "schedule_entry",
            created.id,
            created.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(created)
    }

    /// Creates a server-owned schedule and durably journals its Google create
    /// in the same transaction. The returned schedule remains the canonical UI
    /// record while the provider worker reconciles the deterministic event ID.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration error for malformed schedule data or
    /// an ownership mismatch, and a persistence error when the atomic write
    /// cannot commit.
    pub async fn create_schedule_entry_with_calendar_outbox(
        &self,
        entry: &NewScheduleEntry,
        target: &PrimaryCalendarMutationTarget,
    ) -> Result<ScheduleEntry, StorageError> {
        entry.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ScheduleRow>(
            "\
            INSERT INTO schedule_entries (
                id, user_id, title, notes, starts_at, ends_at, time_zone, source, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual', 'confirmed')
            ON CONFLICT (id) DO NOTHING
            RETURNING id, title, notes, starts_at, ends_at, time_zone, status, source,
                TRUE AS editable, version",
        )
        .bind(entry.id)
        .bind(entry.user_id)
        .bind(entry.title.trim())
        .bind(
            entry
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(entry.starts_at)
        .bind(entry.ends_at)
        .bind(entry.time_zone.trim())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            let existing = sqlx::query_as::<_, ScheduleRow>(
                "\
                SELECT schedule.id, schedule.title, schedule.notes, schedule.starts_at,
                    schedule.ends_at, schedule.time_zone, schedule.status, schedule.source,
                    TRUE AS editable, schedule.version
                FROM schedule_entries AS schedule
                INNER JOIN schedule_calendar_links AS link
                    ON link.schedule_entry_id = schedule.id
                WHERE schedule.id = $1
                  AND schedule.user_id = $2
                  AND schedule.source = 'manual'
                  AND schedule.status = 'confirmed'
                  AND link.user_id = $2
                  AND link.account_id = $3
                  AND link.calendar_id = $4
                  AND EXISTS (
                      SELECT 1 FROM calendar_mutations AS mutation
                      WHERE mutation.schedule_entry_id = schedule.id
                        AND mutation.operation = 'create'
                  )",
            )
            .bind(entry.id)
            .bind(entry.user_id)
            .bind(target.account_id)
            .bind(target.calendar_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?;
            let existing = existing
                .map(ScheduleEntry::try_from)
                .transpose()?
                .filter(|existing| schedule_matches_new(existing, entry))
                .ok_or(StorageError::IdentityConflict)?;
            transaction.commit().await.map_err(classify)?;
            return Ok(existing);
        };
        let created = ScheduleEntry::try_from(row)?;
        let payload = calendar_payload(&created);
        attach_schedule_and_queue_create(
            &mut transaction,
            entry.user_id,
            created.id,
            created.version,
            target,
            &payload,
        )
        .await?;
        append_change(
            &mut transaction,
            entry.user_id,
            "schedule_entry",
            created.id,
            created.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(created)
    }

    /// Lists the owning user's confirmed schedule entries that overlap the
    /// requested half-open UTC window.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for an invalid range or a
    /// classified persistence error for unavailable storage.
    pub async fn schedule_entries_in_range(
        &self,
        user_id: Uuid,
        range_start: OffsetDateTime,
        range_end: OffsetDateTime,
    ) -> Result<Vec<ScheduleEntry>, StorageError> {
        if range_end <= range_start {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, ScheduleRow>(
            "\
            SELECT *
            FROM (
                SELECT id, title, notes, starts_at, ends_at, time_zone, status, source,
                    TRUE AS editable, version
                FROM schedule_entries
                WHERE user_id = $1
                  AND status = 'confirmed'
                  AND starts_at < $2
                  AND ends_at > $3
                UNION ALL
                SELECT id, title, description_text AS notes, start_at AS starts_at, end_at AS ends_at,
                    source_time_zone AS time_zone, 'confirmed'::TEXT AS status,
                    'google_calendar'::TEXT AS source,
                    EXISTS (
                        SELECT 1 FROM calendars
                        WHERE calendars.id = calendar_events.calendar_id
                          AND calendars.access_role IN ('owner', 'writer')
                    ) AS editable, version
                FROM calendar_events
                WHERE user_id = $1
                  AND provider_deleted_at IS NULL
                  AND provider_status IN ('confirmed', 'tentative')
                  AND time_kind = 'date_time'
                  AND start_at < $2
                  AND end_at > $3
                  AND NOT EXISTS (
                      SELECT 1 FROM schedule_calendar_links AS link
                      WHERE link.calendar_id = calendar_events.calendar_id
                        AND link.provider_event_id = calendar_events.provider_event_id
                  )
                UNION ALL
                SELECT id, title, description_text AS notes,
                    (start_date::timestamp AT TIME ZONE 'UTC') AS starts_at,
                    (end_date::timestamp AT TIME ZONE 'UTC') AS ends_at,
                    'UTC'::TEXT AS time_zone, 'confirmed'::TEXT AS status,
                    'google_calendar'::TEXT AS source, FALSE AS editable, version
                FROM calendar_events
                WHERE user_id = $1
                  AND provider_deleted_at IS NULL
                  AND provider_status IN ('confirmed', 'tentative')
                  AND time_kind = 'date'
                  AND start_date < $4
                  AND end_date > $5
                  AND NOT EXISTS (
                      SELECT 1 FROM schedule_calendar_links AS link
                      WHERE link.calendar_id = calendar_events.calendar_id
                        AND link.provider_event_id = calendar_events.provider_event_id
                  )
            ) AS schedule
            ORDER BY schedule.starts_at ASC, schedule.id ASC",
        )
        .bind(user_id)
        .bind(range_end)
        .bind(range_start)
        .bind(range_end.date())
        .bind(range_start.date())
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(ScheduleEntry::try_from).collect()
    }

    /// Returns one visible schedule entry from either the manual or connected
    /// Google read model after a mutation has been reconciled.
    /// Loads one visible schedule entry owned by the current user.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error or invalid-input error.
    pub async fn schedule_entry_by_id(
        &self,
        user_id: Uuid,
        schedule_entry_id: Uuid,
    ) -> Result<Option<ScheduleEntry>, StorageError> {
        if !is_v7(user_id) || !is_v7(schedule_entry_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, ScheduleRow>(
            "\
            SELECT * FROM (
                SELECT id, title, notes, starts_at, ends_at, time_zone, status, source,
                    TRUE AS editable, version
                FROM schedule_entries
                WHERE id = $1 AND user_id = $2 AND status = 'confirmed'
                UNION ALL
                SELECT id, title, description_text AS notes, start_at AS starts_at,
                    end_at AS ends_at, source_time_zone AS time_zone,
                    'confirmed'::TEXT AS status, 'google_calendar'::TEXT AS source,
                    EXISTS (
                        SELECT 1 FROM calendars
                        WHERE calendars.id = calendar_events.calendar_id
                          AND calendars.access_role IN ('owner', 'writer')
                    ) AS editable, version
                FROM calendar_events
                WHERE id = $1 AND user_id = $2
                  AND provider_deleted_at IS NULL
                  AND provider_status IN ('confirmed', 'tentative')
                  AND time_kind = 'date_time'
                  AND NOT EXISTS (
                      SELECT 1 FROM schedule_calendar_links AS link
                      WHERE link.calendar_id = calendar_events.calendar_id
                        AND link.provider_event_id = calendar_events.provider_event_id
                  )
            ) AS schedule
            LIMIT 1",
        )
        .bind(schedule_entry_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(ScheduleEntry::try_from).transpose()
    }

    /// Replaces the editable fields of one owned manual schedule entry when
    /// its version still matches. Provider-backed entries are intentionally
    /// read-only in this storage path.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid input and a
    /// classified persistence error when storage is unavailable.
    pub async fn update_schedule_entry(
        &self,
        update: &ScheduleEntryUpdate,
    ) -> Result<Option<ScheduleEntry>, StorageError> {
        update.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ScheduleRow>(
            "\
            UPDATE schedule_entries
            SET title = $4,
                notes = $5,
                starts_at = $6,
                ends_at = $7,
                time_zone = $8
            WHERE id = $1
              AND user_id = $2
              AND version = $3
              AND source = 'manual'
              AND status = 'confirmed'
            RETURNING id, title, notes, starts_at, ends_at, time_zone, status, source,
                TRUE AS editable, version",
        )
        .bind(update.id)
        .bind(update.user_id)
        .bind(update.expected_version)
        .bind(update.title.trim())
        .bind(
            update
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(update.starts_at)
        .bind(update.ends_at)
        .bind(update.time_zone.trim())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let entry = ScheduleEntry::try_from(row)?;
        append_change(
            &mut transaction,
            update.user_id,
            "schedule_entry",
            entry.id,
            entry.version,
        )
        .await?;
        queue_linked_schedule_mutation(
            &mut transaction,
            update.user_id,
            entry.id,
            entry.version,
            ScheduleCalendarMutationOperation::Update,
            &calendar_payload(&entry),
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(Some(entry))
    }

    /// Cancels one owned manual schedule entry with optimistic concurrency and
    /// appends the matching device change. Rows remain available for audit and
    /// future recovery instead of being physically deleted.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for unsafe IDs or
    /// versions and a classified persistence error when storage is unavailable.
    pub async fn cancel_schedule_entry(
        &self,
        user_id: Uuid,
        schedule_entry_id: Uuid,
        expected_version: i64,
    ) -> Result<Option<i64>, StorageError> {
        if !is_v7(user_id) || !is_v7(schedule_entry_id) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ScheduleRow>(
            "\
            UPDATE schedule_entries
            SET status = 'cancelled'
            WHERE id = $1
              AND user_id = $2
              AND version = $3
              AND source = 'manual'
              AND status = 'confirmed'
            RETURNING id, title, notes, starts_at, ends_at, time_zone, status, source,
                TRUE AS editable, version",
        )
        .bind(schedule_entry_id)
        .bind(user_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let cancelled = row.map(ScheduleEntry::try_from).transpose()?;
        if let Some(entry) = cancelled.as_ref() {
            append_change(
                &mut transaction,
                user_id,
                "schedule_entry",
                schedule_entry_id,
                entry.version,
            )
            .await?;
            queue_linked_schedule_mutation(
                &mut transaction,
                user_id,
                schedule_entry_id,
                entry.version,
                ScheduleCalendarMutationOperation::Delete,
                &calendar_payload(entry),
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(cancelled.map(|entry| entry.version))
    }

    /// Creates an open personal task and appends its matching sync change in
    /// the same transaction.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing personal task text.
    pub async fn create_task(&self, task: &NewTask) -> Result<Task, StorageError> {
        task.validate()?;
        if let Some(project_id) = task.project_id
            && !self
                .project_exists_for_user(task.user_id, project_id)
                .await?
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let user_id = task.user_id;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, TaskRow>(
            "\
            INSERT INTO tasks (id, user_id, project_id, title, notes, status, priority, due_at)
            VALUES ($1, $2, $3, $4, $5, 'open', $6, $7)
            RETURNING id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version",
        )
        .bind(task.id)
        .bind(task.user_id)
        .bind(task.project_id)
        .bind(task.title.trim())
        .bind(
            task.notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(task.priority)
        .bind(task.due_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let task = Task::try_from(row)?;
        append_change(&mut transaction, user_id, "task", task.id, task.version).await?;
        queue_task_webhook_in_transaction(&mut transaction, user_id, &task, "task.created").await?;
        transaction.commit().await.map_err(classify)?;
        Ok(task)
    }

    /// Creates a personal task using the client-generated task ID as the
    /// idempotency key. An exact retry returns the existing task without
    /// appending another sync change; reusing the ID for a different payload
    /// or owner is rejected.
    ///
    /// # Errors
    ///
    /// Returns an identity-conflict error when the idempotency key has already
    /// been used with different task data, and a classified persistence error
    /// when the atomic write cannot commit.
    pub async fn create_task_idempotently(&self, task: &NewTask) -> Result<Task, StorageError> {
        task.validate()?;
        if let Some(project_id) = task.project_id
            && !self
                .project_exists_for_user(task.user_id, project_id)
                .await?
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, TaskRow>(
            "\
            INSERT INTO tasks (id, user_id, project_id, title, notes, status, priority, due_at)
            VALUES ($1, $2, $3, $4, $5, 'open', $6, $7)
            ON CONFLICT (id) DO NOTHING
            RETURNING id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version",
        )
        .bind(task.id)
        .bind(task.user_id)
        .bind(task.project_id)
        .bind(task.title.trim())
        .bind(
            task.notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(task.priority)
        .bind(task.due_at)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            let existing = sqlx::query_as::<_, TaskRow>(
                "\
                SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
                FROM tasks
                WHERE id = $1 AND user_id = $2 AND status = 'open'",
            )
            .bind(task.id)
            .bind(task.user_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?;
            let existing = existing
                .map(Task::try_from)
                .transpose()?
                .filter(|existing| task_matches_new(existing, task))
                .ok_or(StorageError::IdentityConflict)?;
            transaction.commit().await.map_err(classify)?;
            return Ok(existing);
        };
        let created = Task::try_from(row)?;
        append_change(
            &mut transaction,
            task.user_id,
            "task",
            created.id,
            created.version,
        )
        .await?;
        queue_task_webhook_in_transaction(&mut transaction, task.user_id, &created, "task.created")
            .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(created)
    }

    /// Lists active tasks in priority and due-date order for a personal home
    /// screen. Completed and cancelled tasks remain queryable through future
    /// history endpoints but do not clutter this read model.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when storage is unavailable.
    pub async fn open_tasks_for_user(&self, user_id: Uuid) -> Result<Vec<Task>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1 AND status = 'open'
            ORDER BY priority DESC, due_at NULLS LAST, created_at ASC, id ASC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    /// Lists the owning user's completed task history with the most recently
    /// completed work first. Reopened and cancelled tasks are not part of the
    /// completed history because their completion timestamp is cleared.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when storage is unavailable.
    pub async fn completed_tasks_for_user(&self, user_id: Uuid) -> Result<Vec<Task>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1 AND status = 'completed'
            ORDER BY completed_at DESC NULLS LAST, id DESC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    /// Loads one task owned by the current user so callers can compare state
    /// transitions without exposing another user's work item.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when storage is unavailable.
    pub async fn task_for_user(
        &self,
        user_id: Uuid,
        task_id: Uuid,
    ) -> Result<Option<Task>, StorageError> {
        if !is_v7(user_id) || !is_v7(task_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1 AND id = $2",
        )
        .bind(user_id)
        .bind(task_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(Task::try_from).transpose()
    }

    /// Lists the open tasks that belong on the daily home before the supplied
    /// exclusive local-day boundary. Undated tasks remain in the active daily
    /// queue, while explicitly future-dated tasks stay out of today's view.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when storage is unavailable.
    pub async fn home_tasks_for_user(
        &self,
        user_id: Uuid,
        before: OffsetDateTime,
    ) -> Result<Vec<Task>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1
              AND status = 'open'
              AND (due_at IS NULL OR due_at < $2)
            ORDER BY priority DESC, due_at NULLS LAST, created_at ASC, id ASC",
        )
        .bind(user_id)
        .bind(before)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    /// Lists open dated tasks that need deadline attention before the supplied
    /// exclusive boundary. Undated work stays in the normal daily queue.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when storage is unavailable.
    pub async fn deadline_tasks_for_user(
        &self,
        user_id: Uuid,
        before: OffsetDateTime,
    ) -> Result<Vec<Task>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1
              AND status = 'open'
              AND due_at IS NOT NULL
              AND due_at < $2
            ORDER BY due_at ASC, priority DESC, created_at ASC, id ASC",
        )
        .bind(user_id)
        .bind(before)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    /// Lists open tasks for one project owned by the current user.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the project is not
    /// part of the user's work context.
    pub async fn open_tasks_for_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<Task>, StorageError> {
        if !self.project_exists_for_user(user_id, project_id).await? {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1 AND project_id = $2 AND status = 'open'
            ORDER BY priority DESC, due_at NULLS LAST, created_at ASC, id ASC",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    /// Lists the active and completed history for one owned project. Cancelled
    /// entries remain in storage and sync history but are omitted from the
    /// user's project view.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the project is not
    /// part of the user's work context.
    pub async fn tasks_for_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<Task>, StorageError> {
        if !self.project_exists_for_user(user_id, project_id).await? {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE user_id = $1 AND project_id = $2 AND status IN ('open', 'completed')
            ORDER BY
                CASE status WHEN 'open' THEN 0 ELSE 1 END,
                priority DESC,
                due_at NULLS LAST,
                completed_at DESC NULLS LAST,
                created_at ASC,
                id ASC",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    /// Replaces the mutable fields of one owned task when its version still
    /// matches. A cancelled status is a soft delete so sync and audit history
    /// remain intact.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid input and a
    /// classified persistence error when storage is unavailable.
    pub async fn update_task(&self, update: &TaskUpdate) -> Result<Option<Task>, StorageError> {
        update.validate()?;
        if let Some(project_id) = update.project_id
            && !self
                .project_exists_for_user(update.user_id, project_id)
                .await?
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let status = task_status_name(update.status);
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let previous_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM tasks WHERE id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(update.id)
        .bind(update.user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let row = sqlx::query_as::<_, TaskRow>(
            "\
            UPDATE tasks
            SET project_id = $4,
                title = $5,
                notes = $6,
                status = $7,
                priority = $8,
                due_at = $9,
                completed_at = CASE
                    WHEN $7 = 'completed' THEN COALESCE(completed_at, NOW())
                    ELSE NULL
                END
            WHERE id = $1 AND user_id = $2 AND version = $3
            RETURNING id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version",
        )
        .bind(update.id)
        .bind(update.user_id)
        .bind(update.expected_version)
        .bind(update.project_id)
        .bind(update.title.trim())
        .bind(
            update
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(status)
        .bind(update.priority)
        .bind(update.due_at)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let task = Task::try_from(row)?;
        append_change(
            &mut transaction,
            update.user_id,
            "task",
            task.id,
            task.version,
        )
        .await?;
        let event_type = task_update_event_type(previous_status.as_deref(), task.status);
        queue_task_webhook_in_transaction(&mut transaction, update.user_id, &task, event_type)
            .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(Some(task))
    }

    /// Soft deletes one owned task with optimistic concurrency. Repeating a
    /// successful deletion is safe, and missing or foreign tasks are handled
    /// identically to avoid disclosing another user's data.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs or
    /// versions and a classified persistence error when storage is unavailable.
    pub async fn delete_task(
        &self,
        user_id: Uuid,
        task_id: Uuid,
        expected_version: i64,
    ) -> Result<DeleteTaskOutcome, StorageError> {
        if !is_v7(user_id) || !is_v7(task_id) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current = sqlx::query_as::<_, TaskRow>(
            "\
            SELECT id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version
            FROM tasks
            WHERE id = $1 AND user_id = $2
            FOR UPDATE",
        )
        .bind(task_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(current) = current else {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteTaskOutcome::AlreadyAbsent);
        };
        let current = Task::try_from(current)?;
        if current.status == TaskStatus::Cancelled {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteTaskOutcome::AlreadyDeleted);
        }
        if current.version != expected_version {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteTaskOutcome::VersionConflict);
        }

        let deleted = sqlx::query_as::<_, TaskRow>(
            "\
            UPDATE tasks
            SET status = 'cancelled', completed_at = NULL
            WHERE id = $1 AND user_id = $2 AND version = $3
            RETURNING id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version",
        )
        .bind(task_id)
        .bind(user_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(deleted) = deleted else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteTaskOutcome::VersionConflict);
        };
        let deleted = Task::try_from(deleted)?;
        append_change(
            &mut transaction,
            user_id,
            "task",
            deleted.id,
            deleted.version,
        )
        .await?;
        if let Some(project_id) = deleted.project_id {
            let payload = project_event_payload("task.deleted", project_id, deleted.id)?;
            queue_project_event_in_transaction(
                &mut transaction,
                user_id,
                project_id,
                "task.deleted",
                &payload,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(DeleteTaskOutcome::Deleted)
    }

    /// Completes one open task with optimistic version matching. A missing,
    /// already-completed, or concurrently changed task returns `Ok(None)` so
    /// the HTTP layer can reload current state without leaking another user's
    /// task.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for an invalid expected
    /// version or a classified persistence error when storage is unavailable.
    pub async fn complete_task(
        &self,
        user_id: Uuid,
        task_id: Uuid,
        expected_version: i64,
    ) -> Result<Option<Task>, StorageError> {
        if expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, TaskRow>(
            "\
            UPDATE tasks
            SET status = 'completed', completed_at = NOW()
            WHERE id = $1 AND user_id = $2 AND status = 'open' AND version = $3
            RETURNING id, project_id, title, notes, assignee_name, status, priority, due_at, completed_at, version",
        )
        .bind(task_id)
        .bind(user_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let task = Task::try_from(row)?;
        append_change(&mut transaction, user_id, "task", task.id, task.version).await?;
        queue_task_webhook_in_transaction(&mut transaction, user_id, &task, "task.completed")
            .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(Some(task))
    }
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.trim().is_empty())
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

fn valid_time_zone(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 80 && !value.chars().any(char::is_control)
}

fn calendar_payload(entry: &ScheduleEntry) -> ScheduleCalendarMutationPayload {
    ScheduleCalendarMutationPayload {
        title: entry.title.clone(),
        notes: entry.notes.clone(),
        starts_at: entry.starts_at,
        ends_at: entry.ends_at,
        time_zone: entry.time_zone.clone(),
    }
}

fn schedule_matches_new(existing: &ScheduleEntry, requested: &NewScheduleEntry) -> bool {
    existing.title == requested.title.trim()
        && existing.notes.as_deref()
            == requested
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        && existing.starts_at == requested.starts_at
        && existing.ends_at == requested.ends_at
        && existing.time_zone == requested.time_zone.trim()
}

fn task_matches_new(existing: &Task, requested: &NewTask) -> bool {
    existing.project_id == requested.project_id
        && existing.title == requested.title.trim()
        && existing.notes.as_deref()
            == requested
                .notes
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        && existing.priority == requested.priority
        && existing.due_at == requested.due_at
}

pub(crate) async fn queue_task_webhook_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    task: &Task,
    event_type: &str,
) -> Result<(), StorageError> {
    let Some(project_id) = task.project_id else {
        return Ok(());
    };
    let mut payload = project_event_payload(event_type, project_id, task.id)?;
    let object = payload
        .as_object_mut()
        .ok_or(StorageError::PersistenceUnavailable)?;
    object.insert("title".to_owned(), serde_json::json!(task.title));
    object.insert(
        "dueAt".to_owned(),
        task.due_at
            .map(|value| value.format(&Rfc3339))
            .transpose()
            .map_err(|_| StorageError::PersistenceUnavailable)?
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    object.insert(
        "assigneeName".to_owned(),
        task.assignee_name
            .clone()
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    object.insert(
        "message".to_owned(),
        serde_json::Value::String(task_event_message(event_type, task)),
    );
    queue_project_event_in_transaction(transaction, user_id, project_id, event_type, &payload)
        .await?;
    Ok(())
}

fn task_event_message(event_type: &str, task: &Task) -> String {
    let action = match event_type {
        "task.created" => "새 할 일이 등록됐어요.",
        "task.completed" => "할 일을 완료했어요.",
        "task.restored" => "완료한 일을 다시 열었어요.",
        "task.deleted" => "할 일이 삭제됐어요.",
        _ => "할 일이 변경됐어요.",
    };
    let mut lines = vec![action.to_owned(), task.title.clone()];
    if let Some(due_at) = task.due_at
        && let Ok(value) = due_at.format(&Rfc3339)
    {
        lines.push(format!("기한: {value}"));
    }
    lines.join("\n")
}

fn task_update_event_type(
    previous_status: Option<&str>,
    current_status: TaskStatus,
) -> &'static str {
    match (previous_status, current_status) {
        (Some("completed"), TaskStatus::Open) => "task.restored",
        (_, TaskStatus::Completed) => "task.completed",
        (_, TaskStatus::Cancelled) => "task.deleted",
        _ => "task.updated",
    }
}

fn task_status_name(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::Completed => "completed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

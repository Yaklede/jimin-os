//! Server-owned schedule and task persistence used before any external
//! calendar provider is linked.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

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
    version: i64,
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    project_id: Option<Uuid>,
    title: String,
    notes: Option<String>,
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
            status,
            priority: row.priority,
            due_at: row.due_at,
            completed_at: row.completed_at,
            version: row.version,
        })
    }
}

impl Database {
    /// Creates a manual schedule entry and appends the matching sync change in
    /// the same transaction.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing personal entry text.
    pub async fn create_schedule_entry(
        &self,
        entry: &NewScheduleEntry,
    ) -> Result<ScheduleEntry, StorageError> {
        entry.validate()?;
        let user_id = entry.user_id;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ScheduleRow>(
            "\
            INSERT INTO schedule_entries (
                id, user_id, title, notes, starts_at, ends_at, time_zone, source, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual', 'confirmed')
            RETURNING id, title, notes, starts_at, ends_at, time_zone, status, source, version",
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
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let entry = ScheduleEntry::try_from(row)?;
        append_change(
            &mut transaction,
            user_id,
            "schedule_entry",
            entry.id,
            entry.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(entry)
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
                SELECT id, title, notes, starts_at, ends_at, time_zone, status, source, version
                FROM schedule_entries
                WHERE user_id = $1
                  AND status = 'confirmed'
                  AND starts_at < $2
                  AND ends_at > $3
                UNION ALL
                SELECT id, title, description_text AS notes, start_at AS starts_at, end_at AS ends_at,
                    source_time_zone AS time_zone, 'confirmed'::TEXT AS status,
                    'google_calendar'::TEXT AS source, version
                FROM calendar_events
                WHERE user_id = $1
                  AND provider_deleted_at IS NULL
                  AND provider_status IN ('confirmed', 'tentative')
                  AND time_kind = 'date_time'
                  AND start_at < $2
                  AND end_at > $3
                UNION ALL
                SELECT id, title, description_text AS notes,
                    (start_date::timestamp AT TIME ZONE 'UTC') AS starts_at,
                    (end_date::timestamp AT TIME ZONE 'UTC') AS ends_at,
                    'UTC'::TEXT AS time_zone, 'confirmed'::TEXT AS status,
                    'google_calendar'::TEXT AS source, version
                FROM calendar_events
                WHERE user_id = $1
                  AND provider_deleted_at IS NULL
                  AND provider_status IN ('confirmed', 'tentative')
                  AND time_kind = 'date'
                  AND start_date < $4
                  AND end_date > $5
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
            RETURNING id, project_id, title, notes, status, priority, due_at, completed_at, version",
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
        transaction.commit().await.map_err(classify)?;
        Ok(task)
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
            SELECT id, project_id, title, notes, status, priority, due_at, completed_at, version
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
            SELECT id, project_id, title, notes, status, priority, due_at, completed_at, version
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
            SELECT id, project_id, title, notes, status, priority, due_at, completed_at, version
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
            SELECT id, project_id, title, notes, status, priority, due_at, completed_at, version
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
            SELECT id, project_id, title, notes, status, priority, due_at, completed_at, version
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
            RETURNING id, project_id, title, notes, status, priority, due_at, completed_at, version",
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
        transaction.commit().await.map_err(classify)?;
        Ok(Some(task))
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
            RETURNING id, project_id, title, notes, status, priority, due_at, completed_at, version",
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

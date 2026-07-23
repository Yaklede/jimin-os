//! Workspace and project persistence for the personal work operating system.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::{append_change, append_delete_change},
    webhook::{project_event_payload, queue_project_event_in_transaction},
};

const MAX_TITLE_CHARS: usize = 200;
const MAX_OBJECTIVE_CHARS: usize = 10_000;
const MAX_NEXT_ACTION_CHARS: usize = 500;

/// The data boundary that keeps personal and company work separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceScope {
    Personal,
    Company,
}

/// A user-owned work scope safe to return to a signed-in client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: Uuid,
    pub scope: WorkspaceScope,
    pub name: String,
    pub version: i64,
}

/// The lifecycle state of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    Active,
    Paused,
    Completed,
}

/// A project and its current work summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    pub objective: Option<String>,
    pub status: ProjectStatus,
    pub risk_level: i16,
    pub next_action: Option<String>,
    pub due_at: Option<OffsetDateTime>,
    pub open_task_count: i64,
    pub total_task_count: i64,
    pub completed_task_count: i64,
    pub overdue_task_count: i64,
    pub unassigned_task_count: i64,
    pub progress_percent: i16,
    pub version: i64,
}

/// Validated input for creating one project inside an owned workspace.
pub struct NewProject {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    pub objective: Option<String>,
    pub risk_level: i16,
    pub next_action: Option<String>,
    pub due_at: Option<OffsetDateTime>,
}

/// A complete, version-checked replacement of the mutable project fields.
/// Keeping the update complete prevents ambiguous partial-null semantics at
/// the API boundary and makes concurrent edits visible to the client.
pub struct ProjectUpdate {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub objective: Option<String>,
    pub status: ProjectStatus,
    pub risk_level: i16,
    pub next_action: Option<String>,
    pub due_at: Option<OffsetDateTime>,
    pub expected_version: i64,
}

/// Result of deleting one user-owned project with optimistic concurrency.
/// Missing and foreign projects intentionally share [`Self::AlreadyAbsent`]
/// so callers cannot use deletion to discover another workspace's records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteProjectOutcome {
    Deleted,
    AlreadyAbsent,
    VersionConflict,
}

impl NewProject {
    /// Validates a project before database access.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid user input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.workspace_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !self
                .objective
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_OBJECTIVE_CHARS, true))
            || !self
                .next_action
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_NEXT_ACTION_CHARS, true))
            || !(0..=3).contains(&self.risk_level)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl ProjectUpdate {
    /// Validates all mutable project fields before database access.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs,
    /// text, risk, or optimistic version values.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !self
                .objective
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_OBJECTIVE_CHARS, true))
            || !self
                .next_action
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_NEXT_ACTION_CHARS, true))
            || !(0..=3).contains(&self.risk_level)
            || self.expected_version <= 0
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct WorkspaceRow {
    id: Uuid,
    scope: String,
    name: String,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    workspace_id: Uuid,
    title: String,
    objective: Option<String>,
    status: String,
    risk_level: i16,
    next_action: Option<String>,
    due_at: Option<OffsetDateTime>,
    open_task_count: i64,
    total_task_count: i64,
    completed_task_count: i64,
    overdue_task_count: i64,
    unassigned_task_count: i64,
    progress_percent: i16,
    version: i64,
}

impl TryFrom<WorkspaceRow> for Workspace {
    type Error = StorageError;

    fn try_from(row: WorkspaceRow) -> Result<Self, Self::Error> {
        let scope = match row.scope.as_str() {
            "personal" => WorkspaceScope::Personal,
            "company" => WorkspaceScope::Company,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            scope,
            name: row.name,
            version: row.version,
        })
    }
}

impl TryFrom<ProjectRow> for Project {
    type Error = StorageError;

    fn try_from(row: ProjectRow) -> Result<Self, Self::Error> {
        let status = match row.status.as_str() {
            "active" => ProjectStatus::Active,
            "paused" => ProjectStatus::Paused,
            "completed" => ProjectStatus::Completed,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            title: row.title,
            objective: row.objective,
            status,
            risk_level: row.risk_level,
            next_action: row.next_action,
            due_at: row.due_at,
            open_task_count: row.open_task_count,
            total_task_count: row.total_task_count,
            completed_task_count: row.completed_task_count,
            overdue_task_count: row.overdue_task_count,
            unassigned_task_count: row.unassigned_task_count,
            progress_percent: row.progress_percent,
            version: row.version,
        })
    }
}

impl Database {
    /// Returns both work scopes. They are created on demand so existing and
    /// newly provisioned private-server users see the same stable scopes.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error if storage is unavailable.
    pub async fn workspaces_for_user(&self, user_id: Uuid) -> Result<Vec<Workspace>, StorageError> {
        self.ensure_default_workspaces(user_id).await?;
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, scope, name, version\n             FROM workspaces\n             WHERE user_id = $1\n             ORDER BY CASE scope WHEN 'personal' THEN 0 ELSE 1 END",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Workspace::try_from).collect()
    }

    /// Creates a project in a workspace owned by the current user.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] if the requested
    /// workspace is not owned by the user or the input is invalid.
    pub async fn create_project(&self, project: &NewProject) -> Result<Project, StorageError> {
        project.validate()?;
        let user_id = project.user_id;
        self.ensure_default_workspaces(user_id).await?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ProjectRow>(
            "\
            INSERT INTO projects (
                id, user_id, workspace_id, title, objective, status, risk_level, next_action, due_at
            )
            SELECT $1, $2, workspaces.id, $3, $4, 'active', $5, $6, $7
            FROM workspaces
            WHERE workspaces.id = $8 AND workspaces.user_id = $2
            RETURNING id, workspace_id, title, objective, status, risk_level, next_action, due_at,
                0::BIGINT AS open_task_count,
                0::BIGINT AS total_task_count,
                0::BIGINT AS completed_task_count,
                0::BIGINT AS overdue_task_count,
                0::BIGINT AS unassigned_task_count,
                0::SMALLINT AS progress_percent,
                version",
        )
        .bind(project.id)
        .bind(project.user_id)
        .bind(project.title.trim())
        .bind(trim_optional(project.objective.as_ref()))
        .bind(project.risk_level)
        .bind(trim_optional(project.next_action.as_ref()))
        .bind(project.due_at)
        .bind(project.workspace_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(classify)?;
            return Err(StorageError::InvalidConfiguration);
        };
        let project = Project::try_from(row)?;
        let payload = project_event_payload("project.created", project.id, project.id)?;
        queue_project_event_in_transaction(
            &mut transaction,
            user_id,
            project.id,
            "project.created",
            &payload,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "project",
            project.id,
            project.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(project)
    }

    /// Replaces the mutable fields of one owned project when its version still
    /// matches. Missing, foreign, or concurrently changed records return
    /// `Ok(None)` without revealing which condition occurred.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input and
    /// a classified persistence error for database failures.
    pub async fn update_project(
        &self,
        update: &ProjectUpdate,
    ) -> Result<Option<Project>, StorageError> {
        update.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ProjectRow>(
            "\
            UPDATE projects
            SET title = $1,
                objective = $2,
                status = $3,
                risk_level = $4,
                next_action = $5,
                due_at = $6
            WHERE id = $7 AND user_id = $8 AND version = $9
            RETURNING id, workspace_id, title, objective, status, risk_level, next_action, due_at,
                (SELECT COUNT(*)::BIGINT FROM tasks
                 WHERE tasks.project_id = projects.id AND tasks.status = 'open') AS open_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status IN ('open', 'completed')
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS total_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'completed'
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS completed_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'open' AND task.due_at < NOW()
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS overdue_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'open' AND task.assignee_name IS NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS unassigned_task_count,
                COALESCE((
                    SELECT (
                        COUNT(*) FILTER (WHERE task.status = 'completed') * 100
                        / NULLIF(COUNT(*), 0)
                    )::SMALLINT
                    FROM tasks AS task
                    WHERE task.project_id = projects.id
                      AND task.status IN ('open', 'completed')
                      AND NOT EXISTS (
                          SELECT 1 FROM tasks AS child
                          WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                      )
                ), 0::SMALLINT) AS progress_percent,
                version",
        )
        .bind(update.title.trim())
        .bind(trim_optional(update.objective.as_ref()))
        .bind(project_status_name(update.status))
        .bind(update.risk_level)
        .bind(trim_optional(update.next_action.as_ref()))
        .bind(update.due_at)
        .bind(update.id)
        .bind(update.user_id)
        .bind(update.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let project = Project::try_from(row)?;
        let payload = project_event_payload("project.updated", project.id, project.id)?;
        queue_project_event_in_transaction(
            &mut transaction,
            update.user_id,
            project.id,
            "project.updated",
            &payload,
        )
        .await?;
        append_change(
            &mut transaction,
            update.user_id,
            "project",
            project.id,
            project.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(Some(project))
    }

    /// Deletes one owned project after safely detaching its tasks. Replaying a
    /// successful deletion is idempotent. A foreign project is treated as
    /// absent, while a stale version for an owned project remains visible as a
    /// concurrency conflict.
    ///
    /// Task detachment, sync changes, the project deletion webhook snapshot,
    /// and the project tombstone are committed atomically. This prevents a
    /// synced device from retaining a task link to a project that no longer
    /// exists.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs or
    /// versions and a classified persistence error for database failures.
    pub async fn delete_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        expected_version: i64,
    ) -> Result<DeleteProjectOutcome, StorageError> {
        if !is_v7(user_id) || !is_v7(project_id) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM projects WHERE id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(current_version) = current_version else {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteProjectOutcome::AlreadyAbsent);
        };
        if current_version != expected_version {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteProjectOutcome::VersionConflict);
        }

        let payload = project_event_payload("project.deleted", project_id, project_id)?;
        queue_project_event_in_transaction(
            &mut transaction,
            user_id,
            project_id,
            "project.deleted",
            &payload,
        )
        .await?;

        let detached_tasks = sqlx::query_as::<_, (Uuid, i64)>(
            "\
            UPDATE tasks
            SET project_id = NULL
            WHERE user_id = $1 AND project_id = $2
            RETURNING id, version",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        for (task_id, task_version) in detached_tasks {
            append_change(&mut transaction, user_id, "task", task_id, task_version).await?;
        }

        let deleted_version = sqlx::query_scalar::<_, i64>(
            "\
            DELETE FROM projects
            WHERE id = $1 AND user_id = $2 AND version = $3
            RETURNING version",
        )
        .bind(project_id)
        .bind(user_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(deleted_version) = deleted_version else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectOutcome::VersionConflict);
        };
        append_delete_change(
            &mut transaction,
            user_id,
            "project",
            project_id,
            deleted_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(DeleteProjectOutcome::Deleted)
    }

    /// Lists projects in one owned workspace with their open work-item count.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs.
    pub async fn projects_for_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<Project>, StorageError> {
        if !is_v7(workspace_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        self.ensure_default_workspaces(user_id).await?;
        let rows = sqlx::query_as::<_, ProjectRow>(
            "\
            SELECT
                projects.id, projects.workspace_id, projects.title, projects.objective,
                projects.status, projects.risk_level, projects.next_action, projects.due_at,
                projects.version,
                (SELECT COUNT(*)::BIGINT FROM tasks AS task
                 WHERE task.project_id = projects.id AND task.status = 'open') AS open_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status IN ('open', 'completed')
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS total_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'completed'
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS completed_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'open' AND task.due_at < NOW()
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS overdue_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'open' AND task.assignee_name IS NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS unassigned_task_count,
                COALESCE((
                    SELECT (
                        COUNT(*) FILTER (WHERE task.status = 'completed') * 100
                        / NULLIF(COUNT(*), 0)
                    )::SMALLINT
                    FROM tasks AS task
                    WHERE task.project_id = projects.id
                      AND task.status IN ('open', 'completed')
                      AND NOT EXISTS (
                          SELECT 1 FROM tasks AS child
                          WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                      )
                ), 0::SMALLINT) AS progress_percent
            FROM projects
            WHERE projects.user_id = $1 AND projects.workspace_id = $2
            ORDER BY
                CASE projects.status WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END,
                projects.risk_level DESC,
                projects.due_at NULLS LAST,
                projects.updated_at DESC,
                projects.id ASC",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Project::try_from).collect()
    }

    /// Lists every project visible to the current user for assistant context.
    /// Workspace ownership is retained on each result so a validated assistant
    /// presentation can navigate to the correct personal or company scope.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error if storage is unavailable.
    pub async fn projects_for_user(&self, user_id: Uuid) -> Result<Vec<Project>, StorageError> {
        self.ensure_default_workspaces(user_id).await?;
        let rows = sqlx::query_as::<_, ProjectRow>(
            "\
            SELECT
                projects.id, projects.workspace_id, projects.title, projects.objective,
                projects.status, projects.risk_level, projects.next_action, projects.due_at,
                projects.version,
                (SELECT COUNT(*)::BIGINT FROM tasks AS task
                 WHERE task.project_id = projects.id AND task.status = 'open') AS open_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status IN ('open', 'completed')
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS total_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'completed'
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS completed_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'open' AND task.due_at < NOW()
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS overdue_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = projects.id
                   AND task.status = 'open' AND task.assignee_name IS NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS unassigned_task_count,
                COALESCE((
                    SELECT (
                        COUNT(*) FILTER (WHERE task.status = 'completed') * 100
                        / NULLIF(COUNT(*), 0)
                    )::SMALLINT
                    FROM tasks AS task
                    WHERE task.project_id = projects.id
                      AND task.status IN ('open', 'completed')
                      AND NOT EXISTS (
                          SELECT 1 FROM tasks AS child
                          WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                      )
                ), 0::SMALLINT) AS progress_percent
            FROM projects
            WHERE projects.user_id = $1
            ORDER BY
                CASE projects.status WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END,
                projects.risk_level DESC,
                projects.due_at NULLS LAST,
                projects.updated_at DESC,
                projects.id ASC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Project::try_from).collect()
    }

    /// Returns whether a project belongs to the current user.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error if storage is unavailable.
    pub async fn project_exists_for_user(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, StorageError> {
        if !is_v7(project_id) {
            return Ok(false);
        }
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1 AND user_id = $2)")
            .bind(project_id)
            .bind(user_id)
            .fetch_one(self.pool())
            .await
            .map_err(classify)
    }

    pub(crate) async fn ensure_default_workspaces(
        &self,
        user_id: Uuid,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "\
            INSERT INTO workspaces (id, user_id, scope, name)
            VALUES ($1, $3, 'personal', '개인'), ($2, $3, 'company', '회사')
            ON CONFLICT (user_id, scope) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(Uuid::now_v7())
        .bind(user_id)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }
}

fn trim_optional(value: Option<&String>) -> Option<&str> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

const fn project_status_name(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Active => "active",
        ProjectStatus::Paused => "paused",
        ProjectStatus::Completed => "completed",
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

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

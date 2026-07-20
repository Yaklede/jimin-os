//! Owner-scoped goals that connect daily work to a desired outcome.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

const MAX_TITLE_CHARS: usize = 200;
const MAX_OUTCOME_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    Active,
    Paused,
    Achieved,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalHealth {
    OnTrack,
    AtRisk,
    NeedsPlan,
    ReadyToComplete,
    Paused,
    Achieved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalNextActionKind {
    Task,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalNextAction {
    pub kind: GoalNextActionKind,
    pub id: Option<Uuid>,
    pub title: String,
    pub due_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goal {
    pub id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub desired_outcome: String,
    pub status: GoalStatus,
    pub target_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

/// Server-derived evidence that explains how connected work advances a goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalOverview {
    pub goal: Goal,
    pub project_title: Option<String>,
    pub progress_percent: i16,
    pub total_task_count: i64,
    pub open_task_count: i64,
    pub completed_task_count: i64,
    pub completed_last_seven_days: i64,
    pub overdue_task_count: i64,
    pub health: GoalHealth,
    pub next_action: Option<GoalNextAction>,
}

pub struct NewGoal {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub desired_outcome: String,
    pub target_at: Option<OffsetDateTime>,
}

pub struct GoalUpdate {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub desired_outcome: String,
    pub status: GoalStatus,
    pub target_at: Option<OffsetDateTime>,
    pub expected_version: i64,
}

#[derive(sqlx::FromRow)]
struct GoalRow {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    title: String,
    desired_outcome: String,
    status: String,
    target_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct GoalOverviewRow {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    title: String,
    desired_outcome: String,
    status: String,
    target_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
    project_title: Option<String>,
    project_status: Option<String>,
    project_next_action: Option<String>,
    total_task_count: i64,
    open_task_count: i64,
    completed_task_count: i64,
    completed_last_seven_days: i64,
    overdue_task_count: i64,
    next_task_id: Option<Uuid>,
    next_task_title: Option<String>,
    next_task_due_at: Option<OffsetDateTime>,
}

impl NewGoal {
    /// Validates bounded copy and owner-scoped identifiers before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_optional_id(self.workspace_id)
            || !valid_optional_id(self.project_id)
            || (self.project_id.is_some() && self.workspace_id.is_none())
            || !valid_text(&self.title, MAX_TITLE_CHARS)
            || !valid_text(&self.desired_outcome, MAX_OUTCOME_CHARS)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl GoalUpdate {
    /// Validates a version-checked goal replacement before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_optional_id(self.workspace_id)
            || !valid_optional_id(self.project_id)
            || (self.project_id.is_some() && self.workspace_id.is_none())
            || !valid_text(&self.title, MAX_TITLE_CHARS)
            || !valid_text(&self.desired_outcome, MAX_OUTCOME_CHARS)
            || self.expected_version <= 0
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl TryFrom<GoalRow> for Goal {
    type Error = StorageError;

    fn try_from(row: GoalRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            project_id: row.project_id,
            title: row.title,
            desired_outcome: row.desired_outcome,
            status: parse_status(&row.status)?,
            target_at: row.target_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}

impl Database {
    /// Creates one goal after verifying every optional scope belongs to the owner.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, or persistence error.
    pub async fn create_goal(&self, goal: &NewGoal) -> Result<Goal, StorageError> {
        goal.validate()?;
        self.ensure_default_workspaces(goal.user_id).await?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if !goal_scope_is_owned(
            &mut transaction,
            goal.user_id,
            goal.workspace_id,
            goal.project_id,
        )
        .await?
        {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        }
        let row = sqlx::query_as::<_, GoalRow>(
            "INSERT INTO goals (
                id, user_id, workspace_id, project_id, title, desired_outcome,
                status, target_at
             ) VALUES ($1, $2, $3, $4, $5, $6, 'active', $7)
             RETURNING id, workspace_id, project_id, title, desired_outcome,
                 status, target_at, created_at, updated_at, version",
        )
        .bind(goal.id)
        .bind(goal.user_id)
        .bind(goal.workspace_id)
        .bind(goal.project_id)
        .bind(goal.title.trim())
        .bind(goal.desired_outcome.trim())
        .bind(goal.target_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(&mut transaction, goal.user_id, "goal", row.id, row.version).await?;
        transaction.commit().await.map_err(classify)?;
        Goal::try_from(row)
    }

    /// Lists all goals for one owner in active-first priority order.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn goals_for_user(&self, user_id: Uuid) -> Result<Vec<Goal>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, GoalRow>(
            "SELECT id, workspace_id, project_id, title, desired_outcome,
                status, target_at, created_at, updated_at, version
             FROM goals
             WHERE user_id = $1
             ORDER BY
                CASE status
                    WHEN 'active' THEN 0
                    WHEN 'paused' THEN 1
                    WHEN 'achieved' THEN 2
                    ELSE 3
                END,
                target_at ASC NULLS LAST, updated_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Goal::try_from).collect()
    }

    /// Lists goals together with progress and the next executable action
    /// derived from their connected project tasks.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn goal_overviews_for_user(
        &self,
        user_id: Uuid,
        now: OffsetDateTime,
    ) -> Result<Vec<GoalOverview>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, GoalOverviewRow>(
            "SELECT
                goals.id, goals.workspace_id, goals.project_id, goals.title,
                goals.desired_outcome, goals.status, goals.target_at,
                goals.created_at, goals.updated_at, goals.version,
                projects.title AS project_title,
                projects.status AS project_status,
                projects.next_action AS project_next_action,
                COALESCE((
                    SELECT COUNT(*) FROM tasks
                    WHERE tasks.user_id = goals.user_id
                      AND tasks.project_id = goals.project_id
                      AND tasks.status IN ('open', 'completed')
                ), 0)::BIGINT AS total_task_count,
                COALESCE((
                    SELECT COUNT(*) FROM tasks
                    WHERE tasks.user_id = goals.user_id
                      AND tasks.project_id = goals.project_id
                      AND tasks.status = 'open'
                ), 0)::BIGINT AS open_task_count,
                COALESCE((
                    SELECT COUNT(*) FROM tasks
                    WHERE tasks.user_id = goals.user_id
                      AND tasks.project_id = goals.project_id
                      AND tasks.status = 'completed'
                ), 0)::BIGINT AS completed_task_count,
                COALESCE((
                    SELECT COUNT(*) FROM tasks
                    WHERE tasks.user_id = goals.user_id
                      AND tasks.project_id = goals.project_id
                      AND tasks.status = 'completed'
                      AND tasks.completed_at >= $2 - INTERVAL '7 days'
                ), 0)::BIGINT AS completed_last_seven_days,
                COALESCE((
                    SELECT COUNT(*) FROM tasks
                    WHERE tasks.user_id = goals.user_id
                      AND tasks.project_id = goals.project_id
                      AND tasks.status = 'open'
                      AND tasks.due_at < $2
                ), 0)::BIGINT AS overdue_task_count,
                next_task.id AS next_task_id,
                next_task.title AS next_task_title,
                next_task.due_at AS next_task_due_at
             FROM goals
             LEFT JOIN projects
               ON projects.id = goals.project_id
              AND projects.user_id = goals.user_id
             LEFT JOIN LATERAL (
                SELECT tasks.id, tasks.title, tasks.due_at
                FROM tasks
                WHERE tasks.user_id = goals.user_id
                  AND tasks.project_id = goals.project_id
                  AND tasks.status = 'open'
                ORDER BY
                  (tasks.due_at IS NOT NULL AND tasks.due_at < $2) DESC,
                  tasks.priority DESC,
                  tasks.due_at ASC NULLS LAST,
                  tasks.created_at ASC,
                  tasks.id ASC
                LIMIT 1
             ) AS next_task ON TRUE
             WHERE goals.user_id = $1
             ORDER BY
                CASE goals.status
                    WHEN 'active' THEN 0
                    WHEN 'paused' THEN 1
                    WHEN 'achieved' THEN 2
                    ELSE 3
                END,
                goals.target_at ASC NULLS LAST,
                goals.updated_at DESC,
                goals.id DESC",
        )
        .bind(user_id)
        .bind(now)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter()
            .map(|row| goal_overview(row, now))
            .collect()
    }

    /// Loads one owned goal with the same progress evidence used by the list.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn goal_overview_for_user(
        &self,
        user_id: Uuid,
        goal_id: Uuid,
        now: OffsetDateTime,
    ) -> Result<Option<GoalOverview>, StorageError> {
        if !is_v7(goal_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(self
            .goal_overviews_for_user(user_id, now)
            .await?
            .into_iter()
            .find(|overview| overview.goal.id == goal_id))
    }

    /// Replaces mutable goal fields using optimistic concurrency.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, or persistence error.
    pub async fn update_goal(&self, update: &GoalUpdate) -> Result<Option<Goal>, StorageError> {
        update.validate()?;
        self.ensure_default_workspaces(update.user_id).await?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if !goal_scope_is_owned(
            &mut transaction,
            update.user_id,
            update.workspace_id,
            update.project_id,
        )
        .await?
        {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        }
        let row = sqlx::query_as::<_, GoalRow>(
            "UPDATE goals
             SET workspace_id = $3, project_id = $4, title = $5,
                 desired_outcome = $6, status = $7, target_at = $8
             WHERE id = $1 AND user_id = $2 AND version = $9
             RETURNING id, workspace_id, project_id, title, desired_outcome,
                 status, target_at, created_at, updated_at, version",
        )
        .bind(update.id)
        .bind(update.user_id)
        .bind(update.workspace_id)
        .bind(update.project_id)
        .bind(update.title.trim())
        .bind(update.desired_outcome.trim())
        .bind(status_value(update.status))
        .bind(update.target_at)
        .bind(update.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        append_change(
            &mut transaction,
            update.user_id,
            "goal",
            update.id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Goal::try_from(row).map(Some)
    }
}

fn goal_overview(row: GoalOverviewRow, now: OffsetDateTime) -> Result<GoalOverview, StorageError> {
    let goal = Goal {
        id: row.id,
        workspace_id: row.workspace_id,
        project_id: row.project_id,
        title: row.title,
        desired_outcome: row.desired_outcome,
        status: parse_status(&row.status)?,
        target_at: row.target_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version,
    };
    let project_completed = row.project_status.as_deref() == Some("completed");
    let progress_percent = if goal.status == GoalStatus::Achieved || project_completed {
        100
    } else if row.total_task_count == 0 {
        0
    } else {
        i16::try_from(row.completed_task_count.saturating_mul(100) / row.total_task_count)
            .map_err(|_| StorageError::PersistenceUnavailable)?
    };
    let health = goal_health(
        &goal,
        project_completed,
        row.total_task_count,
        row.open_task_count,
        row.overdue_task_count,
        progress_percent,
        now,
    );
    let next_action = if matches!(health, GoalHealth::Achieved | GoalHealth::Paused) {
        None
    } else if let Some(title) = row.next_task_title {
        Some(GoalNextAction {
            kind: GoalNextActionKind::Task,
            id: row.next_task_id,
            title,
            due_at: row.next_task_due_at,
        })
    } else {
        row.project_next_action.map(|title| GoalNextAction {
            kind: GoalNextActionKind::Project,
            id: goal.project_id,
            title,
            due_at: None,
        })
    };
    Ok(GoalOverview {
        goal,
        project_title: row.project_title,
        progress_percent,
        total_task_count: row.total_task_count,
        open_task_count: row.open_task_count,
        completed_task_count: row.completed_task_count,
        completed_last_seven_days: row.completed_last_seven_days,
        overdue_task_count: row.overdue_task_count,
        health,
        next_action,
    })
}

fn goal_health(
    goal: &Goal,
    project_completed: bool,
    total_task_count: i64,
    open_task_count: i64,
    overdue_task_count: i64,
    progress_percent: i16,
    now: OffsetDateTime,
) -> GoalHealth {
    match goal.status {
        GoalStatus::Achieved => GoalHealth::Achieved,
        GoalStatus::Paused | GoalStatus::Cancelled => GoalHealth::Paused,
        GoalStatus::Active if goal.project_id.is_none() => GoalHealth::NeedsPlan,
        GoalStatus::Active
            if project_completed || (total_task_count > 0 && progress_percent == 100) =>
        {
            GoalHealth::ReadyToComplete
        }
        GoalStatus::Active
            if overdue_task_count > 0
                || goal.target_at.is_some_and(|target_at| target_at < now)
                || (open_task_count > 0
                    && goal
                        .target_at
                        .is_some_and(|target_at| target_at <= now + time::Duration::days(7))) =>
        {
            GoalHealth::AtRisk
        }
        GoalStatus::Active if total_task_count == 0 => GoalHealth::NeedsPlan,
        GoalStatus::Active => GoalHealth::OnTrack,
    }
}

async fn goal_scope_is_owned(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> Result<bool, StorageError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT
            ($2::uuid IS NULL OR EXISTS(
                SELECT 1 FROM workspaces WHERE id = $2 AND user_id = $1
            ))
            AND ($3::uuid IS NULL OR EXISTS(
                SELECT 1 FROM projects
                WHERE id = $3 AND user_id = $1 AND workspace_id = $2
            ))",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(project_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

const fn status_value(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Active => "active",
        GoalStatus::Paused => "paused",
        GoalStatus::Achieved => "achieved",
        GoalStatus::Cancelled => "cancelled",
    }
}

fn parse_status(value: &str) -> Result<GoalStatus, StorageError> {
    match value {
        "active" => Ok(GoalStatus::Active),
        "paused" => Ok(GoalStatus::Paused),
        "achieved" => Ok(GoalStatus::Achieved),
        "cancelled" => Ok(GoalStatus::Cancelled),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn valid_optional_id(value: Option<Uuid>) -> bool {
    value.is_none_or(is_v7)
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_text(value: &str, maximum: usize) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{GoalStatus, GoalUpdate, NewGoal};
    use uuid::Uuid;

    #[test]
    fn goal_input_requires_an_outcome_and_consistent_scope() {
        let valid = NewGoal {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            workspace_id: Some(Uuid::now_v7()),
            project_id: Some(Uuid::now_v7()),
            title: "업무 자동화 범위 확대".to_owned(),
            desired_outcome: "반복 업무 시간을 매주 5시간 줄인다.".to_owned(),
            target_at: None,
        };
        assert!(valid.validate().is_ok());

        let invalid = NewGoal {
            workspace_id: None,
            ..valid
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn goal_update_requires_a_positive_version() {
        let update = GoalUpdate {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            workspace_id: None,
            project_id: None,
            title: "순자산 목표".to_owned(),
            desired_outcome: "목표 순자산을 달성한다.".to_owned(),
            status: GoalStatus::Active,
            target_at: None,
            expected_version: 0,
        };
        assert!(update.validate().is_err());
    }
}

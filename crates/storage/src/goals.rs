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

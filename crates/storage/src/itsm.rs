//! Owner-scoped project opt-in for deployment-managed ITSM enrichment.
//!
//! This module deliberately stores no provider origin, token, or authorization
//! material. The agent owns one trusted read-only client at deployment time,
//! while this table decides which owned projects may use it.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::{append_change, append_delete_change},
};

/// Internal project connection state. Public API projections must omit both
/// upstream identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectItsmConnection {
    pub id: Uuid,
    pub project_id: Uuid,
    /// Owner-confirmed upstream project boundary. Never expose this identifier
    /// through the public API.
    pub itsm_project_id: Option<String>,
    /// Agent-detected candidate identifier. This remains internal and is
    /// cleared when the owner confirms the candidate.
    pub candidate_itsm_project_id: Option<String>,
    /// Bounded candidate label safe to show to the owning user.
    pub candidate_itsm_project_name: Option<String>,
    pub enabled: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

/// Validated input for connecting the deployment-managed ITSM client.
pub struct NewProjectItsmConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub enabled: bool,
}

/// Version-checked mutable project ITSM connection state.
pub struct ProjectItsmConnectionUpdate {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub enabled: bool,
    pub expected_version: i64,
}

/// Result of deleting one project connection without exposing foreign records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteProjectItsmConnectionOutcome {
    Deleted,
    AlreadyAbsent,
    VersionConflict,
}

/// Version-checked owner command for confirming one detected candidate.
pub struct ConfirmProjectItsmConnection {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub expected_connection_id: Uuid,
    pub expected_version: i64,
}

/// Version-checked owner command for deleting one exact connection generation.
pub struct DeleteProjectItsmConnection {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub expected_connection_id: Uuid,
    pub expected_version: i64,
}

/// Result of proposing an upstream project boundary from one trusted lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectItsmCandidateOutcome {
    /// The owner already confirmed this upstream project.
    Confirmed,
    /// This candidate is stored but still requires an owner decision.
    ConfirmationRequired,
    /// A confirmed boundary points at another upstream project.
    ProjectMismatch,
    /// Another candidate is already waiting for owner confirmation.
    CandidateMismatch,
    /// The connection is missing or disabled.
    ConnectionUnavailable,
}

/// Result of an owner-confirmed, version-checked candidate transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmProjectItsmConnectionOutcome {
    Confirmed(ProjectItsmConnection),
    CandidateMissing,
    ConnectionUnavailable,
    VersionConflict,
}

#[derive(sqlx::FromRow)]
struct ProjectItsmConnectionRow {
    id: Uuid,
    project_id: Uuid,
    itsm_project_id: Option<String>,
    candidate_itsm_project_id: Option<String>,
    candidate_itsm_project_name: Option<String>,
    enabled: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
}

impl From<ProjectItsmConnectionRow> for ProjectItsmConnection {
    fn from(row: ProjectItsmConnectionRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            itsm_project_id: row.itsm_project_id,
            candidate_itsm_project_id: row.candidate_itsm_project_id,
            candidate_itsm_project_name: row.candidate_itsm_project_name,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        }
    }
}

impl NewProjectItsmConnection {
    /// Validates owner and project identifiers before database access.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for non-v7 identifiers.
    pub fn validate(&self) -> Result<(), StorageError> {
        if ![self.id, self.user_id, self.project_id]
            .into_iter()
            .all(is_v7)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl ProjectItsmConnectionUpdate {
    /// Validates identifiers and the optimistic concurrency version.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if ![self.user_id, self.project_id].into_iter().all(is_v7) || self.expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl ConfirmProjectItsmConnection {
    /// Validates identifiers and the optimistic concurrency version.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if ![self.user_id, self.project_id, self.expected_connection_id]
            .into_iter()
            .all(is_v7)
            || self.expected_version <= 0
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl DeleteProjectItsmConnection {
    /// Validates owner, project, connection generation, and version inputs.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if ![self.user_id, self.project_id, self.expected_connection_id]
            .into_iter()
            .all(is_v7)
            || self.expected_version <= 0
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl Database {
    /// Loads the ITSM opt-in for one owned project.
    ///
    /// Missing and foreign projects both return `None`, preventing ownership
    /// discovery through the connection endpoint.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the lookup cannot run.
    pub async fn project_itsm_connection(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<ProjectItsmConnection>, StorageError> {
        if ![user_id, project_id].into_iter().all(is_v7) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, ProjectItsmConnectionRow>(
            "SELECT connection.id, connection.project_id,
                connection.itsm_project_id,
                connection.candidate_itsm_project_id,
                connection.candidate_itsm_project_name,
                connection.enabled,
                connection.created_at, connection.updated_at, connection.version
             FROM project_itsm_connections AS connection
             JOIN projects AS project
               ON project.id = connection.project_id
              AND project.user_id = connection.user_id
             WHERE connection.user_id = $1 AND connection.project_id = $2",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(self.pool())
        .await
        .map(|row| row.map(ProjectItsmConnection::from))
        .map_err(classify)
    }

    /// Connects the deployment-managed ITSM client to one owned project.
    ///
    /// Returns `None` for a missing/foreign project or an existing connection,
    /// keeping both conditions opaque at the storage boundary. Current
    /// unhandled ready/failed inflow is requeued so the new connection is
    /// immediately useful without revisiting already handled conversations.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when creation cannot commit.
    pub async fn create_project_itsm_connection(
        &self,
        command: &NewProjectItsmConnection,
    ) -> Result<Option<ProjectItsmConnection>, StorageError> {
        command.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ProjectItsmConnectionRow>(
            "INSERT INTO project_itsm_connections (
                id, user_id, project_id, enabled
             )
             SELECT $1, $2, project.id, $4
             FROM projects AS project
             WHERE project.id = $3 AND project.user_id = $2
             ON CONFLICT (project_id) DO NOTHING
             RETURNING id, project_id, itsm_project_id,
                candidate_itsm_project_id, candidate_itsm_project_name, enabled,
                created_at, updated_at, version",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.enabled)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        let connection = ProjectItsmConnection::from(row);
        append_change(
            &mut transaction,
            command.user_id,
            "project_itsm_connection",
            connection.id,
            connection.version,
        )
        .await?;
        if connection.enabled {
            requeue_unhandled_analyses(&mut transaction, command.user_id, command.project_id)
                .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(Some(connection))
    }

    /// Atomically stores a bounded upstream project candidate without
    /// confirming it as the enrichment boundary.
    ///
    /// The row is locked before the confirmed and candidate identifiers are
    /// inspected. Concurrent analyses may therefore repeat the same proposal,
    /// but cannot replace it or establish a different boundary.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when binding cannot run.
    pub async fn propose_project_itsm_candidate(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        detected_itsm_project_id: &str,
        detected_itsm_project_name: &str,
    ) -> Result<ProjectItsmCandidateOutcome, StorageError> {
        let detected_itsm_project_name = detected_itsm_project_name.trim();
        if ![user_id, project_id].into_iter().all(is_v7)
            || !valid_itsm_project_id(detected_itsm_project_id)
            || !valid_itsm_project_name(detected_itsm_project_name)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, Option<String>)>(
            "SELECT id, itsm_project_id, candidate_itsm_project_id,
                candidate_itsm_project_name
             FROM project_itsm_connections
             WHERE user_id = $1 AND project_id = $2 AND enabled = TRUE
             FOR UPDATE",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((connection_id, current_project_id, candidate_project_id, candidate_project_name)) =
            current
        else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ProjectItsmCandidateOutcome::ConnectionUnavailable);
        };
        if let Some(current_project_id) = current_project_id {
            transaction.rollback().await.map_err(classify)?;
            return Ok(if current_project_id == detected_itsm_project_id {
                ProjectItsmCandidateOutcome::Confirmed
            } else {
                ProjectItsmCandidateOutcome::ProjectMismatch
            });
        }
        if let Some(candidate_project_id) = candidate_project_id {
            transaction.rollback().await.map_err(classify)?;
            return Ok(
                if candidate_project_id == detected_itsm_project_id
                    && candidate_project_name.as_deref() == Some(detected_itsm_project_name)
                {
                    ProjectItsmCandidateOutcome::ConfirmationRequired
                } else {
                    ProjectItsmCandidateOutcome::CandidateMismatch
                },
            );
        }
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE project_itsm_connections
             SET candidate_itsm_project_id = $1,
                 candidate_itsm_project_name = $2
             WHERE id = $3 AND user_id = $4 AND project_id = $5
               AND enabled = TRUE AND itsm_project_id IS NULL
               AND candidate_itsm_project_id IS NULL
               AND candidate_itsm_project_name IS NULL
             RETURNING version",
        )
        .bind(detected_itsm_project_id)
        .bind(detected_itsm_project_name)
        .bind(connection_id)
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(version) = version else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ProjectItsmCandidateOutcome::ConnectionUnavailable);
        };
        append_change(
            &mut transaction,
            user_id,
            "project_itsm_connection",
            connection_id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(ProjectItsmCandidateOutcome::ConfirmationRequired)
    }

    /// Confirms the currently detected candidate as the enforced upstream
    /// project boundary for one owned, enabled connection.
    ///
    /// The expected version prevents a stale owner action from confirming a
    /// candidate that changed after it was displayed.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when confirmation cannot run.
    pub async fn confirm_project_itsm_connection(
        &self,
        command: &ConfirmProjectItsmConnection,
    ) -> Result<ConfirmProjectItsmConnectionOutcome, StorageError> {
        command.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current = sqlx::query_as::<_, ProjectItsmConnectionRow>(
            "SELECT id, project_id, itsm_project_id,
                candidate_itsm_project_id, candidate_itsm_project_name,
                enabled, created_at, updated_at, version
             FROM project_itsm_connections
             WHERE user_id = $1 AND project_id = $2
             FOR UPDATE",
        )
        .bind(command.user_id)
        .bind(command.project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(current) = current else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::ConnectionUnavailable);
        };
        if !current.enabled {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::ConnectionUnavailable);
        }
        if current.id != command.expected_connection_id {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::VersionConflict);
        }
        if current.version != command.expected_version {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::VersionConflict);
        }
        if current.itsm_project_id.is_some() {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::Confirmed(
                ProjectItsmConnection::from(current),
            ));
        }
        let Some(candidate_project_id) = current.candidate_itsm_project_id.as_deref() else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::CandidateMissing);
        };
        let row = sqlx::query_as::<_, ProjectItsmConnectionRow>(
            "UPDATE project_itsm_connections
             SET itsm_project_id = $1,
                 candidate_itsm_project_id = NULL,
                 candidate_itsm_project_name = NULL
             WHERE id = $2 AND user_id = $3 AND project_id = $4
               AND enabled = TRUE AND version = $5
               AND itsm_project_id IS NULL
               AND candidate_itsm_project_id = $1
             RETURNING id, project_id, itsm_project_id,
                candidate_itsm_project_id, candidate_itsm_project_name,
                enabled, created_at, updated_at, version",
        )
        .bind(candidate_project_id)
        .bind(command.expected_connection_id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(ConfirmProjectItsmConnectionOutcome::VersionConflict);
        };
        let connection = ProjectItsmConnection::from(row);
        append_change(
            &mut transaction,
            command.user_id,
            "project_itsm_connection",
            connection.id,
            connection.version,
        )
        .await?;
        requeue_unhandled_analyses(&mut transaction, command.user_id, command.project_id).await?;
        transaction.commit().await.map_err(classify)?;
        Ok(ConfirmProjectItsmConnectionOutcome::Confirmed(connection))
    }

    /// Requeues one redacted analysis when its ITSM candidate became confirmed
    /// while the worker was still running.
    ///
    /// Confirmation performs the complementary bulk requeue for rows already
    /// in `ready` or `failed`. Calling this method after completion closes the
    /// opposite ordering without changing claimed/running lease ownership.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the transition cannot run.
    pub async fn requeue_inflow_analysis_after_itsm_confirmation(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        analysis_id: Uuid,
    ) -> Result<bool, StorageError> {
        if ![user_id, project_id, analysis_id].into_iter().all(is_v7) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE project_inflow_analyses AS analysis
             SET state = 'queued', attempt_count = 0, error_code = NULL
             WHERE analysis.id = $3
               AND analysis.user_id = $1
               AND analysis.project_id = $2
               AND analysis.state IN ('ready', 'failed')
               AND analysis.linked_task_id IS NULL
               AND EXISTS (
                    SELECT 1
                    FROM project_itsm_connections AS connection
                    WHERE connection.user_id = $1
                      AND connection.project_id = $2
                      AND connection.enabled = TRUE
                      AND connection.itsm_project_id IS NOT NULL
               )
               AND EXISTS (
                    SELECT 1
                    FROM project_inflow_items AS item
                    WHERE item.user_id = analysis.user_id
                      AND item.project_id = analysis.project_id
                      AND item.source_id = analysis.source_id
                      AND item.status = 'pending'
                      AND (
                        (analysis.conversation_key LIKE 'thread:%'
                            AND item.provider_thread_name =
                                substr(analysis.conversation_key, 8))
                        OR
                        (analysis.conversation_key LIKE 'message:%'
                            AND item.provider_message_name =
                                substr(analysis.conversation_key, 9))
                      )
               )
             RETURNING analysis.version",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(analysis_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(version) = version else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        append_change(
            &mut transaction,
            user_id,
            "project_inflow_analysis",
            analysis_id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Updates the enabled state of one version-matched owned connection.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the update cannot run.
    pub async fn update_project_itsm_connection(
        &self,
        update: &ProjectItsmConnectionUpdate,
    ) -> Result<Option<ProjectItsmConnection>, StorageError> {
        update.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ProjectItsmConnectionRow>(
            "UPDATE project_itsm_connections
             SET enabled = $1
             WHERE user_id = $2 AND project_id = $3 AND version = $4
             RETURNING id, project_id, itsm_project_id,
                candidate_itsm_project_id, candidate_itsm_project_name, enabled,
                created_at, updated_at, version",
        )
        .bind(update.enabled)
        .bind(update.user_id)
        .bind(update.project_id)
        .bind(update.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        let connection = ProjectItsmConnection::from(row);
        append_change(
            &mut transaction,
            update.user_id,
            "project_itsm_connection",
            connection.id,
            connection.version,
        )
        .await?;
        if connection.enabled {
            requeue_unhandled_analyses(&mut transaction, update.user_id, update.project_id).await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(Some(connection))
    }

    /// Deletes one version-matched project connection.
    ///
    /// Missing and foreign rows share [`DeleteProjectItsmConnectionOutcome::AlreadyAbsent`].
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when deletion cannot commit.
    pub async fn delete_project_itsm_connection(
        &self,
        command: &DeleteProjectItsmConnection,
    ) -> Result<DeleteProjectItsmConnectionOutcome, StorageError> {
        command.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current = sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT id, version
             FROM project_itsm_connections
             WHERE user_id = $1 AND project_id = $2
             FOR UPDATE",
        )
        .bind(command.user_id)
        .bind(command.project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((connection_id, current_version)) = current else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectItsmConnectionOutcome::AlreadyAbsent);
        };
        if connection_id != command.expected_connection_id
            || current_version != command.expected_version
        {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectItsmConnectionOutcome::VersionConflict);
        }
        let deleted = sqlx::query_scalar::<_, i64>(
            "DELETE FROM project_itsm_connections
             WHERE id = $1 AND user_id = $2 AND project_id = $3 AND version = $4
             RETURNING version",
        )
        .bind(command.expected_connection_id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(deleted_version) = deleted else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectItsmConnectionOutcome::VersionConflict);
        };
        append_delete_change(
            &mut transaction,
            command.user_id,
            "project_itsm_connection",
            connection_id,
            deleted_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(DeleteProjectItsmConnectionOutcome::Deleted)
    }
}

async fn requeue_unhandled_analyses(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<(), StorageError> {
    let rows = sqlx::query_as::<_, (Uuid, i64)>(
        "UPDATE project_inflow_analyses AS analysis
         SET state = 'queued', attempt_count = 0, error_code = NULL
         WHERE analysis.user_id = $1 AND analysis.project_id = $2
           AND analysis.state IN ('ready', 'failed')
           AND analysis.linked_task_id IS NULL
           AND EXISTS (
                SELECT 1
                FROM project_inflow_items AS item
                WHERE item.user_id = analysis.user_id
                  AND item.project_id = analysis.project_id
                  AND item.source_id = analysis.source_id
                  AND item.status = 'pending'
                  AND (
                    (analysis.conversation_key LIKE 'thread:%'
                        AND item.provider_thread_name =
                            substr(analysis.conversation_key, 8))
                    OR
                    (analysis.conversation_key LIKE 'message:%'
                        AND item.provider_message_name =
                            substr(analysis.conversation_key, 9))
                  )
           )
         RETURNING analysis.id, analysis.version",
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)?;
    for (analysis_id, version) in rows {
        append_change(
            transaction,
            user_id,
            "project_inflow_analysis",
            analysis_id,
            version,
        )
        .await?;
    }
    Ok(())
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_itsm_project_id(value: &str) -> bool {
    (1..=20).contains(&value.len())
        && value.as_bytes()[0].is_ascii_digit()
        && value.as_bytes()[0] != b'0'
        && value.bytes().all(|candidate| candidate.is_ascii_digit())
}

fn valid_itsm_project_name(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed == value
        && value.chars().count() <= 160
        && !value.chars().any(char::is_control)
}

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{
        ConfirmProjectItsmConnection, DeleteProjectItsmConnection, NewProjectItsmConnection,
        ProjectItsmConnectionUpdate, valid_itsm_project_id, valid_itsm_project_name,
    };
    use uuid::Uuid;

    #[test]
    fn connection_commands_require_v7_identifiers_and_positive_versions() {
        let user_id = Uuid::now_v7();
        let project_id = Uuid::now_v7();
        assert!(
            NewProjectItsmConnection {
                id: Uuid::now_v7(),
                user_id,
                project_id,
                enabled: true,
            }
            .validate()
            .is_ok()
        );
        assert!(
            NewProjectItsmConnection {
                id: Uuid::nil(),
                user_id,
                project_id,
                enabled: true,
            }
            .validate()
            .is_err()
        );
        assert!(
            ProjectItsmConnectionUpdate {
                user_id,
                project_id,
                enabled: false,
                expected_version: 1,
            }
            .validate()
            .is_ok()
        );
        assert!(
            ProjectItsmConnectionUpdate {
                user_id,
                project_id,
                enabled: false,
                expected_version: 0,
            }
            .validate()
            .is_err()
        );
        assert!(
            ConfirmProjectItsmConnection {
                user_id,
                project_id,
                expected_connection_id: Uuid::now_v7(),
                expected_version: 1,
            }
            .validate()
            .is_ok()
        );
        assert!(
            ConfirmProjectItsmConnection {
                user_id,
                project_id,
                expected_connection_id: Uuid::now_v7(),
                expected_version: 0,
            }
            .validate()
            .is_err()
        );
        assert!(
            DeleteProjectItsmConnection {
                user_id,
                project_id,
                expected_connection_id: Uuid::now_v7(),
                expected_version: 1,
            }
            .validate()
            .is_ok()
        );
        assert!(
            DeleteProjectItsmConnection {
                user_id,
                project_id,
                expected_connection_id: Uuid::nil(),
                expected_version: 1,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn detected_project_identifiers_are_canonical_positive_decimals() {
        assert!(valid_itsm_project_id("1"));
        assert!(valid_itsm_project_id("18446744073709551615"));
        for invalid in ["", "0", "01", "-1", "42\n", "184467440737095516150"] {
            assert!(!valid_itsm_project_id(invalid));
        }
    }

    #[test]
    fn detected_project_names_are_trimmed_bounded_and_control_free() {
        assert!(valid_itsm_project_name("비스킷링크"));
        assert!(valid_itsm_project_name(&"가".repeat(160)));
        for invalid in ["", " ", " 비스킷링크", "비스킷링크 ", "비스킷\n링크"] {
            assert!(!valid_itsm_project_name(invalid));
        }
        assert!(!valid_itsm_project_name(&"가".repeat(161)));
    }
}

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

/// Safe project connection metadata exposed to authenticated clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectItsmConnection {
    pub id: Uuid,
    pub project_id: Uuid,
    pub itsm_project_id: String,
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
    pub itsm_project_id: String,
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

#[derive(sqlx::FromRow)]
struct ProjectItsmConnectionRow {
    id: Uuid,
    project_id: Uuid,
    itsm_project_id: String,
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
    /// Returns [`StorageError::InvalidConfiguration`] for non-v7 identifiers
    /// or a malformed ITSM parent project identifier.
    pub fn validate(&self) -> Result<(), StorageError> {
        if ![self.id, self.user_id, self.project_id]
            .into_iter()
            .all(is_v7)
            || !valid_itsm_project_id(&self.itsm_project_id)
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
                connection.itsm_project_id, connection.enabled,
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
                id, user_id, project_id, itsm_project_id, enabled
             )
             SELECT $1, $2, project.id, $4, $5
             FROM projects AS project
             WHERE project.id = $3 AND project.user_id = $2
             ON CONFLICT (project_id) DO NOTHING
             RETURNING id, project_id, itsm_project_id, enabled,
                created_at, updated_at, version",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(&command.itsm_project_id)
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
             RETURNING id, project_id, itsm_project_id, enabled,
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
        user_id: Uuid,
        project_id: Uuid,
        expected_version: i64,
    ) -> Result<DeleteProjectItsmConnectionOutcome, StorageError> {
        if ![user_id, project_id].into_iter().all(is_v7) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current = sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT id, version
             FROM project_itsm_connections
             WHERE user_id = $1 AND project_id = $2
             FOR UPDATE",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((connection_id, current_version)) = current else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectItsmConnectionOutcome::AlreadyAbsent);
        };
        if current_version != expected_version {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectItsmConnectionOutcome::VersionConflict);
        }
        let deleted = sqlx::query_scalar::<_, i64>(
            "DELETE FROM project_itsm_connections
             WHERE id = $1 AND user_id = $2 AND project_id = $3 AND version = $4
             RETURNING version",
        )
        .bind(connection_id)
        .bind(user_id)
        .bind(project_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(deleted_version) = deleted else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(DeleteProjectItsmConnectionOutcome::VersionConflict);
        };
        append_delete_change(
            &mut transaction,
            user_id,
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

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{NewProjectItsmConnection, ProjectItsmConnectionUpdate};
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
                itsm_project_id: "42".to_owned(),
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
                itsm_project_id: "42".to_owned(),
                enabled: true,
            }
            .validate()
            .is_err()
        );
        assert!(
            NewProjectItsmConnection {
                id: Uuid::now_v7(),
                user_id,
                project_id,
                itsm_project_id: "0".to_owned(),
                enabled: true,
            }
            .validate()
            .is_err()
        );
        assert!(
            NewProjectItsmConnection {
                id: Uuid::now_v7(),
                user_id,
                project_id,
                itsm_project_id: "123456789012345678901".to_owned(),
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
    }
}

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgPool, postgres::PgPoolOptions};
use thiserror::Error;

pub mod agent;
pub mod auth;
pub mod planning;

pub const EXPECTED_SCHEMA_VERSION: i64 = 6;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    Ready { schema_version: i64 },
    DatabaseUnavailable,
    SchemaUnavailable,
    SchemaMismatch { expected: i64, actual: i64 },
}

struct AppliedMigration {
    version: i64,
    success: bool,
    checksum: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database configuration is invalid")]
    InvalidConfiguration,
    #[error("database migration is unavailable")]
    MigrationUnavailable,
    #[error("database persistence is unavailable")]
    PersistenceUnavailable,
    #[error("stored identity conflicts with the current login")]
    IdentityConflict,
}

impl Database {
    /// Creates a `PostgreSQL` pool without opening a connection immediately.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the URL or pool
    /// bounds are invalid.
    pub fn connect_lazy(
        database_url: &SecretString,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self, StorageError> {
        if max_connections == 0 || acquire_timeout.is_zero() {
            return Err(StorageError::InvalidConfiguration);
        }

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
            .connect_lazy(database_url.expose_secret())
            .map_err(|_| StorageError::InvalidConfiguration)?;

        Ok(Self { pool })
    }

    /// Applies the embedded forward-only migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::MigrationUnavailable`] when `PostgreSQL` cannot
    /// be reached or a migration cannot be applied safely.
    pub async fn migrate(&self) -> Result<(), StorageError> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|_| StorageError::MigrationUnavailable)
    }

    pub async fn readiness(&self, expected_schema_version: i64) -> Readiness {
        if sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .is_err()
        {
            return Readiness::DatabaseUnavailable;
        }

        let application_version = sqlx::query_scalar::<_, i64>(
            "SELECT schema_version FROM jimin_schema_metadata WHERE singleton = TRUE",
        )
        .fetch_one(&self.pool)
        .await;

        let applied_migrations = sqlx::query_as::<_, (i64, bool, Vec<u8>)>(
            "SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(version, success, checksum)| AppliedMigration {
                    version,
                    success,
                    checksum,
                })
                .collect::<Vec<_>>()
        });

        let (Ok(application_version), Ok(applied_migrations)) =
            (application_version, applied_migrations)
        else {
            return Readiness::SchemaUnavailable;
        };

        if !migration_history_matches(
            expected_schema_version,
            application_version,
            &applied_migrations,
        ) {
            let migration_version = applied_migrations
                .iter()
                .rev()
                .find(|migration| migration.success)
                .map_or(0, |migration| migration.version);
            return Readiness::SchemaMismatch {
                expected: expected_schema_version,
                actual: application_version.min(migration_version),
            };
        }

        Readiness::Ready {
            schema_version: application_version,
        }
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub(crate) const fn pool(&self) -> &PgPool {
        &self.pool
    }
}

fn migration_history_matches(
    expected_schema_version: i64,
    application_version: i64,
    applied_migrations: &[AppliedMigration],
) -> bool {
    if application_version != expected_schema_version {
        return false;
    }

    let embedded_migrations: Vec<_> = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .collect();
    if embedded_migrations
        .last()
        .is_none_or(|migration| migration.version != expected_schema_version)
        || embedded_migrations.len() != applied_migrations.len()
    {
        return false;
    }

    embedded_migrations
        .into_iter()
        .zip(applied_migrations)
        .all(|(embedded, applied)| {
            applied.success
                && applied.version == embedded.version
                && applied.checksum.as_slice() == embedded.checksum.as_ref()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied_embedded_migrations() -> Vec<AppliedMigration> {
        MIGRATOR
            .iter()
            .filter(|migration| migration.migration_type.is_up_migration())
            .map(|migration| AppliedMigration {
                version: migration.version,
                success: true,
                checksum: migration.checksum.to_vec(),
            })
            .collect()
    }

    #[test]
    fn rejects_zero_pool_size() {
        let url = SecretString::from("postgres://example.invalid/database".to_owned());

        let result = Database::connect_lazy(&url, 0, Duration::from_secs(1));

        assert!(matches!(result, Err(StorageError::InvalidConfiguration)));
    }

    #[test]
    fn rejects_zero_acquire_timeout() {
        let url = SecretString::from("postgres://example.invalid/database".to_owned());

        let result = Database::connect_lazy(&url, 1, Duration::ZERO);

        assert!(matches!(result, Err(StorageError::InvalidConfiguration)));
    }

    #[test]
    fn rejects_applied_migration_with_modified_checksum() {
        let mut applied = applied_embedded_migrations();
        applied[0].checksum[0] ^= 0xff;

        assert!(!migration_history_matches(
            EXPECTED_SCHEMA_VERSION,
            EXPECTED_SCHEMA_VERSION,
            &applied,
        ));
    }

    #[test]
    fn accepts_only_the_complete_successful_embedded_history() {
        let applied = applied_embedded_migrations();
        assert!(migration_history_matches(
            EXPECTED_SCHEMA_VERSION,
            EXPECTED_SCHEMA_VERSION,
            &applied,
        ));

        let mut failed = applied_embedded_migrations();
        failed[0].success = false;
        assert!(!migration_history_matches(
            EXPECTED_SCHEMA_VERSION,
            EXPECTED_SCHEMA_VERSION,
            &failed,
        ));
    }
}

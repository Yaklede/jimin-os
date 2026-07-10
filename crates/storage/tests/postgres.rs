use std::time::Duration;

use jimin_storage::{Database, EXPECTED_SCHEMA_VERSION, Readiness};
use secrecy::SecretString;

#[tokio::test]
async fn baseline_migration_and_schema_version_are_consistent() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };

    let database_url = SecretString::from(database_url);
    let database = Database::connect_lazy(&database_url, 1, Duration::from_secs(2))
        .expect("test database URL should be valid");

    database
        .migrate()
        .await
        .expect("baseline migration should succeed");

    assert_eq!(
        database.readiness(EXPECTED_SCHEMA_VERSION).await,
        Readiness::Ready {
            schema_version: EXPECTED_SCHEMA_VERSION,
        }
    );
    assert!(matches!(
        database.readiness(EXPECTED_SCHEMA_VERSION + 1).await,
        Readiness::SchemaMismatch { .. }
    ));

    database.close().await;
}

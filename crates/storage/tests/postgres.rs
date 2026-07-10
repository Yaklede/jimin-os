use std::time::Duration;

use jimin_auth::SessionIdentity;
use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use jimin_storage::{
    Database, EXPECTED_SCHEMA_VERSION, Readiness,
    auth::{ProvisionLogin, RefreshRotation, RotateRefreshToken},
};
use secrecy::SecretString;
use time::{Duration as TimeDuration, OffsetDateTime};
use uuid::Uuid;

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

#[tokio::test]
async fn login_provision_is_atomic_and_the_session_guard_is_user_scoped() {
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

    let user_id = Uuid::now_v7();
    let installation_id = Uuid::now_v7();
    let first = provision_login_command(user_id, installation_id);
    let first_session = first.session_id;
    let provisioned = database
        .provision_login(&first)
        .await
        .expect("first login should provision user, device, and session");

    assert_eq!(provisioned.profile.id, user_id);
    assert_eq!(
        provisioned.device.status,
        jimin_storage::auth::DeviceStatus::Active
    );
    assert!(provisioned.sync_cursor >= 2);
    assert!(
        database
            .is_session_active(
                SessionIdentity::new(
                    user_id,
                    first_session,
                    provisioned.device.id,
                    Uuid::now_v7(),
                )
                .expect("guard identity should be valid"),
            )
            .await
            .expect("guard query should succeed")
    );

    let rotation = database
        .rotate_refresh_token(&RotateRefreshToken {
            session_id: first_session,
            presented_verifier: first.refresh_token_verifier.clone(),
            new_refresh_token_id: Uuid::now_v7(),
            new_refresh_token_verifier: vec![22; 32],
            new_refresh_token_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            request_id: Uuid::now_v7(),
        })
        .await
        .expect("active refresh token should rotate");
    assert!(matches!(rotation, RefreshRotation::Rotated(_)));

    let replay = database
        .rotate_refresh_token(&RotateRefreshToken {
            session_id: first_session,
            presented_verifier: first.refresh_token_verifier.clone(),
            new_refresh_token_id: Uuid::now_v7(),
            new_refresh_token_verifier: vec![33; 32],
            new_refresh_token_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            request_id: Uuid::now_v7(),
        })
        .await
        .expect("rotated token replay should be handled safely");
    assert_eq!(replay, RefreshRotation::Reused);
    assert!(
        !database
            .is_session_active(
                SessionIdentity::new(
                    user_id,
                    first_session,
                    provisioned.device.id,
                    Uuid::now_v7(),
                )
                .expect("guard identity should be valid"),
            )
            .await
            .expect("guard query should succeed")
    );

    let second = provision_login_command(user_id, installation_id);
    let reprovisioned = database
        .provision_login(&second)
        .await
        .expect("same owner/device should re-register safely");

    assert_eq!(reprovisioned.profile.id, provisioned.profile.id);
    assert_eq!(reprovisioned.device.id, provisioned.device.id);
    assert!(reprovisioned.profile.version > provisioned.profile.version);
    assert!(reprovisioned.device.version > provisioned.device.version);
    assert!(
        !database
            .is_session_active(
                SessionIdentity::new(
                    Uuid::now_v7(),
                    first_session,
                    provisioned.device.id,
                    Uuid::now_v7(),
                )
                .expect("guard identity should be valid"),
            )
            .await
            .expect("guard query should succeed")
    );

    database.close().await;
}

fn provision_login_command(user_id: Uuid, installation_id: Uuid) -> ProvisionLogin {
    let device = DeviceRegistration::new(
        installation_id,
        ClientPlatform::Macos,
        "M1 integration test Mac",
        "0.1.0-test",
        Some("test-os".to_owned()),
    )
    .expect("test device should be valid");
    let now = OffsetDateTime::now_utc();
    let session_id = Uuid::now_v7();
    let mut refresh_token_verifier = session_id.as_bytes().to_vec();
    refresh_token_verifier.extend_from_slice(session_id.as_bytes());
    ProvisionLogin {
        user_id,
        google_subject: GoogleSubject::parse(format!("integration-subject-{user_id}"))
            .expect("test Google subject should be valid"),
        email: EmailAddress::parse(format!("m1-{user_id}@example.test"))
            .expect("test email should be valid"),
        display_name: Some("M1 integration test owner".to_owned()),
        device,
        session_id,
        family_id: Uuid::now_v7(),
        refresh_token_id: Uuid::now_v7(),
        refresh_token_verifier,
        session_expires_at: now + TimeDuration::days(30),
        refresh_token_expires_at: now + TimeDuration::days(30),
        request_id: Uuid::now_v7(),
    }
}

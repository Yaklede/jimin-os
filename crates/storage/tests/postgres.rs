use std::time::Duration;

use jimin_auth::SessionIdentity;
use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use jimin_storage::{
    Database, EXPECTED_SCHEMA_VERSION, Readiness,
    agent::{AgentJobState, ConversationMessageRole, NewAgentTurn, NewConversation},
    auth::{
        ConsumeDevicePairing, CreateDevicePairing, PairingConsumption, ProvisionLogin,
        RefreshRotation, RotateRefreshToken,
    },
    planning::{NewScheduleEntry, NewTask, TaskStatus},
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
#[allow(
    clippy::too_many_lines,
    reason = "The integration test exercises one complete session lifecycle."
)]
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

    let mut expired_login = provision_login_command(Uuid::now_v7(), Uuid::now_v7());
    let expired_at = OffsetDateTime::now_utc() - TimeDuration::days(1);
    expired_login.session_expires_at = expired_at;
    expired_login.refresh_token_expires_at = expired_at;
    let expired_session_id = expired_login.session_id;
    let expired_refresh_verifier = expired_login.refresh_token_verifier.clone();
    database
        .provision_login(&expired_login)
        .await
        .expect("expired login fixture should persist for rejection coverage");
    let expired_refresh = database
        .rotate_refresh_token(&RotateRefreshToken {
            session_id: expired_session_id,
            presented_verifier: expired_refresh_verifier,
            new_refresh_token_id: Uuid::now_v7(),
            new_refresh_token_verifier: vec![44; 32],
            new_refresh_token_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            request_id: Uuid::now_v7(),
        })
        .await
        .expect("expired refresh token should be rejected safely");
    assert_eq!(expired_refresh, RefreshRotation::Rejected);

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

#[tokio::test]
async fn pairing_consumes_one_short_lived_token_into_one_device_session() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };

    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("pairing migration should succeed");

    let pairing_id = Uuid::now_v7();
    let pairing_verifier = vec![71; 32];
    let created = database
        .create_device_pairing(&CreateDevicePairing {
            owner_user_id: Uuid::now_v7(),
            pairing_id,
            token_verifier: pairing_verifier.clone(),
            expires_at: OffsetDateTime::now_utc() + TimeDuration::minutes(10),
        })
        .await
        .expect("trusted server should create pairing");
    assert_eq!(created.pairing_id, pairing_id);

    let session_id = Uuid::now_v7();
    let device = DeviceRegistration::new(
        Uuid::now_v7(),
        ClientPlatform::Android,
        "M1 integration test Android",
        "0.1.0-test",
        Some("test-os".to_owned()),
    )
    .expect("test device should be valid");
    let consumed = database
        .consume_device_pairing(&ConsumeDevicePairing {
            pairing_id,
            token_verifier: pairing_verifier.clone(),
            device,
            session_id,
            family_id: Uuid::now_v7(),
            refresh_token_id: Uuid::now_v7(),
            refresh_token_verifier: vec![72; 32],
            session_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            refresh_token_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            request_id: Uuid::now_v7(),
        })
        .await
        .expect("valid pairing should consume safely");
    let PairingConsumption::Consumed(session) = consumed else {
        panic!("valid token should issue a session");
    };
    assert_eq!(session.profile.email, None);
    assert_eq!(session.device.platform, ClientPlatform::Android);
    assert!(
        database
            .is_session_active(
                SessionIdentity::new(
                    session.profile.id,
                    session_id,
                    session.device.id,
                    Uuid::now_v7(),
                )
                .expect("guard identity should be valid"),
            )
            .await
            .expect("guard query should succeed")
    );

    let replay = database
        .consume_device_pairing(&ConsumeDevicePairing {
            pairing_id,
            token_verifier: pairing_verifier,
            device: DeviceRegistration::new(
                Uuid::now_v7(),
                ClientPlatform::Android,
                "M1 replay Android",
                "0.1.0-test",
                None,
            )
            .expect("test device should be valid"),
            session_id: Uuid::now_v7(),
            family_id: Uuid::now_v7(),
            refresh_token_id: Uuid::now_v7(),
            refresh_token_verifier: vec![73; 32],
            session_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            refresh_token_expires_at: OffsetDateTime::now_utc() + TimeDuration::days(30),
            request_id: Uuid::now_v7(),
        })
        .await
        .expect("consumed token should reject without leaking state");
    assert_eq!(replay, PairingConsumption::Rejected);

    database.close().await;
}

#[tokio::test]
async fn manual_schedule_and_tasks_are_scoped_and_emit_current_state() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("planning migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let now = OffsetDateTime::now_utc();
    let schedule = database
        .create_schedule_entry(&NewScheduleEntry {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            title: "개인 일정".to_owned(),
            notes: Some("직접 등록한 일정".to_owned()),
            starts_at: now + TimeDuration::hours(1),
            ends_at: now + TimeDuration::hours(2),
            time_zone: "Asia/Seoul".to_owned(),
        })
        .await
        .expect("manual schedule should persist");
    let listed = database
        .schedule_entries_in_range(provisioned.profile.id, now, now + TimeDuration::days(1))
        .await
        .expect("schedule query should succeed");
    assert_eq!(listed, vec![schedule]);

    let task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            title: "오늘 할 일".to_owned(),
            notes: None,
            priority: 3,
            due_at: Some(now + TimeDuration::days(1)),
        })
        .await
        .expect("task should persist");
    assert_eq!(
        database
            .open_tasks_for_user(provisioned.profile.id)
            .await
            .expect("open task query should succeed"),
        vec![task.clone()]
    );
    let completed = database
        .complete_task(provisioned.profile.id, task.id, task.version)
        .await
        .expect("complete should succeed")
        .expect("open task should complete");
    assert_eq!(completed.status, TaskStatus::Completed);
    assert!(
        database
            .open_tasks_for_user(provisioned.profile.id)
            .await
            .expect("open task query should succeed")
            .is_empty()
    );
    assert!(
        database
            .complete_task(provisioned.profile.id, task.id, task.version)
            .await
            .expect("stale completion should not fail")
            .is_none()
    );
    database.close().await;
}

#[tokio::test]
async fn queued_agent_turn_is_leased_and_completed_once() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("agent migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("개인 운영체제".to_owned()),
        })
        .await
        .expect("conversation should persist");
    let queued = database
        .enqueue_agent_turn(&NewAgentTurn {
            job_id: Uuid::now_v7(),
            message_id: Uuid::now_v7(),
            client_message_id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            conversation_id,
            content: "오늘 일정을 정리해줘".to_owned(),
        })
        .await
        .expect("turn should queue");
    let runner_id = "integration-agent";
    let claim = database
        .claim_next_agent_job(runner_id, Duration::from_secs(30))
        .await
        .expect("claim query should succeed")
        .expect("queued job should be claimed");
    assert_eq!(claim.id, queued.job_id);
    assert_eq!(claim.input_content, "오늘 일정을 정리해줘");
    assert!(claim.codex_thread_id.is_none());
    assert!(
        database
            .start_agent_job(
                claim.id,
                runner_id,
                "thread-integration-1",
                Duration::from_secs(30),
            )
            .await
            .expect("job should start")
    );
    assert!(
        database
            .complete_agent_job(
                claim.id,
                runner_id,
                Uuid::now_v7(),
                "오늘 일정은 오후 3시에 하나 있어요.",
            )
            .await
            .expect("job should complete")
    );
    let messages = database
        .conversation_messages_for_user(provisioned.profile.id, conversation_id)
        .await
        .expect("message query should succeed")
        .expect("owner should read messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, ConversationMessageRole::User);
    assert_eq!(messages[1].role, ConversationMessageRole::Assistant);
    assert_eq!(messages[1].content, "오늘 일정은 오후 3시에 하나 있어요.");
    assert_eq!(
        database
            .agent_job_for_user(provisioned.profile.id, claim.id)
            .await
            .expect("job query should succeed")
            .expect("owner should read job")
            .state,
        AgentJobState::Completed
    );
    assert!(
        database
            .claim_next_agent_job(runner_id, Duration::from_secs(30))
            .await
            .expect("claim query should succeed")
            .is_none()
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

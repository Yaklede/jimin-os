use std::time::Duration;

use jimin_auth::SessionIdentity;
use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use jimin_storage::{
    Database, EXPECTED_SCHEMA_VERSION, Readiness,
    agent::{
        AgentActionCommand, AgentJobState, AgentModelCatalogEntry, AgentReasoningEffort,
        AssistantPresentation, AssistantPresentationKind, AssistantPresentationLayout,
        ConversationMessageRole, NewAgentTurn, NewConversation, PendingAgentAction,
        PendingAgentActionDecision,
    },
    auth::{
        ConsumeDevicePairing, CreateDevicePairing, PairingConsumption, ProvisionLogin,
        RefreshRotation, RotateRefreshToken,
    },
    planning::{NewScheduleEntry, NewTask, ScheduleEntryUpdate, TaskStatus, TaskUpdate},
    work::{NewProject, ProjectStatus, ProjectUpdate, WorkspaceScope},
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
async fn agent_model_catalog_and_user_selection_round_trip() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("model preference migration should succeed");

    let user_id = Uuid::now_v7();
    database
        .provision_login(&provision_login_command(user_id, Uuid::now_v7()))
        .await
        .expect("model preference owner should exist");
    let models = vec![
        AgentModelCatalogEntry {
            id: "provider-default".to_owned(),
            display_name: "Provider Default".to_owned(),
            description: "Default fixture model".to_owned(),
            is_default: true,
            default_reasoning_effort: "medium".to_owned(),
            supported_reasoning_efforts: vec![
                AgentReasoningEffort {
                    id: "low".to_owned(),
                    description: "Fast".to_owned(),
                },
                AgentReasoningEffort {
                    id: "medium".to_owned(),
                    description: "Balanced".to_owned(),
                },
            ],
        },
        AgentModelCatalogEntry {
            id: "provider-fast".to_owned(),
            display_name: "Provider Fast".to_owned(),
            description: "Fast fixture model".to_owned(),
            is_default: false,
            default_reasoning_effort: "low".to_owned(),
            supported_reasoning_efforts: vec![AgentReasoningEffort {
                id: "low".to_owned(),
                description: "Fast".to_owned(),
            }],
        },
    ];
    database
        .replace_agent_model_catalog(&models)
        .await
        .expect("runtime model catalog should persist");

    let initial = database
        .agent_model_settings_for_user(user_id)
        .await
        .expect("model settings should load");
    assert_eq!(initial.models, models);
    assert_eq!(initial.selected_model_id, None);
    assert_eq!(initial.selected_reasoning_effort, None);

    let selected = database
        .set_agent_model_for_user(user_id, Some("provider-fast"), Some("low"))
        .await
        .expect("available model should be selectable");
    assert_eq!(selected.selected_model_id.as_deref(), Some("provider-fast"));
    assert_eq!(selected.selected_reasoning_effort.as_deref(), Some("low"));

    let automatic = database
        .set_agent_model_for_user(user_id, None, Some("medium"))
        .await
        .expect("runtime default should be restorable");
    assert_eq!(automatic.selected_model_id, None);
    assert_eq!(
        automatic.selected_reasoning_effort.as_deref(),
        Some("medium")
    );
    let defaults = database
        .set_agent_model_for_user(user_id, None, None)
        .await
        .expect("automatic reasoning should be restorable");
    assert_eq!(defaults.selected_reasoning_effort, None);
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
async fn manual_schedules_are_scoped_and_version_checked() {
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
    assert_eq!(listed, vec![schedule.clone()]);
    let updated_schedule = database
        .update_schedule_entry(&ScheduleEntryUpdate {
            id: schedule.id,
            user_id: provisioned.profile.id,
            title: "수정한 개인 일정".to_owned(),
            notes: Some("시간을 직접 변경함".to_owned()),
            starts_at: now + TimeDuration::hours(2),
            ends_at: now + TimeDuration::hours(3),
            time_zone: "Asia/Seoul".to_owned(),
            expected_version: schedule.version,
        })
        .await
        .expect("manual schedule update should succeed")
        .expect("current manual schedule should update");
    assert_eq!(updated_schedule.title, "수정한 개인 일정");
    assert!(
        database
            .update_schedule_entry(&ScheduleEntryUpdate {
                id: schedule.id,
                user_id: provisioned.profile.id,
                title: "오래된 일정 수정".to_owned(),
                notes: None,
                starts_at: now + TimeDuration::hours(3),
                ends_at: now + TimeDuration::hours(4),
                time_zone: "Asia/Seoul".to_owned(),
                expected_version: schedule.version,
            })
            .await
            .expect("stale schedule update should not fail")
            .is_none()
    );
    database.close().await;
}

#[tokio::test]
async fn tasks_are_scoped_and_emit_current_state() {
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

    let task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            project_id: None,
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
    assert!(
        database
            .home_tasks_for_user(provisioned.profile.id, now + TimeDuration::hours(12))
            .await
            .expect("daily home task query should succeed")
            .is_empty()
    );
    assert_eq!(
        database
            .home_tasks_for_user(provisioned.profile.id, now + TimeDuration::days(2))
            .await
            .expect("later home task query should succeed"),
        vec![task.clone()]
    );
    assert!(
        database
            .deadline_tasks_for_user(provisioned.profile.id, now + TimeDuration::hours(12))
            .await
            .expect("early deadline query should succeed")
            .is_empty()
    );
    assert_eq!(
        database
            .deadline_tasks_for_user(provisioned.profile.id, now + TimeDuration::days(2))
            .await
            .expect("deadline attention query should succeed"),
        vec![task.clone()]
    );
    let completed = database
        .complete_task(provisioned.profile.id, task.id, task.version)
        .await
        .expect("complete should succeed")
        .expect("open task should complete");
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(
        database
            .completed_tasks_for_user(provisioned.profile.id)
            .await
            .expect("completed task query should succeed"),
        vec![completed.clone()]
    );
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
async fn task_update_reopens_soft_deletes_and_rejects_stale_versions() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            project_id: None,
            title: "다시 열 일".to_owned(),
            notes: None,
            priority: 1,
            due_at: None,
        })
        .await
        .expect("task should persist");
    let completed = database
        .complete_task(provisioned.profile.id, task.id, task.version)
        .await
        .expect("complete should succeed")
        .expect("open task should complete");
    let reopened = database
        .update_task(&TaskUpdate {
            id: completed.id,
            user_id: provisioned.profile.id,
            project_id: None,
            title: "다시 연 일".to_owned(),
            notes: Some("수정 내용".to_owned()),
            status: TaskStatus::Open,
            priority: 2,
            due_at: None,
            expected_version: completed.version,
        })
        .await
        .expect("reopen should succeed")
        .expect("current task should update");
    assert_eq!(reopened.status, TaskStatus::Open);
    assert!(reopened.completed_at.is_none());
    assert!(
        database
            .completed_tasks_for_user(provisioned.profile.id)
            .await
            .expect("reopened task should leave completion history")
            .is_empty()
    );
    assert!(
        database
            .update_task(&TaskUpdate {
                id: completed.id,
                user_id: provisioned.profile.id,
                project_id: None,
                title: "오래된 수정".to_owned(),
                notes: None,
                status: TaskStatus::Cancelled,
                priority: 0,
                due_at: None,
                expected_version: completed.version,
            })
            .await
            .expect("stale update should not fail")
            .is_none()
    );
    let cancelled = database
        .update_task(&TaskUpdate {
            id: reopened.id,
            user_id: provisioned.profile.id,
            project_id: None,
            title: reopened.title.clone(),
            notes: reopened.notes.clone(),
            status: TaskStatus::Cancelled,
            priority: reopened.priority,
            due_at: reopened.due_at,
            expected_version: reopened.version,
        })
        .await
        .expect("soft delete should succeed")
        .expect("current task should cancel");
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    database.close().await;
}

#[tokio::test]
async fn project_update_is_scoped_versioned_and_emits_current_state() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("work migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let personal = database
        .workspaces_for_user(provisioned.profile.id)
        .await
        .expect("workspace query should succeed")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let created = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            workspace_id: personal.id,
            title: "개인 운영체제".to_owned(),
            objective: Some("업무 맥락 정리".to_owned()),
            risk_level: 0,
            next_action: Some("프로젝트 수정 기능 만들기".to_owned()),
            due_at: None,
        })
        .await
        .expect("project should persist");
    let due_at = OffsetDateTime::now_utc() + TimeDuration::days(7);
    let updated = database
        .update_project(&ProjectUpdate {
            id: created.id,
            user_id: provisioned.profile.id,
            title: "개인 AI 비서".to_owned(),
            objective: Some("업무 판단과 실행 연결".to_owned()),
            status: ProjectStatus::Paused,
            risk_level: 2,
            next_action: Some("Webhook 계약 확정".to_owned()),
            due_at: Some(due_at),
            expected_version: created.version,
        })
        .await
        .expect("project update should succeed")
        .expect("matching project should update");
    assert_eq!(updated.title, "개인 AI 비서");
    assert_eq!(updated.status, ProjectStatus::Paused);
    assert_eq!(updated.risk_level, 2);
    assert_eq!(updated.due_at, Some(due_at));
    assert!(updated.version > created.version);
    assert!(
        database
            .update_project(&ProjectUpdate {
                id: created.id,
                user_id: provisioned.profile.id,
                title: "오래된 수정".to_owned(),
                objective: None,
                status: ProjectStatus::Active,
                risk_level: 0,
                next_action: None,
                due_at: None,
                expected_version: created.version,
            })
            .await
            .expect("stale project update should not fail")
            .is_none()
    );
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies one durable agent turn lifecycle and its replay path."
)]
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
    let client_message_id = Uuid::now_v7();
    let created_conversation = database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("개인 운영체제".to_owned()),
        })
        .await
        .expect("conversation should persist");
    let replayed_conversation = database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("개인 운영체제".to_owned()),
        })
        .await
        .expect("same client conversation should be replayed");
    assert_eq!(replayed_conversation.id, created_conversation.id);
    let queued = database
        .enqueue_agent_turn(&NewAgentTurn {
            job_id: Uuid::now_v7(),
            message_id: Uuid::now_v7(),
            client_message_id,
            user_id: provisioned.profile.id,
            conversation_id,
            content: "오늘 일정을 정리해줘".to_owned(),
        })
        .await
        .expect("turn should queue");
    let replayed = database
        .enqueue_agent_turn(&NewAgentTurn {
            job_id: Uuid::now_v7(),
            message_id: Uuid::now_v7(),
            client_message_id,
            user_id: provisioned.profile.id,
            conversation_id,
            content: "오늘 일정을 정리해줘".to_owned(),
        })
        .await
        .expect("same client turn should be replayed");
    assert_eq!(replayed.job_id, queued.job_id);
    assert_eq!(replayed.message_id, queued.message_id);
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
    let assistant_message_id = Uuid::now_v7();
    assert!(
        database
            .append_agent_response_delta(claim.id, runner_id, assistant_message_id, "오늘 일정은 ",)
            .await
            .expect("first response delta should persist")
    );
    let streaming_messages = database
        .conversation_messages_for_user(provisioned.profile.id, conversation_id)
        .await
        .expect("streaming messages should load")
        .expect("owner should read messages");
    assert_eq!(streaming_messages.len(), 2);
    assert_eq!(
        streaming_messages[1].role,
        ConversationMessageRole::Assistant
    );
    assert_eq!(
        streaming_messages[1].status,
        jimin_storage::agent::ConversationMessageStatus::Streaming
    );
    assert_eq!(streaming_messages[1].content, "오늘 일정은 ");
    assert!(
        database
            .complete_agent_job(
                claim.id,
                runner_id,
                assistant_message_id,
                "오늘 일정은 오후 3시에 하나 있어요.",
                Some(&AssistantPresentation {
                    kind: AssistantPresentationKind::Summary,
                    title: "오늘 일정".to_owned(),
                    items: Vec::new(),
                    layout: AssistantPresentationLayout::default(),
                    sections: Vec::new(),
                    focus_item_id: None,
                }),
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
        messages[1]
            .presentation
            .as_ref()
            .map(|presentation| presentation.title.as_str()),
        Some("오늘 일정")
    );
    assert_eq!(
        database
            .agent_job_for_user(provisioned.profile.id, claim.id)
            .await
            .expect("job query should succeed")
            .expect("owner should read job")
            .state,
        AgentJobState::Completed
    );
    let latest = database
        .latest_agent_job_for_conversation_for_user(provisioned.profile.id, conversation_id)
        .await
        .expect("latest job query should succeed")
        .expect("owner should read the latest conversation job");
    assert_eq!(latest.id, queued.job_id);
    assert_eq!(latest.state, AgentJobState::Completed);
    assert!(
        database
            .claim_next_agent_job(runner_id, Duration::from_secs(30))
            .await
            .expect("claim query should succeed")
            .is_none()
    );

    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies one approval lifecycle and its idempotency."
)]
async fn approved_conversation_action_creates_one_task_and_finalizes_the_job() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("action approval migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("할 일 추가".to_owned()),
        })
        .await
        .expect("conversation should persist");
    let due_at = (OffsetDateTime::now_utc() + TimeDuration::days(1))
        .replace_nanosecond(0)
        .expect("whole-second due date");
    let queued = database
        .enqueue_agent_action_turn(
            &NewAgentTurn {
                job_id: Uuid::now_v7(),
                message_id: Uuid::now_v7(),
                client_message_id: Uuid::now_v7(),
                user_id: provisioned.profile.id,
                conversation_id,
                content: "내일 할 일에 장보기 추가해 줘".to_owned(),
            },
            PendingAgentAction::CreateTask {
                title: "장보기".to_owned(),
                due_at: Some(due_at),
            },
        )
        .await
        .expect("action should wait for approval");
    assert_eq!(queued.state, AgentJobState::WaitingApproval);
    assert!(
        database
            .claim_next_agent_job("integration-agent", Duration::from_secs(30))
            .await
            .expect("waiting approval must not be claimed")
            .is_none()
    );
    let pending = database
        .agent_job_for_user(provisioned.profile.id, queued.job_id)
        .await
        .expect("job should load")
        .expect("owner should read job");
    assert!(matches!(
        pending.pending_action,
        Some(PendingAgentAction::CreateTask { ref title, due_at: Some(stored_due_at) })
            if title == "장보기" && stored_due_at == due_at
    ));
    assert!(
        database
            .resolve_agent_action(
                provisioned.profile.id,
                queued.job_id,
                PendingAgentActionDecision::Approve,
            )
            .await
            .expect("approval should resolve")
    );
    assert!(
        !database
            .resolve_agent_action(
                provisioned.profile.id,
                queued.job_id,
                PendingAgentActionDecision::Approve,
            )
            .await
            .expect("repeat approval should be safe")
    );
    let tasks = database
        .open_tasks_for_user(provisioned.profile.id)
        .await
        .expect("task should be visible");
    assert!(
        tasks
            .iter()
            .any(|task| task.title == "장보기" && task.due_at == Some(due_at))
    );
    let completed = database
        .agent_job_for_user(provisioned.profile.id, queued.job_id)
        .await
        .expect("job should load")
        .expect("owner should read completed job");
    assert_eq!(completed.state, AgentJobState::Completed);
    assert!(completed.pending_action.is_none());
    let messages = database
        .conversation_messages_for_user(provisioned.profile.id, conversation_id)
        .await
        .expect("messages should load")
        .expect("owner should read messages");
    assert!(
        messages
            .iter()
            .any(|message| message.content == "장보기 할 일을 추가했어요.")
    );

    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies one complete structured action transaction and its audit trail."
)]
async fn structured_agent_action_and_completion_message_commit_together() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        1,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("agent action execution migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("AI 실행".to_owned()),
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
            content: "내일 할 일에 일어나기를 추가해 줘".to_owned(),
        })
        .await
        .expect("turn should queue");
    let runner_id = "structured-action-agent";
    let claim = database
        .claim_next_agent_job(runner_id, Duration::from_secs(30))
        .await
        .expect("claim query should succeed")
        .expect("queued job should be claimed");
    assert!(
        database
            .start_agent_job(
                claim.id,
                runner_id,
                "thread-structured-action",
                Duration::from_secs(30),
            )
            .await
            .expect("job should start")
    );
    let task_id = Uuid::now_v7();
    let due_at = (OffsetDateTime::now_utc() + TimeDuration::days(1))
        .replace_nanosecond(0)
        .expect("whole-second due date");
    assert!(
        database
            .complete_agent_job_with_action(
                claim.id,
                runner_id,
                Uuid::now_v7(),
                "일어나기 할 일을 추가했어요.",
                Some(&AssistantPresentation {
                    kind: AssistantPresentationKind::Summary,
                    title: "할 일을 추가했어요".to_owned(),
                    items: Vec::new(),
                    layout: AssistantPresentationLayout::Stack,
                    sections: Vec::new(),
                    focus_item_id: None,
                }),
                &AgentActionCommand::CreateTask {
                    id: task_id,
                    project_id: None,
                    title: "일어나기".to_owned(),
                    notes: None,
                    priority: 1,
                    due_at: Some(due_at),
                },
            )
            .await
            .expect("action and result should commit")
    );

    let tasks = database
        .open_tasks_for_user(provisioned.profile.id)
        .await
        .expect("created task should load");
    assert!(tasks.iter().any(|task| {
        task.id == task_id && task.title == "일어나기" && task.due_at == Some(due_at)
    }));
    let messages = database
        .conversation_messages_for_user(provisioned.profile.id, conversation_id)
        .await
        .expect("messages should load")
        .expect("owner should read messages");
    assert_eq!(
        messages.last().map(|message| message.content.as_str()),
        Some("일어나기 할 일을 추가했어요.")
    );

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let audit: (Option<String>, Option<Uuid>, bool) = sqlx::query_as(
        "SELECT executed_action_type, executed_entity_id, executed_at IS NOT NULL FROM agent_jobs WHERE id = $1",
    )
    .bind(queued.job_id)
    .fetch_one(&pool)
    .await
    .expect("action audit should load");
    assert_eq!(audit.0.as_deref(), Some("create_task"));
    assert_eq!(audit.1, Some(task_id));
    assert!(audit.2);

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies project deletion, task detachment, sync tombstone, and agent audit in one transaction."
)]
async fn structured_agent_project_delete_commits_a_sync_tombstone() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        1,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let personal = database
        .workspaces_for_user(provisioned.profile.id)
        .await
        .expect("workspaces should load")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let project = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            workspace_id: personal.id,
            title: "비스킷링크".to_owned(),
            objective: None,
            risk_level: 0,
            next_action: None,
            due_at: None,
        })
        .await
        .expect("project should persist");
    let task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            project_id: Some(project.id),
            title: "연결 일감".to_owned(),
            notes: None,
            priority: 1,
            due_at: None,
        })
        .await
        .expect("linked task should persist");
    let conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("프로젝트 제거".to_owned()),
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
            content: "개인 프로젝트에서 비스킷링크 프로젝트 제거해".to_owned(),
        })
        .await
        .expect("turn should queue");
    let runner_id = "project-delete-agent";
    let claim = database
        .claim_next_agent_job(runner_id, Duration::from_secs(30))
        .await
        .expect("claim should succeed")
        .expect("queued job should be claimed");
    assert!(
        database
            .start_agent_job(
                claim.id,
                runner_id,
                "thread-project-delete",
                Duration::from_secs(30),
            )
            .await
            .expect("job should start")
    );
    assert!(
        database
            .complete_agent_job_with_action(
                claim.id,
                runner_id,
                Uuid::now_v7(),
                "비스킷링크 프로젝트를 제거했어요.",
                Some(&AssistantPresentation {
                    kind: AssistantPresentationKind::Summary,
                    title: "프로젝트를 제거했어요".to_owned(),
                    items: Vec::new(),
                    layout: AssistantPresentationLayout::Stack,
                    sections: Vec::new(),
                    focus_item_id: None,
                }),
                &AgentActionCommand::DeleteProject {
                    id: project.id,
                    expected_version: project.version,
                },
            )
            .await
            .expect("deletion and result should commit")
    );
    assert!(
        database
            .projects_for_workspace(provisioned.profile.id, personal.id)
            .await
            .expect("projects should load")
            .is_empty()
    );

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let detached_project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM tasks WHERE id = $1")
            .bind(task.id)
            .fetch_one(&pool)
            .await
            .expect("detached task should load");
    assert_eq!(detached_project_id, None);
    let sync_operation: String = sqlx::query_scalar(
        "SELECT operation FROM sync_changes WHERE user_id = $1 AND entity_type = 'project' AND entity_id = $2 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(provisioned.profile.id)
    .bind(project.id)
    .fetch_one(&pool)
    .await
    .expect("project tombstone should load");
    assert_eq!(sync_operation, "delete");
    let audit: (Option<String>, Option<Uuid>) = sqlx::query_as(
        "SELECT executed_action_type, executed_entity_id FROM agent_jobs WHERE id = $1",
    )
    .bind(queued.job_id)
    .fetch_one(&pool)
    .await
    .expect("agent audit should load");
    assert_eq!(audit, (Some("delete_project".to_owned()), Some(project.id)));
    let ordered_audit: (String, Uuid) = sqlx::query_as(
        "SELECT action_type, entity_id FROM agent_job_action_executions WHERE job_id = $1 AND action_index = 0",
    )
    .bind(queued.job_id)
    .fetch_one(&pool)
    .await
    .expect("ordered agent audit should load");
    assert_eq!(ordered_audit, ("delete_project".to_owned(), project.id));

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies batch mutation atomicity and the ordered audit trail."
)]
async fn structured_agent_batch_actions_commit_as_one_turn() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        1,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database
        .migrate()
        .await
        .expect("batch action migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let first = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            project_id: None,
            title: "회의록 정리".to_owned(),
            notes: None,
            priority: 2,
            due_at: None,
        })
        .await
        .expect("first task should persist");
    let second = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            project_id: None,
            title: "배포 확인".to_owned(),
            notes: None,
            priority: 2,
            due_at: None,
        })
        .await
        .expect("second task should persist");
    let conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id: provisioned.profile.id,
            title: Some("여러 건 완료".to_owned()),
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
            content: "두 할 일을 모두 완료해 줘".to_owned(),
        })
        .await
        .expect("turn should queue");
    let runner_id = "structured-batch-agent";
    let claim = database
        .claim_next_agent_job(runner_id, Duration::from_secs(30))
        .await
        .expect("claim query should succeed")
        .expect("queued job should be claimed");
    assert!(
        database
            .start_agent_job(
                claim.id,
                runner_id,
                "thread-structured-batch",
                Duration::from_secs(30),
            )
            .await
            .expect("job should start")
    );
    let actions = vec![
        AgentActionCommand::SetTaskStatus {
            id: first.id,
            status: TaskStatus::Completed,
            expected_version: first.version,
        },
        AgentActionCommand::SetTaskStatus {
            id: second.id,
            status: TaskStatus::Completed,
            expected_version: second.version,
        },
    ];
    assert!(
        database
            .complete_agent_job_with_actions(
                claim.id,
                runner_id,
                Uuid::now_v7(),
                "할 일 2개를 완료했어요.",
                Some(&AssistantPresentation {
                    kind: AssistantPresentationKind::Summary,
                    title: "할 일 2개를 완료했어요".to_owned(),
                    items: Vec::new(),
                    layout: AssistantPresentationLayout::Stack,
                    sections: Vec::new(),
                    focus_item_id: None,
                }),
                &actions,
            )
            .await
            .expect("batch and result should commit")
    );
    assert!(
        database
            .open_tasks_for_user(provisioned.profile.id)
            .await
            .expect("tasks should load")
            .is_empty()
    );

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let audit: (i16, Option<String>, Option<Uuid>) = sqlx::query_as(
        "SELECT executed_action_count, executed_action_type, executed_entity_id FROM agent_jobs WHERE id = $1",
    )
    .bind(queued.job_id)
    .fetch_one(&pool)
    .await
    .expect("batch audit should load");
    assert_eq!(audit, (2, None, None));
    let action_audit: Vec<(i16, String, Uuid)> = sqlx::query_as(
        "SELECT action_index, action_type, entity_id FROM agent_job_action_executions WHERE job_id = $1 ORDER BY action_index",
    )
    .bind(queued.job_id)
    .fetch_all(&pool)
    .await
    .expect("ordered action audit should load");
    assert_eq!(
        action_audit,
        vec![
            (0, "complete_task".to_owned(), first.id),
            (1, "complete_task".to_owned(), second.id),
        ]
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies that an interrupted provider turn is finalized without replay."
)]
async fn expired_running_turn_is_failed_without_replaying_the_provider_call() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        1,
        Duration::from_secs(2),
    )
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
            title: Some("중단 복구".to_owned()),
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
            content: "중단된 요청을 다시 보내지 마".to_owned(),
        })
        .await
        .expect("turn should queue");
    let runner_id = "recovery-agent";
    let claim = database
        .claim_next_agent_job(runner_id, Duration::from_secs(30))
        .await
        .expect("claim query should succeed")
        .expect("queued job should be claimed");
    assert!(
        database
            .start_agent_job(
                claim.id,
                runner_id,
                "thread-recovery-1",
                Duration::from_secs(30),
            )
            .await
            .expect("job should start")
    );

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    sqlx::query(
        "UPDATE agent_jobs SET claim_expires_at = NOW() - INTERVAL '1 second' WHERE id = $1",
    )
    .bind(queued.job_id)
    .execute(&pool)
    .await
    .expect("lease should expire for recovery test");

    assert_eq!(
        database
            .fail_expired_running_agent_jobs("agent.recovery_required")
            .await
            .expect("interrupted job should be finalized"),
        1
    );
    assert_eq!(
        database
            .agent_job_for_user(provisioned.profile.id, queued.job_id)
            .await
            .expect("job query should succeed")
            .expect("owner should read job")
            .state,
        AgentJobState::Failed
    );
    let (error_code,): (Option<String>,) =
        sqlx::query_as("SELECT error_code FROM agent_jobs WHERE id = $1")
            .bind(queued.job_id)
            .fetch_one(&pool)
            .await
            .expect("recovery error code should persist");
    assert_eq!(error_code.as_deref(), Some("agent.recovery_required"));
    assert!(
        database
            .claim_next_agent_job(runner_id, Duration::from_secs(30))
            .await
            .expect("failed job must never be requeued")
            .is_none()
    );
    assert_eq!(
        database
            .fail_expired_running_agent_jobs("agent.recovery_required")
            .await
            .expect("recovery should be idempotent"),
        0
    );

    pool.close().await;
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

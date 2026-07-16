use std::{collections::HashSet, time::Duration};

use jimin_auth::SessionIdentity;
use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use jimin_storage::{
    Database, EXPECTED_SCHEMA_VERSION, Readiness, StorageError,
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
    calendar::{
        CalendarAccountStatus, CompleteCalendarOAuthAuthorization,
        CreateCalendarOAuthAuthorization, DisconnectCalendarAccountOutcome,
        EncryptedCalendarSecret, PrimaryCalendarMutationTarget, ProviderCalendar,
        ProviderCalendarVisibility,
    },
    calendar_mutation::{ScheduleCalendarMutationOperation, provider_event_id_for_schedule},
    intelligence::{
        DecideRecommendation, DecideRecommendationOutcome, NewRecommendation,
        RecommendationDecision, RecommendationStatus, SuggestedActionKind,
    },
    planning::{
        DeleteTaskOutcome, NewScheduleEntry, NewTask, ScheduleEntryUpdate, TaskStatus, TaskUpdate,
    },
    webhook::{
        EncryptedWebhookSecret, NewProjectWebhook, ProjectWebhookUpdate,
        RetryWebhookDeliveryOutcome, WebhookAuthenticationUpdate,
    },
    work::{DeleteProjectOutcome, NewProject, ProjectStatus, ProjectUpdate, WorkspaceScope},
};
use secrecy::SecretString;
use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct AutomaticWebhookDeliveryRow {
    id: Uuid,
    user_id: Uuid,
    project_id: Uuid,
    webhook_id: Uuid,
    destination_url: String,
    event_type: String,
    payload: serde_json::Value,
    status: String,
    attempt_count: i32,
    lease_owner: Option<String>,
    lease_expires_at: Option<OffsetDateTime>,
}

#[derive(sqlx::FromRow)]
struct ExhaustedCalendarMutationRow {
    status: String,
    attempt_count: i32,
    lease_owner: Option<String>,
    lease_expires_at: Option<OffsetDateTime>,
    next_attempt_at: Option<OffsetDateTime>,
    last_error_code: Option<String>,
    idempotency_state: String,
    response_status: Option<i16>,
    locked_until: Option<OffsetDateTime>,
    response_body: serde_json::Value,
}

#[tokio::test]
async fn first_calendar_oauth_connection_persists_the_account_and_consumes_the_authorization() {
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
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should accept direct checks");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let authorization_id = Uuid::now_v7();
    let mut state_verifier = authorization_id.as_bytes().to_vec();
    state_verifier.extend_from_slice(authorization_id.as_bytes());
    database
        .create_calendar_oauth_authorization(&CreateCalendarOAuthAuthorization {
            id: authorization_id,
            user_id: provisioned.profile.id,
            session_id: provisioned.session_id,
            device_id: provisioned.device.id,
            state_verifier: state_verifier.clone(),
            pkce_verifier: EncryptedCalendarSecret {
                ciphertext: vec![42_u8; 48],
                nonce: vec![43_u8; 24],
                key_version: 1,
            },
            client_kind: ClientPlatform::Android,
            expires_at: OffsetDateTime::now_utc() + TimeDuration::minutes(10),
        })
        .await
        .expect("OAuth authorization should persist");
    let claimed = database
        .claim_calendar_oauth_authorization(&state_verifier)
        .await
        .expect("OAuth authorization claim should succeed")
        .expect("OAuth authorization should be claimable");
    let provider_subject = claimed
        .expected_google_subject
        .expect("the provisioned owner should have a Google subject");
    let account = database
        .complete_calendar_oauth_authorization(&CompleteCalendarOAuthAuthorization {
            authorization_id,
            account_id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            provider_subject,
            email: EmailAddress::parse(format!("calendar-{}@example.test", provisioned.profile.id))
                .expect("fixture email should be valid"),
            granted_scopes: vec![
                "https://www.googleapis.com/auth/calendar.events".to_owned(),
                "https://www.googleapis.com/auth/calendar.calendarlist.readonly".to_owned(),
            ],
            refresh_token: Some(EncryptedCalendarSecret {
                ciphertext: vec![44_u8; 64],
                nonce: vec![45_u8; 24],
                key_version: 1,
            }),
        })
        .await
        .expect("first Calendar account should persist");

    assert_eq!(account.status, CalendarAccountStatus::Connecting);
    assert!(account.last_error_code.is_none());
    let authorization_state: (String, bool) = sqlx::query_as(
        "SELECT status,
            pkce_verifier_ciphertext IS NULL
                AND pkce_nonce IS NULL
                AND encryption_key_version IS NULL
         FROM calendar_oauth_authorizations
         WHERE id = $1",
    )
    .bind(authorization_id)
    .fetch_one(&pool)
    .await
    .expect("completed authorization should load");
    assert_eq!(authorization_state.0, "completed");
    assert!(authorization_state.1);

    database
        .mark_calendar_sync_failure(
            account.id,
            provisioned.profile.id,
            "calendar.sync_data_invalid",
        )
        .await
        .expect("sync failure should be recorded without invalidating OAuth");
    let sync_warning = database
        .calendar_account_for_user(provisioned.profile.id)
        .await
        .expect("Calendar account should load")
        .expect("Calendar account should remain connected");
    assert_eq!(sync_warning.status, CalendarAccountStatus::Active);
    assert_eq!(
        sync_warning.last_error_code.as_deref(),
        Some("calendar.sync_data_invalid")
    );

    sqlx::query("UPDATE calendar_accounts SET status = 'error' WHERE id = $1")
        .bind(account.id)
        .execute(&pool)
        .await
        .expect("legacy error state should persist for recovery fixture");
    assert!(
        database
            .active_calendar_sync_identities()
            .await
            .expect("eligible Calendar accounts should load")
            .iter()
            .any(|identity| identity.account_id == account.id)
    );
    assert!(
        database
            .calendar_sync_connection(account.id, provisioned.profile.id)
            .await
            .expect("legacy connection lookup should succeed")
            .is_some()
    );
    database
        .apply_calendar_list_sync(
            account.id,
            provisioned.profile.id,
            &[ProviderCalendar {
                provider_calendar_id: "primary".to_owned(),
                name: "기본 캘린더".to_owned(),
                description: Some("개인 일정\nGoogle Calendar".to_owned()),
                time_zone: "Asia/Seoul".to_owned(),
                color_id: None,
                access_role: "owner".to_owned(),
                is_primary: true,
                provider_selected: true,
                visibility: ProviderCalendarVisibility::Visible,
                provider_etag: None,
            }],
        )
        .await
        .expect("legacy sync error should recover without OAuth");
    assert_eq!(
        database
            .calendar_account_for_user(provisioned.profile.id)
            .await
            .expect("recovered account should load")
            .expect("recovered account should exist")
            .status,
        CalendarAccountStatus::Active
    );

    database
        .mark_calendar_sync_failure(
            account.id,
            provisioned.profile.id,
            "calendar.authorization_failed",
        )
        .await
        .expect("credential failure should be recorded");
    assert_eq!(
        database
            .calendar_account_for_user(provisioned.profile.id)
            .await
            .expect("reauth account should load")
            .expect("reauth account should exist")
            .status,
        CalendarAccountStatus::ReauthRequired
    );

    pool.close().await;
    database.close().await;
}

fn assert_automatic_webhook_delivery(
    delivery: &AutomaticWebhookDeliveryRow,
    user_id: Uuid,
    project_id: Uuid,
    webhook_id: Uuid,
    event_type: &str,
    entity_id: Uuid,
) {
    assert_eq!(delivery.user_id, user_id);
    assert_eq!(delivery.project_id, project_id);
    assert_eq!(delivery.webhook_id, webhook_id);
    assert_eq!(
        delivery.destination_url,
        "https://automation.example/events"
    );
    assert_eq!(delivery.event_type, event_type);
    assert_eq!(delivery.status, "queued");
    assert_eq!(delivery.attempt_count, 0);
    assert!(delivery.lease_owner.is_none());
    assert!(delivery.lease_expires_at.is_none());
    assert_eq!(delivery.id.get_version_num(), 7);

    let payload = delivery
        .payload
        .as_object()
        .expect("automatic webhook payload should be an object");
    let expected_project_id = project_id.to_string();
    let expected_entity_id = entity_id.to_string();
    assert_eq!(
        payload.len(),
        4,
        "payload must not leak mutable entity data"
    );
    assert_eq!(
        payload.get("event").and_then(serde_json::Value::as_str),
        Some(event_type)
    );
    assert_eq!(
        payload.get("projectId").and_then(serde_json::Value::as_str),
        Some(expected_project_id.as_str())
    );
    assert_eq!(
        payload.get("entityId").and_then(serde_json::Value::as_str),
        Some(expected_entity_id.as_str())
    );
    let occurred_at = payload
        .get("occurredAt")
        .and_then(serde_json::Value::as_str)
        .expect("occurredAt should be an RFC 3339 string");
    let occurred_at = OffsetDateTime::parse(occurred_at, &Rfc3339)
        .expect("occurredAt should contain a parseable RFC 3339 timestamp");
    assert!(occurred_at <= OffsetDateTime::now_utc());
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies destination disablement, mutation finalization, idempotency finalization, and claim exclusion together."
)]
async fn disabled_calendar_terminally_resolves_pending_schedule_mutations() {
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
    let user_id = provisioned.profile.id;
    let account_id = Uuid::now_v7();
    let calendar_id = Uuid::now_v7();
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    sqlx::query(
        "INSERT INTO calendar_accounts (
            id, user_id, provider, provider_subject, email, status, granted_scopes,
            refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
        ) VALUES ($1, $2, 'google', $3, $4, 'active', $5, $6, $7, 1)",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(format!("disabled-calendar-{user_id}"))
    .bind(format!("disabled-{user_id}@example.test"))
    .bind(vec![
        "https://www.googleapis.com/auth/calendar.events".to_owned(),
    ])
    .bind(vec![7_u8; 32])
    .bind(vec![8_u8; 24])
    .execute(&pool)
    .await
    .expect("calendar account should persist");
    sqlx::query(
        "INSERT INTO calendars (
            id, account_id, provider_calendar_id, name, time_zone, access_role,
            is_primary, provider_selected, sync_enabled
        ) VALUES ($1, $2, 'primary', '기본 캘린더', 'Asia/Seoul', 'owner', TRUE, TRUE, TRUE)",
    )
    .bind(calendar_id)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("calendar should persist");
    let now = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("whole-second fixture time");
    let schedule = database
        .create_schedule_entry_with_calendar_outbox(
            &NewScheduleEntry {
                id: Uuid::now_v7(),
                user_id,
                title: "비활성 대상 일정".to_owned(),
                notes: None,
                starts_at: now + TimeDuration::days(1),
                ends_at: now + TimeDuration::days(1) + TimeDuration::hours(1),
                time_zone: "Asia/Seoul".to_owned(),
            },
            &PrimaryCalendarMutationTarget {
                account_id,
                calendar_id,
                provider_calendar_id: "primary".to_owned(),
                time_zone: "Asia/Seoul".to_owned(),
            },
        )
        .await
        .expect("schedule mutation should queue");

    database
        .apply_calendar_list_sync(
            account_id,
            user_id,
            &[ProviderCalendar {
                provider_calendar_id: "primary".to_owned(),
                name: "기본 캘린더".to_owned(),
                description: None,
                time_zone: "Asia/Seoul".to_owned(),
                color_id: None,
                access_role: "owner".to_owned(),
                is_primary: false,
                provider_selected: false,
                visibility: ProviderCalendarVisibility::Hidden,
                provider_etag: None,
            }],
        )
        .await
        .expect("calendar disable sync should apply");
    let mutation_state: (String, Option<String>, String, Option<i16>) = sqlx::query_as(
        "SELECT mutation.status, mutation.last_error_code, idempotency.state,
            idempotency.response_status
         FROM calendar_mutations AS mutation
         INNER JOIN idempotency_records AS idempotency
            ON idempotency.id = mutation.idempotency_record_id
         WHERE mutation.schedule_entry_id = $1",
    )
    .bind(schedule.id)
    .fetch_one(&pool)
    .await
    .expect("terminal mutation state should load");
    assert_eq!(mutation_state.0, "failed");
    assert_eq!(
        mutation_state.1.as_deref(),
        Some("calendar.destination_unavailable")
    );
    assert_eq!(mutation_state.2, "failed");
    assert_eq!(mutation_state.3, Some(409));
    assert!(
        database
            .claim_schedule_calendar_mutations("disabled-calendar-worker", 25)
            .await
            .expect("claim should remain healthy")
            .iter()
            .all(|mutation| mutation.schedule_entry_id != schedule.id)
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test proves expired attempt exhaustion, terminal idempotency, sanitized account visibility, and claim exclusion in one regression."
)]
async fn expired_calendar_mutation_at_attempt_limit_is_terminal_before_reclaim() {
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
    let user_id = provisioned.profile.id;
    let account_id = Uuid::now_v7();
    let calendar_id = Uuid::now_v7();
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    sqlx::query(
        "INSERT INTO calendar_accounts (
            id, user_id, provider, provider_subject, email, status, granted_scopes,
            refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
        ) VALUES ($1, $2, 'google', $3, $4, 'active', $5, $6, $7, 1)",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(format!("exhausted-calendar-{user_id}"))
    .bind(format!("exhausted-{user_id}@example.test"))
    .bind(vec![
        "https://www.googleapis.com/auth/calendar.events".to_owned(),
    ])
    .bind(vec![7_u8; 32])
    .bind(vec![8_u8; 24])
    .execute(&pool)
    .await
    .expect("calendar account should persist");
    sqlx::query(
        "INSERT INTO calendars (
            id, account_id, provider_calendar_id, name, time_zone, access_role,
            is_primary, provider_selected, sync_enabled
        ) VALUES ($1, $2, 'primary', '기본 캘린더', 'Asia/Seoul', 'owner', TRUE, TRUE, TRUE)",
    )
    .bind(calendar_id)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("calendar should persist");
    let now = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("whole-second fixture time");
    let schedule = database
        .create_schedule_entry_with_calendar_outbox(
            &NewScheduleEntry {
                id: Uuid::now_v7(),
                user_id,
                title: "외부 오류에 노출하면 안 되는 일정".to_owned(),
                notes: Some("민감한 일정 메모".to_owned()),
                starts_at: now + TimeDuration::days(1),
                ends_at: now + TimeDuration::days(1) + TimeDuration::hours(1),
                time_zone: "Asia/Seoul".to_owned(),
            },
            &PrimaryCalendarMutationTarget {
                account_id,
                calendar_id,
                provider_calendar_id: "primary".to_owned(),
                time_zone: "Asia/Seoul".to_owned(),
            },
        )
        .await
        .expect("schedule mutation should queue");
    let mutation_id: Uuid =
        sqlx::query_scalar("SELECT id FROM calendar_mutations WHERE schedule_entry_id = $1")
            .bind(schedule.id)
            .fetch_one(&pool)
            .await
            .expect("queued mutation should load");
    sqlx::query(
        "UPDATE calendar_mutations
         SET status = 'sending', attempt_count = 8, lease_owner = 'crashed-calendar-worker',
             lease_expires_at = NOW() - INTERVAL '1 second', last_error_code = NULL
         WHERE id = $1",
    )
    .bind(mutation_id)
    .execute(&pool)
    .await
    .expect("expired final-attempt lease should persist");

    let claimed = database
        .claim_schedule_calendar_mutations("replacement-calendar-worker", 25)
        .await
        .expect("claim should terminalize exhausted leases first");
    assert!(
        claimed.iter().all(|mutation| mutation.id != mutation_id),
        "attempt nine must never be returned for provider dispatch"
    );
    let terminal_state = sqlx::query_as::<_, ExhaustedCalendarMutationRow>(
        "SELECT mutation.status, mutation.attempt_count, mutation.lease_owner,
            mutation.lease_expires_at, mutation.next_attempt_at, mutation.last_error_code,
            idempotency.state AS idempotency_state, idempotency.response_status,
            idempotency.locked_until,
            idempotency.response_body
         FROM calendar_mutations AS mutation
         INNER JOIN idempotency_records AS idempotency
            ON idempotency.id = mutation.idempotency_record_id
         WHERE mutation.id = $1",
    )
    .bind(mutation_id)
    .fetch_one(&pool)
    .await
    .expect("terminal mutation and idempotency state should load");
    assert_eq!(terminal_state.status, "failed");
    assert_eq!(
        terminal_state.attempt_count, 8,
        "terminalization must not create attempt nine"
    );
    assert!(terminal_state.lease_owner.is_none());
    assert!(terminal_state.lease_expires_at.is_none());
    assert!(terminal_state.next_attempt_at.is_none());
    assert_eq!(
        terminal_state.last_error_code.as_deref(),
        Some("calendar.provider_unavailable")
    );
    assert_eq!(terminal_state.idempotency_state, "failed");
    assert_eq!(terminal_state.response_status, Some(503));
    assert!(terminal_state.locked_until.is_none());
    assert_eq!(terminal_state.response_body, serde_json::json!({}));
    let account_state: (String, Option<String>) =
        sqlx::query_as("SELECT status, last_error_code FROM calendar_accounts WHERE id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("visible calendar account error should load");
    assert_eq!(account_state.0, "active");
    assert_eq!(
        account_state.1.as_deref(),
        Some("calendar.provider_unavailable")
    );

    let reclaimed = database
        .claim_schedule_calendar_mutations("another-calendar-worker", 25)
        .await
        .expect("terminal mutation must remain excluded from claims");
    assert!(reclaimed.iter().all(|mutation| mutation.id != mutation_id));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT attempt_count FROM calendar_mutations WHERE id = $1",)
            .bind(mutation_id)
            .fetch_one(&pool)
            .await
            .expect("terminal attempt count should load"),
        8
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
async fn disconnect_purges_a_connection_without_revocation_material() {
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
    let user_id = provisioned.profile.id;
    let account_id = Uuid::now_v7();
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    sqlx::query(
        "INSERT INTO calendar_accounts (
            id, user_id, provider, provider_subject, email, status
        ) VALUES ($1, $2, 'google', $3, $4, 'connecting')",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(format!("incomplete-calendar-{user_id}"))
    .bind(format!("incomplete-{user_id}@example.test"))
    .execute(&pool)
    .await
    .expect("incomplete connection should persist");

    let outcome = database
        .disconnect_calendar_account(user_id, 1)
        .await
        .expect("local purge must not require a usable refresh credential");
    assert!(matches!(
        outcome,
        DisconnectCalendarAccountOutcome::Disconnected(None)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM calendar_accounts WHERE id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("account count should load"),
        0
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test verifies configuration, snapshot retention, lease recovery, completion, retry, and safe history together."
)]
async fn project_webhook_queue_keeps_a_safe_delivery_snapshot() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        2,
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
        .expect("workspace query should succeed")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let project = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            workspace_id: personal.id,
            title: "Webhook 검증".to_owned(),
            objective: None,
            risk_level: 0,
            next_action: None,
            due_at: None,
        })
        .await
        .expect("project should persist");
    let webhook = database
        .create_project_webhook(&NewProjectWebhook {
            id: Uuid::now_v7(),
            user_id: provisioned.profile.id,
            project_id: project.id,
            url: "https://automation.example/webhook".to_owned(),
            events: vec!["task.created".to_owned(), "task.updated".to_owned()],
            authentication: Some(EncryptedWebhookSecret {
                ciphertext: vec![7; 32],
                nonce: vec![8; 24],
            }),
        })
        .await
        .expect("webhook should persist");
    for event_type in ["task.created", "task.updated"] {
        assert_eq!(
            database
                .queue_project_webhook_event(
                    provisioned.profile.id,
                    project.id,
                    event_type,
                    &serde_json::json!({ "event": event_type }),
                )
                .await
                .expect("event should queue"),
            1
        );
    }
    assert!(
        database
            .delete_project_webhook(
                provisioned.profile.id,
                project.id,
                webhook.id,
                webhook.version,
            )
            .await
            .expect("webhook should delete")
    );

    let first_worker = "webhook-snapshot-worker-1";
    let claimed = database
        .claim_webhook_deliveries(first_worker, 10)
        .await
        .expect("snapshotted deliveries should remain claimable");
    assert_eq!(claimed.len(), 2);
    assert!(claimed.iter().all(|delivery| {
        delivery.url == "https://automation.example/webhook"
            && delivery
                .auth_header_ciphertext
                .as_deref()
                .is_some_and(|value| value == [7; 32])
            && delivery
                .auth_header_nonce
                .as_deref()
                .is_some_and(|value| value == [8; 24])
    }));
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    sqlx::query(
        "UPDATE webhook_deliveries SET lease_expires_at = NOW() - INTERVAL '1 second' WHERE id = $1",
    )
    .bind(claimed[0].id)
    .execute(&pool)
    .await
    .expect("simulated crashed worker lease should expire");
    let recovered = database
        .claim_webhook_deliveries("webhook-snapshot-worker-2", 10)
        .await
        .expect("expired delivery should be recovered");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].id, claimed[0].id);
    assert_eq!(recovered[0].attempt_count, claimed[0].attempt_count + 1);
    assert!(
        !database
            .complete_webhook_delivery(claimed[0].id, first_worker, claimed[0].attempt_count, 204,)
            .await
            .expect("stale worker completion should be ignored")
    );
    assert!(
        !database
            .fail_webhook_delivery(
                claimed[0].id,
                first_worker,
                claimed[0].attempt_count,
                None,
                "webhook.destination_unavailable",
            )
            .await
            .expect("stale worker failure should be ignored")
    );
    assert!(
        database
            .complete_webhook_delivery(
                recovered[0].id,
                "webhook-snapshot-worker-2",
                recovered[0].attempt_count,
                204,
            )
            .await
            .expect("lease owner should complete recovered delivery")
    );
    assert!(
        database
            .fail_webhook_delivery(
                claimed[1].id,
                first_worker,
                claimed[1].attempt_count,
                Some(503),
                "webhook.destination_rejected",
            )
            .await
            .expect("failed delivery should enter retry wait")
    );
    let history = database
        .webhook_delivery_history(provisioned.profile.id, project.id)
        .await
        .expect("delivery history should load");
    assert_eq!(history.len(), 2);
    assert!(
        history
            .iter()
            .any(|delivery| delivery.status == "delivered")
    );
    assert!(history.iter().any(|delivery| {
        delivery.status == "retry_wait"
            && delivery.response_code == Some(503)
            && delivery.attempt_count == 1
    }));
    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The test exercises every automatic project webhook mutation through public storage methods and verifies the resulting durable contracts together."
)]
async fn automatic_work_mutations_queue_safe_unique_webhook_deliveries() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        2,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let user_id = owner.profile.id;
    let personal = database
        .workspaces_for_user(user_id)
        .await
        .expect("workspace query should succeed")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let project = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id,
            workspace_id: personal.id,
            title: "자동 웹훅 검증".to_owned(),
            objective: Some("외부에 노출되면 안 되는 프로젝트 설명".to_owned()),
            risk_level: 0,
            next_action: None,
            due_at: None,
        })
        .await
        .expect("project should persist");
    let webhook = database
        .create_project_webhook(&NewProjectWebhook {
            id: Uuid::now_v7(),
            user_id,
            project_id: project.id,
            url: "https://automation.example/events".to_owned(),
            events: vec![
                "project.updated".to_owned(),
                "task.created".to_owned(),
                "task.updated".to_owned(),
                "task.completed".to_owned(),
                "task.restored".to_owned(),
                "task.deleted".to_owned(),
            ],
            authentication: Some(EncryptedWebhookSecret {
                ciphertext: vec![3; 32],
                nonce: vec![4; 24],
            }),
        })
        .await
        .expect("automatic event webhook should persist");

    database
        .update_project(&ProjectUpdate {
            id: project.id,
            user_id,
            title: "자동 웹훅 검증 수정".to_owned(),
            objective: Some("외부에 노출되면 안 되는 수정 설명".to_owned()),
            status: ProjectStatus::Active,
            risk_level: 1,
            next_action: Some("자동 이벤트 확인".to_owned()),
            due_at: None,
            expected_version: project.version,
        })
        .await
        .expect("project update should succeed")
        .expect("current project should update");
    let created_task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id,
            project_id: Some(project.id),
            title: "자동 이벤트 할 일".to_owned(),
            notes: Some("외부에 노출되면 안 되는 할 일 메모".to_owned()),
            priority: 1,
            due_at: None,
        })
        .await
        .expect("linked task should persist");
    let updated_task = database
        .update_task(&TaskUpdate {
            id: created_task.id,
            user_id,
            project_id: Some(project.id),
            title: "자동 이벤트 할 일 수정".to_owned(),
            notes: created_task.notes.clone(),
            status: TaskStatus::Open,
            priority: 2,
            due_at: None,
            expected_version: created_task.version,
        })
        .await
        .expect("task update should succeed")
        .expect("current task should update");
    let completed_task = database
        .complete_task(user_id, updated_task.id, updated_task.version)
        .await
        .expect("task completion should succeed")
        .expect("open task should complete");
    let restored_task = database
        .update_task(&TaskUpdate {
            id: completed_task.id,
            user_id,
            project_id: Some(project.id),
            title: completed_task.title.clone(),
            notes: completed_task.notes.clone(),
            status: TaskStatus::Open,
            priority: completed_task.priority,
            due_at: completed_task.due_at,
            expected_version: completed_task.version,
        })
        .await
        .expect("task restore should succeed")
        .expect("completed task should restore");
    assert_eq!(
        database
            .delete_task(user_id, restored_task.id, restored_task.version)
            .await
            .expect("task deletion should succeed"),
        DeleteTaskOutcome::Deleted
    );

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let deliveries = sqlx::query_as::<_, AutomaticWebhookDeliveryRow>(
        "SELECT id, user_id, project_id, webhook_id, destination_url, event_type, payload,
            status, attempt_count, lease_owner, lease_expires_at
         FROM webhook_deliveries
         WHERE user_id = $1 AND project_id = $2 AND webhook_id = $3",
    )
    .bind(user_id)
    .bind(project.id)
    .bind(webhook.id)
    .fetch_all(&pool)
    .await
    .expect("automatic webhook deliveries should load");
    let expected_events = [
        ("project.updated", project.id),
        ("task.created", created_task.id),
        ("task.updated", created_task.id),
        ("task.completed", created_task.id),
        ("task.restored", created_task.id),
        ("task.deleted", created_task.id),
    ];
    assert_eq!(deliveries.len(), expected_events.len());
    for (event_type, entity_id) in expected_events {
        let matching = deliveries
            .iter()
            .filter(|delivery| delivery.event_type == event_type)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "each mutation must queue exactly once");
        assert_automatic_webhook_delivery(
            matching[0],
            user_id,
            project.id,
            webhook.id,
            event_type,
            entity_id,
        );
    }
    assert!(deliveries.iter().all(|delivery| {
        let payload = delivery.payload.to_string();
        !payload.contains("노출되면 안 되는")
            && !payload.contains("자동 웹훅 검증 수정")
            && !payload.contains("자동 이벤트 할 일 수정")
    }));

    // The worker sends this immutable delivery ID as both X-Jimin-Delivery
    // and Idempotency-Key, so one logical mutation must have one unique ID.
    let idempotency_keys = deliveries
        .iter()
        .map(|delivery| delivery.id)
        .collect::<HashSet<_>>();
    assert_eq!(idempotency_keys.len(), expected_events.len());
    assert!(
        database
            .webhook_delivery_history(other.profile.id, project.id)
            .await
            .expect("foreign history query should remain safe")
            .is_empty()
    );

    sqlx::query("DELETE FROM webhook_deliveries WHERE webhook_id = $1")
        .bind(webhook.id)
        .execute(&pool)
        .await
        .expect("automatic webhook delivery fixtures should clean up");

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The test verifies all explicit secret mutation modes and ownership/version isolation as one update contract."
)]
async fn project_webhook_update_preserves_replaces_and_removes_secrets() {
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
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let personal = database
        .workspaces_for_user(owner.profile.id)
        .await
        .expect("workspace query should succeed")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let project = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            workspace_id: personal.id,
            title: "웹훅 수정".to_owned(),
            objective: None,
            risk_level: 0,
            next_action: None,
            due_at: None,
        })
        .await
        .expect("project should persist");
    let webhook = database
        .create_project_webhook(&NewProjectWebhook {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            project_id: project.id,
            url: "https://automation.example/original".to_owned(),
            events: vec!["task.created".to_owned()],
            authentication: Some(EncryptedWebhookSecret {
                ciphertext: vec![7; 32],
                nonce: vec![8; 24],
            }),
        })
        .await
        .expect("webhook should persist");
    assert!(
        database
            .update_project_webhook(&ProjectWebhookUpdate {
                id: webhook.id,
                user_id: other.profile.id,
                project_id: project.id,
                url: webhook.url.clone(),
                events: webhook.events.clone(),
                enabled: true,
                authentication: WebhookAuthenticationUpdate::Keep,
                expected_version: webhook.version,
            })
            .await
            .expect("foreign update should remain safe")
            .is_none()
    );
    assert!(matches!(
        database
            .update_project_webhook(&ProjectWebhookUpdate {
                id: webhook.id,
                user_id: owner.profile.id,
                project_id: project.id,
                url: webhook.url.clone(),
                events: vec!["task.created".to_owned(), "task.created".to_owned()],
                enabled: true,
                authentication: WebhookAuthenticationUpdate::Keep,
                expected_version: webhook.version,
            })
            .await,
        Err(StorageError::InvalidConfiguration)
    ));
    let kept = database
        .update_project_webhook(&ProjectWebhookUpdate {
            id: webhook.id,
            user_id: owner.profile.id,
            project_id: project.id,
            url: "https://automation.example/kept".to_owned(),
            events: vec!["task.updated".to_owned()],
            enabled: false,
            authentication: WebhookAuthenticationUpdate::Keep,
            expected_version: webhook.version,
        })
        .await
        .expect("webhook update should succeed")
        .expect("current webhook should update");
    assert!(kept.has_authentication);
    assert!(!kept.enabled);
    assert!(kept.version > webhook.version);

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let kept_secret: (Option<Vec<u8>>, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT auth_header_ciphertext, auth_header_nonce FROM project_webhooks WHERE id = $1",
    )
    .bind(webhook.id)
    .fetch_one(&pool)
    .await
    .expect("kept secret should load");
    assert_eq!(kept_secret, (Some(vec![7; 32]), Some(vec![8; 24])));

    let replaced = database
        .update_project_webhook(&ProjectWebhookUpdate {
            id: webhook.id,
            user_id: owner.profile.id,
            project_id: project.id,
            url: kept.url.clone(),
            events: kept.events.clone(),
            enabled: true,
            authentication: WebhookAuthenticationUpdate::Replace(EncryptedWebhookSecret {
                ciphertext: vec![9; 48],
                nonce: vec![10; 24],
            }),
            expected_version: kept.version,
        })
        .await
        .expect("secret replacement should succeed")
        .expect("current webhook should update");
    assert!(replaced.has_authentication);
    let replaced_secret: (Option<Vec<u8>>, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT auth_header_ciphertext, auth_header_nonce FROM project_webhooks WHERE id = $1",
    )
    .bind(webhook.id)
    .fetch_one(&pool)
    .await
    .expect("replaced secret should load");
    assert_eq!(replaced_secret, (Some(vec![9; 48]), Some(vec![10; 24])));

    let removed = database
        .update_project_webhook(&ProjectWebhookUpdate {
            id: webhook.id,
            user_id: owner.profile.id,
            project_id: project.id,
            url: replaced.url.clone(),
            events: replaced.events.clone(),
            enabled: replaced.enabled,
            authentication: WebhookAuthenticationUpdate::Remove,
            expected_version: replaced.version,
        })
        .await
        .expect("secret removal should succeed")
        .expect("current webhook should update");
    assert!(!removed.has_authentication);
    let removed_secret: (Option<Vec<u8>>, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT auth_header_ciphertext, auth_header_nonce FROM project_webhooks WHERE id = $1",
    )
    .bind(webhook.id)
    .fetch_one(&pool)
    .await
    .expect("removed secret state should load");
    assert_eq!(removed_secret, (None, None));
    assert!(
        database
            .update_project_webhook(&ProjectWebhookUpdate {
                id: webhook.id,
                user_id: owner.profile.id,
                project_id: project.id,
                url: removed.url.clone(),
                events: removed.events.clone(),
                enabled: removed.enabled,
                authentication: WebhookAuthenticationUpdate::Keep,
                expected_version: replaced.version,
            })
            .await
            .expect("stale update should remain safe")
            .is_none()
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The test verifies owner isolation and every retry state transition while retaining one delivery ID."
)]
async fn failed_webhook_delivery_retries_with_the_same_id_idempotently() {
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
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let personal = database
        .workspaces_for_user(owner.profile.id)
        .await
        .expect("workspace query should succeed")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let project = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            workspace_id: personal.id,
            title: "웹훅 재전송".to_owned(),
            objective: None,
            risk_level: 0,
            next_action: None,
            due_at: None,
        })
        .await
        .expect("project should persist");
    let webhook = database
        .create_project_webhook(&NewProjectWebhook {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            project_id: project.id,
            url: "https://automation.example/retry".to_owned(),
            events: vec!["webhook.test".to_owned()],
            authentication: None,
        })
        .await
        .expect("webhook should persist");
    let delivery_id = database
        .queue_webhook_test(
            owner.profile.id,
            project.id,
            webhook.id,
            &serde_json::json!({"event": "webhook.test"}),
        )
        .await
        .expect("test delivery should queue")
        .expect("enabled webhook should produce a delivery");
    assert_eq!(
        database
            .retry_webhook_delivery(owner.profile.id, project.id, delivery_id)
            .await
            .expect("queued retry should remain safe"),
        RetryWebhookDeliveryOutcome::AlreadyQueued
    );

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    sqlx::query(
        "UPDATE webhook_deliveries SET status = 'failed', attempt_count = 5, response_code = 503, last_error_code = 'webhook.destination_rejected' WHERE id = $1",
    )
    .bind(delivery_id)
    .execute(&pool)
    .await
    .expect("terminal failure fixture should persist");
    assert_eq!(
        database
            .retry_webhook_delivery(other.profile.id, project.id, delivery_id)
            .await
            .expect("foreign retry should remain safe"),
        RetryWebhookDeliveryOutcome::Conflict
    );
    assert_eq!(
        database
            .retry_webhook_delivery(owner.profile.id, project.id, delivery_id)
            .await
            .expect("failed delivery should requeue"),
        RetryWebhookDeliveryOutcome::Queued
    );
    let queued: (Uuid, String, i32, Option<i32>, Option<String>) = sqlx::query_as(
        "SELECT id, status, attempt_count, response_code, last_error_code FROM webhook_deliveries WHERE id = $1",
    )
    .bind(delivery_id)
    .fetch_one(&pool)
    .await
    .expect("requeued delivery should load");
    assert_eq!(queued, (delivery_id, "queued".to_owned(), 0, None, None));
    assert_eq!(
        database
            .retry_webhook_delivery(owner.profile.id, project.id, delivery_id)
            .await
            .expect("replayed manual retry should remain safe"),
        RetryWebhookDeliveryOutcome::AlreadyQueued
    );
    let retry_worker = "webhook-manual-retry-worker";
    sqlx::query(
        "UPDATE webhook_deliveries SET status = 'sending', attempt_count = 1, lease_owner = $2, lease_expires_at = NOW() + INTERVAL '30 seconds' WHERE id = $1",
    )
        .bind(delivery_id)
        .bind(retry_worker)
        .execute(&pool)
        .await
        .expect("sending fixture should persist");
    assert_eq!(
        database
            .retry_webhook_delivery(owner.profile.id, project.id, delivery_id)
            .await
            .expect("sending retry should remain safe"),
        RetryWebhookDeliveryOutcome::Conflict
    );
    assert!(
        database
            .complete_webhook_delivery(delivery_id, retry_worker, 1, 204)
            .await
            .expect("delivery should complete")
    );
    assert_eq!(
        database
            .retry_webhook_delivery(owner.profile.id, project.id, delivery_id)
            .await
            .expect("delivered retry should remain safe"),
        RetryWebhookDeliveryOutcome::Conflict
    );
    sqlx::query("DELETE FROM webhook_deliveries WHERE id = $1")
        .bind(delivery_id)
        .execute(&pool)
        .await
        .expect("test delivery cleanup should succeed");

    pool.close().await;
    database.close().await;
}

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
async fn unconnected_schedule_retries_once_for_manual_and_voice_callers() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        2,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let mutation_id = Uuid::now_v7();
    let starts_at = (OffsetDateTime::now_utc() + TimeDuration::days(1))
        .replace_nanosecond(0)
        .expect("whole-second start time");
    let ends_at = starts_at + TimeDuration::hours(1);
    let request = NewScheduleEntry {
        id: mutation_id,
        user_id: owner.profile.id,
        title: "  계약 검토 회의  ".to_owned(),
        notes: Some("  법무 의견 확인  ".to_owned()),
        starts_at,
        ends_at,
        time_zone: "  Asia/Seoul  ".to_owned(),
    };

    let created = database
        .create_schedule_entry(&request)
        .await
        .expect("first unconnected schedule request should persist");
    let replayed = database
        .create_schedule_entry(&request)
        .await
        .expect("an exact manual or voice retry should replay safely");
    assert_eq!(replayed, created);
    assert_eq!(created.id, mutation_id);
    assert_eq!(created.title, "계약 검토 회의");
    assert_eq!(created.notes.as_deref(), Some("법무 의견 확인"));
    assert_eq!(created.time_zone, "Asia/Seoul");

    let mismatched = database
        .create_schedule_entry(&NewScheduleEntry {
            title: "다른 일정".to_owned(),
            ..request
        })
        .await;
    assert!(matches!(mismatched, Err(StorageError::IdentityConflict)));
    let foreign_reuse = database
        .create_schedule_entry(&NewScheduleEntry {
            id: mutation_id,
            user_id: other.profile.id,
            title: "계약 검토 회의".to_owned(),
            notes: Some("법무 의견 확인".to_owned()),
            starts_at,
            ends_at,
            time_zone: "Asia/Seoul".to_owned(),
        })
        .await;
    assert!(matches!(foreign_reuse, Err(StorageError::IdentityConflict)));

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let schedule_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schedule_entries WHERE id = $1")
            .bind(mutation_id)
            .fetch_one(&pool)
            .await
            .expect("schedule count should load");
    let sync_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_changes
         WHERE entity_type = 'schedule_entry' AND entity_id = $1",
    )
    .bind(mutation_id)
    .fetch_one(&pool)
    .await
    .expect("schedule sync count should load");
    assert_eq!(schedule_count, 1);
    assert_eq!(sync_count, 1);

    pool.close().await;
    database.close().await;
}

#[tokio::test]
async fn client_mutation_id_replays_one_voice_task_and_rejects_payload_reuse() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        2,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let mutation_id = Uuid::now_v7();
    let due_at = (OffsetDateTime::now_utc() + TimeDuration::days(1))
        .replace_nanosecond(0)
        .expect("whole-second due date");
    let request = NewTask {
        id: mutation_id,
        user_id: owner.profile.id,
        project_id: None,
        title: "  계약서 검토  ".to_owned(),
        notes: Some("  법무 의견 반영  ".to_owned()),
        priority: 1,
        due_at: Some(due_at),
    };

    let created = database
        .create_task_idempotently(&request)
        .await
        .expect("first voice task request should persist");
    let replayed = database
        .create_task_idempotently(&request)
        .await
        .expect("an exact client mutation retry should replay safely");
    assert_eq!(replayed, created);
    assert_eq!(created.id, mutation_id);
    assert_eq!(created.title, "계약서 검토");
    assert_eq!(created.notes.as_deref(), Some("법무 의견 반영"));

    let mismatched = database
        .create_task_idempotently(&NewTask {
            title: "다른 할 일".to_owned(),
            ..request
        })
        .await;
    assert!(matches!(mismatched, Err(StorageError::IdentityConflict)));
    let foreign_reuse = database
        .create_task_idempotently(&NewTask {
            id: mutation_id,
            user_id: other.profile.id,
            project_id: None,
            title: "계약서 검토".to_owned(),
            notes: Some("법무 의견 반영".to_owned()),
            priority: 1,
            due_at: Some(due_at),
        })
        .await;
    assert!(matches!(foreign_reuse, Err(StorageError::IdentityConflict)));

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE id = $1")
        .bind(mutation_id)
        .fetch_one(&pool)
        .await
        .expect("task count should load");
    let sync_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_changes WHERE entity_type = 'task' AND entity_id = $1",
    )
    .bind(mutation_id)
    .fetch_one(&pool)
    .await
    .expect("task sync count should load");
    assert_eq!(task_count, 1);
    assert_eq!(sync_count, 1);

    pool.close().await;
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
    reason = "The integration test verifies ownership, concurrency, idempotency, task detachment, and sync tombstones as one deletion contract."
)]
async fn project_and_task_deletions_are_scoped_versioned_and_idempotent() {
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
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let personal = database
        .workspaces_for_user(owner.profile.id)
        .await
        .expect("workspace query should succeed")
        .into_iter()
        .find(|workspace| workspace.scope == WorkspaceScope::Personal)
        .expect("personal workspace should exist");
    let project = database
        .create_project(&NewProject {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            workspace_id: personal.id,
            title: "삭제할 프로젝트".to_owned(),
            objective: None,
            risk_level: 0,
            next_action: None,
            due_at: None,
        })
        .await
        .expect("project should persist");
    let linked_task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            project_id: Some(project.id),
            title: "연결된 할 일".to_owned(),
            notes: None,
            priority: 1,
            due_at: None,
        })
        .await
        .expect("linked task should persist");
    let webhook = database
        .create_project_webhook(&NewProjectWebhook {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            project_id: project.id,
            url: "https://example.com/project-delete".to_owned(),
            events: vec!["project.deleted".to_owned()],
            authentication: None,
        })
        .await
        .expect("project deletion webhook should persist");

    assert_eq!(
        database
            .delete_project(other.profile.id, project.id, project.version)
            .await
            .expect("foreign deletion should remain safe"),
        DeleteProjectOutcome::AlreadyAbsent
    );
    assert_eq!(
        database
            .delete_project(owner.profile.id, project.id, project.version + 1)
            .await
            .expect("stale deletion should not fail"),
        DeleteProjectOutcome::VersionConflict
    );
    assert_eq!(
        database
            .delete_project(owner.profile.id, project.id, project.version)
            .await
            .expect("owned project should delete"),
        DeleteProjectOutcome::Deleted
    );
    assert_eq!(
        database
            .delete_project(owner.profile.id, project.id, project.version)
            .await
            .expect("replayed deletion should be safe"),
        DeleteProjectOutcome::AlreadyAbsent
    );
    let detached = database
        .task_for_user(owner.profile.id, linked_task.id)
        .await
        .expect("detached task should load")
        .expect("task should remain after project deletion");
    assert!(detached.project_id.is_none());
    assert_eq!(detached.status, TaskStatus::Open);
    assert!(detached.version > linked_task.version);
    assert!(
        database
            .projects_for_workspace(owner.profile.id, personal.id)
            .await
            .expect("project list should load after deletion")
            .is_empty()
    );

    let task = database
        .create_task(&NewTask {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            project_id: None,
            title: "삭제할 할 일".to_owned(),
            notes: None,
            priority: 1,
            due_at: None,
        })
        .await
        .expect("task should persist");
    assert_eq!(
        database
            .delete_task(other.profile.id, task.id, task.version)
            .await
            .expect("foreign task deletion should remain safe"),
        DeleteTaskOutcome::AlreadyAbsent
    );
    assert_eq!(
        database
            .delete_task(owner.profile.id, task.id, task.version + 1)
            .await
            .expect("stale task deletion should not fail"),
        DeleteTaskOutcome::VersionConflict
    );
    assert_eq!(
        database
            .delete_task(owner.profile.id, task.id, task.version)
            .await
            .expect("owned task should delete"),
        DeleteTaskOutcome::Deleted
    );
    assert_eq!(
        database
            .delete_task(owner.profile.id, task.id, task.version)
            .await
            .expect("replayed task deletion should be safe"),
        DeleteTaskOutcome::AlreadyDeleted
    );
    let deleted = database
        .task_for_user(owner.profile.id, task.id)
        .await
        .expect("soft-deleted task should load")
        .expect("soft-deleted task should remain stored");
    assert_eq!(deleted.status, TaskStatus::Cancelled);
    assert!(deleted.completed_at.is_none());

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let detached_sync_version: i64 = sqlx::query_scalar(
        "SELECT entity_version FROM sync_changes WHERE user_id = $1 AND entity_type = 'task' AND entity_id = $2 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(owner.profile.id)
    .bind(linked_task.id)
    .fetch_one(&pool)
    .await
    .expect("detached task sync change should exist");
    assert_eq!(detached_sync_version, detached.version);
    let project_operation: String = sqlx::query_scalar(
        "SELECT operation FROM sync_changes WHERE user_id = $1 AND entity_type = 'project' AND entity_id = $2 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(owner.profile.id)
    .bind(project.id)
    .fetch_one(&pool)
    .await
    .expect("project tombstone should exist");
    assert_eq!(project_operation, "delete");
    let delivery_snapshot: (String, String, String) = sqlx::query_as(
        "SELECT event_type, status, destination_url FROM webhook_deliveries WHERE user_id = $1 AND project_id = $2 AND webhook_id = $3 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(owner.profile.id)
    .bind(project.id)
    .bind(webhook.id)
    .fetch_one(&pool)
    .await
    .expect("project deletion delivery snapshot should remain queued");
    assert_eq!(
        delivery_snapshot,
        (
            "project.deleted".to_owned(),
            "queued".to_owned(),
            "https://example.com/project-delete".to_owned(),
        )
    );
    let live_webhook_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_webhooks WHERE id = $1")
            .bind(webhook.id)
            .fetch_one(&pool)
            .await
            .expect("live webhook cascade should be queryable");
    assert_eq!(live_webhook_count, 0);
    let deleted_project_history = database
        .webhook_delivery_history(owner.profile.id, project.id)
        .await
        .expect("deleted project delivery history should remain available");
    assert_eq!(deleted_project_history.len(), 1);
    assert_eq!(deleted_project_history[0].event_type, "project.deleted");
    assert!(
        database
            .webhook_delivery_history(other.profile.id, project.id)
            .await
            .expect("foreign history query should remain safe")
            .is_empty()
    );
    sqlx::query(
        "UPDATE webhook_deliveries SET status = 'failed', attempt_count = 5, response_code = 503, last_error_code = 'webhook.destination_rejected' WHERE id = $1",
    )
    .bind(deleted_project_history[0].id)
    .execute(&pool)
    .await
    .expect("deleted project terminal delivery fixture should persist");
    assert_eq!(
        database
            .retry_webhook_delivery(other.profile.id, project.id, deleted_project_history[0].id)
            .await
            .expect("foreign deleted-project retry should remain safe"),
        RetryWebhookDeliveryOutcome::Conflict
    );
    assert_eq!(
        database
            .retry_webhook_delivery(owner.profile.id, project.id, deleted_project_history[0].id)
            .await
            .expect("owner should retry a deleted-project delivery snapshot"),
        RetryWebhookDeliveryOutcome::Queued
    );
    sqlx::query("DELETE FROM webhook_deliveries WHERE user_id = $1 AND project_id = $2")
        .bind(owner.profile.id)
        .bind(project.id)
        .execute(&pool)
        .await
        .expect("test delivery snapshot cleanup should succeed");

    pool.close().await;
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

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test proves atomic queueing, lease recovery, deduplication, and assistant integration."
)]
async fn connected_schedules_use_one_durable_google_mutation_stream() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database = Database::connect_lazy(
        &SecretString::from(database_url.clone()),
        2,
        Duration::from_secs(2),
    )
    .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let provisioned = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let user_id = provisioned.profile.id;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let account_id = Uuid::now_v7();
    let calendar_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO calendar_accounts (
            id, user_id, provider, provider_subject, email, status, granted_scopes,
            refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
        ) VALUES ($1, $2, 'google', $3, $4, 'active', $5, $6, $7, 1)",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(format!("calendar-subject-{user_id}"))
    .bind(format!("calendar-{user_id}@example.test"))
    .bind(vec![
        "https://www.googleapis.com/auth/calendar.events".to_owned(),
    ])
    .bind(vec![7_u8; 32])
    .bind(vec![8_u8; 24])
    .execute(&pool)
    .await
    .expect("calendar account should persist");
    sqlx::query(
        "INSERT INTO calendars (
            id, account_id, provider_calendar_id, name, time_zone, access_role,
            is_primary, provider_selected, sync_enabled
        ) VALUES ($1, $2, 'primary', '기본 캘린더', 'Asia/Seoul', 'owner', TRUE, TRUE, TRUE)",
    )
    .bind(calendar_id)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("provider calendar should persist");
    let target = PrimaryCalendarMutationTarget {
        account_id,
        calendar_id,
        provider_calendar_id: "primary".to_owned(),
        time_zone: "Asia/Seoul".to_owned(),
    };
    let now = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("whole-second fixture time");
    let schedule = database
        .create_schedule_entry_with_calendar_outbox(
            &NewScheduleEntry {
                id: Uuid::now_v7(),
                user_id,
                title: "Google 반영 일정".to_owned(),
                notes: Some("중복 없이 반영".to_owned()),
                starts_at: now + TimeDuration::days(1),
                ends_at: now + TimeDuration::days(1) + TimeDuration::hours(1),
                time_zone: "Asia/Seoul".to_owned(),
            },
            &target,
        )
        .await
        .expect("connected create should commit locally and journal atomically");
    let provider_event_id = provider_event_id_for_schedule(schedule.id);
    let replayed_schedule = database
        .create_schedule_entry_with_calendar_outbox(
            &NewScheduleEntry {
                id: schedule.id,
                user_id,
                title: schedule.title.clone(),
                notes: schedule.notes.clone(),
                starts_at: schedule.starts_at,
                ends_at: schedule.ends_at,
                time_zone: schedule.time_zone.clone(),
            },
            &target,
        )
        .await
        .expect("same client mutation should return the original schedule");
    assert_eq!(replayed_schedule, schedule);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM calendar_mutations WHERE schedule_entry_id = $1",
        )
        .bind(schedule.id)
        .fetch_one(&pool)
        .await
        .expect("mutation count should load"),
        1
    );
    let claimed = database
        .claim_schedule_calendar_mutations("calendar-worker-a", 10)
        .await
        .expect("create should be claimable");
    let claimed_mutation = claimed
        .iter()
        .find(|mutation| mutation.schedule_entry_id == schedule.id)
        .cloned()
        .expect("this schedule's create should be claimed");
    assert_eq!(
        claimed
            .iter()
            .filter(|mutation| mutation.schedule_entry_id == schedule.id)
            .count(),
        1
    );
    assert_eq!(
        claimed_mutation.operation,
        ScheduleCalendarMutationOperation::Create
    );
    assert_eq!(claimed_mutation.provider_event_id, provider_event_id);
    let concurrently_claimed = database
        .claim_schedule_calendar_mutations("calendar-worker-b", 25)
        .await
        .expect("leased mutation should not be double claimed");
    assert!(
        concurrently_claimed
            .iter()
            .all(|mutation| mutation.id != claimed_mutation.id)
    );
    assert!(matches!(
        database
            .disconnect_calendar_account(user_id, 1)
            .await
            .expect("an active claim should block disconnect"),
        DisconnectCalendarAccountOutcome::MutationInProgress
    ));
    assert!(
        database
            .begin_schedule_calendar_mutation_dispatch(
                claimed_mutation.id,
                claimed_mutation.attempt_count,
                "calendar-worker-a",
            )
            .await
            .expect("dispatch transition should persist")
            .is_some()
    );
    assert!(matches!(
        database
            .disconnect_calendar_account(user_id, 1)
            .await
            .expect("a sending provider call should block disconnect"),
        DisconnectCalendarAccountOutcome::MutationInProgress
    ));
    assert!(
        database
            .fail_schedule_calendar_mutation(
                claimed_mutation.id,
                claimed_mutation.attempt_count,
                "calendar-worker-a",
                "calendar.provider_unavailable",
                true,
            )
            .await
            .expect("retry should persist")
    );
    assert_eq!(
        database
            .calendar_account_for_user(user_id)
            .await
            .expect("connection should load")
            .expect("connection should remain active")
            .last_error_code
            .as_deref(),
        Some("calendar.provider_unavailable")
    );
    sqlx::query("UPDATE calendar_mutations SET next_attempt_at = NOW() WHERE id = $1")
        .bind(claimed_mutation.id)
        .execute(&pool)
        .await
        .expect("retry should become due");
    let retried = database
        .claim_schedule_calendar_mutations("calendar-worker-b", 25)
        .await
        .expect("retry should be claimable");
    let retried = retried
        .into_iter()
        .find(|mutation| mutation.id == claimed_mutation.id)
        .expect("this schedule's retry should be reclaimed");
    assert_eq!(retried.provider_event_id, provider_event_id);
    assert_eq!(retried.attempt_count, 2);
    assert!(
        database
            .begin_schedule_calendar_mutation_dispatch(
                retried.id,
                retried.attempt_count,
                "calendar-worker-b",
            )
            .await
            .expect("retry dispatch transition should persist")
            .is_some()
    );
    assert!(
        database
            .complete_schedule_calendar_mutation(
                retried.id,
                retried.attempt_count,
                "calendar-worker-b",
                Some("provider-etag-1"),
            )
            .await
            .expect("retried create should complete")
    );
    assert!(
        database
            .calendar_account_for_user(user_id)
            .await
            .expect("connection should load")
            .expect("connection should remain")
            .last_error_code
            .is_none()
    );

    sqlx::query(
        "INSERT INTO calendar_events (
            id, user_id, calendar_id, provider_event_id, provider_etag,
            provider_status, event_type, title, description_text, time_kind,
            start_at, end_at, source_time_zone, is_editable, sync_state
        ) VALUES ($1, $2, $3, $4, 'provider-etag-1', 'confirmed', 'default',
            'Google 반영 일정', '중복 없이 반영', 'date_time', $5, $6,
            'Asia/Seoul', TRUE, 'synced')",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(calendar_id)
    .bind(&provider_event_id)
    .bind(schedule.starts_at)
    .bind(schedule.ends_at)
    .execute(&pool)
    .await
    .expect("provider read model should persist");
    let visible = database
        .schedule_entries_in_range(
            user_id,
            schedule.starts_at - TimeDuration::hours(1),
            schedule.ends_at + TimeDuration::hours(1),
        )
        .await
        .expect("schedule range should load");
    assert_eq!(
        visible.len(),
        1,
        "provider projection must not duplicate the canonical schedule"
    );
    assert_eq!(visible[0].id, schedule.id);

    let updated = database
        .update_schedule_entry(&ScheduleEntryUpdate {
            id: schedule.id,
            user_id,
            title: "수정된 Google 일정".to_owned(),
            notes: None,
            starts_at: schedule.starts_at + TimeDuration::hours(1),
            ends_at: schedule.ends_at + TimeDuration::hours(1),
            time_zone: schedule.time_zone.clone(),
            expected_version: schedule.version,
        })
        .await
        .expect("linked update should succeed")
        .expect("linked schedule should exist");
    assert!(updated.version > schedule.version);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM calendar_mutations WHERE schedule_entry_id = $1",
        )
        .bind(schedule.id)
        .fetch_one(&pool)
        .await
        .expect("mutation count should load"),
        2
    );
    let update_mutation = database
        .claim_schedule_calendar_mutations("calendar-worker-c", 25)
        .await
        .expect("linked update should be claimable")
        .into_iter()
        .find(|mutation| {
            mutation.schedule_entry_id == schedule.id
                && mutation.operation == ScheduleCalendarMutationOperation::Update
        })
        .expect("this schedule's update should be claimed");
    assert!(
        database
            .begin_schedule_calendar_mutation_dispatch(
                update_mutation.id,
                update_mutation.attempt_count,
                "calendar-worker-c",
            )
            .await
            .expect("update dispatch should start")
            .is_some()
    );
    assert!(
        database
            .fail_schedule_calendar_mutation(
                update_mutation.id,
                update_mutation.attempt_count,
                "calendar-worker-c",
                "calendar.event_not_found",
                false,
            )
            .await
            .expect("event-level failure should persist")
    );
    let account_state: (String, Option<String>) =
        sqlx::query_as("SELECT status, last_error_code FROM calendar_accounts WHERE id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("calendar account state should load");
    assert_eq!(account_state.0, "active");
    assert_eq!(account_state.1.as_deref(), Some("calendar.event_not_found"));
    assert!(
        database
            .cancel_schedule_entry(user_id, schedule.id, updated.version)
            .await
            .expect("linked delete should succeed")
            .is_some()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM calendar_mutations WHERE schedule_entry_id = $1",
        )
        .bind(schedule.id)
        .fetch_one(&pool)
        .await
        .expect("mutation count should load"),
        3
    );

    let conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: conversation_id,
            user_id,
            title: Some("일정 비서".to_owned()),
        })
        .await
        .expect("conversation should persist");
    let queued = database
        .enqueue_agent_turn(&NewAgentTurn {
            job_id: Uuid::now_v7(),
            message_id: Uuid::now_v7(),
            client_message_id: Uuid::now_v7(),
            user_id,
            conversation_id,
            content: "내일 회의 일정을 추가해 줘".to_owned(),
        })
        .await
        .expect("assistant turn should queue");
    let runner_id = "calendar-assistant-test";
    let job = database
        .claim_next_agent_job(runner_id, Duration::from_secs(30))
        .await
        .expect("assistant job should claim")
        .expect("assistant job should exist");
    assert!(
        database
            .start_agent_job(
                job.id,
                runner_id,
                "thread-calendar",
                Duration::from_secs(30)
            )
            .await
            .expect("assistant job should start")
    );
    let assistant_schedule_id = Uuid::now_v7();
    assert!(
        database
            .complete_agent_job_with_actions(
                job.id,
                runner_id,
                Uuid::now_v7(),
                "회의 일정을 추가했어요.",
                None,
                &[AgentActionCommand::CreateSchedule {
                    id: assistant_schedule_id,
                    title: "회의".to_owned(),
                    notes: None,
                    starts_at: now + TimeDuration::days(2),
                    ends_at: now + TimeDuration::days(2) + TimeDuration::hours(1),
                    time_zone: "Asia/Seoul".to_owned(),
                }],
            )
            .await
            .expect("assistant schedule and outbox should commit together")
    );
    assert_eq!(job.id, queued.job_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM calendar_mutations WHERE schedule_entry_id = $1 AND operation = 'create'",
        )
        .bind(assistant_schedule_id)
        .fetch_one(&pool)
        .await
        .expect("assistant mutation count should load"),
        1
    );

    let approval_conversation_id = Uuid::now_v7();
    database
        .create_conversation(&NewConversation {
            id: approval_conversation_id,
            user_id,
            title: Some("승인 일정".to_owned()),
        })
        .await
        .expect("approval conversation should persist");
    let approval = database
        .enqueue_agent_action_turn(
            &NewAgentTurn {
                job_id: Uuid::now_v7(),
                message_id: Uuid::now_v7(),
                client_message_id: Uuid::now_v7(),
                user_id,
                conversation_id: approval_conversation_id,
                content: "승인 후 회의를 추가해 줘".to_owned(),
            },
            PendingAgentAction::CreateSchedule {
                title: "승인 회의".to_owned(),
                starts_at: now + TimeDuration::days(3),
                ends_at: now + TimeDuration::days(3) + TimeDuration::hours(1),
                time_zone: "Asia/Seoul".to_owned(),
            },
        )
        .await
        .expect("approval action should queue");
    assert!(
        database
            .resolve_agent_action(
                user_id,
                approval.job_id,
                PendingAgentActionDecision::Approve,
            )
            .await
            .expect("approved schedule should persist")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM calendar_mutations AS mutation
             INNER JOIN schedule_entries AS schedule ON schedule.id = mutation.schedule_entry_id
             WHERE schedule.user_id = $1 AND schedule.title = '승인 회의'
               AND mutation.operation = 'create'",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("approved assistant mutation count should load"),
        1
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "The integration test proves the complete shared Google credential purge boundary."
)]
async fn calendar_disconnect_is_versioned_idempotent_and_preserves_manual_schedules() {
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
    let user_id = provisioned.profile.id;
    let now = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("whole-second fixture time");
    let manual_schedule = database
        .create_schedule_entry(&NewScheduleEntry {
            id: Uuid::now_v7(),
            user_id,
            title: "직접 만든 일정".to_owned(),
            notes: None,
            starts_at: now + TimeDuration::days(1),
            ends_at: now + TimeDuration::days(1) + TimeDuration::hours(1),
            time_zone: "Asia/Seoul".to_owned(),
        })
        .await
        .expect("manual schedule should persist");
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let account_id = Uuid::now_v7();
    let calendar_id = Uuid::now_v7();
    let event_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO calendar_accounts (
            id, user_id, provider, provider_subject, email, status, granted_scopes,
            refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
        ) VALUES ($1, $2, 'google', $3, $4, 'active', $5, $6, $7, 1)",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(format!("calendar-subject-{user_id}"))
    .bind(format!("calendar-{user_id}@example.test"))
    .bind(vec![
        "https://www.googleapis.com/auth/calendar.events".to_owned(),
    ])
    .bind(vec![7_u8; 32])
    .bind(vec![8_u8; 24])
    .execute(&pool)
    .await
    .expect("calendar account should persist");
    sqlx::query(
        "INSERT INTO calendars (
            id, account_id, provider_calendar_id, name, time_zone, access_role,
            is_primary, provider_selected, sync_enabled
        ) VALUES ($1, $2, 'primary', '기본 캘린더', 'Asia/Seoul', 'owner', TRUE, TRUE, TRUE)",
    )
    .bind(calendar_id)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("provider calendar should persist");
    let linked_schedule = database
        .create_schedule_entry_with_calendar_outbox(
            &NewScheduleEntry {
                id: Uuid::now_v7(),
                user_id,
                title: "연결된 직접 일정".to_owned(),
                notes: None,
                starts_at: now + TimeDuration::days(2),
                ends_at: now + TimeDuration::days(2) + TimeDuration::hours(1),
                time_zone: "Asia/Seoul".to_owned(),
            },
            &PrimaryCalendarMutationTarget {
                account_id,
                calendar_id,
                provider_calendar_id: "primary".to_owned(),
                time_zone: "Asia/Seoul".to_owned(),
            },
        )
        .await
        .expect("linked schedule should journal before disconnect");
    sqlx::query(
        "INSERT INTO calendar_events (
            id, user_id, calendar_id, provider_event_id, provider_status, event_type,
            title, time_kind, start_at, end_at, source_time_zone, is_editable, sync_state
        ) VALUES (
            $1, $2, $3, 'provider-event', 'confirmed', 'default',
            '가져온 일정', 'date_time', $4, $5, 'Asia/Seoul', TRUE, 'synced'
        )",
    )
    .bind(event_id)
    .bind(user_id)
    .bind(calendar_id)
    .bind(now + TimeDuration::hours(2))
    .bind(now + TimeDuration::hours(3))
    .execute(&pool)
    .await
    .expect("provider event should persist");
    let idempotency_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO idempotency_records (
            id, user_id, idempotency_key, operation, request_hash, state
        ) VALUES ($1, $2, $3, 'calendar.update', $4, 'pending')",
    )
    .bind(idempotency_id)
    .bind(user_id)
    .bind(format!("calendar-disconnect-{idempotency_id}"))
    .bind(vec![9_u8; 32])
    .execute(&pool)
    .await
    .expect("idempotency fixture should persist");
    sqlx::query(
        "INSERT INTO calendar_mutations (
            id, user_id, event_id, operation, status, idempotency_record_id,
            desired_payload, expected_event_version, provider_event_id
        ) VALUES ($1, $2, $3, 'update', 'queued', $4, $5, 1, 'provider-event')",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(event_id)
    .bind(idempotency_id)
    .bind(serde_json::json!({ "title": "변경 대기" }))
    .execute(&pool)
    .await
    .expect("pending provider mutation should persist");
    let authorization_id = Uuid::now_v7();
    let mut state_verifier = authorization_id.as_bytes().to_vec();
    state_verifier.extend_from_slice(authorization_id.as_bytes());
    sqlx::query(
        "INSERT INTO calendar_oauth_authorizations (
            id, user_id, session_id, device_id, state_verifier,
            pkce_verifier_ciphertext, pkce_nonce, encryption_key_version,
            client_kind, status, expires_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 1, 'macos', 'pending', $8)",
    )
    .bind(authorization_id)
    .bind(user_id)
    .bind(provisioned.session_id)
    .bind(provisioned.device.id)
    .bind(state_verifier)
    .bind(vec![10_u8; 32])
    .bind(vec![11_u8; 24])
    .bind(now + TimeDuration::minutes(10))
    .execute(&pool)
    .await
    .expect("pending authorization should persist");
    sqlx::query("INSERT INTO gmail_sync_states (user_id, status) VALUES ($1, 'idle')")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("Gmail sync state should persist");
    sqlx::query(
        "INSERT INTO gmail_messages (
            id, user_id, provider_message_id, provider_thread_id, subject
        ) VALUES ($1, $2, 'provider-message', 'provider-thread', '연결 메일')",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("Gmail metadata should persist");

    assert!(matches!(
        database
            .disconnect_calendar_account(user_id, 2)
            .await
            .expect("version mismatch should be classified"),
        DisconnectCalendarAccountOutcome::VersionConflict
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM calendar_accounts WHERE user_id = $1",)
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("account count should load"),
        1
    );

    let detached = database
        .disconnect_calendar_account(user_id, 1)
        .await
        .expect("disconnect should succeed");
    let DisconnectCalendarAccountOutcome::Disconnected(connection) = detached else {
        panic!("the active connection should be detached");
    };
    let connection = connection.expect("valid revocation material should be returned");
    assert_eq!(connection.account_id, account_id);
    assert_eq!(connection.refresh_token.ciphertext, vec![7_u8; 32]);
    assert!(matches!(
        database
            .disconnect_calendar_account(user_id, 1)
            .await
            .expect("repeat disconnect should be safe"),
        DisconnectCalendarAccountOutcome::AlreadyDisconnected
    ));
    let account_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_accounts WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("calendar account count should load");
    let calendar_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendars WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("calendar count should load");
    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("calendar event count should load");
    let mutation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM calendar_mutations WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("calendar mutation count should load");
    let gmail_state_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gmail_sync_states WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("Gmail sync state count should load");
    let gmail_message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gmail_messages WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("Gmail message count should load");
    assert_eq!(account_count, 0);
    assert_eq!(calendar_count, 0);
    assert_eq!(event_count, 0);
    assert_eq!(mutation_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM idempotency_records WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("calendar idempotency records should be purged"),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM schedule_calendar_links WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("schedule links should be purged"),
        0
    );
    assert_eq!(gmail_state_count, 0);
    assert_eq!(gmail_message_count, 0);
    let authorization_status: String =
        sqlx::query_scalar("SELECT status FROM calendar_oauth_authorizations WHERE id = $1")
            .bind(authorization_id)
            .fetch_one(&pool)
            .await
            .expect("authorization status should load");
    let authorization_still_has_pkce: bool = sqlx::query_scalar(
        "SELECT pkce_verifier_ciphertext IS NOT NULL
             OR pkce_nonce IS NOT NULL
             OR encryption_key_version IS NOT NULL
         FROM calendar_oauth_authorizations WHERE id = $1",
    )
    .bind(authorization_id)
    .fetch_one(&pool)
    .await
    .expect("authorization secret lifetime should load");
    assert_eq!(authorization_status, "cancelled");
    assert!(!authorization_still_has_pkce);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM schedule_entries WHERE id = $1")
            .bind(manual_schedule.id)
            .fetch_one(&pool)
            .await
            .expect("manual schedule count should load"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM schedule_entries WHERE id = $1")
            .bind(linked_schedule.id)
            .fetch_one(&pool)
            .await
            .expect("linked manual schedule should remain after disconnect"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sync_changes
             WHERE user_id = $1 AND operation = 'delete'
               AND ((entity_type = 'calendar_account' AND entity_id = $2)
                 OR (entity_type = 'calendar_event' AND entity_id = $3))",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .expect("disconnect tombstones should load"),
        2
    );

    pool.close().await;
    database.close().await;
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Keep the complete create, isolate, decide, and replay lifecycle visible.
async fn recommendation_decisions_are_scoped_versioned_and_idempotent() {
    let Ok(database_url) = std::env::var("JIMIN_TEST_DATABASE_URL") else {
        return;
    };
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 1, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    let owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("fixture owner should exist");
    let other_owner = database
        .provision_login(&provision_login_command(Uuid::now_v7(), Uuid::now_v7()))
        .await
        .expect("second fixture owner should exist");
    let recommendation_id = Uuid::now_v7();
    let recommendation = database
        .create_recommendation(&NewRecommendation {
            id: recommendation_id,
            user_id: owner.profile.id,
            workspace_id: None,
            project_id: None,
            goal_id: None,
            signal_id: None,
            title: "마감이 가까운 일을 먼저 정리하세요".to_owned(),
            rationale: "오늘이 기한인 열린 일이 있습니다.".to_owned(),
            expected_effect: "마감 지연 위험을 줄입니다.".to_owned(),
            risk_summary: Some("다른 일의 시작이 늦어질 수 있습니다.".to_owned()),
            confidence: 94,
            urgency: 3,
            impact: 2,
            risk_level: 1,
            effort_minutes: Some(20),
            suggested_action_kind: Some(SuggestedActionKind::Review),
            suggested_entity_id: None,
            valid_until: Some(OffsetDateTime::now_utc() + TimeDuration::days(1)),
        })
        .await
        .expect("recommendation should persist");
    assert_eq!(recommendation.status, RecommendationStatus::Pending);

    let active = database
        .active_recommendations_for_user(owner.profile.id, OffsetDateTime::now_utc(), 10)
        .await
        .expect("owner recommendations should load");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, recommendation_id);
    assert!(
        database
            .active_recommendations_for_user(other_owner.profile.id, OffsetDateTime::now_utc(), 10,)
            .await
            .expect("other owner recommendations should load")
            .is_empty()
    );

    let revisit_at = OffsetDateTime::now_utc() + TimeDuration::hours(1);
    let DecideRecommendationOutcome::Applied(deferred) = database
        .decide_recommendation(&DecideRecommendation {
            id: Uuid::now_v7(),
            user_id: owner.profile.id,
            recommendation_id,
            decision: RecommendationDecision::Defer,
            reason: Some("점심 이후 다시 확인합니다.".to_owned()),
            revisit_at: Some(revisit_at),
            expected_version: recommendation.version,
        })
        .await
        .expect("defer decision should persist")
    else {
        panic!("defer decision should be applied");
    };
    assert!(
        database
            .active_recommendations_for_user(owner.profile.id, OffsetDateTime::now_utc(), 10)
            .await
            .expect("deferred inbox should load")
            .is_empty()
    );
    let revisited = database
        .active_recommendations_for_user(
            owner.profile.id,
            revisit_at + TimeDuration::seconds(1),
            10,
        )
        .await
        .expect("due deferred recommendation should load");
    assert_eq!(revisited[0].status, RecommendationStatus::Deferred);

    let decision_id = Uuid::now_v7();
    let command = DecideRecommendation {
        id: decision_id,
        user_id: owner.profile.id,
        recommendation_id,
        decision: RecommendationDecision::Approve,
        reason: Some("오늘 먼저 확인합니다.".to_owned()),
        revisit_at: None,
        expected_version: deferred.version,
    };
    let DecideRecommendationOutcome::Applied(approved) = database
        .decide_recommendation(&command)
        .await
        .expect("decision should persist")
    else {
        panic!("first decision should be applied");
    };
    assert_eq!(approved.status, RecommendationStatus::Approved);
    assert_eq!(approved.version, deferred.version + 1);

    assert!(matches!(
        database
            .decide_recommendation(&command)
            .await
            .expect("identical decision should replay"),
        DecideRecommendationOutcome::Replayed(replayed)
            if replayed.status == RecommendationStatus::Approved
    ));
    assert!(matches!(
        database
            .decide_recommendation(&DecideRecommendation {
                decision: RecommendationDecision::Reject,
                ..command
            })
            .await
            .expect("conflicting replay should be classified"),
        DecideRecommendationOutcome::VersionConflict
    ));
    assert!(matches!(
        database
            .decide_recommendation(&DecideRecommendation {
                id: Uuid::now_v7(),
                user_id: other_owner.profile.id,
                recommendation_id,
                decision: RecommendationDecision::Approve,
                reason: None,
                revisit_at: None,
                expected_version: 1,
            })
            .await
            .expect("cross-owner lookup should not leak existence"),
        DecideRecommendationOutcome::NotFound
    ));

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

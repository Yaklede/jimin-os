use std::time::Duration;

use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use jimin_storage::{
    Database,
    auth::{ProvisionLogin, ProvisionedLogin},
    meetings::{DeleteMeetingOutcome, NewMeeting},
    planning::NewTask,
};
use secrecy::SecretString;
use time::{Duration as TimeDuration, OffsetDateTime};
use uuid::Uuid;

#[tokio::test]
async fn meeting_delete_is_owner_scoped_versioned_and_preserves_promoted_tasks() {
    let Some((database, setup_pool)) = test_database().await else {
        return;
    };
    let owner = provision_owner(&database).await;
    let foreign_owner = provision_owner(&database).await;
    let promoted_task_id = Uuid::now_v7();
    database
        .create_task(&NewTask {
            id: promoted_task_id,
            user_id: owner.profile.id,
            project_id: None,
            parent_task_id: None,
            title: "회의에서 확정한 후속 일".to_owned(),
            notes: Some("회의가 삭제되어도 유지되어야 해요.".to_owned()),
            assignee_name: Some("조지민".to_owned()),
            priority: 2,
            due_at: Some(OffsetDateTime::now_utc() + TimeDuration::days(1)),
        })
        .await
        .expect("promoted task should persist");
    let (meeting_id, version) =
        create_review_ready_meeting(&database, &setup_pool, owner.profile.id, promoted_task_id)
            .await;
    assert_eq!(
        database
            .delete_meeting(foreign_owner.profile.id, meeting_id, version)
            .await
            .expect("foreign deletion should be hidden"),
        DeleteMeetingOutcome::AlreadyAbsent
    );
    assert_eq!(
        database
            .delete_meeting(owner.profile.id, meeting_id, version - 1)
            .await
            .expect("stale deletion should be classified"),
        DeleteMeetingOutcome::VersionConflict
    );
    assert_eq!(
        database
            .delete_meeting(owner.profile.id, meeting_id, version)
            .await
            .expect("current meeting should delete"),
        DeleteMeetingOutcome::Deleted
    );
    assert_eq!(
        database
            .delete_meeting(owner.profile.id, meeting_id, version)
            .await
            .expect("deletion replay should be idempotent"),
        DeleteMeetingOutcome::AlreadyAbsent
    );
    assert!(
        database
            .task_for_user(owner.profile.id, promoted_task_id)
            .await
            .expect("task lookup should work")
            .is_some(),
        "a task already promoted from the meeting must remain"
    );
    let meeting_children = sqlx::query_scalar::<_, i64>(
        "SELECT
            (SELECT COUNT(*) FROM meeting_action_items WHERE meeting_id = $1)
          + (SELECT COUNT(*) FROM meeting_decisions WHERE meeting_id = $1)
          + (SELECT COUNT(*) FROM meeting_recordings WHERE meeting_id = $1)
          + (SELECT COUNT(*) FROM meeting_speakers WHERE meeting_id = $1)
          + (SELECT COUNT(*) FROM meeting_transcript_segments WHERE meeting_id = $1)",
    )
    .bind(meeting_id)
    .fetch_one(&setup_pool)
    .await
    .expect("meeting child count should load");
    assert_eq!(meeting_children, 0);
    let delete_change = sqlx::query_scalar::<_, String>(
        "SELECT operation FROM sync_changes
         WHERE user_id = $1 AND entity_type = 'meeting' AND entity_id = $2
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(owner.profile.id)
    .bind(meeting_id)
    .fetch_one(&setup_pool)
    .await
    .expect("meeting tombstone should persist");
    assert_eq!(delete_change, "delete");

    database.close().await;
    setup_pool.close().await;
}

#[tokio::test]
async fn meeting_delete_rejects_an_active_processing_pipeline() {
    let Some((database, setup_pool)) = test_database().await else {
        return;
    };
    let owner = provision_owner(&database).await;
    let processing_meeting_id = Uuid::now_v7();
    let processing_meeting = database
        .create_meeting(&NewMeeting {
            id: processing_meeting_id,
            user_id: owner.profile.id,
            workspace_id: None,
            project_id: None,
            title: "정리 중인 회의".to_owned(),
            purpose: None,
            participants: Vec::new(),
            transcript: "아직 정리 중이다.".to_owned(),
            started_at: None,
            duration_seconds: None,
        })
        .await
        .expect("processing meeting should persist");
    assert_eq!(
        database
            .delete_meeting(
                owner.profile.id,
                processing_meeting_id,
                processing_meeting.version,
            )
            .await
            .expect("processing deletion should be classified"),
        DeleteMeetingOutcome::Processing
    );
    assert!(
        database
            .meeting_detail_for_user(owner.profile.id, processing_meeting_id)
            .await
            .expect("processing meeting lookup should work")
            .is_some()
    );

    database.close().await;
    setup_pool.close().await;
}

async fn test_database() -> Option<(Database, sqlx::PgPool)> {
    let database_url = std::env::var("JIMIN_TEST_DATABASE_URL").ok()?;
    let setup_pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("test setup pool should connect");
    let database =
        Database::connect_lazy(&SecretString::from(database_url), 2, Duration::from_secs(2))
            .expect("test database URL should be valid");
    database.migrate().await.expect("migration should succeed");
    Some((database, setup_pool))
}

async fn create_review_ready_meeting(
    database: &Database,
    setup_pool: &sqlx::PgPool,
    user_id: Uuid,
    promoted_task_id: Uuid,
) -> (Uuid, i64) {
    let meeting_id = Uuid::now_v7();
    database
        .create_meeting(&NewMeeting {
            id: meeting_id,
            user_id,
            workspace_id: None,
            project_id: None,
            title: "삭제할 회의".to_owned(),
            purpose: Some("삭제 경계를 확인한다.".to_owned()),
            participants: vec!["조지민".to_owned()],
            transcript: "후속 일을 만들기로 했다.".to_owned(),
            started_at: Some(OffsetDateTime::now_utc()),
            duration_seconds: Some(300),
        })
        .await
        .expect("meeting should persist");
    sqlx::query(
        "UPDATE meetings
         SET status = 'review_ready', summary = '후속 일을 확정했어요.', version = version + 1
         WHERE id = $1 AND user_id = $2",
    )
    .bind(meeting_id)
    .bind(user_id)
    .execute(setup_pool)
    .await
    .expect("meeting should become deletable");
    sqlx::query(
        "UPDATE meeting_analysis_jobs
         SET state = 'completed', finished_at = NOW(), version = version + 1
         WHERE meeting_id = $1",
    )
    .bind(meeting_id)
    .execute(setup_pool)
    .await
    .expect("analysis job should become completed");
    insert_meeting_owned_sources(setup_pool, meeting_id, user_id).await;
    sqlx::query(
        "INSERT INTO meeting_action_items (
            id, meeting_id, kind, title, notes, priority, source_excerpt,
            confidence, status, target_entity_id, applied_at
         ) VALUES ($1, $2, 'task', '회의에서 확정한 후속 일', $3, 2, $4,
             98, 'applied', $5, NOW())",
    )
    .bind(Uuid::now_v7())
    .bind(meeting_id)
    .bind("회의가 삭제되어도 유지되어야 해요.")
    .bind("후속 일을 만들기로 했다.")
    .bind(promoted_task_id)
    .execute(setup_pool)
    .await
    .expect("applied meeting action should persist");
    let version = database
        .meeting_detail_for_user(user_id, meeting_id)
        .await
        .expect("meeting should load")
        .expect("meeting should exist")
        .meeting
        .version;
    (meeting_id, version)
}

async fn insert_meeting_owned_sources(setup_pool: &sqlx::PgPool, meeting_id: Uuid, user_id: Uuid) {
    let recording_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO meeting_recordings (
            id, meeting_id, user_id, state, duration_milliseconds,
            finalized_at, finished_at
         ) VALUES ($1, $2, $3, 'completed', 300000, NOW(), NOW())",
    )
    .bind(recording_id)
    .bind(meeting_id)
    .bind(user_id)
    .execute(setup_pool)
    .await
    .expect("meeting recording should persist");
    let speaker_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO meeting_speakers (
            id, meeting_id, speaker_key, display_name, ordinal
         ) VALUES ($1, $2, 'speaker-1', '조지민', 0)",
    )
    .bind(speaker_id)
    .bind(meeting_id)
    .execute(setup_pool)
    .await
    .expect("meeting speaker should persist");
    sqlx::query(
        "INSERT INTO meeting_transcript_segments (
            id, meeting_id, speaker_id, ordinal, starts_at_milliseconds,
            ends_at_milliseconds, text, confidence
         ) VALUES ($1, $2, $3, 0, 0, 3000, '후속 일을 만들기로 했다.', 98)",
    )
    .bind(Uuid::now_v7())
    .bind(meeting_id)
    .bind(speaker_id)
    .execute(setup_pool)
    .await
    .expect("meeting transcript segment should persist");
    sqlx::query(
        "INSERT INTO meeting_decisions (
            id, meeting_id, content, rationale, source_excerpt
         ) VALUES ($1, $2, '후속 일을 만든다.', '담당자가 필요하다.', $3)",
    )
    .bind(Uuid::now_v7())
    .bind(meeting_id)
    .bind("후속 일을 만들기로 했다.")
    .execute(setup_pool)
    .await
    .expect("meeting decision should persist");
}

async fn provision_owner(database: &Database) -> ProvisionedLogin {
    let user_id = Uuid::now_v7();
    let installation_id = Uuid::now_v7();
    let device = DeviceRegistration::new(
        installation_id,
        ClientPlatform::Macos,
        "meeting deletion test Mac",
        "0.1.0-test",
        Some("test-os".to_owned()),
    )
    .expect("test device should be valid");
    let now = OffsetDateTime::now_utc();
    let session_id = Uuid::now_v7();
    let mut refresh_token_verifier = session_id.as_bytes().to_vec();
    refresh_token_verifier.extend_from_slice(session_id.as_bytes());
    database
        .provision_login(&ProvisionLogin {
            user_id,
            google_subject: GoogleSubject::parse(format!("meeting-delete-subject-{user_id}"))
                .expect("test Google subject should be valid"),
            email: EmailAddress::parse(format!("meeting-delete-{user_id}@example.test"))
                .expect("test email should be valid"),
            display_name: Some("meeting deletion test owner".to_owned()),
            device,
            session_id,
            family_id: Uuid::now_v7(),
            refresh_token_id: Uuid::now_v7(),
            refresh_token_verifier,
            session_expires_at: now + TimeDuration::days(30),
            refresh_token_expires_at: now + TimeDuration::days(30),
            request_id: Uuid::now_v7(),
        })
        .await
        .expect("test owner should exist")
}

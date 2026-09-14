use jimin_storage::{
    Database,
    scheduled_work::{ScheduledWorkDefinition, WorkDestination, WorkScope},
};
use secrecy::SecretString;
use sqlx::PgPool;
use std::{borrow::Cow, time::Duration};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

fn at(value: &str) -> OffsetDateTime {
    OffsetDateTime::parse(value, &Rfc3339).unwrap()
}
fn rule(workspace: Uuid, project: Uuid, webhook: Uuid) -> ScheduledWorkDefinition {
    ScheduledWorkDefinition {
        title: "마감 안내".into(),
        workspace_id: workspace,
        project_id: Some(project),
        webhook_id: Some(webhook),
        weekdays: vec![1, 2, 3, 4, 5],
        time: "09:00".into(),
        follow_up_time: Some("16:00".into()),
        time_zone: "Asia/Seoul".into(),
        task_scope: WorkScope::Today,
        include_overdue: true,
        assignee_names: vec![],
        destination: WorkDestination::GoogleChat,
        mention_assignees: true,
        mention_names: vec![],
        include_schedules: false,
    }
}
async fn owner(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let (user, workspace, project, webhook) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    sqlx::query("INSERT INTO users(id,google_sub,email,normalized_email,status,last_login_at) VALUES($1,$2,$2,$2,'active',NOW())").bind(user).bind(format!("{user}@example.test")).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO workspaces(id,user_id,scope,name) VALUES($1,$2,'company','테스트 회사')",
    )
    .bind(workspace)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO projects(id,user_id,workspace_id,title) VALUES($1,$2,$3,'예약 검증')")
        .bind(project)
        .bind(user)
        .bind(workspace)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO project_webhooks(id,user_id,project_id,provider,destination_ciphertext,destination_nonce,destination_hint,mention_directory,events,enabled) VALUES($1,$2,$3,'google_chat',$4,$5,'테스트 전용',$6,ARRAY['task.created'],TRUE)")
        .bind(webhook).bind(user).bind(project).bind(vec![7u8;48]).bind(vec![8u8;24]).bind(serde_json::json!({"users":{"홍길동":"users/123456789012345678901","이담당":"users/123456789012345678902"}})).execute(pool).await.unwrap();
    (user, workspace, project, webhook)
}
async fn task(pool: &PgPool, user: Uuid, project: Uuid, title: &str, due: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO tasks(id,user_id,project_id,title,due_at,assignee_name) VALUES($1,$2,$3,$4,$5,'홍길동')").bind(id).bind(user).bind(project).bind(title).bind(at(due)).execute(pool).await.unwrap();
    id
}

#[tokio::test]
#[ignore = "requires a fresh isolated JIMIN_TEST_DATABASE_URL; run explicitly with --ignored"]
#[allow(clippy::too_many_lines)] // One isolated DB exercises schema55->56 and the complete durable lifecycle.
async fn scheduled_work_upgrade_delivery_and_recovery_contract() {
    let url = std::env::var("JIMIN_TEST_DATABASE_URL")
        .expect("an isolated scheduled-work test database is required");
    let pool = PgPool::connect(&url).await.unwrap();
    let mut migrations = sqlx::migrate!("../../migrations");
    migrations.migrations = Cow::Owned(
        migrations
            .iter()
            .filter(|m| m.version <= 55)
            .cloned()
            .collect(),
    );
    migrations.run(&pool).await.unwrap();
    let db = Database::connect_lazy(&SecretString::from(url), 8, Duration::from_secs(5)).unwrap();
    let (user, workspace, project, webhook) = owner(&pool).await;
    db.migrate().await.unwrap();
    let schema: i64 = sqlx::query_scalar("SELECT schema_version FROM jimin_schema_metadata")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(schema, 56);
    let initial = at("2026-09-14T08:00:00+09:00");
    let definition = rule(workspace, project, webhook);
    let id = Uuid::now_v7();
    let first = task(
        &pool,
        user,
        project,
        "첫 할 일",
        "2026-09-14T23:59:00+09:00",
    )
    .await;
    let second = task(
        &pool,
        user,
        project,
        "둘째 할 일",
        "2026-09-14T23:59:00+09:00",
    )
    .await;
    let tomorrow = task(&pool, user, project, "내일 일", "2026-09-15T00:00:00+09:00").await;
    sqlx::query("INSERT INTO task_assignment_public_details(task_id,user_id,summary,action_items,reference_links) VALUES($1,$2,'확인할 상세 내용',ARRAY['설정 검증'],ARRAY['https://example.test/docs/task'])").bind(first).bind(user).execute(&pool).await.unwrap();
    let preview = db
        .preview_scheduled_work(user, &definition, initial)
        .await
        .unwrap();
    assert_eq!(preview.task_ids.len(), 2);
    assert!(!preview.task_ids.contains(&tomorrow));
    assert!(
        preview
            .messages
            .join("\n")
            .contains("https://example.test/docs/task")
    );
    assert!(preview.messages.join("\n").contains("@홍길동"));
    assert!(
        preview
            .messages
            .join("\n")
            .contains("2026년 9월 14일 23:59")
    );
    let (other, other_workspace, _, _) = owner(&pool).await;
    assert!(
        db.preview_scheduled_work(other, &definition, initial)
            .await
            .is_err()
    );
    let mut bad = definition.clone();
    bad.workspace_id = other_workspace;
    assert!(
        db.save_scheduled_work(user, id, &bad, true, None, initial)
            .await
            .is_err()
    );
    let created = db
        .save_scheduled_work(user, id, &definition, true, None, initial)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created.next_run_at, at("2026-09-14T09:00:00+09:00"));
    assert!(
        db.save_scheduled_work(user, id, &definition, true, Some(9), initial)
            .await
            .unwrap()
            .is_none()
    );
    // A change after registration must affect the actual execution, not the preview snapshot.
    sqlx::query("UPDATE tasks SET assignee_name='이담당' WHERE id=$1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE tasks SET status='completed',completed_at=NOW() WHERE id=$1")
        .bind(second)
        .execute(&pool)
        .await
        .unwrap();
    let clock = at("2026-09-14T09:00:05+09:00");
    let (a, b) = tokio::join!(
        db.process_due_scheduled_work(clock),
        db.process_due_scheduled_work(clock)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    let runs = db.scheduled_work_runs(user, Some(id), None).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "delivering");
    assert_eq!(runs[0].task_ids, vec![first]);
    assert!(runs[0].messages.join("\n").contains("@이담당"));
    assert!(
        db.scheduled_work_runs(other, Some(id), None)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("UPDATE webhook_deliveries SET status='delivered',delivered_at=NOW()")
        .execute(&pool)
        .await
        .unwrap();
    db.reconcile_scheduled_work_runs().await.unwrap();
    // A new same-day task is not part of the morning follow-up set.
    let late = task(
        &pool,
        user,
        project,
        "오전 안내 후 새로 들어온 일",
        "2026-09-14T23:59:00+09:00",
    )
    .await;
    assert!(
        db.process_due_scheduled_work(at("2026-09-14T16:00:01+09:00"))
            .await
            .unwrap()
    );
    let runs = db.scheduled_work_runs(user, Some(id), None).await.unwrap();
    assert_eq!(runs[0].kind, "followup");
    assert_eq!(runs[0].task_ids, vec![first]);
    assert!(!runs[0].task_ids.contains(&late));
    assert!(db.pause_scheduled_work(user, id, 1).await.unwrap());
    assert!(!db.scheduled_work_for_user(user).await.unwrap()[0].enabled);
    assert_eq!(
        db.scheduled_work_runs(user, Some(id), None).await.unwrap()[0].status,
        "skipped"
    );
    // Manual retry uses one receipt/outbox, even when the client repeats the request.
    let request = Uuid::now_v7();
    let one = db
        .run_scheduled_work_now(user, id, request, 2, clock)
        .await
        .unwrap()
        .unwrap();
    let replay = db
        .run_scheduled_work_now(user, id, request, 2, clock + time::Duration::seconds(1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(one.id, replay.id);
    let device = Uuid::now_v7();
    sqlx::query("INSERT INTO devices(id,user_id,installation_id,platform,name,app_version,status,last_seen_at) VALUES($1,$2,$1,'android','격리 검증 기기','test','active',NOW())")
        .bind(device).bind(user).execute(&pool).await.unwrap();
    db.register_push_token(
        Uuid::now_v7(),
        user,
        device,
        &jimin_storage::push::EncryptedPushToken {
            ciphertext: vec![1; 48],
            nonce: vec![2; 24],
            fingerprint: vec![3; 32],
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE webhook_deliveries SET status='failed',last_error_code='test.delivery_failed' WHERE id IN (SELECT delivery_id FROM scheduled_work_run_deliveries WHERE run_id=$1)")
        .bind(one.id).execute(&pool).await.unwrap();
    db.reconcile_scheduled_work_runs().await.unwrap();
    db.reconcile_scheduled_work_runs().await.unwrap();
    let failed = db.scheduled_work_runs(user, Some(id), None).await.unwrap();
    let failed = failed.iter().find(|run| run.id == one.id).unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.reason.as_deref(), Some("delivery_failed"));
    let notification_count: i64 = sqlx::query_scalar("SELECT count(*) FROM push_deliveries WHERE item_id=$1 AND item_type='scheduled_work' AND destination='home'")
        .bind(one.id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        notification_count, 1,
        "reconciliation must notify only once"
    );
    db.queue_due_push_reminders(OffsetDateTime::now_utc())
        .await
        .unwrap();
    let notifications = db
        .claim_push_deliveries("scheduled-work-isolated-test", 50)
        .await
        .unwrap();
    assert!(
        notifications
            .iter()
            .any(|notification| notification.item_id == one.id
                && notification.item_type == "scheduled_work"
                && notification.destination == "home")
    );
    assert!(
        db.run_scheduled_work_now(other, id, Uuid::now_v7(), 2, clock)
            .await
            .unwrap()
            .is_none()
    );
    // A missing mention must fail visibly and never silently send without it.
    sqlx::query("UPDATE tasks SET assignee_name='등록 안 된 사람' WHERE id=$1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    let missing = db
        .run_scheduled_work_now(user, id, Uuid::now_v7(), 2, clock)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(missing.status, "failed");
    assert_eq!(missing.reason.as_deref(), Some("mention_missing"));
    assert!(
        db.save_scheduled_work(user, id, &definition, true, Some(2), initial)
            .await
            .is_err()
    );
    sqlx::query("UPDATE tasks SET assignee_name='홍길동' WHERE id=$1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    db.save_scheduled_work(
        user,
        id,
        &definition,
        true,
        Some(2),
        at("2026-09-15T08:00:00+09:00"),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        db.skip_scheduled_work_once(user, id, 3, at("2026-09-15T08:00:00+09:00"))
            .await
            .unwrap()
    );
    assert_eq!(
        db.scheduled_work_for_user(user).await.unwrap()[0].next_run_at,
        at("2026-09-15T16:00:00+09:00")
    );
    // A long outage skips historical messages and advances directly to a future slot.
    assert!(
        db.process_due_scheduled_work(at("2026-09-16T14:00:00+09:00"))
            .await
            .unwrap()
    );
    assert_eq!(
        db.scheduled_work_runs(user, Some(id), None).await.unwrap()[0]
            .reason
            .as_deref(),
        Some("missed_window")
    );
    assert!(
        !db.process_due_scheduled_work(at("2026-09-16T14:00:00+09:00"))
            .await
            .unwrap()
    );
    // In-app briefs remain inspectable without a registered mobile push device.
    let mut brief = definition.clone();
    brief.destination = WorkDestination::InApp;
    brief.webhook_id = None;
    brief.mention_assignees = false;
    brief.task_scope = WorkScope::AllOpen;
    brief.follow_up_time = None;
    let bid = Uuid::now_v7();
    db.save_scheduled_work(user, bid, &brief, false, None, initial)
        .await
        .unwrap()
        .unwrap();
    let run = db
        .run_scheduled_work_now(user, bid, Uuid::now_v7(), 1, clock)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "completed");
    assert_eq!(run.task_ids.len(), 3);
    assert!(db.delete_scheduled_work(user, bid, 1).await.unwrap());
    brief.assignee_names = vec!["대상 없는 담당자".into()];
    let empty_id = Uuid::now_v7();
    db.save_scheduled_work(user, empty_id, &brief, false, None, initial)
        .await
        .unwrap()
        .unwrap();
    let empty = db
        .run_scheduled_work_now(user, empty_id, Uuid::now_v7(), 1, clock)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(empty.status, "skipped");
    assert_eq!(empty.reason.as_deref(), Some("no_matching_work"));
    // Turning off a webhook still allows pausing the schedule.
    sqlx::query("UPDATE project_webhooks SET enabled=FALSE WHERE id=$1")
        .bind(webhook)
        .execute(&pool)
        .await
        .unwrap();
    assert!(db.pause_scheduled_work(user, id, 4).await.unwrap());
    assert!(db.delete_scheduled_work(user, id, 5).await.unwrap());
}

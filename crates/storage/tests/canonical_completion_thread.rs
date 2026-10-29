//! Isolated provider-receipt contracts; no real account, message or task is touched.
use jimin_storage::{
    Database, StorageError,
    google_chat::ProviderGoogleChatMessage,
    planning::{NewTask, Task, TaskStatus, TaskUpdate},
    scheduled_chat_reply::task_code,
    scheduled_work::ScheduledWorkDefinition,
    task_chat::TaskCompletionTarget,
    webhook::ClaimedWebhookDelivery,
};
use secrecy::SecretString;
use sqlx::PgPool;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

const WORKER: &str = "canonical-contract";
const SPACE: &str = "spaces/developers";
async fn task(db: &Database, owner: Uuid, project: Uuid, title: &str) -> Task {
    db.create_task(&NewTask {
        id: Uuid::now_v7(),
        user_id: owner,
        project_id: Some(project),
        parent_task_id: None,
        title: title.into(),
        notes: None,
        assignee_name: Some("김경주".into()),
        priority: 1,
        due_at: None,
    })
    .await
    .unwrap()
}
fn find(items: &[ClaimedWebhookDelivery], task: Uuid, kind: &str) -> ClaimedWebhookDelivery {
    items
        .iter()
        .find(|d| d.payload["entityId"] == task.to_string() && d.payload["taskChatKind"] == kind)
        .unwrap()
        .clone()
}
async fn ack(db: &Database, d: &ClaimedWebhookDelivery, thread: &str) {
    assert!(
        db.complete_scheduled_reply_delivery(
            d.id,
            WORKER,
            d.attempt_count,
            &format!("{SPACE}/messages/client-jimin-{}", d.id.simple()),
            thread
        )
        .await
        .unwrap()
    );
}
async fn count_task(pool: &PgPool, task: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM task_chat_deliveries WHERE task_id=$1 AND kind='completion'",
    )
    .bind(task)
    .fetch_one(pool)
    .await
    .unwrap()
}
fn in_thread(target: &TaskCompletionTarget, assignment: Uuid, thread: &str) {
    assert_eq!(
        target,
        &TaskCompletionTarget::AssignmentThread {
            assignment_delivery_id: assignment,
            thread_name: thread.into()
        }
    );
}

#[tokio::test]
#[ignore = "requires an empty isolated JIMIN_TEST_DATABASE_URL"]
#[allow(
    clippy::too_many_lines,
    reason = "One isolated fixture covers canonical destination, races, leases and both completion entrypoints."
)]
async fn canonical_completion_destination_and_idempotency_contract() {
    let url = std::env::var("JIMIN_TEST_DATABASE_URL").expect("isolated database required");
    let pool = PgPool::connect(&url).await.unwrap();
    let db = Database::connect_lazy(&SecretString::from(url), 8, Duration::from_secs(3)).unwrap();
    db.migrate().await.unwrap();
    let (owner, workspace, project, account, webhook, source, ops) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    sqlx::query("INSERT INTO users(id,google_sub,email,normalized_email,status,last_login_at) VALUES($1,'canonical-owner','owner@example.test','owner@example.test','active',NOW())").bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workspaces(id,user_id,scope,name) VALUES($1,$2,'company','회사')")
        .bind(workspace)
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO projects(id,user_id,workspace_id,title) VALUES($1,$2,$3,'완료 위치 검증')",
    )
    .bind(project)
    .bind(owner)
    .bind(workspace)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO google_chat_accounts(id,user_id,provider_subject,email,status,granted_scopes,refresh_token_ciphertext,refresh_token_nonce,encryption_key_version) VALUES($1,$2,'789','company@example.test','active',ARRAY['https://www.googleapis.com/auth/chat.messages.readonly','https://www.googleapis.com/auth/chat.messages.create'],$3,$4,1)").bind(account).bind(owner).bind(vec![9u8;48]).bind(vec![10u8;24]).execute(&pool).await.unwrap();
    for (id, provider) in [(webhook, "google_chat"), (Uuid::now_v7(), "discord")] {
        sqlx::query("INSERT INTO project_webhooks(id,user_id,project_id,provider,destination_ciphertext,destination_nonce,destination_hint,mention_directory,events,enabled) VALUES($1,$2,$3,$4,$5,$6,'격리 연결',$7,ARRAY['task.completed'],TRUE)")
            .bind(id).bind(owner).bind(project).bind(provider).bind(vec![7u8;48]).bind(vec![8u8;24]).bind(serde_json::json!({"users":{"김경주":"users/123"}})).execute(&pool).await.unwrap();
    }
    for (id, space, purpose) in [
        (source, SPACE, "both"),
        (ops, "spaces/operations", "inflow"),
    ] {
        sqlx::query("INSERT INTO project_google_chat_sources(id,user_id,project_id,account_id,space_name,display_name,purpose) VALUES($1,$2,$3,$4,$5,'격리 대화',$6)").bind(id).bind(owner).bind(project).bind(account).bind(space).bind(purpose).execute(&pool).await.unwrap();
    }
    let legacy = task(&db, owner, project, "배정 기록이 없는 기존 일감").await;
    db.save_task_chat_route(owner, project, source, webhook, 0)
        .await
        .unwrap()
        .unwrap();
    let unrelated = task(&db, owner, project, "다른 일감").await;
    let assigned = find(
        &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
        unrelated.id,
        "assignment",
    );
    ack(&db, &assigned, &format!("{SPACE}/threads/unrelated")).await;
    db.complete_task(owner, legacy.id, legacy.version)
        .await
        .unwrap()
        .unwrap();
    let delivery = find(
        &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
        legacy.id,
        "completion",
    );
    assert_eq!(
        db.prepare_task_completion_target(delivery.id, WORKER, delivery.attempt_count)
            .await
            .unwrap(),
        TaskCompletionTarget::Space
    );
    assert!(
        db.prepare_task_completion_target(delivery.id, "wrong-worker", delivery.attempt_count)
            .await
            .is_err()
    );
    assert!(
        db.prepare_task_completion_target(delivery.id, WORKER, delivery.attempt_count + 1)
            .await
            .is_err()
    );
    ack(&db, &delivery, &format!("{SPACE}/threads/fallback")).await;
    assert_eq!(count_task(&pool, legacy.id).await, 1);

    // A delayed second assignment acknowledged before the first completion attempt is preferred.
    for (name, late_before_pin) in [("latest", true), ("pinned", false)] {
        let original = task(&db, owner, project, name).await;
        let first = find(
            &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
            original.id,
            "assignment",
        );
        let first_thread = format!("{SPACE}/threads/{name}-first");
        ack(&db, &first, &first_thread).await;
        let changed = db
            .update_task(&TaskUpdate {
                id: original.id,
                user_id: owner,
                project_id: Some(project),
                parent_task_id: None,
                title: name.into(),
                notes: None,
                assignee_name: original.assignee_name.clone(),
                status: TaskStatus::Open,
                priority: 1,
                due_at: None,
                expected_version: original.version,
            })
            .await
            .unwrap()
            .unwrap();
        db.complete_task(owner, changed.id, changed.version)
            .await
            .unwrap()
            .unwrap();
        let items = db.claim_webhook_deliveries(WORKER, 50).await.unwrap();
        let second = find(&items, changed.id, "assignment");
        let completed = find(&items, changed.id, "completion");
        let second_thread = format!("{SPACE}/threads/{name}-second");
        if late_before_pin {
            ack(&db, &second, &second_thread).await;
        }
        let chosen = db
            .prepare_task_completion_target(completed.id, WORKER, completed.attempt_count)
            .await
            .unwrap();
        in_thread(
            &chosen,
            if late_before_pin { second.id } else { first.id },
            if late_before_pin {
                &second_thread
            } else {
                &first_thread
            },
        );
        if !late_before_pin {
            ack(&db, &second, &second_thread).await;
        }
        assert_eq!(
            db.prepare_task_completion_target(completed.id, WORKER, completed.attempt_count)
                .await
                .unwrap(),
            chosen
        );
        assert!(
            db.complete_scheduled_reply_delivery(
                completed.id,
                WORKER,
                completed.attempt_count,
                &format!("{SPACE}/messages/wrong"),
                &format!("{SPACE}/threads/report-not-assignment")
            )
            .await
            .is_err()
        );
        db.fail_webhook_delivery(
            completed.id,
            WORKER,
            completed.attempt_count,
            None,
            "google_chat.provider_unavailable",
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE webhook_deliveries SET next_attempt_at=NOW()-INTERVAL '1 second' WHERE id=$1",
        )
        .bind(completed.id)
        .execute(&pool)
        .await
        .unwrap();
        let retry = find(
            &db.claim_webhook_deliveries("retry", 50).await.unwrap(),
            changed.id,
            "completion",
        );
        assert_eq!(retry.attempt_count, completed.attempt_count + 1);
        assert!(
            db.prepare_task_completion_target(completed.id, WORKER, completed.attempt_count)
                .await
                .is_err()
        );
        assert_eq!(
            db.prepare_task_completion_target(retry.id, "retry", retry.attempt_count)
                .await
                .unwrap(),
            chosen
        );
        let TaskCompletionTarget::AssignmentThread { thread_name, .. } = chosen else {
            panic!("thread expected")
        };
        assert!(
            db.complete_scheduled_reply_delivery(
                retry.id,
                "retry",
                retry.attempt_count,
                &format!("{SPACE}/messages/result-{name}"),
                &thread_name
            )
            .await
            .unwrap()
        );
        assert!(
            !db.complete_scheduled_reply_delivery(
                retry.id,
                "retry",
                retry.attempt_count,
                &format!("{SPACE}/messages/result-{name}"),
                &thread_name
            )
            .await
            .unwrap()
        );
        assert!(
            db.complete_task(owner, changed.id, changed.version)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(count_task(&pool, changed.id).await, 1);
    }
    // No acknowledged assignment at the first attempt pins room fallback, even if one arrives later.
    let late = task(&db, owner, project, "늦게 도착한 배정").await;
    db.complete_task(owner, late.id, late.version)
        .await
        .unwrap()
        .unwrap();
    let items = db.claim_webhook_deliveries(WORKER, 50).await.unwrap();
    let a = find(&items, late.id, "assignment");
    let c = find(&items, late.id, "completion");
    assert_eq!(
        db.prepare_task_completion_target(c.id, WORKER, c.attempt_count)
            .await
            .unwrap(),
        TaskCompletionTarget::Space
    );
    ack(&db, &a, &format!("{SPACE}/threads/late")).await;
    assert_eq!(
        db.prepare_task_completion_target(c.id, WORKER, c.attempt_count)
            .await
            .unwrap(),
        TaskCompletionTarget::Space
    );
    ack(&db, &c, &format!("{SPACE}/threads/late-fallback")).await;

    // An uncertain old worker may already have posted to the room. Never move that retry
    // into a thread; new-format attempts that failed before pinning may still choose one.
    for old_worker in [true, false] {
        let item = task(&db, owner, project, "배포 경계 재시도").await;
        let assignment = find(
            &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
            item.id,
            "assignment",
        );
        let thread = format!("{SPACE}/threads/rollout-{}", item.id.simple());
        ack(&db, &assignment, &thread).await;
        db.complete_task(owner, item.id, item.version)
            .await
            .unwrap()
            .unwrap();
        let completed = find(
            &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
            item.id,
            "completion",
        );
        if old_worker {
            sqlx::query("UPDATE webhook_deliveries SET payload=payload-'taskChatCompletionRoutingVersion' WHERE id=$1")
                .bind(completed.id).execute(&pool).await.unwrap();
        }
        db.fail_webhook_delivery(
            completed.id,
            WORKER,
            completed.attempt_count,
            None,
            "google_chat.provider_unavailable",
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE webhook_deliveries SET next_attempt_at=NOW()-INTERVAL '1 second' WHERE id=$1",
        )
        .bind(completed.id)
        .execute(&pool)
        .await
        .unwrap();
        let retry = find(
            &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
            item.id,
            "completion",
        );
        assert_eq!(retry.attempt_count, 2);
        let target = db
            .prepare_task_completion_target(retry.id, WORKER, retry.attempt_count)
            .await
            .unwrap();
        if old_worker {
            assert_eq!(target, TaskCompletionTarget::Space);
            ack(&db, &retry, &format!("{SPACE}/threads/old-room-message")).await;
        } else {
            in_thread(&target, assignment.id, &thread);
            ack(&db, &retry, &thread).await;
        }
        assert_eq!(count_task(&pool, item.id).await, 1);
    }

    // Reporting in a reminder thread must still finish in the task's assignment thread.
    let reviewed = task(&db, owner, project, "보고 승인 검증").await;
    let assignment = find(
        &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
        reviewed.id,
        "assignment",
    );
    let assignment_thread = format!("{SPACE}/threads/reviewed-assignment");
    ack(&db, &assignment, &assignment_thread).await;
    sqlx::query("INSERT INTO task_itsm_links(task_id,user_id,enabled,issue_url,issue_id) VALUES($1,$2,TRUE,'https://itsm.example.test/issues/9903',9903)").bind(reviewed.id).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO project_inflow_items(id,user_id,project_id,source_id,provider_message_name,provider_thread_name,content_text,received_at,status,promoted_task_id) VALUES($1,$2,$3,$4,'spaces/operations/messages/request','spaces/operations/threads/request','원본 CS',NOW(),'promoted',$5)").bind(Uuid::now_v7()).bind(owner).bind(project).bind(ops).bind(reviewed.id).execute(&pool).await.unwrap();
    let definition:ScheduledWorkDefinition=serde_json::from_value(serde_json::json!({"title":"예약 보고","workspaceId":workspace,"projectId":project,"webhookId":webhook,"replyCompletionSourceId":source,"weekdays":[1,2,3,4,5],"time":"09:00","timeZone":"Asia/Seoul","taskScope":"all_open","includeOverdue":true,"assigneeNames":[],"destination":"google_chat","mentionAssignees":true,"mentionNames":[],"includeSchedules":false})).unwrap();
    let rule = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    db.save_scheduled_work(owner, rule, &definition, true, None, now)
        .await
        .unwrap()
        .unwrap();
    db.run_scheduled_work_now(owner, rule, Uuid::now_v7(), 1, now)
        .await
        .unwrap()
        .unwrap();
    let items = db.claim_webhook_deliveries(WORKER, 50).await.unwrap();
    let reminder = items
        .iter()
        .find(|d| {
            d.payload.get("replyCompletion").is_some() && d.payload.get("taskChatKind").is_none()
        })
        .unwrap();
    let report_thread = format!("{SPACE}/threads/reminder");
    ack(&db, reminder, &report_thread).await;
    let connection = db
        .google_chat_source_sync_connection(source)
        .await
        .unwrap()
        .unwrap();
    let report = ProviderGoogleChatMessage {
        provider_message_name: format!("{SPACE}/messages/report"),
        provider_thread_name: Some(report_thread),
        sender_provider_name: Some("users/123".into()),
        sender_name: Some("김경주".into()),
        content_text: format!("{} 완료", task_code(reviewed.id)),
        received_at: OffsetDateTime::now_utc(),
    };
    db.process_scheduled_reply(&connection, &report)
        .await
        .unwrap();
    let reviews = db.completion_reviews(owner, None).await.unwrap();
    let review = reviews.iter().find(|r| r.task_id == reviewed.id).unwrap();
    assert!(matches!(
        db.complete_task(owner, reviewed.id, reviewed.version).await,
        Err(StorageError::IdentityConflict)
    ));
    let (x, y) = tokio::join!(
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            true,
            None
        ),
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            true,
            None
        )
    );
    assert!(x.unwrap() && y.unwrap());
    assert_eq!(count_task(&pool, reviewed.id).await, 1);
    let final_delivery = find(
        &db.claim_webhook_deliveries(WORKER, 50).await.unwrap(),
        reviewed.id,
        "completion",
    );
    sqlx::query("UPDATE project_google_chat_sources SET enabled=FALSE WHERE id=$1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        db.scheduled_reply_context(final_delivery.id).await,
        Err(StorageError::InvalidConfiguration)
    ));
    assert!(
        db.prepare_task_completion_target(final_delivery.id, WORKER, final_delivery.attempt_count)
            .await
            .is_err(),
        "revoked destination must not send"
    );
    sqlx::query("UPDATE project_google_chat_sources SET enabled=TRUE WHERE id=$1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE webhook_deliveries SET lease_expires_at=NOW()-INTERVAL '1 second' WHERE id=$1",
    )
    .bind(final_delivery.id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        db.prepare_task_completion_target(final_delivery.id, WORKER, final_delivery.attempt_count)
            .await
            .is_err(),
        "expired lease must not send"
    );
    sqlx::query(
        "UPDATE webhook_deliveries SET lease_expires_at=NOW()+INTERVAL '1 minute' WHERE id=$1",
    )
    .bind(final_delivery.id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        final_delivery.payload["message"],
        "보고 승인 검증 · 완료되었어요."
    );
    assert!(final_delivery.payload.get("notes").is_none());
    let target = db
        .prepare_task_completion_target(final_delivery.id, WORKER, final_delivery.attempt_count)
        .await
        .unwrap();
    in_thread(&target, assignment.id, &assignment_thread);
    // A pinned receipt cannot be swapped for another task's valid thread, even in the same room.
    let pinned: serde_json::Value = sqlx::query_scalar(
        "SELECT payload->'taskChatCompletionTarget' FROM webhook_deliveries WHERE id=$1",
    )
    .bind(final_delivery.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE webhook_deliveries SET payload=jsonb_set(payload,'{taskChatCompletionTarget}',$2) WHERE id=$1").bind(final_delivery.id).bind(serde_json::to_value(TaskCompletionTarget::AssignmentThread{assignment_delivery_id:assigned.id,thread_name:format!("{SPACE}/threads/unrelated")}).unwrap()).execute(&pool).await.unwrap();
    assert!(
        db.prepare_task_completion_target(final_delivery.id, WORKER, final_delivery.attempt_count)
            .await
            .is_err()
    );
    sqlx::query("UPDATE webhook_deliveries SET payload=jsonb_set(payload,'{taskChatCompletionTarget}',$2) WHERE id=$1").bind(final_delivery.id).bind(pinned).execute(&pool).await.unwrap();
    ack(&db, &final_delivery, &assignment_thread).await;
    let extra: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM scheduled_chat_reply_commands WHERE outcome='review_approved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(extra, 0, "no second final message in the report thread");
    let raw_google:i64=sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE event_type='task.completed' AND provider='google_chat' AND payload->>'taskChatKind' IS NULL").fetch_one(&pool).await.unwrap();
    assert_eq!(raw_google, 0);
    let discord:i64=sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE provider='discord' AND event_type='task.completed' AND payload->>'entityId'=$1").bind(reviewed.id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(discord, 1);
    let cs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM google_chat_task_completion_deliveries WHERE task_id=$1",
    )
    .bind(reviewed.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cs, 1);
    let itsm: i64 =
        sqlx::query_scalar("SELECT count(*) FROM itsm_issue_sync_jobs WHERE issue_id=9903")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(itsm, 1);
}

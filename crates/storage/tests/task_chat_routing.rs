//! No real account, message, notification or Redmine request is used here.
use jimin_storage::{
    Database, StorageError,
    google_chat::{NewProjectGoogleChatSource, ProviderGoogleChatMessage},
    planning::{NewTask, Task, TaskStatus, TaskUpdate},
    task_chat::ChatSourcePurpose,
};
use secrecy::SecretString;
use sqlx::PgPool;
use std::{borrow::Cow, time::Duration};
use time::OffsetDateTime;
use uuid::Uuid;

async fn number(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}
fn message(
    space: &str,
    id: &str,
    thread: &str,
    actor: &str,
    text: &str,
) -> ProviderGoogleChatMessage {
    ProviderGoogleChatMessage {
        provider_message_name: format!("spaces/{space}/messages/{id}"),
        provider_thread_name: Some(format!("spaces/{space}/threads/{thread}")),
        sender_provider_name: Some(actor.into()),
        sender_name: Some("이름만 같은 사람".into()),
        content_text: text.into(),
        received_at: OffsetDateTime::now_utc(),
    }
}
async fn new_task(db: &Database, user: Uuid, project: Uuid, title: &str) -> Task {
    db.create_task(&NewTask {
        id: Uuid::now_v7(),
        user_id: user,
        project_id: Some(project),
        parent_task_id: None,
        title: title.into(),
        notes: Some("직접 확인하고 수정한 배정 내용과 참고 문서 https://example.test/guide".into()),
        assignee_name: Some("김경주, 송인준".into()),
        priority: 2,
        due_at: Some(OffsetDateTime::now_utc() + time::Duration::days(1)),
    })
    .await
    .unwrap()
}
async fn deliver_assignment(db: &Database, task: Uuid, thread: &str) -> Uuid {
    let claimed = db
        .claim_webhook_deliveries("assignment-contract", 50)
        .await
        .unwrap();
    let chosen = claimed
        .iter()
        .find(|d| {
            d.payload["taskChatKind"] == "assignment"
                && d.payload["replyCompletion"]["tasks"][0]["id"] == task.to_string()
        })
        .unwrap();
    let context = db
        .scheduled_reply_context(chosen.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(context.space_name, "spaces/developers");
    assert_eq!(context.envelope().unwrap().tasks[0].id, task);
    assert_assignment_body(&chosen.payload);
    assert!(
        db.complete_scheduled_reply_delivery(
            chosen.id,
            "assignment-contract",
            chosen.attempt_count,
            "spaces/operations/messages/no",
            "spaces/operations/threads/no"
        )
        .await
        .is_err()
    );
    let canonical = format!(
        "spaces/developers/messages/client-jimin-{}",
        chosen.id.simple()
    );
    assert!(
        !db.complete_scheduled_reply_delivery(
            chosen.id,
            "wrong-worker",
            chosen.attempt_count,
            &canonical,
            thread
        )
        .await
        .unwrap()
    );
    assert!(
        db.complete_scheduled_reply_delivery(
            chosen.id,
            "assignment-contract",
            chosen.attempt_count,
            &canonical,
            thread
        )
        .await
        .unwrap()
    );
    for item in claimed.iter().filter(|d| d.id != chosen.id) {
        if item.payload["entityId"] == task.to_string() {
            assert_eq!(
                item.provider, "discord",
                "no duplicate Google Chat task webhook for this task/version"
            );
        }
        if item.payload["taskChatKind"] == "assignment" {
            assert!(
                db.complete_scheduled_reply_delivery(
                    item.id,
                    "assignment-contract",
                    item.attempt_count,
                    &format!(
                        "spaces/developers/messages/client-jimin-{}",
                        item.id.simple()
                    ),
                    &format!("spaces/developers/threads/other-{}", item.id.simple())
                )
                .await
                .unwrap()
            );
            continue;
        }
        assert!(
            db.complete_webhook_delivery(item.id, "assignment-contract", item.attempt_count, 200)
                .await
                .unwrap()
        );
    }
    chosen.id
}

fn assert_assignment_body(payload: &serde_json::Value) {
    let body = payload["message"].as_str().unwrap();
    assert!(body.contains("직접 확인하고 수정한 배정 내용"));
    assert!(!body.contains("정하지 않음"));
    for reference in payload["referenceLinks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
    {
        assert!(body.contains(reference));
    }
}

#[tokio::test]
#[ignore = "requires an empty isolated JIMIN_TEST_DATABASE_URL"]
#[allow(
    clippy::too_many_lines,
    reason = "One isolated database exercises upgrade, role separation, delivery, review and compatibility atomically."
)]
async fn separate_collection_and_general_assignment_completion_contract() {
    let url = std::env::var("JIMIN_TEST_DATABASE_URL").expect("isolated test database required");
    let pool = PgPool::connect(&url).await.unwrap();
    let mut baseline = sqlx::migrate!("../../migrations");
    baseline.migrations = Cow::Owned(
        baseline
            .iter()
            .filter(|m| m.version <= 65)
            .cloned()
            .collect(),
    );
    baseline.run(&pool).await.unwrap();
    let (owner, workspace, project, account, webhook, legacy_source, legacy_rule) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    sqlx::query("INSERT INTO users(id,google_sub,email,normalized_email,status,last_login_at) VALUES($1,'routing-owner','owner@example.test','owner@example.test','active',NOW())").bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workspaces(id,user_id,scope,name) VALUES($1,$2,'company','회사')")
        .bind(workspace)
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO projects(id,user_id,workspace_id,title) VALUES($1,$2,$3,'역할 분리 검증')",
    )
    .bind(project)
    .bind(owner)
    .bind(workspace)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO google_chat_accounts(id,user_id,provider_subject,email,status,granted_scopes,refresh_token_ciphertext,refresh_token_nonce,encryption_key_version) VALUES($1,$2,'789','company@example.test','active',ARRAY['https://www.googleapis.com/auth/chat.messages.readonly','https://www.googleapis.com/auth/chat.messages.create'],$3,$4,1)")
        .bind(account).bind(owner).bind(vec![9u8;48]).bind(vec![10u8;24]).execute(&pool).await.unwrap();
    for provider in ["google_chat", "discord"] {
        sqlx::query("INSERT INTO project_webhooks(id,user_id,project_id,provider,destination_ciphertext,destination_nonce,destination_hint,mention_directory,events,enabled) VALUES($1,$2,$3,$4,$5,$6,'격리 명단',$7,ARRAY['task.created','task.updated','task.completed','task.restored'],TRUE)")
            .bind(if provider=="google_chat"{webhook}else{Uuid::now_v7()}).bind(owner).bind(project).bind(provider).bind(vec![7u8;48]).bind(vec![8u8;24]).bind(serde_json::json!({"users":{"김경주":"users/123","송인준":"users/456"}})).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO project_google_chat_sources(id,user_id,project_id,account_id,space_name,display_name) VALUES($1,$2,$3,$4,'spaces/legacy','기존 예약방')").bind(legacy_source).bind(owner).bind(project).bind(account).execute(&pool).await.unwrap();
    let legacy_definition = serde_json::json!({"title":"기존 예약","workspaceId":workspace,"projectId":project,"webhookId":webhook,"replyCompletionSourceId":legacy_source,"weekdays":[1,2,3,4,5],"time":"09:00","followUpTime":null,"timeZone":"Asia/Seoul","taskScope":"all_open","includeOverdue":true,"assigneeNames":[],"destination":"google_chat","mentionAssignees":true,"mentionNames":[],"includeSchedules":false});
    sqlx::query("INSERT INTO scheduled_work(id,user_id,workspace_id,project_id,definition,next_run_at) VALUES($1,$2,$3,$4,$5,NOW()+INTERVAL '1 day')").bind(legacy_rule).bind(owner).bind(workspace).bind(project).bind(&legacy_definition).execute(&pool).await.unwrap();
    let db = Database::connect_lazy(&SecretString::from(url), 8, Duration::from_secs(3)).unwrap();
    db.migrate().await.unwrap();
    db.migrate().await.unwrap();
    assert_eq!(
        number(&pool, "SELECT schema_version FROM jimin_schema_metadata").await,
        66
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM _sqlx_migrations WHERE success").await,
        66
    );
    let legacy_purpose: String =
        sqlx::query_scalar("SELECT purpose FROM project_google_chat_sources WHERE id=$1")
            .bind(legacy_source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(legacy_purpose, "both");
    let unchanged: serde_json::Value =
        sqlx::query_scalar("SELECT definition FROM scheduled_work WHERE id=$1")
            .bind(legacy_rule)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unchanged, legacy_definition);
    assert!(db.task_chat_route(owner, project).await.unwrap().is_none());
    let legacy_task = new_task(&db, owner, project, "기존 웹훅 유지").await;
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM webhook_deliveries WHERE provider='google_chat'"
        )
        .await,
        1
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM task_chat_deliveries").await,
        0
    );
    sqlx::query(
        "UPDATE webhook_deliveries SET status='delivered',delivered_at=NOW(),response_code=200",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut sources = Vec::new();
    for (space, purpose, ack) in [
        ("operations", ChatSourcePurpose::Inflow, true),
        ("developers", ChatSourcePurpose::Delivery, false),
    ] {
        sources.push(
            db.create_project_google_chat_source(&NewProjectGoogleChatSource {
                id: Uuid::now_v7(),
                user_id: owner,
                project_id: project,
                account_id: account,
                space_name: format!("spaces/{space}"),
                display_name: space.into(),
                acknowledge_with_reaction: ack,
                import_history: false,
                purpose,
            })
            .await
            .unwrap(),
        );
    }
    let (operations, developers) = (&sources[0], &sources[1]);
    let ops = db
        .google_chat_source_sync_connection(operations.id)
        .await
        .unwrap()
        .unwrap();
    let dev = db
        .google_chat_source_sync_connection(developers.id)
        .await
        .unwrap()
        .unwrap();
    let ordinary = message(
        "developers",
        "ordinary",
        "discussion",
        "users/123",
        "새 기능에 대한 개발 대화입니다",
    );
    assert!(
        db.apply_google_chat_messages(&dev, &[ordinary])
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM project_inflow_items").await,
        0
    );
    let request = message(
        "operations",
        "request",
        "request",
        "users/456",
        "요청 내용을 확인하고 수정해 주세요",
    );
    assert_eq!(
        db.apply_google_chat_messages(&ops, &[request])
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM project_inflow_items").await,
        1
    );
    assert!(
        db.save_task_chat_route(owner, project, operations.id, webhook, 0)
            .await
            .is_err()
    );
    assert!(
        db.save_task_chat_route(Uuid::now_v7(), project, developers.id, webhook, 0)
            .await
            .is_err()
    );
    let route = db
        .save_task_chat_route(owner, project, developers.id, webhook, 0)
        .await
        .unwrap()
        .unwrap();
    assert!(
        db.save_task_chat_route(owner, project, developers.id, webhook, 0)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        db.update_chat_source_purpose(
            owner,
            project,
            developers.id,
            developers.version,
            ChatSourcePurpose::Inflow
        )
        .await
        .is_err()
    );
    assert!(
        db.delete_project_google_chat_source(owner, project, developers.id, developers.version)
            .await
            .is_err()
    );
    assert!(
        !db.delete_task_chat_route(owner, project, route.version + 1)
            .await
            .unwrap()
    );
    assert!(
        db.update_chat_source_purpose(
            owner,
            project,
            operations.id,
            operations.version,
            ChatSourcePurpose::Both
        )
        .await
        .unwrap()
    );
    assert!(
        !db.update_chat_source_purpose(
            owner,
            project,
            operations.id,
            operations.version,
            ChatSourcePurpose::Delivery
        )
        .await
        .unwrap()
    );

    let task = new_task(&db, owner, project, "티켓 없는 일반 배정 업무").await;
    // Match the approved-public-content boundary used by inflow promotion.
    // Private/manual notes alone are not an allowlist for document URLs.
    sqlx::query("INSERT INTO task_assignment_public_details(task_id,user_id,summary,action_items,completion_criteria,reference_links,reviewed_content) VALUES($1,$2,'자동 요약 초안',ARRAY['검증'], '회귀 확인',ARRAY['https://example.test/guide'],'직접 확인하고 수정한 배정 내용\n담당자가 확인할 상세 설명입니다.')")
        .bind(task.id).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("UPDATE webhook_deliveries SET status='failed',last_error_code='test.superseded' WHERE status='queued' AND payload->>'entityId'=$1").bind(task.id.to_string()).execute(&pool).await.unwrap();
    let task = db
        .update_task(&TaskUpdate {
            id: task.id,
            user_id: owner,
            project_id: task.project_id,
            parent_task_id: None,
            title: task.title.clone(),
            notes: task.notes.clone(),
            assignee_name: task.assignee_name.clone(),
            status: TaskStatus::Open,
            priority: task.priority,
            due_at: task.due_at,
            expected_version: task.version,
        })
        .await
        .unwrap()
        .unwrap();
    sqlx::query(
        "UPDATE project_inflow_items SET status='promoted',promoted_task_id=$1 WHERE source_id=$2",
    )
    .bind(task.id)
    .bind(operations.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO task_itsm_links(task_id,user_id,enabled,issue_url,issue_id) VALUES($1,$2,TRUE,'https://itsm.example.test/issues/4412',4412)").bind(task.id).bind(owner).execute(&pool).await.unwrap();
    let thread = "spaces/developers/threads/assignment";
    let delivery = deliver_assignment(&db, task.id, thread).await;
    let public_body: serde_json::Value =
        sqlx::query_scalar("SELECT payload FROM webhook_deliveries WHERE id=$1")
            .bind(delivery)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        public_body["referenceLinks"],
        serde_json::json!(["https://example.test/guide"])
    );
    assert!(
        public_body["message"]
            .as_str()
            .unwrap()
            .contains("https://example.test/guide")
    );
    let echo = message(
        "developers",
        &format!("client-jimin-{}", delivery.simple()),
        "assignment",
        "users/789",
        "표시가 없는 완료 알림도 수집하지 않아요",
    );
    assert!(db.is_scheduled_reply_echo(&dev, &echo).await.unwrap());
    let wrong = message("developers", "fake", "assignment", "users/999", "완료");
    assert!(db.process_scheduled_reply(&dev, &wrong).await.unwrap());
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM task_completion_reviews").await,
        0
    );
    assert!(
        !db.process_scheduled_reply(
            &dev,
            &message(
                "developers",
                "other-thread",
                "unrelated",
                "users/123",
                "완료"
            )
        )
        .await
        .unwrap()
    );
    let report = message(
        "developers",
        "done",
        "assignment",
        "users/123",
        &format!(
            "{} 완료\n회귀 테스트와 문서를 확인했습니다.",
            jimin_storage::scheduled_chat_reply::task_code(task.id)
        ),
    );
    assert!(db.process_scheduled_reply(&dev, &report).await.unwrap());
    assert!(db.process_scheduled_reply(&dev, &report).await.unwrap());
    // Explicit codes in either order use the same authorization/review path as plain completion.
    for (name, text) in [
        ("done-plain", "완료\n추가 검증 메모".into()),
        (
            "done-code-first",
            format!(
                "{} 완료\n추가 검증 메모",
                jimin_storage::scheduled_chat_reply::task_code(task.id)
            ),
        ),
        (
            "done-complete-first",
            format!(
                "완료 {}\n추가 검증 메모",
                jimin_storage::scheduled_chat_reply::task_code(task.id)
            ),
        ),
    ] {
        let explicit = message("developers", name, "assignment", "users/123", &text);
        assert!(db.process_scheduled_reply(&dev, &explicit).await.unwrap());
        assert!(db.process_scheduled_reply(&dev, &explicit).await.unwrap());
        let outcome: String = sqlx::query_scalar("SELECT outcome FROM scheduled_chat_reply_commands WHERE source_id=$1 AND provider_message_name=$2")
            .bind(developers.id).bind(&explicit.provider_message_name).fetch_one(&pool).await.unwrap();
        assert_eq!(outcome, "already_pending");
    }
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM task_completion_reviews WHERE status='pending'"
        )
        .await,
        1
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM tasks WHERE status='completed'").await,
        0
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM itsm_issue_sync_jobs").await,
        0
    );
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM google_chat_task_completion_deliveries"
        )
        .await,
        0
    );
    assert!(matches!(
        db.complete_task(owner, task.id, task.version).await,
        Err(StorageError::IdentityConflict)
    ));
    let reviews = db.completion_reviews(owner, None).await.unwrap();
    let review = reviews.first().unwrap();
    assert_eq!(
        review.completion_note.as_deref(),
        Some("회귀 테스트와 문서를 확인했습니다.")
    );
    assert!(
        !db.decide_completion_review(
            Uuid::now_v7(),
            review.id,
            review.version,
            review.task_version,
            true,
            None
        )
        .await
        .unwrap()
    );
    assert!(
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            true,
            None
        )
        .await
        .unwrap()
    );
    assert!(
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            true,
            None
        )
        .await
        .unwrap()
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM tasks WHERE status='completed'").await,
        1
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM itsm_issue_sync_jobs").await,
        1
    );
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM google_chat_task_completion_deliveries"
        )
        .await,
        1
    );
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM task_chat_deliveries WHERE kind='completion'"
        )
        .await,
        1,
        "approval uses the same single completion outbox as direct completion"
    );
    let final_text: String = sqlx::query_scalar(
        "SELECT payload->>'message' FROM webhook_deliveries WHERE payload->>'taskChatKind'='completion'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_text, "티켓 없는 일반 배정 업무 · 완료되었어요.");
    let cs_source: Uuid =
        sqlx::query_scalar("SELECT source_id FROM google_chat_task_completion_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cs_source, operations.id);
    let duplicates:i64=sqlx::query_scalar("SELECT COUNT(*) FROM webhook_deliveries WHERE provider='google_chat' AND event_type='task.completed' AND payload->>'taskChatKind' IS NULL").fetch_one(&pool).await.unwrap();
    assert_eq!(duplicates, 0);

    let rework = new_task(&db, owner, project, "재작업 확인").await;
    deliver_assignment(&db, rework.id, "spaces/developers/threads/rework").await;
    assert!(
        db.process_scheduled_reply(
            &dev,
            &message(
                "developers",
                "rework-done",
                "rework",
                "users/456",
                "완료\n확인했습니다"
            )
        )
        .await
        .unwrap()
    );
    let pending = db.completion_reviews(owner, None).await.unwrap();
    let r = &pending[0];
    assert!(
        db.decide_completion_review(
            owner,
            r.id,
            r.version,
            r.task_version,
            false,
            Some("모바일에서도 확인해 주세요")
        )
        .await
        .unwrap()
    );
    assert_eq!(
        db.task_for_user(owner, rework.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Open
    );
    assert_eq!(number(&pool,"SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='review_rejected' AND feedback_text LIKE '%모바일에서도 확인해 주세요%'" ).await,1);

    let stale = new_task(&db, owner, project, "변경 전 배정").await;
    deliver_assignment(&db, stale.id, "spaces/developers/threads/stale").await;
    let changed = db
        .update_task(&TaskUpdate {
            id: stale.id,
            user_id: owner,
            project_id: stale.project_id,
            parent_task_id: None,
            title: stale.title.clone(),
            notes: stale.notes.clone(),
            assignee_name: Some("송인준".into()),
            status: TaskStatus::Open,
            priority: stale.priority,
            due_at: stale.due_at,
            expected_version: stale.version,
        })
        .await
        .unwrap()
        .unwrap();
    assert!(changed.version > stale.version);
    assert!(
        db.process_scheduled_reply(
            &dev,
            &message("developers", "stale-done", "stale", "users/456", "완료")
        )
        .await
        .unwrap()
    );
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='stale'"
        )
        .await,
        1
    );
    assert_eq!(
        number(
            &pool,
            "SELECT COUNT(*) FROM task_completion_reviews WHERE status='pending'"
        )
        .await,
        0
    );

    // Directory reuse is independent from its legacy webhook's enabled flag.
    sqlx::query("UPDATE project_webhooks SET enabled=FALSE WHERE id=$1")
        .bind(webhook)
        .execute(&pool)
        .await
        .unwrap();
    let directory_version: i64 =
        sqlx::query_scalar("SELECT version FROM project_webhooks WHERE id=$1")
            .bind(webhook)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        !db.delete_project_webhook(owner, project, webhook, directory_version)
            .await
            .unwrap()
    );
    assert!(
        !db.user_task_chat_directories(owner).await.unwrap()[0]
            .1
            .users
            .is_empty()
    );
    let manual = new_task(&db, owner, project, "앱에서 직접 완료").await;
    deliver_assignment(&db, manual.id, "spaces/developers/threads/manual").await;
    let closed = db
        .complete_task(owner, manual.id, manual.version)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(closed.status, TaskStatus::Completed);
    let deliveries = db
        .claim_webhook_deliveries("completion-contract", 50)
        .await
        .unwrap();
    let notice = deliveries
        .iter()
        .find(|d| d.payload["taskChatKind"] == "completion")
        .unwrap();
    assert_eq!(
        notice.payload["message"],
        "앱에서 직접 완료 · 완료되었어요."
    );
    assert!(
        !notice.payload["message"]
            .as_str()
            .unwrap()
            .contains("직접 확인하고 수정한 배정 내용")
    );
    assert_eq!(
        db.scheduled_reply_context(notice.id)
            .await
            .unwrap()
            .unwrap()
            .space_name,
        "spaces/developers"
    );
    let target = db
        .prepare_task_completion_target(notice.id, "completion-contract", notice.attempt_count)
        .await
        .unwrap();
    assert!(
        matches!(target,jimin_storage::task_chat::TaskCompletionTarget::AssignmentThread{ref thread_name,..} if thread_name=="spaces/developers/threads/manual")
    );
    assert!(
        db.complete_scheduled_reply_delivery(
            notice.id,
            "completion-contract",
            notice.attempt_count,
            "spaces/developers/messages/completed",
            "spaces/developers/threads/manual"
        )
        .await
        .unwrap()
    );
    assert!(
        db.delete_task_chat_route(owner, project, route.version)
            .await
            .unwrap()
    );
    assert!(db.task_chat_route(owner, project).await.unwrap().is_none());
    sqlx::query("UPDATE project_webhooks SET enabled=TRUE WHERE id=$1")
        .bind(webhook)
        .execute(&pool)
        .await
        .unwrap();
    let fallback = new_task(&db, owner, project, "명시적 연결 해제 후 기존 웹훅").await;
    let fallback_google:i64=sqlx::query_scalar("SELECT COUNT(*) FROM webhook_deliveries WHERE provider='google_chat' AND payload->>'entityId'=$1 AND payload->>'taskChatKind' IS NULL").bind(fallback.id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(fallback_google, 1);
    let new_route = db
        .save_task_chat_route(owner, project, developers.id, webhook, 0)
        .await
        .unwrap()
        .unwrap();
    assert!(
        new_route.version > route.version,
        "unlink/relink cannot recycle a stale configuration version"
    );
    assert!(
        !db.delete_task_chat_route(owner, project, route.version)
            .await
            .unwrap()
    );
    assert!(
        db.task_for_user(owner, legacy_task.id)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        number(&pool, "SELECT COUNT(*) FROM project_inflow_items").await,
        1,
        "developer conversations never pollute the work inbox"
    );
}

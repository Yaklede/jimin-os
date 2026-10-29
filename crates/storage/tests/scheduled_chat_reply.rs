use jimin_storage::{
    Database,
    google_chat::{GoogleChatSourceSyncConnection, ProviderGoogleChatMessage},
    scheduled_chat_reply::{delivery_tag, task_code},
    scheduled_work::ScheduledWorkDefinition,
};
use secrecy::SecretString;
use sqlx::PgPool;
use std::{borrow::Cow, time::Duration};
use time::OffsetDateTime;
use uuid::Uuid;

fn reply(name: &str, thread: &str, actor: Option<&str>, text: &str) -> ProviderGoogleChatMessage {
    ProviderGoogleChatMessage {
        provider_message_name: format!("spaces/test/messages/{name}"),
        provider_thread_name: Some(thread.into()),
        sender_provider_name: actor.map(str::to_owned),
        sender_name: Some("같은 표시 이름".into()),
        content_text: text.into(),
        received_at: OffsetDateTime::now_utc(),
    }
}
async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}
async fn delivered(db: &Database, rule: Uuid, user: Uuid, version: i64, thread: &str) -> Uuid {
    let run = db
        .run_scheduled_work_now(
            user,
            rule,
            Uuid::now_v7(),
            version,
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(!run.task_ids.is_empty());
    let items = db
        .claim_webhook_deliveries("reminder-test", 10)
        .await
        .unwrap();
    let delivery = items
        .iter()
        .find(|d| {
            d.payload.get("replyCompletion").is_some()
                && d.payload["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("T-"))
        })
        .unwrap();
    let context = db
        .scheduled_reply_context(delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(context.space_name, "spaces/test");
    assert!(
        !db.complete_scheduled_reply_delivery(
            delivery.id,
            "wrong-worker",
            delivery.attempt_count,
            "spaces/test/messages/reminder",
            thread
        )
        .await
        .unwrap()
    );
    assert!(
        db.complete_scheduled_reply_delivery(
            delivery.id,
            "reminder-test",
            delivery.attempt_count,
            &format!("spaces/test/messages/{}", delivery.id.simple()),
            thread
        )
        .await
        .unwrap()
    );
    delivery.id
}

#[tokio::test]
#[ignore = "requires an empty isolated JIMIN_TEST_DATABASE_URL"]
#[allow(
    clippy::too_many_lines,
    reason = "One isolated database verifies the complete upgrade, delivery, command and recovery contract."
)]
async fn company_oauth_reply_completion_contract() {
    let url = std::env::var("JIMIN_TEST_DATABASE_URL").expect("isolated database required");
    let pool = PgPool::connect(&url).await.unwrap();
    let mut baseline = sqlx::migrate!("../../migrations");
    baseline.migrations = Cow::Owned(
        baseline
            .iter()
            .filter(|m| m.version <= 63)
            .cloned()
            .collect(),
    );
    baseline.run(&pool).await.unwrap();
    let db = Database::connect_lazy(&SecretString::from(url), 8, Duration::from_secs(3)).unwrap();
    db.migrate().await.unwrap();
    let (owner, workspace, project, webhook, source, account, task, rule) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    sqlx::query("INSERT INTO users(id,google_sub,email,normalized_email,status,last_login_at) VALUES($1,'owner','owner@example.test','owner@example.test','active',NOW())").bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workspaces(id,user_id,scope,name) VALUES($1,$2,'company','회사')")
        .bind(workspace)
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO projects(id,user_id,workspace_id,title) VALUES($1,$2,$3,'답글 완료')")
        .bind(project)
        .bind(owner)
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO project_webhooks(id,user_id,project_id,provider,destination_ciphertext,destination_nonce,destination_hint,mention_directory,events,enabled) VALUES($1,$2,$3,'google_chat',$4,$5,'테스트 연결',$6,ARRAY['task.completed'],TRUE)")
        .bind(webhook).bind(owner).bind(project).bind(vec![7u8;48]).bind(vec![8u8;24]).bind(serde_json::json!({"users":{"김경주":"users/123","송인준":"users/456"}})).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO google_chat_accounts(id,user_id,provider_subject,email,status,granted_scopes,refresh_token_ciphertext,refresh_token_nonce,encryption_key_version) VALUES($1,$2,'789','company@example.test','active',ARRAY['https://www.googleapis.com/auth/chat.messages.readonly','https://www.googleapis.com/auth/chat.messages.create'],$3,$4,1)")
        .bind(account).bind(owner).bind(vec![9u8;48]).bind(vec![10u8;24]).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO project_google_chat_sources(id,user_id,project_id,account_id,space_name,display_name,purpose) VALUES($1,$2,$3,$4,'spaces/test','회사 CS','both')").bind(source).bind(owner).bind(project).bind(account).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tasks(id,user_id,project_id,title,assignee_name) VALUES($1,$2,$3,'티켓 없는 업무 확인','김경주, 송인준')").bind(task).bind(owner).bind(project).execute(&pool).await.unwrap();
    let device = Uuid::now_v7();
    sqlx::query("INSERT INTO devices(id,user_id,installation_id,platform,name,app_version,status,last_seen_at) VALUES($1,$2,$1,'android','격리 검증 기기','test','active',NOW())")
        .bind(device).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO push_registrations(id,user_id,device_id,token_ciphertext,token_nonce,token_fingerprint) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::now_v7()).bind(owner).bind(device).bind(vec![1u8;48]).bind(vec![2u8;24]).bind(vec![3u8;32]).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO project_inflow_items(id,user_id,project_id,source_id,provider_message_name,provider_thread_name,content_text,received_at,status,promoted_task_id) VALUES($1,$2,$3,$4,'spaces/test/messages/request','spaces/test/threads/request','원본 업무',NOW(),'promoted',$5)")
        .bind(Uuid::now_v7()).bind(owner).bind(project).bind(source).bind(task).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO task_itsm_links(task_id,user_id,enabled,issue_url,issue_id) VALUES($1,$2,TRUE,'https://itsm.example.test/issues/4390',4390)").bind(task).bind(owner).execute(&pool).await.unwrap();
    let definition:ScheduledWorkDefinition=serde_json::from_value(serde_json::json!({"title":"오늘 업무 안내","workspaceId":workspace,"projectId":project,"webhookId":webhook,"replyCompletionSourceId":source,"weekdays":[1,2,3,4,5],"time":"09:00","followUpTime":null,"timeZone":"Asia/Seoul","taskScope":"all_open","includeOverdue":true,"assigneeNames":[],"destination":"google_chat","mentionAssignees":true,"mentionNames":[],"includeSchedules":false})).unwrap();
    let now = OffsetDateTime::now_utc();
    let code = task_code(task);
    let preview = db
        .preview_scheduled_work(owner, &definition, now)
        .await
        .unwrap();
    assert!(preview.messages.join("\n").contains(&code));
    assert!(preview.messages.join("\n").contains("다음 동기화"));
    let serialized = serde_json::to_value(&preview).unwrap();
    assert!(serialized.get("replyTasks").is_none());
    let mut legacy = definition.clone();
    legacy.reply_completion_source_id = None;
    assert!(
        !db.preview_scheduled_work(owner, &legacy, now)
            .await
            .unwrap()
            .messages
            .join("\n")
            .contains(&code)
    );
    db.save_scheduled_work(owner, rule, &definition, true, None, now)
        .await
        .unwrap()
        .unwrap();
    let thread = "spaces/test/threads/reminder1";
    let id = delivered(&db, rule, owner, 1, thread).await;
    let connection: GoogleChatSourceSyncConnection = db
        .google_chat_source_sync_connection(source)
        .await
        .unwrap()
        .unwrap();
    let echo = reply(
        "self",
        thread,
        Some("users/789"),
        &format!("{}\n今日 안내", delivery_tag(id)),
    );
    assert!(
        db.is_scheduled_reply_echo(&connection, &echo)
            .await
            .unwrap()
    );
    let fake_echo = reply("self2", thread, Some("users/123"), &echo.content_text);
    assert!(
        !db.is_scheduled_reply_echo(&connection, &fake_echo)
            .await
            .unwrap()
    );
    assert!(
        !db.process_scheduled_reply(
            &connection,
            &reply(
                "foreign",
                "spaces/test/threads/other",
                Some("users/123"),
                &format!("완료 {code}")
            )
        )
        .await
        .unwrap()
    );
    assert!(
        db.process_scheduled_reply(
            &connection,
            &reply("chatter", thread, Some("users/123"), "완료했습니다")
        )
        .await
        .unwrap()
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM scheduled_chat_reply_commands").await,
        0
    );
    for (name, actor, text) in [
        ("wrong-person", Some("users/999"), format!("완료 {code}")),
        (
            "wrong-person-reversed",
            Some("users/999"),
            format!("{code} 완료"),
        ),
        ("no-sender", None, format!("완료 {code}")),
        ("no-sender-reversed", None, format!("{code} 완료")),
        ("invalid", Some("users/123"), "완료 T-XYZ".into()),
        ("invalid-reversed", Some("users/123"), "T-XYZ 완료".into()),
        ("unknown", Some("users/123"), "완료 T-000000000000".into()),
    ] {
        assert!(
            db.process_scheduled_reply(&connection, &reply(name, thread, actor, &text))
                .await
                .unwrap()
        );
    }
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM tasks WHERE status='completed'").await,
        0
    );
    sqlx::query("UPDATE tasks SET title='변경된 업무' WHERE id=$1")
        .bind(task)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        db.process_scheduled_reply(
            &connection,
            &reply("stale", thread, Some("users/123"), &format!("완료 {code}"))
        )
        .await
        .unwrap()
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='stale'"
        )
        .await,
        1
    );
    let thread2 = "spaces/test/threads/reminder2";
    delivered(&db, rule, owner, 1, thread2).await;
    let child = Uuid::now_v7();
    sqlx::query("INSERT INTO tasks(id,user_id,project_id,parent_task_id,title) VALUES($1,$2,$3,$4,'진행 중인 하위 일')").bind(child).bind(owner).bind(project).bind(task).execute(&pool).await.unwrap();
    assert!(
        db.process_scheduled_reply(
            &connection,
            &reply(
                "blocked-child",
                thread2,
                Some("users/123"),
                &format!("완료 {code}")
            )
        )
        .await
        .unwrap()
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='blocked'"
        )
        .await,
        1
    );
    sqlx::query("DELETE FROM tasks WHERE id=$1")
        .bind(child)
        .execute(&pool)
        .await
        .unwrap();
    let one = reply(
        "complete1",
        thread2,
        Some("users/123"),
        &format!("완료 {code}\n검증 후 완료했습니다."),
    );
    let two = reply(
        "complete2",
        thread2,
        Some("users/456"),
        &format!("{code} 완료\n검증 후 완료했습니다."),
    );
    sqlx::query("INSERT INTO project_inflow_items(id,user_id,project_id,source_id,provider_message_name,provider_thread_name,content_text,received_at,status) VALUES($1,$2,$3,$4,$5,$6,$7,NOW(),'pending')")
        .bind(Uuid::now_v7()).bind(owner).bind(project).bind(source).bind(&one.provider_message_name).bind(thread2).bind(&one.content_text).execute(&pool).await.unwrap();
    let (a, b) = tokio::join!(
        db.process_scheduled_reply(&connection, &one),
        db.process_scheduled_reply(&connection, &two)
    );
    assert!(a.unwrap() && b.unwrap());
    let imported: String = sqlx::query_scalar(
        "SELECT status FROM project_inflow_items WHERE source_id=$1 AND provider_message_name=$2",
    )
    .bind(source)
    .bind(&one.provider_message_name)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(imported, "dismissed");
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='review_pending'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='already_pending'"
        )
        .await,
        1
    );
    assert!(db.process_scheduled_reply(&connection, &one).await.unwrap());
    assert!(db.process_scheduled_reply(&connection, &two).await.unwrap());
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='not_assigned'"
        )
        .await,
        4
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='invalid'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='review_pending'"
        )
        .await,
        1
    );
    // Reporting is not completion. Neither webhook, source CS nor Redmine may run yet.
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM tasks WHERE status='completed'").await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM webhook_deliveries WHERE event_type='task.completed'"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM google_chat_task_completion_deliveries"
        )
        .await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM itsm_issue_sync_jobs").await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM push_deliveries WHERE item_type='task_completion_review'"
        )
        .await,
        1
    );
    let reviews = db.completion_reviews(owner, None).await.unwrap();
    assert_eq!(reviews.len(), 1);
    let review = &reviews[0];
    assert_eq!(
        review.completion_note.as_deref(),
        Some("검증 후 완료했습니다.")
    );
    assert!(
        db.completion_reviews(Uuid::now_v7(), None)
            .await
            .unwrap()
            .is_empty()
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
    assert!(matches!(
        db.complete_task_with_note(owner, task, review.task_version, None)
            .await,
        Err(jimin_storage::StorageError::IdentityConflict)
    ));
    // The assistant's direct SQL mutations must not bypass owner review either.
    let (conversation, input, job) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    sqlx::query("INSERT INTO conversations(id,user_id,title) VALUES($1,$2,'완료 우회 차단 검증')")
        .bind(conversation)
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO messages(id,conversation_id,role,content,status) VALUES($1,$2,'user','완료해 줘','completed')").bind(input).bind(conversation).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO agent_jobs(id,user_id,conversation_id,input_message_id,state,claim_owner) VALUES($1,$2,$3,$4,'running','review-test')")
        .bind(job).bind(owner).bind(conversation).bind(input).execute(&pool).await.unwrap();
    for action in [
        jimin_storage::agent::AgentActionCommand::SetTaskStatus {
            id: task,
            status: jimin_storage::planning::TaskStatus::Completed,
            expected_version: review.task_version,
        },
        jimin_storage::agent::AgentActionCommand::UpdateTask {
            id: task,
            project_id: Some(project),
            parent_task_id: None,
            title: "변경된 업무".into(),
            notes: None,
            assignee_name: Some("김경주, 송인준".into()),
            priority: 1,
            due_at: None,
            status: jimin_storage::planning::TaskStatus::Completed,
            expected_status: jimin_storage::planning::TaskStatus::Open,
            expected_version: review.task_version,
        },
    ] {
        let outcome = db
            .complete_agent_job_with_action(
                job,
                "review-test",
                Uuid::now_v7(),
                "완료했어요.",
                None,
                &action,
            )
            .await;
        match action {
            jimin_storage::agent::AgentActionCommand::SetTaskStatus { .. } => assert!(
                matches!(outcome, Err(jimin_storage::StorageError::IdentityConflict)),
                "{outcome:?}"
            ),
            _ => assert!(
                matches!(
                    outcome,
                    Err(jimin_storage::StorageError::InvalidConfiguration)
                ),
                "{outcome:?}"
            ),
        }
    }
    let feed = db
        .inflow_notifications(owner, 0, Uuid::nil(), OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert_eq!(
        feed.iter()
            .filter(|n| n.item_type == "task_completion_review")
            .count(),
        1
    );
    let push = db
        .claim_push_deliveries("review-notification", 10)
        .await
        .unwrap();
    assert_eq!(push.len(), 1);
    assert_eq!(push[0].item_type, "task_completion_review");
    assert_eq!(push[0].destination, "home");
    db.complete_push_delivery(
        push[0].id,
        "review-notification",
        push[0].attempt_count,
        200,
    )
    .await
    .unwrap();
    // An edited task requires a fresh, explicit owner review, not a stale approval.
    sqlx::query("UPDATE tasks SET title='검토한 업무' WHERE id=$1")
        .bind(task)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !db.decide_completion_review(
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
    let fresh = db.completion_reviews(owner, None).await.unwrap();
    let fresh = &fresh[0];
    let (approved_a, approved_b) = tokio::join!(
        db.decide_completion_review(
            owner,
            fresh.id,
            fresh.version,
            fresh.task_version,
            true,
            None
        ),
        db.decide_completion_review(
            owner,
            fresh.id,
            fresh.version,
            fresh.task_version,
            true,
            None
        )
    );
    assert!(approved_a.unwrap() && approved_b.unwrap());
    assert!(db.completion_reviews(owner, None).await.unwrap().is_empty());
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM scheduled_chat_reply_commands WHERE outcome='review_approved'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM webhook_deliveries WHERE event_type='task.completed'"
        )
        .await,
        1
    );
    let webhook_payload: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM webhook_deliveries WHERE event_type='task.completed' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(webhook_payload["message"], "검토한 업무 · 완료되었어요.");
    assert!(webhook_payload.get("notes").is_none());
    assert!(webhook_payload.get("actionItems").is_none());
    assert_eq!(count(&pool,"SELECT COUNT(*) FROM google_chat_task_completion_deliveries WHERE completion_note='검증 후 완료했습니다.'").await,1);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM itsm_issue_sync_jobs WHERE issue_id=4390"
        )
        .await,
        1
    );
    let note: Option<String> = sqlx::query_scalar("SELECT completion_note FROM tasks WHERE id=$1")
        .bind(task)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(note.as_deref(), Some("검증 후 완료했습니다."));
    db.advance_scheduled_reply_source_cursor(source, owner, one.received_at)
        .await
        .unwrap();
    let cursor: Option<OffsetDateTime> = sqlx::query_scalar(
        "SELECT last_provider_message_at FROM project_google_chat_sources WHERE id=$1",
    )
    .bind(source)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(cursor.is_some());
    let catch_up = db.scheduled_reply_catch_up(source, owner).await.unwrap();
    assert_eq!(catch_up.len(), 2);
    // Catch-up is independent of a newer general space cursor; it commits only explicitly.
    assert_eq!(
        db.scheduled_reply_catch_up(source, owner)
            .await
            .unwrap()
            .len(),
        2
    );
    db.finish_scheduled_reply_catch_up(catch_up[0].id, owner)
        .await
        .unwrap();
    assert_eq!(
        db.scheduled_reply_catch_up(source, owner)
            .await
            .unwrap()
            .len(),
        1
    );
    // Drain every fixture batch before testing retries; the worker leases at most ten at once.
    let mut feedback = Vec::new();
    loop {
        let batch = db
            .claim_scheduled_reply_feedback(source, "feedback1")
            .await
            .unwrap();
        let more = batch.len() == 10;
        feedback.extend(batch);
        if !more {
            break;
        }
    }
    assert!(!feedback.is_empty());
    let first = &feedback[0];
    assert!(
        !db.record_scheduled_reply_feedback(first.id, "wrong-worker", first.attempt_count, None)
            .await
            .unwrap()
    );
    assert!(
        db.record_scheduled_reply_feedback(first.id, "feedback1", first.attempt_count, None)
            .await
            .unwrap()
    );
    for item in &feedback[1..] {
        db.record_scheduled_reply_feedback(
            item.id,
            "feedback1",
            item.attempt_count,
            Some("google_chat.provider_unavailable"),
        )
        .await
        .unwrap();
    }
    sqlx::query("UPDATE scheduled_chat_reply_commands SET next_attempt_at=NOW()-INTERVAL '1 second' WHERE status='retry_wait'").execute(&pool).await.unwrap();
    let retry = db
        .claim_scheduled_reply_feedback(source, "feedback2")
        .await
        .unwrap();
    assert!(retry.iter().all(|r| r.attempt_count == 2));
    if let Some(item) = retry.first() {
        sqlx::query("UPDATE scheduled_chat_reply_commands SET lease_expires_at=NOW()-INTERVAL '1 second' WHERE id=$1").bind(item.id).execute(&pool).await.unwrap();
        let reclaimed = db
            .claim_scheduled_reply_feedback(source, "feedback3")
            .await
            .unwrap();
        assert!(
            reclaimed
                .iter()
                .any(|r| r.id == item.id && r.attempt_count == 3)
        );
        assert!(
            !db.record_scheduled_reply_feedback(item.id, "feedback2", 2, None)
                .await
                .unwrap()
        );
    }
    sqlx::query(
        "UPDATE tasks SET status='open',completed_at=NULL,completion_note=NULL WHERE id=$1",
    )
    .bind(task)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        db.process_scheduled_reply(
            &connection,
            &reply(
                "reopened",
                thread2,
                Some("users/123"),
                &format!("완료 {code}")
            )
        )
        .await
        .unwrap()
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM tasks WHERE status='completed'").await,
        0
    );
    let thread3 = "spaces/test/threads/reminder3";
    delivered(&db, rule, owner, 1, thread3).await;
    assert!(
        db.process_scheduled_reply(
            &connection,
            &reply(
                "no-note",
                thread3,
                Some("users/123"),
                &format!("완료 {code}")
            )
        )
        .await
        .unwrap()
    );
    let note: Option<String> = sqlx::query_scalar("SELECT completion_note FROM tasks WHERE id=$1")
        .bind(task)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(note.is_none());
    let review = db.completion_reviews(owner, None).await.unwrap().remove(0);
    assert!(
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            false,
            None
        )
        .await
        .is_err()
    );
    assert!(
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            false,
            Some("테스트 결과를 다시 확인해 주세요.")
        )
        .await
        .unwrap()
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM tasks WHERE status='completed'").await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM task_completion_reviews WHERE status='rejected'"
        )
        .await,
        1
    );
    assert!(
        !db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            false,
            Some("다른 사유로 조용히 덮어쓰지 않기")
        )
        .await
        .unwrap()
    );
    assert!(
        db.process_scheduled_reply(
            &connection,
            &reply(
                "retry-after-rework",
                thread3,
                Some("users/123"),
                &format!("완료 {code}")
            )
        )
        .await
        .unwrap()
    );
    let review = db.completion_reviews(owner, None).await.unwrap().remove(0);
    mixed_and_batch_reports(&db, &pool, owner, project, source, rule, &connection).await;
    sqlx::query("DELETE FROM project_google_chat_sources WHERE id=$1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(db.completion_reviews(owner, None).await.unwrap().len(), 1);
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
    sqlx::query("UPDATE project_google_chat_sources SET enabled=FALSE WHERE id=$1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    assert!(db.scheduled_reply_context(id).await.unwrap().is_none());
    db.migrate().await.unwrap();
}

#[allow(
    clippy::too_many_lines,
    reason = "Exercise mixed status, concurrent batch authorization and independent review effects in the same isolated fixture."
)]
async fn mixed_and_batch_reports(
    db: &Database,
    pool: &PgPool,
    owner: Uuid,
    project: Uuid,
    source: Uuid,
    rule: Uuid,
    connection: &GoogleChatSourceSyncConnection,
) {
    let (waiting, done_a, done_b, other_assignee, stale) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    for (task, name, assignee) in [
        (waiting, "업체 확인 대기", "김경주"),
        (done_a, "완료 첫 번째", "김경주"),
        (done_b, "완료 두 번째", "김경주"),
        (other_assignee, "다른 담당자 업무", "송인준"),
        (stale, "수정 전 업무", "김경주"),
    ] {
        sqlx::query(
            "INSERT INTO tasks(id,user_id,project_id,title,assignee_name) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(task)
        .bind(owner)
        .bind(project)
        .bind(name)
        .bind(assignee)
        .execute(pool)
        .await
        .unwrap();
    }
    for (task, issue) in [(done_a, 9901_i64), (done_b, 9902)] {
        sqlx::query("INSERT INTO task_itsm_links(task_id,user_id,enabled,issue_url,issue_id) VALUES($1,$2,TRUE,$3,$4)")
            .bind(task).bind(owner).bind(format!("https://itsm.example.test/issues/{issue}")).bind(issue).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO project_inflow_items(id,user_id,project_id,source_id,provider_message_name,provider_thread_name,content_text,received_at,status,promoted_task_id) VALUES($1,$2,$3,$4,$5,$6,'배치 보고 원문',NOW(),'promoted',$7)")
            .bind(Uuid::now_v7()).bind(owner).bind(project).bind(source).bind(format!("spaces/test/messages/request-{issue}")).bind(format!("spaces/test/threads/request-{issue}")).bind(task).execute(pool).await.unwrap();
    }
    let (wait_code, a, b, other, old) = (
        task_code(waiting),
        task_code(done_a),
        task_code(done_b),
        task_code(other_assignee),
        task_code(stale),
    );
    let thread = "spaces/test/threads/mixed-report";
    delivered(db, rule, owner, 1, thread).await;
    let mixed = reply(
        "mixed-report",
        thread,
        Some("users/123"),
        &format!(
            "**업체 확인 대기중**\n- [{wait_code}] 업체 확인이 필요합니다\n**금일 배포**\n- 완료 : [{a}] 첫 번째 일감 검증"
        ),
    );
    assert!(
        db.process_scheduled_reply(connection, &mixed)
            .await
            .unwrap()
    );
    let reviews = db.completion_reviews(owner, None).await.unwrap();
    assert!(!reviews.iter().any(|r| r.task_id == waiting));
    let review = reviews.iter().find(|r| r.task_id == done_a).unwrap();
    assert_eq!(review.completion_note.as_deref(), Some("첫 번째 일감 검증"));
    assert!(!reviews.iter().any(|r| r.task_id == done_b));
    assert!(
        db.decide_completion_review(
            owner,
            review.id,
            review.version,
            review.task_version,
            false,
            Some("추가 검증 후 다시 보고해 주세요")
        )
        .await
        .unwrap()
    );

    let thread = "spaces/test/threads/batch-report";
    delivered(db, rule, owner, 1, thread).await;
    sqlx::query("UPDATE tasks SET title='배정 후 바뀐 업무' WHERE id=$1")
        .bind(stale)
        .execute(pool)
        .await
        .unwrap();
    let first = reply(
        "batch-report1",
        thread,
        Some("users/123"),
        &format!(
            "**업체 확인 대기중**\n- [{wait_code}] 업체 확인\n**금일 배포**\n- 완료 : [{b}] 두 번째 검증\n- 완료 : [{a}] 첫 번째 검증\n- 완료 : [{other}] 다른 담당자\n- 완료 : [{old}] 변경 전 배정"
        ),
    );
    let second = reply(
        "batch-report2",
        thread,
        Some("users/123"),
        &format!("**금일 배포**\n- [{a}] 완료: 첫 번째 검증\n- [{b}] 완료: 두 번째 검증"),
    );
    let (first_result, second_result) = tokio::join!(
        db.process_scheduled_reply(connection, &first),
        db.process_scheduled_reply(connection, &second)
    );
    assert!(first_result.unwrap() && second_result.unwrap());
    assert!(
        db.process_scheduled_reply(connection, &first)
            .await
            .unwrap()
    );
    assert!(
        db.process_scheduled_reply(connection, &second)
            .await
            .unwrap()
    );
    let reports = db.completion_reviews(owner, None).await.unwrap();
    let reports: Vec<_> = reports
        .iter()
        .filter(|r| [waiting, done_a, done_b, other_assignee, stale].contains(&r.task_id))
        .collect();
    assert_eq!(reports.len(), 2);
    assert!(
        reports
            .iter()
            .all(|r| [done_a, done_b].contains(&r.task_id))
    );
    let (parent_task,feedback): (Option<Uuid>,String) = sqlx::query_as("SELECT task_id,feedback_text FROM scheduled_chat_reply_commands WHERE source_id=$1 AND provider_message_name=$2")
        .bind(source).bind(&first.provider_message_name).fetch_one(pool).await.unwrap();
    assert!(parent_task.is_none());
    assert!(feedback.contains(&format!("{other}는 현재 담당자만")));
    assert!(feedback.contains(&format!("{old}의 내용이나 상태가 바뀌어")));
    assert!(!feedback.contains(&wait_code));
    let pending_push: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM push_deliveries p JOIN task_completion_reviews r ON r.id=p.item_id WHERE p.item_type='task_completion_review' AND r.task_id=ANY($1) AND r.status='pending' AND p.status='queued'")
        .bind(vec![done_a,done_b]).fetch_one(pool).await.unwrap();
    assert_eq!(pending_push, 2);
    let completed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE id=ANY($1) AND status='completed'")
            .bind(vec![waiting, done_a, done_b, other_assignee, stale])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(completed, 0);
    let jobs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM itsm_issue_sync_jobs WHERE issue_id IN (9901,9902)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(jobs, 0);
    let cs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM google_chat_task_completion_deliveries WHERE task_id=ANY($1)",
    )
    .bind(vec![done_a, done_b])
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(cs, 0);
    for r in reports {
        let approve = r.task_id == done_a;
        assert!(
            db.decide_completion_review(
                owner,
                r.id,
                r.version,
                r.task_version,
                approve,
                if approve {
                    None
                } else {
                    Some("추가 검증 후 다시 보고해 주세요")
                }
            )
            .await
            .unwrap()
        );
        let feedback_task: Uuid = sqlx::query_scalar(
            "SELECT task_id FROM scheduled_chat_reply_commands WHERE provider_message_name=$1",
        )
        .bind(format!(
            "review:{}:{}",
            r.id,
            if approve { "approved" } else { "rejected" }
        ))
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            feedback_task, r.task_id,
            "each approval/rejection must reference its review task, not the shared message receipt"
        );
    }
    let final_a = db.task_for_user(owner, done_a).await.unwrap().unwrap();
    assert_eq!(
        final_a.status,
        jimin_storage::planning::TaskStatus::Completed
    );
    assert_eq!(final_a.completion_note.as_deref(), Some("첫 번째 검증"));
    assert_eq!(
        db.task_for_user(owner, done_b)
            .await
            .unwrap()
            .unwrap()
            .status,
        jimin_storage::planning::TaskStatus::Open
    );
    let jobs: Vec<i64> = sqlx::query_scalar(
        "SELECT issue_id FROM itsm_issue_sync_jobs WHERE issue_id IN (9901,9902)",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(jobs, vec![9901]);
    let cs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT task_id FROM google_chat_task_completion_deliveries WHERE task_id=ANY($1)",
    )
    .bind(vec![done_a, done_b])
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(cs, vec![done_a]);
}

//! Existing company OAuth transport and deterministic completion-reply ingestion.
use crate::{
    google_chat_oauth::{GoogleChatOAuthError, GoogleChatOAuthRuntime},
    webhook::expand_google_chat_mentions,
};
use jimin_storage::{
    Database,
    google_chat::{GoogleChatSourceSyncConnection, ProviderGoogleChatMessage},
    scheduled_chat_reply::delivery_tag,
};
use uuid::Uuid;

/// Sends only a genuine, scoped scheduled-run delivery, never an arbitrary marker.
/// # Errors
/// Returns a classified scope, persistence or provider failure.
pub(crate) async fn deliver_reminder(
    db: &Database,
    runtime: &GoogleChatOAuthRuntime,
    id: Uuid,
) -> Result<jimin_google::GoogleChatMessageReceipt, GoogleChatOAuthError> {
    let context = db
        .scheduled_reply_context(id)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?
        .ok_or(GoogleChatOAuthError::ProviderRejected)?;
    let connection = db
        .google_chat_source_sync_connection(context.source_id)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?
        .filter(|c| {
            c.user_id == context.user_id
                && c.project_id == context.project_id
                && c.account_id == context.account_id
                && c.space_name == context.space_name
        })
        .ok_or(GoogleChatOAuthError::ProviderRejected)?;
    let text = delivery_text(id, &context.payload, &context.directory)?;
    runtime
        .send_scheduled_reminder(&connection, id, &text)
        .await
}

fn delivery_text(
    id: Uuid,
    payload: &serde_json::Value,
    directory: &jimin_storage::webhook::GoogleChatMentionDirectory,
) -> Result<String, GoogleChatOAuthError> {
    if payload["taskChatKind"] == "completion" {
        let title = payload["title"]
            .as_str()
            .filter(|title| !title.trim().is_empty() && title.chars().count() <= 200)
            .ok_or(GoogleChatOAuthError::Configuration)?;
        return Ok(format!("{} · 완료되었어요.", title.trim()));
    }
    let message = payload["message"]
        .as_str()
        .filter(|s| s.chars().count() <= 1800)
        .ok_or(GoogleChatOAuthError::Configuration)?;
    let text = if payload["taskChatKind"] == "assignment" {
        let mentions = jimin_storage::task_chat::assignment_mentions(
            payload["assigneeName"].as_str(),
            directory,
        );
        format!(
            "[일감 배정 A-{}]\n{}\n{}",
            &id.simple().to_string()[20..].to_ascii_uppercase(),
            expand_google_chat_mentions(&mentions, directory),
            message
        )
    } else {
        format!(
            "{}\n{}",
            delivery_tag(id),
            expand_google_chat_mentions(message, directory)
        )
    };
    if text.len() > 31_000 {
        return Err(GoogleChatOAuthError::Configuration);
    }
    Ok(text)
}

/// Handles only server-tracked reminder threads before ordinary work parsing.
/// # Errors
/// Returns persistence failures so the source cursor is not advanced past a command.
pub(crate) async fn filter_completion_replies(
    db: &Database,
    connection: &GoogleChatSourceSyncConnection,
    messages: Vec<ProviderGoogleChatMessage>,
) -> Result<Vec<ProviderGoogleChatMessage>, GoogleChatOAuthError> {
    let mut ordinary = Vec::new();
    for message in messages {
        if db
            .is_scheduled_reply_echo(connection, &message)
            .await
            .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?
        {
            continue;
        }
        if !db
            .process_scheduled_reply(connection, &message)
            .await
            .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?
        {
            ordinary.push(message);
        }
    }
    Ok(ordinary)
}

/// Reconciles replies that arrived before the sender's HTTP receipt was persisted.
/// # Errors
/// Leaves the catch-up marker unset on any provider or storage failure.
pub(crate) async fn catch_up_new_threads(
    db: &Database,
    runtime: &GoogleChatOAuthRuntime,
    connection: &GoogleChatSourceSyncConnection,
) -> Result<(), GoogleChatOAuthError> {
    let items = db
        .scheduled_reply_catch_up(connection.source_id, connection.user_id)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
    for item in items {
        let messages = runtime
            .list_scheduled_reminder_replies(
                connection,
                &item.thread_name,
                item.created_at - time::Duration::seconds(1),
            )
            .await?;
        for message in messages {
            db.process_scheduled_reply(connection, &message)
                .await
                .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
        }
        db.finish_scheduled_reply_catch_up(item.id, connection.user_id)
            .await
            .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
    }
    Ok(())
}

/// Feedback retries have their own durable lease; task completion is never repeated.
/// # Errors
/// Returns persistence or provider errors while preserving the queued outcome.
pub(crate) async fn flush_feedback(
    db: &Database,
    runtime: &GoogleChatOAuthRuntime,
    connection: &GoogleChatSourceSyncConnection,
) -> Result<(), GoogleChatOAuthError> {
    let worker = format!("scheduled-reply-{}", Uuid::now_v7());
    let items = db
        .claim_scheduled_reply_feedback(connection.source_id, &worker)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
    let mut failure = None;
    for item in items {
        let result = runtime
            .send_scheduled_reply_feedback(
                connection,
                item.id,
                &item.thread_name,
                &item.feedback_text,
            )
            .await;
        db.record_scheduled_reply_feedback(
            item.id,
            &worker,
            item.attempt_count,
            result.as_ref().err().map(|e| e.failure_code()),
        )
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
        if let Err(error) = result {
            failure = Some(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_uses_only_title_and_assignment_retains_reviewed_body() {
        let id = Uuid::now_v7();
        let directory = jimin_storage::webhook::GoogleChatMentionDirectory {
            users: [("김경주".to_owned(), "users/123456789".to_owned())].into(),
        };
        let complete = serde_json::json!({
            "taskChatKind":"completion", "title":"테스트 일감",
            "message":"노출하면 안 되는 상세 내용".repeat(2000), "notes":"private"
        });
        assert_eq!(
            delivery_text(id, &complete, &directory).unwrap(),
            "테스트 일감 · 완료되었어요."
        );
        let assign = serde_json::json!({
            "taskChatKind":"assignment", "assigneeName":"김경주",
            "message":"직접 수정한 배정 내용\nhttps://example.test/document\n완료 T-123"
        });
        let text = delivery_text(id, &assign, &directory).unwrap();
        assert!(text.starts_with("[일감 배정 A-"));
        assert!(text.contains("<users/123456789>"));
        assert!(text.ends_with(assign["message"].as_str().unwrap()));
        let legacy = serde_json::json!({"message":"예약 요약 @김경주"});
        assert!(
            delivery_text(id, &legacy, &directory)
                .unwrap()
                .starts_with(&delivery_tag(id))
        );
        assert!(
            delivery_text(
                id,
                &serde_json::json!({"message":"x".repeat(1801)}),
                &directory
            )
            .is_err()
        );
    }
}

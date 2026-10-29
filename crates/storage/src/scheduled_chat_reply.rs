//! Completion commands received through an existing, owner-scoped company OAuth source.
mod report_parser;
mod report_processing;
use crate::{
    Database, StorageError,
    google_chat::{GoogleChatSourceSyncConnection, ProviderGoogleChatMessage},
    webhook::GoogleChatMentionDirectory,
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplyTask {
    pub id: Uuid,
    pub version: i64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplyEnvelope {
    pub source_id: Uuid,
    pub tasks: Vec<ReplyTask>,
}
#[derive(sqlx::FromRow)]
pub struct ReplyContext {
    pub account_id: Uuid,
    pub delivery_id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub source_id: Uuid,
    pub space_name: String,
    pub message_name: Option<String>,
    pub thread_name: Option<String>,
    pub delivered_at: Option<OffsetDateTime>,
    #[sqlx(json)]
    pub directory: GoogleChatMentionDirectory,
    #[sqlx(json)]
    pub payload: serde_json::Value,
    pub run_task_ids: Vec<Uuid>,
}
#[derive(sqlx::FromRow)]
pub struct ReplyFeedback {
    pub id: Uuid,
    pub thread_name: String,
    pub feedback_text: String,
    pub attempt_count: i32,
}
#[derive(sqlx::FromRow)]
pub struct ReplyCatchUp {
    pub id: Uuid,
    pub thread_name: String,
    pub created_at: OffsetDateTime,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ParsedCommand<'a> {
    Ignore,
    Invalid,
    Complete { code: String, note: Option<&'a str> },
}

#[must_use]
pub fn task_code(id: Uuid) -> String {
    format!("T-{}", id.simple().to_string()[20..].to_ascii_uppercase())
}
#[must_use]
pub fn delivery_tag(id: Uuid) -> String {
    format!(
        "[예약 알림 R-{}]",
        id.simple().to_string()[20..].to_ascii_uppercase()
    )
}
#[must_use]
pub fn valid_actor(value: &str) -> bool {
    value
        .strip_prefix("users/")
        .is_some_and(|s| !s.is_empty() && s.len() <= 40 && s.bytes().all(|b| b.is_ascii_digit()))
}
#[must_use]
pub fn valid_resource(value: &str, kind: &str) -> bool {
    let p: Vec<_> = value.split('/').collect();
    p.len() == 4
        && p[0] == "spaces"
        && p[2] == kind
        && [p[1], p[3]].iter().all(|s| {
            !s.is_empty()
                && s.len() <= 240
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        })
}
#[must_use]
pub fn actor_assigned(
    names: Option<&str>,
    actor: &str,
    directory: &GoogleChatMentionDirectory,
) -> bool {
    valid_actor(actor)
        && names.is_some_and(|s| {
            s.split(',')
                .any(|n| directory.users.get(n.trim()).is_some_and(|id| id == actor))
        })
}
#[must_use]
pub fn parse_command(text: &str) -> ParsedCommand<'_> {
    let mut lines = text.trim().splitn(2, '\n');
    let first = lines.next().unwrap_or("").trim();
    let Some(code) = first
        .strip_prefix("완료 ")
        .or_else(|| first.strip_suffix(" 완료"))
    else {
        return ParsedCommand::Ignore;
    };
    let code = code.trim().to_ascii_uppercase();
    if code.len() != 14
        || !code.starts_with("T-")
        || !code[2..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return ParsedCommand::Invalid;
    }
    let note = lines.next().map(str::trim).filter(|s| !s.is_empty());
    if note.is_some_and(|s| {
        s.chars().count() > 2000
            || s.chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    }) {
        return ParsedCommand::Invalid;
    }
    ParsedCommand::Complete { code, note }
}

/// Only a server-tracked, single-task assignment may omit the explicit task code.
#[must_use]
pub fn normalize_assignment_command(text: &str, task: Uuid) -> Option<String> {
    let mut lines = text.trim().splitn(2, '\n');
    if lines.next()?.trim() != "완료" {
        return None;
    }
    let note = lines
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    Some(format!(
        "완료 {}{}",
        task_code(task),
        note.map_or_else(String::new, |value| format!("\n{value}"))
    ))
}
fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

impl ReplyContext {
    /// Validates the marker against a genuine owner-scoped scheduled-run receipt.
    /// # Errors
    /// Returns invalid configuration for unbounded, malformed or unscoped tasks.
    pub fn envelope(&self) -> Result<ReplyEnvelope, StorageError> {
        let value: ReplyEnvelope = serde_json::from_value(
            self.payload
                .get("replyCompletion")
                .cloned()
                .ok_or(StorageError::InvalidConfiguration)?,
        )
        .map_err(|_| StorageError::InvalidConfiguration)?;
        if value.source_id != self.source_id
            || value.tasks.is_empty()
            || value.tasks.len() > 2000
            || value.tasks.iter().enumerate().any(|(i, t)| {
                t.version <= 0
                    || !self.run_task_ids.contains(&t.id)
                    || value.tasks[..i].iter().any(|x| x.id == t.id)
            })
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(value)
    }
}
async fn contexts(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    source: Uuid,
    delivery: Option<Uuid>,
    thread: Option<&str>,
) -> Result<Vec<ReplyContext>, StorageError> {
    let mut result = sqlx::query_as("SELECT a.id AS account_id,d.id AS delivery_id,d.user_id,d.project_id,s.id AS source_id,s.space_name,d.google_chat_message_name AS message_name,d.google_chat_thread_name AS thread_name,d.delivered_at,h.mention_directory AS directory,d.payload,r.task_ids AS run_task_ids
        FROM webhook_deliveries d JOIN scheduled_work_run_deliveries l ON l.delivery_id=d.id
        JOIN scheduled_work_runs r ON r.id=l.run_id AND r.user_id=d.user_id
        JOIN scheduled_work w ON w.id=r.scheduled_work_id AND w.user_id=d.user_id AND w.project_id=d.project_id
        JOIN project_webhooks h ON h.id=d.webhook_id AND h.user_id=d.user_id AND h.project_id=d.project_id AND h.enabled AND h.provider='google_chat'
        JOIN project_google_chat_sources s ON s.id::text=d.payload->'replyCompletion'->>'sourceId' AND s.user_id=d.user_id AND s.project_id=d.project_id AND s.enabled AND s.purpose IN ('delivery','both')
        JOIN google_chat_accounts a ON a.id=s.account_id AND a.user_id=s.user_id AND a.status IN ('active','error','reauth_required')
        JOIN projects p ON p.id=d.project_id AND p.user_id=d.user_id AND p.status='active'
        JOIN users u ON u.id=d.user_id AND u.status='active'
        WHERE d.user_id=$1 AND s.id=$2 AND ($3::UUID IS NULL OR d.id=$3) AND ($4::TEXT IS NULL OR d.google_chat_thread_name=$4)
          AND w.definition->>'replyCompletionSourceId'=s.id::text AND w.definition->>'webhookId'=h.id::text
        ORDER BY d.id DESC LIMIT 50 FOR SHARE OF w,h,s,a,p,u")
        .bind(owner).bind(source).bind(delivery).bind(thread).fetch_all(&mut **tx).await.map_err(classify)?;
    result
        .extend(crate::task_chat::reply_contexts(tx, owner, source, delivery, thread, true).await?);
    Ok(result)
}

impl Database {
    /// Finds newly acknowledged reminder threads for a one-time replay independent of the general source cursor.
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn scheduled_reply_catch_up(
        &self,
        source: Uuid,
        owner: Uuid,
    ) -> Result<Vec<ReplyCatchUp>, StorageError> {
        sqlx::query_as("SELECT id,google_chat_thread_name AS thread_name,created_at FROM webhook_deliveries WHERE user_id=$1 AND payload->'replyCompletion'->>'sourceId'=$2 AND status='delivered' AND google_chat_thread_name IS NOT NULL AND google_chat_replies_reconciled_at IS NULL ORDER BY id LIMIT 10")
            .bind(owner).bind(source.to_string()).fetch_all(self.pool()).await.map_err(classify)
    }
    /// Marks catch-up only after every observed command was processed durably.
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn finish_scheduled_reply_catch_up(
        &self,
        id: Uuid,
        owner: Uuid,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE webhook_deliveries SET google_chat_replies_reconciled_at=NOW() WHERE id=$1 AND user_id=$2 AND status='delivered'").bind(id).bind(owner).execute(self.pool()).await.map_err(classify)?;
        Ok(())
    }
    /// Includes consumed completion replies in the source cursor, without creating inflow items.
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn advance_scheduled_reply_source_cursor(
        &self,
        source: Uuid,
        owner: Uuid,
        latest: OffsetDateTime,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE project_google_chat_sources SET last_provider_message_at=GREATEST(COALESCE(last_provider_message_at,$3),$3) WHERE id=$1 AND user_id=$2 AND enabled")
            .bind(source).bind(owner).bind(latest).execute(self.pool()).await.map_err(classify)?;
        Ok(())
    }
    /// Loads the exact source selected by a server-created reminder marker.
    /// # Errors
    /// Returns validation or persistence failures, never credentials in errors.
    pub async fn scheduled_reply_context(
        &self,
        id: Uuid,
    ) -> Result<Option<ReplyContext>, StorageError> {
        if let Some(context) = crate::task_chat::delivery_context(self, id).await? {
            return Ok(Some(context));
        }
        let raw: Option<(Uuid, serde_json::Value)> =
            sqlx::query_as("SELECT user_id,payload FROM webhook_deliveries WHERE id=$1")
                .bind(id)
                .fetch_optional(self.pool())
                .await
                .map_err(classify)?;
        let Some((owner, payload)) = raw else {
            return Ok(None);
        };
        let envelope: ReplyEnvelope = serde_json::from_value(
            payload
                .get("replyCompletion")
                .cloned()
                .ok_or(StorageError::InvalidConfiguration)?,
        )
        .map_err(|_| StorageError::InvalidConfiguration)?;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let context = contexts(&mut tx, owner, envelope.source_id, Some(id), None)
            .await?
            .into_iter()
            .next();
        if let Some(c) = &context {
            c.envelope()?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(context)
    }

    /// Saves Google canonical names and delivery success under the same lease fence.
    /// # Errors
    /// Returns invalid metadata or persistence failures; stale leases return false.
    pub async fn complete_scheduled_reply_delivery(
        &self,
        id: Uuid,
        worker: &str,
        attempt: i32,
        message: &str,
        thread: &str,
    ) -> Result<bool, StorageError> {
        if !valid_resource(message, "messages")
            || !valid_resource(thread, "threads")
            || worker.is_empty()
            || worker.len() > 200
            || !(1..=5).contains(&attempt)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let context = self
            .scheduled_reply_context(id)
            .await?
            .ok_or(StorageError::IdentityConflict)?;
        if !message.starts_with(&format!("{}/messages/", context.space_name))
            || !thread.starts_with(&format!("{}/threads/", context.space_name))
        {
            return Err(StorageError::IdentityConflict);
        }
        if context.payload["taskChatKind"] == "completion" {
            let target = serde_json::from_value::<crate::task_chat::TaskCompletionTarget>(
                context
                    .payload
                    .get("taskChatCompletionTarget")
                    .cloned()
                    .ok_or(StorageError::IdentityConflict)?,
            )
            .map_err(|_| StorageError::IdentityConflict)?;
            if let crate::task_chat::TaskCompletionTarget::AssignmentThread { thread_name, .. } =
                target
                && thread_name != thread
            {
                return Err(StorageError::IdentityConflict);
            }
        }
        let count=sqlx::query("UPDATE webhook_deliveries SET status='delivered',response_code=200,delivered_at=NOW(),last_error_code=NULL,lease_owner=NULL,lease_expires_at=NULL,next_attempt_at=NULL,google_chat_message_name=$4,google_chat_thread_name=$5 WHERE id=$1 AND status='sending' AND lease_owner=$2 AND attempt_count=$3")
            .bind(id).bind(worker).bind(attempt).bind(message).bind(thread).execute(self.pool()).await.map_err(classify)?.rows_affected();
        Ok(count == 1)
    }

    /// Recognizes this server's own outgoing reminder before its send receipt commits.
    /// # Errors
    /// Returns a classified persistence failure.
    pub async fn is_scheduled_reply_echo(
        &self,
        connection: &GoogleChatSourceSyncConnection,
        message: &ProviderGoogleChatMessage,
    ) -> Result<bool, StorageError> {
        if message.sender_provider_name.as_deref()
            != Some(format!("users/{}", connection.provider_subject).as_str())
        {
            return Ok(false);
        }
        let known:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM webhook_deliveries d WHERE d.user_id=$1 AND d.project_id=$2 AND (d.payload->'replyCompletion'->>'sourceId'=$3 OR d.payload->>'taskChatSourceId'=$3) AND (d.google_chat_message_name=$4 OR $4=CONCAT($5,'/messages/client-jimin-',replace(d.id::TEXT,'-',''))))")
            .bind(connection.user_id).bind(connection.project_id).bind(connection.source_id.to_string()).bind(&message.provider_message_name).bind(&connection.space_name).fetch_one(self.pool()).await.map_err(classify)?;
        if known {
            return Ok(true);
        }
        let Some(tag) = message
            .content_text
            .lines()
            .next()
            .and_then(|s| {
                s.strip_prefix("[예약 알림 R-")
                    .or_else(|| s.strip_prefix("[일감 배정 A-"))
            })
            .and_then(|s| s.strip_suffix(']'))
        else {
            return Ok(false);
        };
        if tag.len() != 12 || !tag.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(false);
        }
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM webhook_deliveries WHERE user_id=$1 AND project_id=$2 AND payload->'replyCompletion'->>'sourceId'=$3 AND right(replace(id::text,'-',''),12)=$4)")
            .bind(connection.user_id).bind(connection.project_id).bind(connection.source_id.to_string()).bind(tag.to_ascii_lowercase()).fetch_one(self.pool()).await.map_err(classify)
    }

    /// Processes only a reply to a delivered reminder, using current sender identity.
    /// True also suppresses ordinary reminder-thread chatter from the new-work inbox.
    /// # Errors
    /// Persistence failure rolls back completion, effects and command receipt together.
    #[allow(
        clippy::too_many_lines,
        reason = "One atomic command authorization/completion/feedback receipt boundary."
    )]
    pub async fn process_scheduled_reply(
        &self,
        connection: &GoogleChatSourceSyncConnection,
        message: &ProviderGoogleChatMessage,
    ) -> Result<bool, StorageError> {
        let Some(thread) = message.provider_thread_name.as_deref() else {
            return Ok(false);
        };
        if !valid_resource(thread, "threads")
            || !valid_resource(&message.provider_message_name, "messages")
            || !thread.starts_with(&format!("{}/threads/", connection.space_name))
            || !message
                .provider_message_name
                .starts_with(&format!("{}/messages/", connection.space_name))
        {
            return Ok(false);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let contexts = contexts(
            &mut tx,
            connection.user_id,
            connection.source_id,
            None,
            Some(thread),
        )
        .await?;
        let contexts: Vec<_> = contexts
            .into_iter()
            .filter(|c| {
                c.project_id == connection.project_id
                    && c.account_id == connection.account_id
                    && c.space_name == connection.space_name
                    && c.message_name.is_some()
                    && c.delivered_at.is_some()
            })
            .collect();
        if contexts.is_empty() {
            return Ok(false);
        }
        // An early reply may have been ingested before the sender's HTTP receipt.
        // Remove only unpromoted automation chatter from that exact message/source.
        let imported:Option<(Uuid,i64)>=sqlx::query_as("UPDATE project_inflow_items SET status='dismissed' WHERE user_id=$1 AND source_id=$2 AND provider_message_name=$3 AND status='pending' RETURNING id,version")
            .bind(connection.user_id).bind(connection.source_id).bind(&message.provider_message_name).fetch_optional(&mut *tx).await.map_err(classify)?;
        if let Some((id, version)) = imported {
            crate::auth::append_change(
                &mut tx,
                connection.user_id,
                "project_inflow_item",
                id,
                version,
            )
            .await?;
        }
        let single_assignment =
            contexts.len() == 1 && contexts[0].payload["taskChatKind"] == "assignment";
        let normalized = if single_assignment {
            contexts[0]
                .envelope()?
                .tasks
                .first()
                .and_then(|task| normalize_assignment_command(&message.content_text, task.id))
        } else {
            None
        };
        let commands =
            report_parser::parse_reply(normalized.as_deref().unwrap_or(&message.content_text));
        if commands.as_ref().is_ok_and(Vec::is_empty)
            || contexts
                .iter()
                .any(|c| c.message_name.as_deref() == Some(&message.provider_message_name))
        {
            tx.commit().await.map_err(classify)?;
            return Ok(true);
        }
        let receipt = Uuid::now_v7();
        let inserted=sqlx::query("INSERT INTO scheduled_chat_reply_commands(id,user_id,source_id,provider_message_name,thread_name,actor_name,outcome,feedback_text) VALUES($1,$2,$3,$4,$5,$6,'invalid','완료 번호를 확인해 주세요.') ON CONFLICT(source_id,provider_message_name) DO NOTHING")
            .bind(receipt).bind(connection.user_id).bind(connection.source_id).bind(&message.provider_message_name).bind(thread).bind(&message.sender_provider_name).execute(&mut *tx).await.map_err(classify)?.rows_affected();
        if inserted == 0 {
            tx.commit().await.map_err(classify)?;
            return Ok(true);
        }
        let (outcome, feedback, task_id, delivery) = if let Ok(commands) = commands {
            let mut outcomes = Vec::new();
            for command in commands {
                outcomes.push(
                    report_processing::process_completion(
                        &mut tx,
                        &contexts,
                        receipt,
                        message.sender_provider_name.as_deref(),
                        &command,
                    )
                    .await?,
                );
            }
            let primary = outcomes
                .iter()
                .find(|r| r.outcome == "review_pending")
                .or_else(|| outcomes.iter().find(|r| r.outcome == "already_pending"))
                .or_else(|| outcomes.first())
                .ok_or(StorageError::InvalidConfiguration)?;
            let outcome = primary.outcome;
            // A batch receipt belongs to the message; each review owns its separate task.
            let task_id = if outcomes.len() == 1 {
                outcomes[0].task_id
            } else {
                None
            };
            let delivery = if outcomes.len() == 1 {
                outcomes[0].delivery_id
            } else {
                None
            };
            let feedback = outcomes
                .into_iter()
                .map(|r| r.feedback)
                .collect::<Vec<_>>()
                .join("\n");
            (outcome, feedback, task_id, delivery)
        } else {
            ("invalid","완료할 할 일 번호를 확인해 주세요. 첫 줄에 ‘완료 T-번호’를 적고, 완료 답글은 다음 줄에 적어 주세요.".into(),None,None)
        };
        sqlx::query("UPDATE scheduled_chat_reply_commands SET outcome=$2,feedback_text=$3,task_id=$4,delivery_id=$5 WHERE id=$1")
            .bind(receipt).bind(outcome).bind(feedback).bind(task_id).bind(delivery).execute(&mut *tx).await.map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Leases bounded feedback, independently of the task completion transaction.
    /// # Errors
    /// Returns invalid worker or persistence failures.
    pub async fn claim_scheduled_reply_feedback(
        &self,
        source: Uuid,
        worker: &str,
    ) -> Result<Vec<ReplyFeedback>, StorageError> {
        if worker.is_empty() || worker.len() > 200 {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query("UPDATE scheduled_chat_reply_commands SET status='failed',last_error_code='scheduled_reply.feedback_lease_expired',lease_owner=NULL,lease_expires_at=NULL WHERE source_id=$1 AND status='sending' AND attempt_count>=5 AND lease_expires_at<NOW()")
            .bind(source).execute(self.pool()).await.map_err(classify)?;
        sqlx::query_as("WITH picked AS (SELECT id FROM scheduled_chat_reply_commands WHERE source_id=$1 AND attempt_count<5 AND ((status IN ('queued','retry_wait') AND (next_attempt_at IS NULL OR next_attempt_at<=NOW())) OR (status='sending' AND lease_expires_at<NOW())) ORDER BY created_at LIMIT 10 FOR UPDATE SKIP LOCKED)
            UPDATE scheduled_chat_reply_commands c SET status='sending',attempt_count=attempt_count+1,lease_owner=$2,lease_expires_at=NOW()+INTERVAL '5 minutes' FROM picked WHERE c.id=picked.id RETURNING c.id,c.thread_name,c.feedback_text,c.attempt_count")
            .bind(source).bind(worker).fetch_all(self.pool()).await.map_err(classify)
    }

    /// Acknowledges only the current lease; failures never undo a completed task.
    /// # Errors
    /// Returns invalid or persistence failures.
    pub async fn record_scheduled_reply_feedback(
        &self,
        id: Uuid,
        worker: &str,
        attempt: i32,
        error: Option<&str>,
    ) -> Result<bool, StorageError> {
        if !(1..=5).contains(&attempt) || error.is_some_and(|s| s.len() > 120) {
            return Err(StorageError::InvalidConfiguration);
        }
        let count=sqlx::query("UPDATE scheduled_chat_reply_commands SET status=CASE WHEN $4::TEXT IS NULL THEN 'sent' WHEN attempt_count>=5 THEN 'failed' ELSE 'retry_wait' END,sent_at=CASE WHEN $4::TEXT IS NULL THEN NOW() ELSE NULL END,last_error_code=$4,next_attempt_at=CASE WHEN $4::TEXT IS NOT NULL AND attempt_count<5 THEN NOW()+INTERVAL '30 seconds'*attempt_count ELSE NULL END,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1 AND lease_owner=$2 AND attempt_count=$3 AND status='sending'")
            .bind(id).bind(worker).bind(attempt).bind(error).execute(self.pool()).await.map_err(classify)?.rows_affected();
        Ok(count == 1)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn plain_completion_is_only_normalized_for_a_tracked_single_assignment() {
        let task = uuid::Uuid::now_v7();
        assert_eq!(super::parse_command("완료"), super::ParsedCommand::Ignore);
        let text = super::normalize_assignment_command(" 완료\n검증 메모 ", task).unwrap();
        assert_eq!(text, format!("완료 {}\n검증 메모", super::task_code(task)));
        assert!(matches!(
            super::parse_command(&text),
            super::ParsedCommand::Complete { .. }
        ));
        for ignored in [
            "아직 완료 아님",
            "완료했습니다",
            "확인 완료 예정",
            "미완료",
            "완료 T-ABCDEF123456",
        ] {
            assert!(super::normalize_assignment_command(ignored, task).is_none());
        }
    }
    use super::*;
    #[test]
    fn completion_accepts_both_orders_with_the_same_code_and_note() {
        let code = "T-9E50ABD103B5";
        for text in [
            format!("완료 {code}"),
            format!("{code} 완료"),
            format!("  완료   {}  ", code.to_ascii_lowercase()),
            format!("  {}   완료  ", code.to_ascii_lowercase()),
        ] {
            assert_eq!(
                parse_command(&text),
                ParsedCommand::Complete {
                    code: code.into(),
                    note: None,
                },
                "{text:?}"
            );
        }
        for first in [format!("완료 {code}"), format!("{code} 완료")] {
            for newline in ["\n", "\r\n"] {
                let text = format!("{first}{newline} 검증 후 완료했습니다. {newline}모바일 확인 ");
                assert_eq!(
                    parse_command(&text),
                    ParsedCommand::Complete {
                        code: code.into(),
                        note: Some(if newline == "\n" {
                            "검증 후 완료했습니다. \n모바일 확인"
                        } else {
                            "검증 후 완료했습니다. \r\n모바일 확인"
                        }),
                    }
                );
            }
        }
    }

    #[test]
    fn either_order_rejects_ambiguous_codes_chatter_and_unsafe_notes() {
        let code = "T-9E50ABD103B5";
        for text in [
            format!("{code} 미완료"),
            format!("{code} 완료 예정"),
            format!("{code} 완료했습니다"),
            format!("아직 {code} 완료"),
            format!("완료 {code} {code}"),
            format!("{code} {code} 완료"),
            format!("{code}\n완료"),
            format!("\n확인 요청\n{code} 완료"),
            "T-XYZ 완료".into(),
            "T-9E50ABD103B 완료".into(),
            "T-9E50ABD103B55 완료".into(),
            "T-9E50ABD103BG 완료".into(),
        ] {
            assert!(
                !matches!(parse_command(&text), ParsedCommand::Complete { .. }),
                "{text:?}"
            );
        }
        for first in [format!("완료 {code}"), format!("{code} 완료")] {
            assert!(matches!(
                parse_command(&format!("{first}\n{}", "가".repeat(2000))),
                ParsedCommand::Complete { .. }
            ));
            for note in [
                "가".repeat(2001),
                "확인\0완료".into(),
                "확인\u{7f}완료".into(),
            ] {
                assert_eq!(
                    parse_command(&format!("{first}\n{note}")),
                    ParsedCommand::Invalid
                );
            }
        }
    }

    #[test]
    fn exact_commands_are_bounded_and_do_not_infer_completion_from_chatter() {
        let code = task_code(Uuid::now_v7());
        assert!(matches!(
            parse_command(&format!("완료 {code}\n검증 완료")),
            ParsedCommand::Complete {
                note: Some("검증 완료"),
                ..
            }
        ));
        for text in [
            "완료했습니다",
            "아직 완료 아님",
            "완료 예정",
            "완료 T-XYZ",
            "완료 T-123456789ABC T-ABCDEF123456",
        ] {
            assert!(!matches!(
                parse_command(text),
                ParsedCommand::Complete { .. }
            ));
        }
        assert!(matches!(
            parse_command(&format!("완료 {code}\n{}", "x".repeat(2001))),
            ParsedCommand::Invalid
        ));
    }
    #[test]
    fn actor_matches_current_multi_assignee_ids_not_display_names() {
        let d = GoogleChatMentionDirectory {
            users: [
                ("김경주".into(), "users/123".into()),
                ("송인준".into(), "users/456".into()),
            ]
            .into(),
        };
        assert!(actor_assigned(Some("송인준, 김경주"), "users/123", &d));
        assert!(!actor_assigned(Some("송인준"), "users/123", &d));
        assert!(!valid_actor("users/app"));
    }
}

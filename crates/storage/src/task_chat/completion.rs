use super::{CompletionFeedback, classify};
use crate::{
    Database, StorageError,
    scheduled_chat_reply::{ReplyContext, valid_resource},
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// An immutable destination chosen before the first provider request, not on every retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskCompletionTarget {
    Space,
    AssignmentThread {
        assignment_delivery_id: Uuid,
        thread_name: String,
    },
}

/// A current owner approval also authorizes its existing report source when no project route exists.
pub(super) async fn approved_review_destination(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    project: Uuid,
    task: Uuid,
    feedback: CompletionFeedback,
) -> Result<Option<(Uuid, Uuid)>, StorageError> {
    sqlx::query_as("SELECT s.id,h.id FROM task_completion_reviews r
        JOIN scheduled_chat_reply_commands c ON c.id=r.command_id AND c.user_id=r.user_id
        JOIN webhook_deliveries d ON d.user_id=r.user_id AND d.project_id=$2 AND d.status='delivered'
          AND d.google_chat_message_name IS NOT NULL AND d.google_chat_thread_name=c.thread_name
          AND (c.delivery_id=d.id OR c.delivery_id IS NULL)
        JOIN project_google_chat_sources s ON s.id=c.source_id AND s.user_id=r.user_id AND s.project_id=d.project_id
          AND s.enabled AND s.purpose IN ('delivery','both') AND d.payload->'replyCompletion'->>'sourceId'=s.id::text
        JOIN project_webhooks h ON h.id=d.webhook_id AND h.user_id=r.user_id AND h.project_id=d.project_id AND h.provider='google_chat'
        JOIN google_chat_accounts a ON a.id=s.account_id AND a.user_id=s.user_id AND a.status IN ('active','error','reauth_required')
        WHERE r.id=$4 AND r.user_id=$1 AND r.task_id=$3 AND r.status='approved' AND s.id=$5
          AND d.payload->'replyCompletion'->'tasks' @> jsonb_build_array(jsonb_build_object('id',$3::UUID))
          AND (EXISTS(SELECT 1 FROM task_chat_deliveries t WHERE t.delivery_id=d.id AND t.user_id=$1 AND t.project_id=$2 AND t.task_id=$3 AND t.kind='assignment')
            OR EXISTS(SELECT 1 FROM scheduled_work_run_deliveries l JOIN scheduled_work_runs w ON w.id=l.run_id AND w.user_id=$1 WHERE l.delivery_id=d.id AND $3=ANY(w.task_ids)))
        ORDER BY d.id DESC LIMIT 1 FOR SHARE OF r,c,d,s,h,a")
        .bind(user).bind(project).bind(task).bind(feedback.review_id).bind(feedback.source_id)
        .fetch_optional(&mut **tx).await.map_err(classify)
}

#[derive(sqlx::FromRow)]
struct CompletionRow {
    user_id: Uuid,
    project_id: Uuid,
    source_id: Uuid,
    task_id: Uuid,
    webhook_id: Uuid,
    payload: serde_json::Value,
}

pub(super) async fn context(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<ReplyContext>, StorageError> {
    let row: Option<CompletionRow> = sqlx::query_as("SELECT t.user_id,t.project_id,t.source_id,t.task_id,d.webhook_id,d.payload FROM task_chat_deliveries t JOIN webhook_deliveries d ON d.id=t.delivery_id AND d.user_id=t.user_id AND d.project_id=t.project_id WHERE t.delivery_id=$1 AND t.kind='completion'")
        .bind(id).fetch_optional(&mut **tx).await.map_err(classify)?;
    let Some(row) = row else { return Ok(None) };
    if row.payload["taskChatKind"] != "completion"
        || row.payload["taskChatSourceId"] != row.source_id.to_string()
    {
        return Err(StorageError::InvalidConfiguration);
    }
    let authorized = if let Some(review) = row.payload.get("taskChatCompletionReviewRouteId") {
        let review = review
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .filter(|id| id.get_version_num() == 7)
            .ok_or(StorageError::InvalidConfiguration)?;
        approved_review_destination(
            tx,
            row.user_id,
            row.project_id,
            row.task_id,
            CompletionFeedback {
                source_id: row.source_id,
                review_id: review,
            },
        )
        .await?
            == Some((row.source_id, row.webhook_id))
    } else {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project_task_chat_routes WHERE user_id=$1 AND project_id=$2 AND source_id=$3 AND directory_webhook_id=$4)")
            .bind(row.user_id).bind(row.project_id).bind(row.source_id).bind(row.webhook_id).fetch_one(&mut **tx).await.map_err(classify)?
    };
    if !authorized {
        return Ok(None);
    }
    sqlx::query_as("SELECT a.id AS account_id,d.id AS delivery_id,t.user_id,t.project_id,s.id AS source_id,s.space_name,d.google_chat_message_name AS message_name,d.google_chat_thread_name AS thread_name,d.delivered_at,h.mention_directory AS directory,d.payload,ARRAY[t.task_id] AS run_task_ids
        FROM task_chat_deliveries t JOIN webhook_deliveries d ON d.id=t.delivery_id AND d.user_id=t.user_id AND d.project_id=t.project_id
        JOIN project_google_chat_sources s ON s.id=t.source_id AND s.user_id=t.user_id AND s.project_id=t.project_id AND s.enabled AND s.purpose IN ('delivery','both')
        JOIN project_webhooks h ON h.id=d.webhook_id AND h.user_id=t.user_id AND h.project_id=t.project_id AND h.provider='google_chat'
        JOIN google_chat_accounts a ON a.id=s.account_id AND a.user_id=s.user_id AND a.status IN ('active','error','reauth_required')
        JOIN projects p ON p.id=t.project_id AND p.user_id=t.user_id AND p.status='active' JOIN users u ON u.id=t.user_id AND u.status='active'
        WHERE t.delivery_id=$1 AND t.kind='completion' ORDER BY d.id LIMIT 1 FOR SHARE OF s,h,a,p,u")
        .bind(id).fetch_optional(&mut **tx).await.map_err(classify)
}

async fn assignment_thread(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    assignment: Option<Uuid>,
) -> Result<Option<(Uuid, String)>, StorageError> {
    sqlx::query_as("SELECT a.delivery_id,d.google_chat_thread_name FROM task_chat_deliveries c
        JOIN task_chat_deliveries a ON a.user_id=c.user_id AND a.project_id=c.project_id AND a.source_id=c.source_id AND a.task_id=c.task_id AND a.kind='assignment' AND a.task_version<c.task_version
        JOIN webhook_deliveries d ON d.id=a.delivery_id AND d.user_id=a.user_id AND d.project_id=a.project_id AND d.status='delivered' AND d.delivered_at IS NOT NULL AND d.google_chat_message_name IS NOT NULL AND d.google_chat_thread_name IS NOT NULL
        WHERE c.delivery_id=$1 AND c.kind='completion' AND ($2::UUID IS NULL OR a.delivery_id=$2)
          AND d.payload->'replyCompletion'->'tasks'=jsonb_build_array(jsonb_build_object('id',a.task_id,'version',a.task_version))
        ORDER BY a.task_version DESC,d.delivered_at DESC,a.delivery_id DESC LIMIT 1 FOR SHARE OF a,d")
        .bind(id).bind(assignment).fetch_optional(&mut **tx).await.map_err(classify)
}

impl Database {
    /// Pins a genuine task assignment (or explicit no-thread fallback) under the delivery lease.
    /// A timeout/retry cannot switch destinations and create a second completion message.
    /// # Errors
    /// Fails closed on invalid scope, tampered receipts, expired/wrong leases or persistence failure.
    pub async fn prepare_task_completion_target(
        &self,
        id: Uuid,
        worker: &str,
        attempt: i32,
    ) -> Result<TaskCompletionTarget, StorageError> {
        if worker.is_empty() || worker.len() > 200 || !(1..=5).contains(&attempt) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let payload: Option<serde_json::Value> = sqlx::query_scalar("SELECT d.payload FROM webhook_deliveries d JOIN task_chat_deliveries t ON t.delivery_id=d.id AND t.kind='completion' WHERE d.id=$1 AND d.status='sending' AND d.lease_owner=$2 AND d.attempt_count=$3 AND d.lease_expires_at>NOW() FOR UPDATE OF d")
            .bind(id).bind(worker).bind(attempt).fetch_optional(&mut *tx).await.map_err(classify)?;
        let payload = payload.ok_or(StorageError::IdentityConflict)?;
        let context = context(&mut tx, id)
            .await?
            .ok_or(StorageError::IdentityConflict)?;
        let routing_version = payload.get("taskChatCompletionRoutingVersion");
        if routing_version.is_some_and(|value| value != 1) {
            return Err(StorageError::InvalidConfiguration);
        }
        let target = if let Some(value) = payload.get("taskChatCompletionTarget") {
            serde_json::from_value::<TaskCompletionTarget>(value.clone())
                .map_err(|_| StorageError::InvalidConfiguration)?
        } else if routing_version.is_none() && attempt > 1 {
            // Old workers always posted to the room. An uncertain old attempt must not
            // switch threads on retry; its deterministic message ID may already exist.
            TaskCompletionTarget::Space
        } else {
            assignment_thread(&mut tx, id, None).await?.map_or(
                TaskCompletionTarget::Space,
                |(assignment_delivery_id, thread_name)| TaskCompletionTarget::AssignmentThread {
                    assignment_delivery_id,
                    thread_name,
                },
            )
        };
        if let TaskCompletionTarget::AssignmentThread {
            assignment_delivery_id,
            thread_name,
        } = &target
            && (!valid_resource(thread_name, "threads")
                || !thread_name.starts_with(&format!("{}/threads/", context.space_name))
                || assignment_thread(&mut tx, id, Some(*assignment_delivery_id)).await?
                    != Some((*assignment_delivery_id, thread_name.clone())))
        {
            return Err(StorageError::IdentityConflict);
        }
        if payload.get("taskChatCompletionTarget").is_none() {
            sqlx::query("UPDATE webhook_deliveries SET payload=jsonb_set(jsonb_set(payload,'{taskChatCompletionTarget}',$2),'{taskChatCompletionRoutingVersion}','1') WHERE id=$1")
                .bind(id).bind(serde_json::to_value(&target).map_err(|_| StorageError::InvalidConfiguration)?).execute(&mut *tx).await.map_err(classify)?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(target)
    }
}

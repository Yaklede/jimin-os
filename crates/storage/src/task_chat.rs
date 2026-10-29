//! Explicit project assignment destination, independent from incoming-work collection.
mod completion;
use crate::{
    Database, StorageError,
    auth::append_change,
    planning::Task,
    scheduled_chat_reply::{ReplyContext, ReplyEnvelope, ReplyTask, task_code},
    webhook::GoogleChatMentionDirectory,
};
pub use completion::TaskCompletionTarget;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction, types::Json};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) struct CompletionFeedback {
    pub source_id: Uuid,
    pub review_id: Uuid,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatSourcePurpose {
    #[default]
    Inflow,
    Delivery,
    Both,
}
impl ChatSourcePurpose {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inflow => "inflow",
            Self::Delivery => "delivery",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TaskChatRoute {
    pub project_id: Uuid,
    pub source_id: Uuid,
    pub directory_webhook_id: Uuid,
    pub version: i64,
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}
fn valid(ids: &[Uuid]) -> bool {
    ids.iter().all(|id| id.get_version_num() == 7)
}

impl Database {
    /// Directories selected by explicit routes override legacy task-webhook availability.
    /// # Errors
    /// Returns only the owner's configured names or a sanitized persistence failure.
    pub async fn user_task_chat_directories(
        &self,
        user: Uuid,
    ) -> Result<Vec<(Uuid, Json<GoogleChatMentionDirectory>)>, StorageError> {
        if !valid(&[user]) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as("SELECT r.project_id,CASE WHEN s.enabled AND s.purpose IN ('delivery','both') THEN h.mention_directory ELSE '{\"users\":{}}'::jsonb END FROM project_task_chat_routes r JOIN project_google_chat_sources s ON s.id=r.source_id AND s.user_id=r.user_id AND s.project_id=r.project_id JOIN project_webhooks h ON h.id=r.directory_webhook_id AND h.user_id=r.user_id AND h.project_id=r.project_id WHERE r.user_id=$1")
            .bind(user).fetch_all(self.pool()).await.map_err(classify)
    }

    /// Reads an owned project's route, never another owner's connection metadata.
    /// # Errors
    /// Returns invalid scope or a sanitized persistence failure.
    pub async fn task_chat_route(
        &self,
        user: Uuid,
        project: Uuid,
    ) -> Result<Option<TaskChatRoute>, StorageError> {
        if !valid(&[user, project]) {
            return Err(StorageError::InvalidConfiguration);
        }
        let owned: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND user_id=$2)")
                .bind(project)
                .bind(user)
                .fetch_one(self.pool())
                .await
                .map_err(classify)?;
        if !owned {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as("SELECT project_id,source_id,directory_webhook_id,version FROM project_task_chat_routes WHERE project_id=$1 AND user_id=$2")
            .bind(project).bind(user).fetch_optional(self.pool()).await.map_err(classify)
    }

    /// Saves an explicitly selected delivery room and same-project mention directory.
    /// # Errors
    /// Invalid scope/role/credentials fails; a stale route version returns None.
    pub async fn save_task_chat_route(
        &self,
        user: Uuid,
        project: Uuid,
        source: Uuid,
        directory: Uuid,
        expected: i64,
    ) -> Result<Option<TaskChatRoute>, StorageError> {
        if !valid(&[user, project, source, directory]) || expected < 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let owned: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM projects WHERE id=$1 AND user_id=$2 AND status='active' FOR UPDATE",
        )
        .bind(project)
        .bind(user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if owned.is_none() {
            return Err(StorageError::InvalidConfiguration);
        }
        let permitted: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project_google_chat_sources s JOIN google_chat_accounts a ON a.id=s.account_id AND a.user_id=s.user_id JOIN project_webhooks h ON h.id=$4 AND h.project_id=s.project_id AND h.user_id=s.user_id WHERE s.id=$3 AND s.user_id=$1 AND s.project_id=$2 AND s.enabled AND s.purpose IN ('delivery','both') AND a.status IN ('active','error') AND a.refresh_token_ciphertext IS NOT NULL AND a.granted_scopes @> ARRAY['https://www.googleapis.com/auth/chat.messages.readonly','https://www.googleapis.com/auth/chat.messages.create']::TEXT[] AND h.provider='google_chat')")
            .bind(user).bind(project).bind(source).bind(directory).fetch_one(&mut *tx).await.map_err(classify)?;
        if !permitted {
            return Err(StorageError::InvalidConfiguration);
        }
        let current: Option<i64>=sqlx::query_scalar("SELECT version FROM project_task_chat_routes WHERE project_id=$1 AND user_id=$2 FOR UPDATE")
            .bind(project).bind(user).fetch_optional(&mut *tx).await.map_err(classify)?;
        if current.unwrap_or(0) != expected {
            return Ok(None);
        }
        let route=sqlx::query_as::<_,TaskChatRoute>("INSERT INTO project_task_chat_routes(project_id,user_id,source_id,directory_webhook_id) VALUES($1,$2,$3,$4) ON CONFLICT(project_id) DO UPDATE SET source_id=$3,directory_webhook_id=$4,version=EXCLUDED.version,updated_at=NOW() RETURNING project_id,source_id,directory_webhook_id,version")
            .bind(project).bind(user).bind(source).bind(directory).fetch_one(&mut *tx).await.map_err(classify)?;
        append_change(
            &mut tx,
            user,
            "project_task_chat_route",
            project,
            route.version,
        )
        .await?;
        tx.commit().await.map_err(classify)?;
        Ok(Some(route))
    }

    /// Stops automatic project delivery without deleting tasks or receipt history.
    /// # Errors
    /// Returns invalid input/persistence failure; stale versions return false.
    pub async fn delete_task_chat_route(
        &self,
        user: Uuid,
        project: Uuid,
        expected: i64,
    ) -> Result<bool, StorageError> {
        if !valid(&[user, project]) || expected <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let deleted=sqlx::query("DELETE FROM project_task_chat_routes WHERE project_id=$1 AND user_id=$2 AND version=$3")
            .bind(project).bind(user).bind(expected).execute(&mut *tx).await.map_err(classify)?.rows_affected();
        if deleted == 1 {
            crate::auth::append_delete_change(
                &mut tx,
                user,
                "project_task_chat_route",
                project,
                expected,
            )
            .await?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(deleted == 1)
    }

    /// Changes collection/reporting purposes without deleting captured work.
    /// # Errors
    /// Existing delivery/opt-in rules cannot silently lose their reporting room.
    pub async fn update_chat_source_purpose(
        &self,
        user: Uuid,
        project: Uuid,
        source: Uuid,
        expected: i64,
        purpose: ChatSourcePurpose,
    ) -> Result<bool, StorageError> {
        if !valid(&[user, project, source]) || expected <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let owned: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM projects WHERE id=$1 AND user_id=$2 FOR UPDATE")
                .bind(project)
                .bind(user)
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        if owned.is_none() {
            return Err(StorageError::InvalidConfiguration);
        }
        if purpose == ChatSourcePurpose::Inflow {
            let has_dependents:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project_task_chat_routes WHERE user_id=$1 AND project_id=$2 AND source_id=$3) OR EXISTS(SELECT 1 FROM scheduled_work WHERE user_id=$1 AND project_id=$2 AND definition->>'replyCompletionSourceId'=$3::UUID::TEXT)")
                .bind(user).bind(project).bind(source).fetch_one(&mut *tx).await.map_err(classify)?;
            if has_dependents {
                return Err(StorageError::InvalidConfiguration);
            }
        }
        let version:Option<i64>=sqlx::query_scalar("UPDATE project_google_chat_sources SET purpose=$5,last_provider_message_at=CASE WHEN purpose='delivery' AND $5 IN ('inflow','both') THEN NOW() ELSE last_provider_message_at END WHERE id=$3 AND user_id=$1 AND project_id=$2 AND version=$4 RETURNING version")
            .bind(user).bind(project).bind(source).bind(expected).bind(purpose.as_str()).fetch_optional(&mut *tx).await.map_err(classify)?;
        if let Some(version) = version {
            append_change(&mut tx, user, "project_google_chat_source", source, version).await?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(version.is_some())
    }
}

/// Enqueues a genuine task-version delivery using a source selected by the owner.
/// True replaces Google Chat task webhooks only; original CS replies and Discord stay independent.
pub(crate) async fn queue_task_delivery(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    task: &Task,
    event: &str,
    payload: &serde_json::Value,
    feedback: Option<CompletionFeedback>,
) -> Result<bool, StorageError> {
    let Some(project) = task.project_id else {
        return Ok(false);
    };
    if !matches!(
        event,
        "task.created" | "task.updated" | "task.restored" | "task.completed"
    ) {
        return Ok(false);
    }
    let route:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT source_id,directory_webhook_id FROM project_task_chat_routes WHERE user_id=$1 AND project_id=$2 FOR SHARE")
        .bind(user).bind(project).fetch_optional(&mut **tx).await.map_err(classify)?;
    let (route, review_route) = if route.is_some() || event != "task.completed" {
        (route, None)
    } else if let Some(feedback) = feedback {
        (
            completion::approved_review_destination(tx, user, project, task.id, feedback).await?,
            Some(feedback.review_id),
        )
    } else {
        (None, None)
    };
    let Some((source, directory)) = route else {
        return Ok(false);
    };
    let kind = if event == "task.completed" {
        "completion"
    } else {
        "assignment"
    };
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM task_chat_deliveries WHERE task_id=$1 AND task_version=$2 AND kind=$3)")
        .bind(task.id).bind(task.version).bind(kind).fetch_one(&mut **tx).await.map_err(classify)?;
    if exists {
        return Ok(true);
    }
    let mut body = payload.clone();
    body["taskChatKind"] = kind.into();
    body["taskChatSourceId"] = source.to_string().into();
    if kind == "completion" {
        body["taskChatCompletionRoutingVersion"] = 1.into();
    }
    if let Some(review) = review_route {
        body["taskChatCompletionReviewRouteId"] = review.to_string().into();
    }
    if kind == "assignment" {
        body["replyCompletion"] = serde_json::to_value(ReplyEnvelope {
            source_id: source,
            tasks: vec![ReplyTask {
                id: task.id,
                version: task.version,
            }],
        })
        .map_err(|_| StorageError::InvalidConfiguration)?;
        let message = body["message"]
            .as_str()
            .ok_or(StorageError::InvalidConfiguration)?;
        body["message"]=format!("[{}]\n{message}\n\n이 배정 글의 답글 첫 줄에 ‘완료’를 적어 주세요. 완료 메모는 다음 줄에 적을 수 있어요. 최종 확인 뒤 완료돼요.",task_code(task.id)).into();
    }
    let delivery = Uuid::now_v7();
    let queued=sqlx::query("INSERT INTO webhook_deliveries(id,user_id,project_id,webhook_id,provider,destination_ciphertext,destination_nonce,mention_directory,event_type,payload,status) SELECT $1,h.user_id,h.project_id,h.id,h.provider,h.destination_ciphertext,h.destination_nonce,h.mention_directory,$6,$7,'queued' FROM project_webhooks h WHERE h.id=$2 AND h.user_id=$3 AND h.project_id=$4 AND h.provider='google_chat' AND EXISTS(SELECT 1 FROM project_google_chat_sources s WHERE s.id=$5 AND s.user_id=$3 AND s.project_id=$4 AND s.enabled AND s.purpose IN ('delivery','both'))")
        .bind(delivery).bind(directory).bind(user).bind(project).bind(source).bind(event).bind(Json(body)).execute(&mut **tx).await.map_err(classify)?.rows_affected();
    if queued != 1 {
        return Err(StorageError::InvalidConfiguration);
    }
    sqlx::query("INSERT INTO task_chat_deliveries(delivery_id,user_id,project_id,source_id,task_id,task_version,kind) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(delivery).bind(user).bind(project).bind(source).bind(task.id).bind(task.version).bind(kind).execute(&mut **tx).await.map_err(classify)?;
    Ok(true)
}

pub(crate) async fn reply_contexts(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    source: Uuid,
    delivery: Option<Uuid>,
    thread: Option<&str>,
    assignments_only: bool,
) -> Result<Vec<ReplyContext>, StorageError> {
    sqlx::query_as("SELECT a.id AS account_id,d.id AS delivery_id,d.user_id,d.project_id,s.id AS source_id,s.space_name,d.google_chat_message_name AS message_name,d.google_chat_thread_name AS thread_name,d.delivered_at,h.mention_directory AS directory,d.payload,ARRAY[t.task_id] AS run_task_ids
        FROM task_chat_deliveries t JOIN webhook_deliveries d ON d.id=t.delivery_id AND d.user_id=t.user_id AND d.project_id=t.project_id
        JOIN project_task_chat_routes r ON r.user_id=t.user_id AND r.project_id=t.project_id AND r.source_id=t.source_id AND r.directory_webhook_id=d.webhook_id
        JOIN project_google_chat_sources s ON s.id=t.source_id AND s.user_id=t.user_id AND s.project_id=t.project_id AND s.enabled AND s.purpose IN ('delivery','both')
        JOIN project_webhooks h ON h.id=r.directory_webhook_id AND h.user_id=t.user_id AND h.project_id=t.project_id AND h.provider='google_chat'
        JOIN google_chat_accounts a ON a.id=s.account_id AND a.user_id=s.user_id AND a.status IN ('active','error','reauth_required')
        JOIN projects p ON p.id=t.project_id AND p.user_id=t.user_id AND p.status='active'
        JOIN users u ON u.id=t.user_id AND u.status='active'
        WHERE t.user_id=$1 AND t.source_id=$2 AND ($3::UUID IS NULL OR t.delivery_id=$3) AND ($4::TEXT IS NULL OR d.google_chat_thread_name=$4) AND (NOT $5 OR t.kind='assignment')
          AND (t.kind='completion' OR d.payload->'replyCompletion'->'tasks'=jsonb_build_array(jsonb_build_object('id',t.task_id,'version',t.task_version)))
        ORDER BY d.id DESC LIMIT 50 FOR SHARE OF r,s,h,a,p,u")
        .bind(owner).bind(source).bind(delivery).bind(thread).bind(assignments_only).fetch_all(&mut **tx).await.map_err(classify)
}

/// Uses server-created receipt metadata, not caller-provided payload markers.
pub(crate) async fn delivery_context(
    db: &Database,
    id: Uuid,
) -> Result<Option<ReplyContext>, StorageError> {
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT user_id,source_id,kind FROM task_chat_deliveries WHERE delivery_id=$1",
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await
    .map_err(classify)?;
    let Some((owner, source, kind)) = row else {
        return Ok(None);
    };
    let mut tx = db.pool().begin().await.map_err(classify)?;
    if kind == "completion" {
        let context = completion::context(&mut tx, id).await?;
        tx.commit().await.map_err(classify)?;
        return Ok(context);
    }
    let context = reply_contexts(&mut tx, owner, source, Some(id), None, false)
        .await?
        .into_iter()
        .next();
    if let Some(c) = &context
        && c.payload["taskChatKind"] == "assignment"
    {
        c.envelope()?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(context)
}

/// Formats the registered-name prefix only; unknown names never become authorization IDs.
#[must_use]
pub fn assignment_mentions(names: Option<&str>, directory: &GoogleChatMentionDirectory) -> String {
    names
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| directory.users.contains_key(*name))
        .map(|name| format!("@{name}"))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
}

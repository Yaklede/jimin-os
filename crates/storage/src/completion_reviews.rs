//! Owner confirmation separates an assignee report from irreversible external completion.
use crate::{
    Database, StorageError, auth::append_change,
    planning::complete_task_with_feedback_in_transaction,
};
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CompletionReview {
    pub id: Uuid,
    pub task_id: Uuid,
    pub project_id: Option<Uuid>,
    pub project_title: Option<String>,
    pub title: String,
    pub task_notes: Option<String>,
    pub assignee_name: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub due_at: Option<OffsetDateTime>,
    pub reporter_name: String,
    pub completion_note: Option<String>,
    pub reported_task_version: i64,
    pub task_version: i64,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: OffsetDateTime,
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[derive(sqlx::FromRow)]
struct DecisionRow {
    status: String,
    version: i64,
    completion_note: Option<String>,
    command_id: Option<Uuid>,
    feedback_source_id: Option<Uuid>,
    owner_note: Option<String>,
}

/// Caller already holds the owned task lock, fencing duplicate reports and decisions.
pub(crate) async fn report(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    task: Uuid,
    task_version: i64,
    command: Uuid,
    reporter: &str,
    note: Option<&str>,
) -> Result<bool, StorageError> {
    let pending: Option<(Uuid,i64)> = sqlx::query_as("SELECT id,reported_task_version FROM task_completion_reviews WHERE task_id=$1 AND user_id=$2 AND status='pending' FOR UPDATE")
        .bind(task).bind(user).fetch_optional(&mut **tx).await.map_err(classify)?;
    if pending.is_some_and(|(_, version)| version == task_version) {
        return Ok(false);
    }
    if let Some((id, _)) = pending {
        sqlx::query("UPDATE task_completion_reviews SET status='superseded',version=version+1,resolved_at=NOW() WHERE id=$1")
            .bind(id).execute(&mut **tx).await.map_err(classify)?;
    }
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO task_completion_reviews(id,user_id,task_id,command_id,reported_task_version,reporter_name,completion_note) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(id).bind(user).bind(task).bind(command).bind(task_version).bind(reporter).bind(note)
        .execute(&mut **tx).await.map_err(classify)?;
    append_change(tx, user, "task_completion_review", id, 1).await?;
    // The existing FCM worker retries independently. No token or credential enters the review.
    let devices: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT p.device_id,t.title
        FROM push_registrations p JOIN devices d ON d.id=p.device_id AND d.user_id=p.user_id
        JOIN tasks t ON t.id=$2 AND t.user_id=$1
        WHERE p.user_id=$1 AND p.status='active' AND d.status='active'
        ORDER BY p.device_id",
    )
    .bind(user)
    .bind(task)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    for (device, title) in devices {
        sqlx::query("INSERT INTO push_deliveries(id,user_id,device_id,item_type,item_id,item_version,destination,title,body,target_at,notify_at)
            VALUES($1,$2,$3,'task_completion_review',$4,1,'home',$5,$6,NOW()+INTERVAL '1 day',NOW())
            ON CONFLICT(device_id,item_type,item_id,item_version) DO NOTHING")
            .bind(Uuid::now_v7()).bind(user).bind(device).bind(id)
            .bind(format!("완료 확인 요청 · {title}").chars().take(120).collect::<String>())
            .bind(format!("{reporter}님이 완료를 보고했어요. 홈에서 확인해 주세요."))
            .execute(&mut **tx).await.map_err(classify)?;
    }
    Ok(true)
}

pub(crate) async fn ensure_no_pending(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    task: Uuid,
) -> Result<(), StorageError> {
    sqlx::query("SELECT id FROM tasks WHERE id=$1 AND user_id=$2 FOR UPDATE")
        .bind(task)
        .bind(user)
        .fetch_optional(&mut **tx)
        .await
        .map_err(classify)?;
    let pending: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM task_completion_reviews WHERE user_id=$1 AND task_id=$2 AND status='pending')")
        .bind(user).bind(task).fetch_one(&mut **tx).await.map_err(classify)?;
    if pending {
        Err(StorageError::IdentityConflict)
    } else {
        Ok(())
    }
}

impl Database {
    /// Lists only this owner's pending reports, with current task details and version.
    /// # Errors
    /// Returns invalid input or a classified persistence failure.
    pub async fn completion_reviews(
        &self,
        user: Uuid,
        before: Option<Uuid>,
    ) -> Result<Vec<CompletionReview>, StorageError> {
        if user.get_version_num() != 7 || before.is_some_and(|id| id.get_version_num() != 7) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as("SELECT r.id,r.task_id,t.project_id,p.title AS project_title,t.title,t.notes AS task_notes,t.assignee_name,t.due_at,
            r.reporter_name,r.completion_note,r.reported_task_version,t.version AS task_version,r.version,r.created_at
            FROM task_completion_reviews r JOIN tasks t ON t.id=r.task_id AND t.user_id=r.user_id
            LEFT JOIN projects p ON p.id=t.project_id AND p.user_id=r.user_id
            WHERE r.user_id=$1 AND r.status='pending' AND t.status='open' AND ($2::UUID IS NULL OR r.id<$2)
            ORDER BY r.id DESC LIMIT 51")
            .bind(user).bind(before).fetch_all(self.pool()).await.map_err(classify)
    }

    /// Owner-only, version-fenced decision. Approval and all completion outboxes commit together.
    /// # Errors
    /// Returns validation/persistence errors. False means missing/stale; retrying the same decision is safe.
    #[allow(
        clippy::too_many_arguments,
        reason = "Both review and current task versions are explicit concurrency fences."
    )]
    pub async fn decide_completion_review(
        &self,
        user: Uuid,
        id: Uuid,
        review_version: i64,
        task_version: i64,
        approve: bool,
        note: Option<&str>,
    ) -> Result<bool, StorageError> {
        if user.get_version_num() != 7
            || id.get_version_num() != 7
            || review_version <= 0
            || review_version == i64::MAX
            || task_version <= 0
            || note.is_some_and(|s| {
                s.trim().is_empty()
                    || s.chars().count() > 2000
                    || s.chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            })
            || (!approve && note.is_none())
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let task: Option<Uuid> = sqlx::query_scalar(
            "SELECT task_id FROM task_completion_reviews WHERE id=$1 AND user_id=$2",
        )
        .bind(id)
        .bind(user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some(task) = task else { return Ok(false) };
        // Every path uses task -> review lock order, including concurrent incoming replies.
        let current: Option<(String, i64)> = sqlx::query_as(
            "SELECT status,version FROM tasks WHERE id=$1 AND user_id=$2 FOR UPDATE",
        )
        .bind(task)
        .bind(user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let row:DecisionRow=sqlx::query_as("SELECT status,version,completion_note,command_id,owner_note,(SELECT source_id FROM scheduled_chat_reply_commands c WHERE c.id=r.command_id AND c.user_id=$2) AS feedback_source_id FROM task_completion_reviews r WHERE id=$1 AND user_id=$2 FOR UPDATE")
            .bind(id).bind(user).fetch_one(&mut *tx).await.map_err(classify)?;
        let decision = if approve { "approved" } else { "rejected" };
        if row.status == decision && row.version == review_version + 1 {
            return Ok(row.owner_note.as_deref() == note.map(str::trim));
        }
        if row.status != "pending"
            || row.version != review_version
            || current != Some(("open".to_owned(), task_version))
        {
            return Ok(false);
        }
        sqlx::query("UPDATE task_completion_reviews SET status=$2,owner_note=$3,version=version+1,resolved_at=NOW() WHERE id=$1")
            .bind(id).bind(decision).bind(note.map(str::trim)).execute(&mut *tx).await.map_err(classify)?;
        if approve
            && complete_task_with_feedback_in_transaction(
                &mut tx,
                user,
                task,
                task_version,
                row.completion_note.as_deref(),
                row.feedback_source_id
                    .map(|source_id| crate::task_chat::CompletionFeedback {
                        source_id,
                        review_id: id,
                    }),
            )
            .await?
            .is_none()
        {
            return Ok(false);
        }
        append_change(
            &mut tx,
            user,
            "task_completion_review",
            id,
            review_version + 1,
        )
        .await?;
        sqlx::query("UPDATE push_deliveries SET status='cancelled',lease_owner=NULL,lease_expires_at=NULL,next_attempt_at=NULL WHERE user_id=$1 AND item_type='task_completion_review' AND item_id=$2 AND status IN ('queued','retry_wait')")
            .bind(user).bind(id).execute(&mut *tx).await.map_err(classify)?;
        // Final approvals use the same canonical completion outbox as direct completion.
        // Only a rework request remains a reply to the original report thread.
        if !approve {
            let text = format!(
                "{} · 재작업을 요청했어요. 할 일은 열린 상태로 유지해요.\n{}",
                crate::scheduled_chat_reply::task_code(task),
                note.unwrap_or_default().trim()
            );
            sqlx::query("INSERT INTO scheduled_chat_reply_commands(id,user_id,source_id,delivery_id,provider_message_name,thread_name,task_id,outcome,feedback_text)
            SELECT $1,user_id,source_id,delivery_id,$2,thread_name,$7,$3,$4 FROM scheduled_chat_reply_commands WHERE id=$5 AND user_id=$6")
            .bind(Uuid::now_v7()).bind(format!("review:{id}:{decision}"))
            .bind("review_rejected").bind(text).bind(row.command_id).bind(user).bind(task)
            .execute(&mut *tx).await.map_err(classify)?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(true)
    }
}

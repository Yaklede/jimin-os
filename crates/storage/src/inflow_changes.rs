//! Explicit approval of a current conversation revision, without creating another task.
use crate::{
    Database, StorageError,
    auth::append_change,
    planning::{
        queue_owned_task_webhook_by_id_in_transaction, validate_task_hierarchy_in_transaction,
    },
};
use time::OffsetDateTime;
use uuid::Uuid;
pub struct ApproveInflowChange {
    pub user_id: Uuid,
    pub item_id: Uuid,
    pub expected_item_version: i64,
    pub task_id: Uuid,
    pub expected_task_version: i64,
    pub source_revision: i32,
    pub notes: String,
    pub due_at: Option<OffsetDateTime>,
}
impl Database {
    /// Saves task and conversation review atomically, guarded by both versions and analysis revision.
    /// # Errors
    /// Returns validation/persistence errors; stale or already-reviewed proposals return false.
    pub async fn approve_inflow_change(
        &self,
        command: &ApproveInflowChange,
    ) -> Result<bool, StorageError> {
        if ![command.user_id, command.item_id, command.task_id]
            .into_iter()
            .all(|id| id.get_version_num() == 7)
            || command.expected_item_version <= 0
            || command.expected_task_version <= 0
            || command.source_revision <= 0
            || command.notes.chars().count() > 10_000
            || command.notes.contains('\0')
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let context=sqlx::query_as::<_,(Uuid,Uuid,Option<String>,OffsetDateTime)>(
            "SELECT a.id,i.source_id,i.provider_thread_name,i.received_at
             FROM project_inflow_items i JOIN project_inflow_analyses a ON a.user_id=i.user_id AND a.source_id=i.source_id
             AND a.conversation_key=CASE WHEN i.provider_thread_name IS NULL THEN 'message:'||i.provider_message_name ELSE 'thread:'||i.provider_thread_name END
             WHERE i.user_id=$1 AND i.id=$2 AND i.version=$3 AND i.status='pending' AND i.reviewed_at IS NULL
             AND COALESCE(i.promoted_task_id,a.linked_task_id)=$4
             AND a.state='ready' AND a.classification='follow_up' AND a.source_revision=$5 AND a.analyzed_revision=$5
             AND NOT EXISTS (SELECT 1 FROM inflow_change_reviews r WHERE r.analysis_id=a.id AND r.source_revision=$5)
             FOR UPDATE OF i,a"
        ).bind(command.user_id).bind(command.item_id).bind(command.expected_item_version).bind(command.task_id).bind(command.source_revision)
        .fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some((analysis, source, thread, received)) = context else {
            return Ok(false);
        };
        let previous=sqlx::query_as::<_,(Option<String>,Option<OffsetDateTime>,Option<Uuid>,Option<Uuid>)>("SELECT notes,due_at,project_id,parent_task_id FROM tasks WHERE id=$1 AND user_id=$2 AND status='open' AND version=$3 FOR UPDATE")
            .bind(command.task_id).bind(command.user_id).bind(command.expected_task_version).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some((previous_notes, previous_due, project, parent)) = previous else {
            return Ok(false);
        };
        validate_task_hierarchy_in_transaction(
            &mut tx,
            command.user_id,
            command.task_id,
            project,
            parent,
            command.due_at,
        )
        .await?;
        let version=sqlx::query_scalar::<_,i64>("UPDATE tasks SET notes=$4,due_at=$5 WHERE id=$1 AND user_id=$2 AND version=$3 RETURNING version")
            .bind(command.task_id).bind(command.user_id).bind(command.expected_task_version).bind(command.notes.trim()).bind(command.due_at)
            .fetch_one(&mut *tx).await.map_err(classify)?;
        sqlx::query("INSERT INTO inflow_change_reviews (id,user_id,analysis_id,source_revision,task_id,task_version,previous_notes,approved_notes,previous_due_at,approved_due_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(Uuid::now_v7()).bind(command.user_id).bind(analysis).bind(command.source_revision).bind(command.task_id).bind(version)
            .bind(previous_notes).bind(command.notes.trim()).bind(previous_due).bind(command.due_at).execute(&mut *tx).await.map_err(classify)?;
        let reviewed=sqlx::query_as::<_,(Uuid,i64)>("UPDATE project_inflow_items SET reviewed_at=NOW() WHERE user_id=$1 AND source_id=$2 AND status='pending' AND (received_at,id)<=($3,$4) AND (($5::TEXT IS NULL AND id=$4) OR ($5 IS NOT NULL AND provider_thread_name=$5)) RETURNING id,version")
            .bind(command.user_id).bind(source).bind(received).bind(command.item_id).bind(thread).fetch_all(&mut *tx).await.map_err(classify)?;
        append_change(&mut tx, command.user_id, "task", command.task_id, version).await?;
        for (id, revision) in reviewed {
            append_change(
                &mut tx,
                command.user_id,
                "project_inflow_item",
                id,
                revision,
            )
            .await?;
        }
        // The public projection has bounded fields. Keep all explicitly approved
        // content in ordered chunks, not only the first summary segment.
        let chars: Vec<char> = command.notes.trim().chars().collect();
        let mut chunks = chars
            .chunks(1000)
            .map(|part| part.iter().collect::<String>());
        let summary = chunks.next();
        let actions = chunks.collect::<Vec<_>>();
        sqlx::query("UPDATE task_assignment_public_details SET summary=$3,action_items=$4,reviewed_content=CASE WHEN reviewed_content IS NOT NULL THEN $5 ELSE NULL END WHERE user_id=$1 AND task_id=$2")
            .bind(command.user_id).bind(command.task_id).bind(summary).bind(actions).bind(command.notes.trim()).execute(&mut *tx).await.map_err(classify)?;
        queue_owned_task_webhook_by_id_in_transaction(
            &mut tx,
            command.user_id,
            command.task_id,
            "task.updated",
        )
        .await?;
        tx.commit().await.map_err(classify)?;
        Ok(true)
    }
}
fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

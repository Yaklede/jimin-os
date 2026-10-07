//! Read-only, owner-scoped notification feed. Provider history and decisions are
//! not mutated by notification delivery.
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{Database, StorageError};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InflowNotification {
    pub id: Uuid,
    pub item_type: String,
    pub revision: i32,
    pub project_id: Option<Uuid>,
    pub existing_task: bool,
    pub title: String,
    pub body: String,
    pub occurred_at_epoch_millis: i64,
}

impl Database {
    /// Lists analyzed, unread requests after a timestamp/ID cursor. Only live
    /// messages newer than their source connection are eligible, never imports.
    ///
    /// # Errors
    /// Returns an error for invalid cursors, identity, or unavailable storage.
    pub async fn inflow_notifications(
        &self,
        user_id: Uuid,
        after: i64,
        after_id: Uuid,
        now: OffsetDateTime,
    ) -> Result<Vec<InflowNotification>, StorageError> {
        let now_millis = epoch_millis(now);
        if user_id.get_version_num() != 7 || after < 0 || after > now_millis {
            return Err(StorageError::InvalidConfiguration);
        }
        let oldest = epoch_millis(now - Duration::days(1));
        sqlx::query_as::<_, InflowNotification>(
            "WITH eligible AS (
                SELECT analysis.id, 'google_chat_inflow'::TEXT AS item_type,
                    analysis.source_revision AS revision, analysis.project_id,
                    COALESCE(analysis.linked_task_id, representative.promoted_task_id)
                        IS NOT NULL AS existing_task,
                    COALESCE(analysis.suggested_task_title, analysis.summary, project.title) AS title,
                    COALESCE(analysis.summary, '새 대화를 확인해 주세요.') AS body,
                    FLOOR(EXTRACT(EPOCH FROM analysis.analyzed_at) * 1000)::BIGINT
                        AS occurred_at_epoch_millis
                FROM project_inflow_analyses analysis
                JOIN projects project ON project.id = analysis.project_id AND project.user_id = analysis.user_id
                JOIN project_google_chat_sources source ON source.id = analysis.source_id AND source.user_id = analysis.user_id
                JOIN google_chat_accounts account ON account.id = source.account_id AND account.user_id = analysis.user_id
                JOIN project_inflow_items representative ON representative.id = analysis.representative_item_id
                WHERE analysis.user_id = $1 AND source.enabled AND account.status = 'active'
                    AND analysis.state = 'ready' AND analysis.analyzed_revision = analysis.source_revision
                    AND (analysis.classification IN ('new_task', 'follow_up', 'question')
                        OR (analysis.classification = 'status_update' AND
                            COALESCE(analysis.linked_task_id, representative.promoted_task_id) IS NOT NULL))
                    AND representative.received_at >= source.created_at
                    AND representative.received_at >= $4 - INTERVAL '1 day'
                    AND representative.status = 'pending' AND representative.reviewed_at IS NULL
                    AND representative.sender_provider_name IS DISTINCT FROM CONCAT('users/', account.provider_subject)
                UNION ALL
                SELECT candidate.id, 'gmail_inflow'::TEXT, candidate.source_revision,
                    candidate.promoted_project_id, candidate.promoted_task_id IS NOT NULL,
                    COALESCE(candidate.suggested_task_title, candidate.summary, '새 메일'),
                    COALESCE(candidate.summary, '새 메일을 확인해 주세요.'),
                    FLOOR(EXTRACT(EPOCH FROM candidate.analyzed_at) * 1000)::BIGINT
                FROM gmail_inflow_candidates candidate
                JOIN gmail_accounts account ON account.id = candidate.account_id AND account.user_id = candidate.user_id
                JOIN gmail_messages message ON message.id = candidate.representative_message_id
                WHERE candidate.user_id = $1 AND account.status = 'active'
                    AND candidate.analysis_state = 'ready' AND candidate.analyzed_revision = candidate.source_revision
                    AND candidate.decision_status = 'pending'
                    AND (candidate.classification IN ('new_task', 'follow_up', 'question')
                        OR (candidate.classification = 'status_update' AND candidate.promoted_task_id IS NOT NULL))
                    AND message.received_at >= account.created_at
                    AND message.received_at >= $4 - INTERVAL '1 day'
            ) SELECT * FROM eligible
              WHERE (occurred_at_epoch_millis, id) > ($2, $3)
                AND occurred_at_epoch_millis <= $5
              ORDER BY occurred_at_epoch_millis, id LIMIT 201",
        )
        .bind(user_id)
        .bind(after.max(oldest))
        .bind(after_id)
        .bind(now)
        .bind(now_millis)
        .fetch_all(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)
    }
}

pub(crate) fn epoch_millis(value: OffsetDateTime) -> i64 {
    value.unix_timestamp() * 1_000 + i64::from(value.millisecond())
}

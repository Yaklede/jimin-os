//! Owner-scoped delivery metadata, never provider payloads or credentials.
use crate::{Database, StorageError};
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryRecovery {
    pub id: Uuid,
    pub kind: String,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub status: String,
    pub can_retry: bool,
    pub attempt_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String)]
    pub created_at: OffsetDateTime,
    pub error_code: Option<String>,
    pub reference_url: Option<String>,
}
impl Database {
    /// Only unresolved deliveries, not successful history.
    /// # Errors
    /// Returns persistence errors.
    pub async fn delivery_recovery_for_user(
        &self,
        user: Uuid,
    ) -> Result<Vec<DeliveryRecovery>, StorageError> {
        sqlx::query_as(
            "SELECT * FROM (
              SELECT d.id,'webhook'::TEXT AS kind,d.project_id,
                COALESCE(NULLIF(d.payload->>'title',''),p.title,'프로젝트 알림') AS title,
                d.status,d.status='failed' AS can_retry,d.attempt_count,d.created_at,
                NULL::TEXT AS error_code,NULL::TEXT AS reference_url
              FROM webhook_deliveries d LEFT JOIN projects p ON p.id=d.project_id AND p.user_id=d.user_id
              WHERE d.user_id=$1 AND d.status IN ('failed','retry_wait')
              UNION ALL
              SELECT d.id,'push'::TEXT AS kind,d.project_id,d.title,d.status,
                COALESCE(d.status='failed' AND r.status='active' AND d.target_at>NOW(),FALSE) AS can_retry,
                d.attempt_count,d.created_at,NULL::TEXT AS error_code,NULL::TEXT AS reference_url
              FROM push_deliveries d LEFT JOIN push_registrations r ON r.device_id=d.device_id AND r.user_id=d.user_id
              WHERE d.user_id=$1 AND d.status IN ('failed','retry_wait')
              UNION ALL
              SELECT j.id,'itsm'::TEXT AS kind,t.project_id,t.title,j.status,j.status='failed' AS can_retry,
                j.attempt_count,j.created_at,j.last_error_code AS error_code,j.issue_url AS reference_url
              FROM itsm_issue_sync_jobs j JOIN LATERAL (
                SELECT t.project_id,t.title FROM task_itsm_links l JOIN tasks t ON t.id=l.task_id AND t.user_id=l.user_id
                WHERE l.issue_url=j.issue_url AND l.enabled AND l.user_id=$1 ORDER BY l.created_at,l.task_id LIMIT 1
              ) t ON TRUE WHERE j.status<>'succeeded'
            ) failures ORDER BY created_at DESC,id DESC LIMIT 200"
        ).bind(user).fetch_all(self.pool()).await.map_err(|_| StorageError::PersistenceUnavailable)
    }
    /// Reuses the receipt and worker relevance checks; never repeats a successful notification.
    /// # Errors
    /// Returns validation or persistence errors.
    pub async fn retry_failed_push_delivery(
        &self,
        user: Uuid,
        id: Uuid,
    ) -> Result<bool, StorageError> {
        if user.get_version_num() != 7 || id.get_version_num() != 7 {
            return Err(StorageError::InvalidConfiguration);
        }
        let result=sqlx::query(
            "UPDATE push_deliveries d SET status='queued',attempt_count=0,next_attempt_at=NULL,
             response_code=NULL,last_error_code=NULL,lease_owner=NULL,lease_expires_at=NULL
             WHERE d.user_id=$1 AND d.id=$2 AND d.status='failed'
             AND d.target_at>NOW()
             AND EXISTS (SELECT 1 FROM push_registrations r WHERE r.user_id=d.user_id AND r.device_id=d.device_id AND r.status='active')"
        ).bind(user).bind(id).execute(self.pool()).await.map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(result.rows_affected() == 1)
    }
}

//! Paged live task evidence using the same leaf-task rules as weekly counters.
use crate::{
    Database, StorageError,
    planning::{Task, TaskRow},
};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub struct ReportEvidenceQuery {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub metric: String,
    pub period_start: OffsetDateTime,
    pub period_end: OffsetDateTime,
    pub after: Option<Uuid>,
}

impl Database {
    /// Returns at most 101 rows (one look-ahead), never tasks outside the owned project.
    ///
    /// # Errors
    /// Invalid IDs, metrics or bounds are rejected before querying persistence.
    pub async fn weekly_report_evidence(
        &self,
        query: &ReportEvidenceQuery,
    ) -> Result<Vec<Task>, StorageError> {
        if query.user_id.get_version_num() != 7
            || query.project_id.get_version_num() != 7
            || query.after.is_some_and(|id| id.get_version_num() != 7)
            || query.period_start >= query.period_end
            || query.period_end - query.period_start > Duration::days(8)
            || query.period_end > OffsetDateTime::now_utc() + Duration::minutes(1)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        if !matches!(
            query.metric.as_str(),
            "created"
                | "completed"
                | "backlog"
                | "overdue"
                | "stale"
                | "unassigned"
                | "waiting"
                | "on_time"
        ) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_,TaskRow>("SELECT t.id,t.user_id,t.project_id,t.parent_task_id,t.title,t.notes,t.assignee_name,t.status,t.priority,t.due_at,t.completed_at,t.work_kind,t.completion_note,t.workflow_state,t.waiting_reason,t.version FROM tasks t JOIN projects p ON p.id=t.project_id AND p.user_id=t.user_id WHERE t.user_id=$1 AND p.id=$2 AND p.reporting_enabled=TRUE AND ($5::UUID IS NULL OR t.id>$5) AND NOT EXISTS(SELECT 1 FROM tasks c WHERE c.parent_task_id=t.id AND c.status<>'cancelled') AND CASE $6::TEXT
          WHEN 'created' THEN t.status<>'cancelled' AND t.created_at>=$3 AND t.created_at<$4
          WHEN 'completed' THEN t.status='completed' AND t.completed_at>=$3 AND t.completed_at<$4
          WHEN 'backlog' THEN t.status='open' AND t.created_at<$4
          WHEN 'overdue' THEN t.status='open' AND t.due_at<$4
          WHEN 'stale' THEN t.status='open' AND t.updated_at<$4-make_interval(days=>p.stale_threshold_days::INTEGER)
          WHEN 'unassigned' THEN t.status='open' AND t.assignee_name IS NULL
          WHEN 'waiting' THEN t.status='open' AND t.workflow_state='waiting'
          WHEN 'on_time' THEN t.status='completed' AND t.completed_at>=$3 AND t.completed_at<$4 AND t.due_at IS NOT NULL
          ELSE FALSE END ORDER BY t.id LIMIT 101").bind(query.user_id).bind(query.project_id).bind(query.period_start).bind(query.period_end).bind(query.after).bind(&query.metric)
            .fetch_all(self.pool()).await.map_err(|_|StorageError::PersistenceUnavailable)?
            .into_iter().map(Task::try_from).collect()
    }
}

//! Live weekly operating reports derived from project and task state.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

/// One project's contribution to the current weekly report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyProjectReport {
    pub project_id: Uuid,
    pub title: String,
    pub management_mode: String,
    pub created_task_count: i64,
    pub completed_task_count: i64,
    pub backlog_start_count: i64,
    pub backlog_end_count: i64,
    pub overdue_task_count: i64,
    pub stale_task_count: i64,
    pub unassigned_task_count: i64,
    pub average_cycle_time_hours: i64,
    pub on_time_completion_percent: Option<i16>,
}

/// A workspace report for the current Monday-to-now period in Korea time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyWorkspaceReport {
    pub workspace_id: Uuid,
    pub period_start: OffsetDateTime,
    pub period_end: OffsetDateTime,
    pub projects: Vec<WeeklyProjectReport>,
}

#[derive(sqlx::FromRow)]
struct WeeklyProjectReportRow {
    project_id: Uuid,
    title: String,
    management_mode: String,
    period_start: OffsetDateTime,
    period_end: OffsetDateTime,
    created_task_count: i64,
    completed_task_count: i64,
    backlog_start_count: i64,
    backlog_end_count: i64,
    overdue_task_count: i64,
    stale_task_count: i64,
    unassigned_task_count: i64,
    average_cycle_time_hours: i64,
    on_time_completion_percent: Option<i16>,
}

impl Database {
    /// Builds a live weekly report for projects that opted into reporting.
    ///
    /// The week starts at Monday 00:00 in Asia/Seoul and ends at the current
    /// instant. Passing a project ID narrows the report without changing the
    /// workspace ownership boundary.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs and a
    /// classified persistence error when the database is unavailable.
    // Keeping the complete aggregate query here makes the report contract
    // reviewable as one statement instead of scattering its counters.
    #[allow(clippy::too_many_lines)]
    pub async fn weekly_report_for_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        project_id: Option<Uuid>,
    ) -> Result<WeeklyWorkspaceReport, StorageError> {
        if workspace_id.get_version_num() != 7
            || project_id.is_some_and(|value| value.get_version_num() != 7)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let period = sqlx::query_as::<_, (OffsetDateTime, OffsetDateTime)>(
            "\
            SELECT
                date_trunc('week', NOW() AT TIME ZONE 'Asia/Seoul')
                    AT TIME ZONE 'Asia/Seoul' AS period_start,
                NOW() AS period_end",
        )
        .fetch_one(self.pool())
        .await
        .map_err(classify)?;
        let rows = sqlx::query_as::<_, WeeklyProjectReportRow>(
            "\
            WITH bounds AS (
                SELECT $4::TIMESTAMPTZ AS period_start, $5::TIMESTAMPTZ AS period_end
            )
            SELECT
                project.id AS project_id,
                project.title,
                project.management_mode,
                bounds.period_start,
                bounds.period_end,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status <> 'cancelled'
                   AND task.created_at >= bounds.period_start
                   AND task.created_at < bounds.period_end
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS created_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status = 'completed'
                   AND task.completed_at >= bounds.period_start
                   AND task.completed_at < bounds.period_end
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS completed_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status <> 'cancelled'
                   AND task.created_at < bounds.period_start
                   AND (task.completed_at IS NULL OR task.completed_at >= bounds.period_start)
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS backlog_start_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status = 'open'
                   AND task.created_at < bounds.period_end
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS backlog_end_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status = 'open'
                   AND task.due_at < bounds.period_end
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS overdue_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status = 'open'
                   AND task.updated_at < bounds.period_end
                       - make_interval(days => project.stale_threshold_days::INTEGER)
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS stale_task_count,
                (SELECT COUNT(*)::BIGINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status = 'open'
                   AND task.assignee_name IS NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS unassigned_task_count,
                COALESCE((
                    SELECT (
                        EXTRACT(EPOCH FROM AVG(task.completed_at - task.created_at)) / 3600
                    )::BIGINT
                    FROM tasks AS task
                    WHERE task.project_id = project.id
                      AND task.status = 'completed'
                      AND task.completed_at >= bounds.period_start
                      AND task.completed_at < bounds.period_end
                      AND NOT EXISTS (
                          SELECT 1 FROM tasks AS child
                          WHERE child.parent_task_id = task.id
                            AND child.status <> 'cancelled'
                      )
                ), 0::BIGINT) AS average_cycle_time_hours,
                (SELECT (
                    COUNT(*) FILTER (WHERE task.completed_at <= task.due_at) * 100
                    / NULLIF(COUNT(*), 0)
                )::SMALLINT
                 FROM tasks AS task
                 WHERE task.project_id = project.id
                   AND task.status = 'completed'
                   AND task.completed_at >= bounds.period_start
                   AND task.completed_at < bounds.period_end
                   AND task.due_at IS NOT NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM tasks AS child
                       WHERE child.parent_task_id = task.id AND child.status <> 'cancelled'
                   )) AS on_time_completion_percent
            FROM projects AS project
            CROSS JOIN bounds
            WHERE project.user_id = $1
              AND project.workspace_id = $2
              AND project.reporting_enabled = TRUE
              AND ($3::UUID IS NULL OR project.id = $3)
            ORDER BY project.title, project.id",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(project_id)
        .bind(period.0)
        .bind(period.1)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        let projects = rows
            .into_iter()
            .map(|row| {
                debug_assert_eq!(row.period_start, period.0);
                debug_assert_eq!(row.period_end, period.1);
                WeeklyProjectReport {
                    project_id: row.project_id,
                    title: row.title,
                    management_mode: row.management_mode,
                    created_task_count: row.created_task_count,
                    completed_task_count: row.completed_task_count,
                    backlog_start_count: row.backlog_start_count,
                    backlog_end_count: row.backlog_end_count,
                    overdue_task_count: row.overdue_task_count,
                    stale_task_count: row.stale_task_count,
                    unassigned_task_count: row.unassigned_task_count,
                    average_cycle_time_hours: row.average_cycle_time_hours,
                    on_time_completion_percent: row.on_time_completion_percent,
                }
            })
            .collect();
        Ok(WeeklyWorkspaceReport {
            workspace_id,
            period_start: period.0,
            period_end: period.1,
            projects,
        })
    }
}

// `map_err` consumes `sqlx::Error`, so this adapter intentionally accepts it
// by value even though classification only inspects the variant.
#[allow(clippy::needless_pass_by_value)]
fn classify(error: sqlx::Error) -> StorageError {
    match error {
        sqlx::Error::Configuration(_)
        | sqlx::Error::Protocol(_)
        | sqlx::Error::TypeNotFound { .. }
        | sqlx::Error::Decode(_)
        | sqlx::Error::ColumnDecode { .. }
        | sqlx::Error::ColumnNotFound(_)
        | sqlx::Error::Migrate(_) => StorageError::InvalidConfiguration,
        _ => StorageError::PersistenceUnavailable,
    }
}

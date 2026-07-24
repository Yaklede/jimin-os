//! Live weekly operating reports derived from project and task state.

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

/// One project's contribution to the current weekly report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyWorkspaceReport {
    pub workspace_id: Uuid,
    pub period_start: OffsetDateTime,
    pub period_end: OffsetDateTime,
    pub projects: Vec<WeeklyProjectReport>,
}

/// A durable weekly report revision used for history and one-time reminders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyReportSnapshot {
    pub id: Uuid,
    pub generated_at: OffsetDateTime,
    pub report: WeeklyWorkspaceReport,
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

#[derive(sqlx::FromRow)]
struct WeeklyReportSnapshotRow {
    id: Uuid,
    workspace_id: Uuid,
    period_start: OffsetDateTime,
    period_end: OffsetDateTime,
    generated_at: OffsetDateTime,
    projects: Json<Vec<WeeklyProjectReport>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WeeklyReportTotals {
    created: i64,
    completed: i64,
    backlog_start: i64,
    backlog_end: i64,
    overdue: i64,
    stale: i64,
    unassigned: i64,
}

const WEEKLY_PROJECT_REPORT_QUERY: &str = "\
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
    ORDER BY project.title, project.id";

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
    pub async fn weekly_report_for_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        project_id: Option<Uuid>,
    ) -> Result<WeeklyWorkspaceReport, StorageError> {
        self.weekly_report_for_workspace_at(
            user_id,
            workspace_id,
            project_id,
            OffsetDateTime::now_utc(),
        )
        .await
    }

    /// Builds a deterministic weekly report ending at the supplied instant.
    ///
    /// This is used by the background snapshot worker and integration tests so
    /// the report period and counters share the same clock boundary.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration or persistence error.
    pub async fn weekly_report_for_workspace_at(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        project_id: Option<Uuid>,
        now: OffsetDateTime,
    ) -> Result<WeeklyWorkspaceReport, StorageError> {
        if workspace_id.get_version_num() != 7
            || project_id.is_some_and(|value| value.get_version_num() != 7)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let period = sqlx::query_as::<_, (OffsetDateTime, OffsetDateTime)>(
            "\
            SELECT
                date_trunc('week', $1 AT TIME ZONE 'Asia/Seoul')
                    AT TIME ZONE 'Asia/Seoul' AS period_start,
                $1::TIMESTAMPTZ AS period_end",
        )
        .bind(now)
        .fetch_one(self.pool())
        .await
        .map_err(classify)?;
        let rows = sqlx::query_as::<_, WeeklyProjectReportRow>(WEEKLY_PROJECT_REPORT_QUERY)
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

    /// Refreshes the current weekly snapshot for every owned workspace that
    /// has at least one reporting-enabled project.
    ///
    /// The `(owner, workspace, week)` key keeps the refresh idempotent. The
    /// snapshot ID and version stay stable so Friday push delivery is queued
    /// at most once while counters continue to update.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration or persistence error.
    pub async fn refresh_weekly_report_snapshots_for_user(
        &self,
        user_id: Uuid,
        now: OffsetDateTime,
    ) -> Result<usize, StorageError> {
        if user_id.get_version_num() != 7 {
            return Err(StorageError::InvalidConfiguration);
        }
        let workspaces = self.workspaces_for_user(user_id).await?;
        let mut refreshed = 0_usize;
        for workspace in workspaces {
            let report = self
                .weekly_report_for_workspace_at(user_id, workspace.id, None, now)
                .await?;
            if report.projects.is_empty() {
                continue;
            }
            self.save_weekly_report_snapshot(user_id, &report, now)
                .await?;
            refreshed += 1;
        }
        Ok(refreshed)
    }

    /// Returns newest-first weekly snapshots for one owned workspace.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration or persistence error.
    pub async fn weekly_report_history_for_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WeeklyReportSnapshot>, StorageError> {
        if user_id.get_version_num() != 7
            || workspace_id.get_version_num() != 7
            || !(1..=52).contains(&limit)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, WeeklyReportSnapshotRow>(
            "SELECT id, workspace_id, period_start, period_end, generated_at, projects
             FROM weekly_report_snapshots
             WHERE user_id = $1 AND workspace_id = $2
             ORDER BY period_start DESC
             LIMIT $3",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(rows
            .into_iter()
            .map(|row| WeeklyReportSnapshot {
                id: row.id,
                generated_at: row.generated_at,
                report: WeeklyWorkspaceReport {
                    workspace_id: row.workspace_id,
                    period_start: row.period_start,
                    period_end: row.period_end,
                    projects: row.projects.0,
                },
            })
            .collect())
    }

    async fn save_weekly_report_snapshot(
        &self,
        user_id: Uuid,
        report: &WeeklyWorkspaceReport,
        generated_at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        if report.period_end <= report.period_start || generated_at < report.period_start {
            return Err(StorageError::InvalidConfiguration);
        }
        let totals = weekly_report_totals(report);
        let projects = serde_json::to_value(&report.projects)
            .map_err(|_| StorageError::InvalidConfiguration)?;
        sqlx::query(
            "INSERT INTO weekly_report_snapshots (
                id, user_id, workspace_id, period_start, period_end,
                created_task_count, completed_task_count,
                backlog_start_count, backlog_end_count,
                overdue_task_count, stale_task_count, unassigned_task_count,
                projects, generated_at
             ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
             )
             ON CONFLICT (user_id, workspace_id, period_start) DO UPDATE SET
                period_end = EXCLUDED.period_end,
                created_task_count = EXCLUDED.created_task_count,
                completed_task_count = EXCLUDED.completed_task_count,
                backlog_start_count = EXCLUDED.backlog_start_count,
                backlog_end_count = EXCLUDED.backlog_end_count,
                overdue_task_count = EXCLUDED.overdue_task_count,
                stale_task_count = EXCLUDED.stale_task_count,
                unassigned_task_count = EXCLUDED.unassigned_task_count,
                projects = EXCLUDED.projects,
                generated_at = EXCLUDED.generated_at,
                updated_at = NOW()",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(report.workspace_id)
        .bind(report.period_start)
        .bind(report.period_end)
        .bind(totals.created)
        .bind(totals.completed)
        .bind(totals.backlog_start)
        .bind(totals.backlog_end)
        .bind(totals.overdue)
        .bind(totals.stale)
        .bind(totals.unassigned)
        .bind(projects)
        .bind(generated_at)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }
}

fn weekly_report_totals(report: &WeeklyWorkspaceReport) -> WeeklyReportTotals {
    report.projects.iter().fold(
        WeeklyReportTotals {
            created: 0,
            completed: 0,
            backlog_start: 0,
            backlog_end: 0,
            overdue: 0,
            stale: 0,
            unassigned: 0,
        },
        |totals, project| WeeklyReportTotals {
            created: totals.created + project.created_task_count,
            completed: totals.completed + project.completed_task_count,
            backlog_start: totals.backlog_start + project.backlog_start_count,
            backlog_end: totals.backlog_end + project.backlog_end_count,
            overdue: totals.overdue + project.overdue_task_count,
            stale: totals.stale + project.stale_task_count,
            unassigned: totals.unassigned + project.unassigned_task_count,
        },
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_snapshot_totals_preserve_attention_counts() {
        let report = WeeklyWorkspaceReport {
            workspace_id: Uuid::now_v7(),
            period_start: OffsetDateTime::UNIX_EPOCH,
            period_end: OffsetDateTime::UNIX_EPOCH + time::Duration::days(5),
            projects: vec![
                project_report(3, 2, 4, 5, 1, 2, 3),
                project_report(5, 4, 2, 1, 0, 1, 0),
            ],
        };

        assert_eq!(
            weekly_report_totals(&report),
            WeeklyReportTotals {
                created: 8,
                completed: 6,
                backlog_start: 6,
                backlog_end: 6,
                overdue: 1,
                stale: 3,
                unassigned: 3,
            }
        );
    }

    fn project_report(
        created: i64,
        completed: i64,
        backlog_start: i64,
        backlog_end: i64,
        overdue: i64,
        stale: i64,
        unassigned: i64,
    ) -> WeeklyProjectReport {
        WeeklyProjectReport {
            project_id: Uuid::now_v7(),
            title: "운영 프로젝트".to_owned(),
            management_mode: "operation".to_owned(),
            created_task_count: created,
            completed_task_count: completed,
            backlog_start_count: backlog_start,
            backlog_end_count: backlog_end,
            overdue_task_count: overdue,
            stale_task_count: stale,
            unassigned_task_count: unassigned,
            average_cycle_time_hours: 12,
            on_time_completion_percent: Some(80),
        }
    }
}

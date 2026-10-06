//! Durable report documents built from owner-scoped work data.

use serde_json::Value;
use sqlx::types::Json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

pub const PROJECT_WEEKLY_REPORT: &str = "project_weekly";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportStatus {
    Draft,
    Finalized,
    Archived,
    Failed,
}

impl ReportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Finalized => "finalized",
            Self::Archived => "archived",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub report_type: String,
    pub title: String,
    pub period_start: OffsetDateTime,
    pub period_end: OffsetDateTime,
    pub status: ReportStatus,
    pub current_version: i64,
    pub content: Value,
    pub generated_at: OffsetDateTime,
    pub finalized_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

pub struct NewReport {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub report_type: String,
    pub title: String,
    pub period_start: OffsetDateTime,
    pub period_end: OffsetDateTime,
    pub content: Value,
    pub generated_by: String,
    pub generated_at: OffsetDateTime,
}

pub struct ReportUpdate {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: Value,
    pub generated_by: String,
    pub expected_version: i64,
}

#[derive(sqlx::FromRow)]
struct ReportRow {
    id: Uuid,
    workspace_id: Uuid,
    project_id: Uuid,
    report_type: String,
    title: String,
    period_start: OffsetDateTime,
    period_end: OffsetDateTime,
    status: String,
    current_version: i64,
    content: Json<Value>,
    generated_at: OffsetDateTime,
    finalized_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
}

impl NewReport {
    fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !is_v7(self.workspace_id)
            || !is_v7(self.project_id)
            || self.report_type != PROJECT_WEEKLY_REPORT
            || !valid_text(&self.title, 200)
            || !valid_text(&self.generated_by, 32)
            || self.period_start >= self.period_end
            || self.generated_at < self.period_start
            || !valid_content(&self.content)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl ReportUpdate {
    fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_text(&self.generated_by, 32)
            || self.expected_version <= 0
            || !valid_content(&self.content)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl Database {
    /// Creates the first durable version of a project report.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the report is invalid, the project is not
    /// owned by the caller, or the database cannot commit the document.
    pub async fn create_report(&self, report: &NewReport) -> Result<Report, StorageError> {
        report.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        ensure_report_scope(
            &mut transaction,
            report.user_id,
            report.workspace_id,
            report.project_id,
        )
        .await?;
        let _row = sqlx::query_as::<_, ReportRow>(
            "INSERT INTO reports (
                id, user_id, workspace_id, project_id, report_type, title,
                period_start, period_end, generated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id, workspace_id, project_id, report_type, title,
                period_start, period_end, status, current_version,
                '{}'::JSONB AS content, generated_at, finalized_at,
                created_at, updated_at, version",
        )
        .bind(report.id)
        .bind(report.user_id)
        .bind(report.workspace_id)
        .bind(report.project_id)
        .bind(&report.report_type)
        .bind(&report.title)
        .bind(report.period_start)
        .bind(report.period_end)
        .bind(report.generated_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let content = Json(report.content.clone());
        sqlx::query(
            "INSERT INTO report_versions (
                id, report_id, version, content, generated_by
             ) VALUES ($1, $2, 1, $3, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(report.id)
        .bind(content)
        .bind(&report.generated_by)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        self.report_for_user(report.user_id, report.id).await
    }

    /// Lists the caller's saved reports for one project.
    ///
    /// # Errors
    ///
    /// Returns a storage error when identifiers or the requested limit are
    /// invalid, or when the database cannot read the report history.
    pub async fn reports_for_project(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Report>, StorageError> {
        if !is_v7(user_id)
            || !is_v7(workspace_id)
            || !is_v7(project_id)
            || !(1..=52).contains(&limit)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, ReportRow>(
            "SELECT report.id, report.workspace_id, report.project_id,
                report.report_type, report.title, report.period_start,
                report.period_end, report.status, report.current_version,
                version.content, report.generated_at, report.finalized_at,
                report.created_at, report.updated_at, report.version
             FROM reports AS report
             JOIN report_versions AS version
               ON version.report_id = report.id
              AND version.version = report.current_version
             WHERE report.user_id = $1
               AND report.workspace_id = $2
               AND report.project_id = $3
             ORDER BY report.period_start DESC, report.updated_at DESC
             LIMIT $4",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(project_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(Report::try_from).collect()
    }

    /// Loads one saved report while enforcing the owner boundary.
    ///
    /// # Errors
    ///
    /// Returns a storage error when identifiers are invalid, the report does
    /// not belong to the caller, or the database cannot read the report.
    pub async fn report_for_user(
        &self,
        user_id: Uuid,
        report_id: Uuid,
    ) -> Result<Report, StorageError> {
        if !is_v7(user_id) || !is_v7(report_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, ReportRow>(
            "SELECT report.id, report.workspace_id, report.project_id,
                report.report_type, report.title, report.period_start,
                report.period_end, report.status, report.current_version,
                version.content, report.generated_at, report.finalized_at,
                report.created_at, report.updated_at, report.version
             FROM reports AS report
             JOIN report_versions AS version
               ON version.report_id = report.id
              AND version.version = report.current_version
             WHERE report.user_id = $1 AND report.id = $2",
        )
        .bind(user_id)
        .bind(report_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?
        .ok_or(StorageError::IdentityConflict)?;
        Report::try_from(row)
    }

    /// Appends a user or assistant revision to a draft report.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the revision is invalid or the database
    /// cannot commit it. A missing report, finalized report, or stale version
    /// is returned as `Ok(None)` for the API to map to a conflict response.
    pub async fn update_report(
        &self,
        update: &ReportUpdate,
    ) -> Result<Option<Report>, StorageError> {
        update.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let Some((workspace_id, project_id, status, current_version)) =
            sqlx::query_as::<_, (Uuid, Uuid, String, i64)>(
                "SELECT workspace_id, project_id, status, current_version
                 FROM reports
                 WHERE id = $1 AND user_id = $2
                 FOR UPDATE",
            )
            .bind(update.id)
            .bind(update.user_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?
        else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        if status != ReportStatus::Draft.as_str() || current_version != update.expected_version {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        }
        let next_version = current_version + 1;
        sqlx::query(
            "INSERT INTO report_versions (
                id, report_id, version, content, generated_by
             ) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::now_v7())
        .bind(update.id)
        .bind(next_version)
        .bind(Json(update.content.clone()))
        .bind(&update.generated_by)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        sqlx::query(
            "UPDATE reports
             SET current_version = $1, updated_at = NOW(), version = version + 1
             WHERE id = $2 AND user_id = $3",
        )
        .bind(next_version)
        .bind(update.id)
        .bind(update.user_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        let _ = (workspace_id, project_id);
        self.report_for_user(update.user_id, update.id)
            .await
            .map(Some)
    }

    /// Finalizes a draft report with an optimistic version check.
    ///
    /// # Errors
    ///
    /// Returns a storage error when identifiers are invalid or the database
    /// cannot commit the state change. A missing, finalized, or stale report is
    /// returned as `Ok(None)` for the API to map to a conflict response.
    pub async fn finalize_report(
        &self,
        user_id: Uuid,
        report_id: Uuid,
        expected_version: i64,
    ) -> Result<Option<Report>, StorageError> {
        if !is_v7(user_id) || !is_v7(report_id) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, ReportRow>(
            "UPDATE reports
             SET status = 'finalized', finalized_at = NOW(),
                 updated_at = NOW(), version = version + 1
             WHERE id = $1 AND user_id = $2 AND status = 'draft'
               AND version = $3
             RETURNING id, workspace_id, project_id, report_type, title,
                period_start, period_end, status, current_version,
                '{}'::JSONB AS content, generated_at, finalized_at,
                created_at, updated_at, version",
        )
        .bind(report_id)
        .bind(user_id)
        .bind(expected_version)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        let Some(row) = row else { return Ok(None) };
        self.report_for_user(user_id, row.id).await.map(Some)
    }
}

impl TryFrom<ReportRow> for Report {
    type Error = StorageError;

    fn try_from(row: ReportRow) -> Result<Self, Self::Error> {
        let status = match row.status.as_str() {
            "draft" => ReportStatus::Draft,
            "finalized" => ReportStatus::Finalized,
            "archived" => ReportStatus::Archived,
            "failed" => ReportStatus::Failed,
            _ => return Err(StorageError::InvalidConfiguration),
        };
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            project_id: row.project_id,
            report_type: row.report_type,
            title: row.title,
            period_start: row.period_start,
            period_end: row.period_end,
            status,
            current_version: row.current_version,
            content: row.content.0,
            generated_at: row.generated_at,
            finalized_at: row.finalized_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}

async fn ensure_report_scope(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
    project_id: Uuid,
) -> Result<(), StorageError> {
    let owned = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM projects
            WHERE id = $1 AND user_id = $2 AND workspace_id = $3
        )",
    )
    .bind(project_id)
    .bind(user_id)
    .bind(workspace_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)?;
    if !owned {
        return Err(StorageError::IdentityConflict);
    }
    Ok(())
}

fn valid_content(value: &Value) -> bool {
    if !value.is_object() || value.to_string().len() > 100_000 {
        return false;
    }
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("kind").and_then(Value::as_str) != Some(PROJECT_WEEKLY_REPORT)
        || object
            .get("summary")
            .and_then(Value::as_str)
            .is_none_or(|summary| !valid_text(summary, 5_000))
    {
        return false;
    }
    let Some(period) = object.get("period").and_then(Value::as_object) else {
        return false;
    };
    if !matches!(period.get("start").and_then(Value::as_str), Some(start) if valid_text(start, 64))
        || !matches!(period.get("end").and_then(Value::as_str), Some(end) if valid_text(end, 64))
    {
        return false;
    }
    let Some(metrics) = object.get("metrics").and_then(Value::as_array) else {
        return false;
    };
    if metrics.len() > 32
        || metrics.iter().any(|metric| {
            let Some(metric) = metric.as_object() else {
                return true;
            };
            !matches!(metric.get("key").and_then(Value::as_str), Some(key) if valid_text(key, 80))
                || !matches!(metric.get("label").and_then(Value::as_str), Some(label) if valid_text(label, 120))
                || !metric
                    .get("value")
                    .is_some_and(|value| value.is_null() || value.is_number())
        })
    {
        return false;
    }
    let Some(focus) = object.get("focus").and_then(Value::as_array) else {
        return false;
    };
    if focus.len() > 32
        || focus
            .iter()
            .any(|item| !matches!(item.as_str(), Some(text) if valid_text(text, 500)))
    {
        return false;
    }
    object
        .get("evidence")
        .and_then(Value::as_array)
        .is_some_and(|evidence| {
            evidence.len() <= 32
                && evidence.iter().all(|item| {
                    let Some(item) = item.as_object() else {
                        return false;
                    };
                    matches!(item.get("type").and_then(Value::as_str), Some(value) if valid_text(value, 80))
                        && matches!(item.get("workspaceId").and_then(Value::as_str), Some(value) if valid_text(value, 80))
                        && matches!(item.get("projectId").and_then(Value::as_str), Some(value) if valid_text(value, 80))
                })
        })
}

fn valid_text(value: &str, max_chars: usize) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.chars().count() <= max_chars
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

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
    fn report_content_must_be_a_bounded_object() {
        assert!(valid_content(&serde_json::json!({
            "kind": PROJECT_WEEKLY_REPORT,
            "period": {"start": "2026-07-27T00:00:00Z", "end": "2026-08-03T00:00:00Z"},
            "summary": "ok",
            "metrics": [],
            "focus": [],
            "evidence": []
        })));
        assert!(!valid_content(&serde_json::json!({"summary": "ok"})));
        assert!(!valid_content(&serde_json::json!(["not a report"])));
        assert!(!valid_content(&serde_json::json!(null)));
    }

    #[test]
    fn status_names_are_stable_for_api_contracts() {
        assert_eq!(ReportStatus::Draft.as_str(), "draft");
        assert_eq!(ReportStatus::Finalized.as_str(), "finalized");
    }
}

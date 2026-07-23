//! Durable AI classification for Google Chat project inflow.
//!
//! Provider messages stay immutable in `project_inflow_items`. This module
//! maintains one revisioned analysis per source conversation and only exposes
//! validated, structured task candidates to the application layer.

use std::time::Duration;

use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

const MAX_SUMMARY_CHARS: usize = 2_000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_DETAIL_CHARS: usize = 2_000;
const MAX_ASSIGNEE_CHARS: usize = 80;
const MAX_ACTION_ITEMS: usize = 8;
const MAX_ERROR_CODE_BYTES: usize = 120;
const ANALYSIS_VERSION: &str = "inflow-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflowAnalysisState {
    Queued,
    Claimed,
    Running,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflowClassification {
    NewTask,
    FollowUp,
    Question,
    StatusUpdate,
    Noise,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInflowAnalysis {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_id: Uuid,
    pub conversation_key: String,
    pub representative_item_id: Uuid,
    pub state: InflowAnalysisState,
    pub classification: Option<InflowClassification>,
    pub confidence: Option<i16>,
    pub summary: Option<String>,
    pub suggested_task_title: Option<String>,
    pub suggested_action_items: Vec<String>,
    pub suggested_completion_criteria: Option<String>,
    pub suggested_assignee_name: Option<String>,
    pub suggested_due_at: Option<OffsetDateTime>,
    pub suggested_priority: Option<i16>,
    pub linked_task_id: Option<Uuid>,
    pub error_code: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InflowAnalysisMessage {
    pub id: Uuid,
    pub sender_name: Option<String>,
    pub sent_by_owner: bool,
    pub content_text: String,
    pub received_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedInflowAnalysis {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub project_title: String,
    pub source_id: Uuid,
    pub source_name: String,
    pub conversation_key: String,
    pub representative_item_id: Uuid,
    pub source_revision: i32,
    pub messages: Vec<InflowAnalysisMessage>,
    pub linked_task_id: Option<Uuid>,
    pub linked_task_title: Option<String>,
    pub linked_task_notes: Option<String>,
    pub linked_task_assignee_name: Option<String>,
    pub assignee_options: Vec<String>,
    pub processing_model_id: Option<String>,
    pub processing_reasoning_effort: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InflowAnalysisResult {
    pub classification: InflowClassification,
    pub confidence: i16,
    pub summary: String,
    pub suggested_task_title: Option<String>,
    pub suggested_action_items: Vec<String>,
    pub suggested_completion_criteria: Option<String>,
    pub suggested_assignee_name: Option<String>,
    pub suggested_due_at: Option<OffsetDateTime>,
    pub suggested_priority: Option<i16>,
}

#[derive(sqlx::FromRow)]
struct ProjectInflowAnalysisRow {
    id: Uuid,
    project_id: Uuid,
    source_id: Uuid,
    conversation_key: String,
    representative_item_id: Uuid,
    state: String,
    classification: Option<String>,
    confidence: Option<i16>,
    summary: Option<String>,
    suggested_task_title: Option<String>,
    suggested_action_items: Vec<String>,
    suggested_completion_criteria: Option<String>,
    suggested_assignee_name: Option<String>,
    suggested_due_at: Option<OffsetDateTime>,
    suggested_priority: Option<i16>,
    linked_task_id: Option<Uuid>,
    error_code: Option<String>,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct ClaimedInflowAnalysisRow {
    id: Uuid,
    user_id: Uuid,
    project_id: Uuid,
    project_title: String,
    source_id: Uuid,
    source_name: String,
    conversation_key: String,
    representative_item_id: Uuid,
    source_revision: i32,
    linked_task_id: Option<Uuid>,
    linked_task_title: Option<String>,
    linked_task_notes: Option<String>,
    linked_task_assignee_name: Option<String>,
    assignee_options: Vec<String>,
    processing_model_id: Option<String>,
    processing_reasoning_effort: Option<String>,
}

#[derive(sqlx::FromRow)]
struct InflowAnalysisMessageRow {
    id: Uuid,
    sender_name: Option<String>,
    sent_by_owner: bool,
    content_text: String,
    received_at: OffsetDateTime,
}

impl InflowAnalysisResult {
    /// Validates that the structured result matches its classification contract.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when required task fields
    /// are missing, non-task fields leak into another classification, or a
    /// bounded value exceeds its accepted range.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !(0..=100).contains(&self.confidence)
            || !valid_text(&self.summary, MAX_SUMMARY_CHARS)
            || self.suggested_action_items.len() > MAX_ACTION_ITEMS
            || self
                .suggested_action_items
                .iter()
                .any(|value| !valid_text(value, MAX_DETAIL_CHARS))
            || self
                .suggested_assignee_name
                .as_deref()
                .is_some_and(|value| !valid_text(value, MAX_ASSIGNEE_CHARS))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        if self.classification == InflowClassification::NewTask {
            if !self
                .suggested_task_title
                .as_deref()
                .is_some_and(|value| valid_text(value, MAX_TITLE_CHARS))
                || self.suggested_action_items.is_empty()
                || !self
                    .suggested_completion_criteria
                    .as_deref()
                    .is_some_and(|value| valid_text(value, MAX_DETAIL_CHARS))
                || !self
                    .suggested_priority
                    .is_some_and(|value| (0..=3).contains(&value))
            {
                return Err(StorageError::InvalidConfiguration);
            }
        } else if self.suggested_task_title.is_some()
            || !self.suggested_action_items.is_empty()
            || self.suggested_completion_criteria.is_some()
            || self.suggested_assignee_name.is_some()
            || self.suggested_due_at.is_some()
            || self.suggested_priority.is_some()
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl TryFrom<ProjectInflowAnalysisRow> for ProjectInflowAnalysis {
    type Error = StorageError;

    fn try_from(row: ProjectInflowAnalysisRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            project_id: row.project_id,
            source_id: row.source_id,
            conversation_key: row.conversation_key,
            representative_item_id: row.representative_item_id,
            state: parse_state(&row.state)?,
            classification: row
                .classification
                .as_deref()
                .map(parse_classification)
                .transpose()?,
            confidence: row.confidence,
            summary: row.summary,
            suggested_task_title: row.suggested_task_title,
            suggested_action_items: row.suggested_action_items,
            suggested_completion_criteria: row.suggested_completion_criteria,
            suggested_assignee_name: row.suggested_assignee_name,
            suggested_due_at: row.suggested_due_at,
            suggested_priority: row.suggested_priority,
            linked_task_id: row.linked_task_id,
            error_code: row.error_code,
            version: row.version,
        })
    }
}

impl Database {
    /// Lists the latest persisted inflow analyses owned by one user.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the owner is invalid or
    /// the analyses cannot be read.
    pub async fn project_inflow_analyses_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ProjectInflowAnalysis>, StorageError> {
        analyses(self, user_id, None).await
    }

    /// Lists the latest persisted inflow analyses for one owned project.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the identifiers are
    /// invalid or the analyses cannot be read.
    pub async fn project_inflow_analyses(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ProjectInflowAnalysis>, StorageError> {
        analyses(self, user_id, Some(project_id)).await
    }

    /// Claims the oldest queued inflow conversation for one worker.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the worker lease is
    /// invalid or the claim and its contextual data cannot be loaded.
    #[allow(
        clippy::too_many_lines,
        reason = "The claim query atomically selects one job and loads its full AI context."
    )]
    pub async fn claim_next_inflow_analysis(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedInflowAnalysis>, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let row = sqlx::query_as::<_, ClaimedInflowAnalysisRow>(
            "WITH recovered AS (
                UPDATE project_inflow_analyses
                SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL
                WHERE state = 'claimed' AND claim_expires_at < NOW()
             ), candidate AS (
                SELECT id FROM project_inflow_analyses
                WHERE state = 'queued'
                ORDER BY created_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             ), claimed AS (
                UPDATE project_inflow_analyses AS analysis
                SET state = 'claimed', claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE analysis.id = candidate.id
                RETURNING analysis.*
             )
             SELECT claimed.id, claimed.user_id, claimed.project_id,
                project.title AS project_title, claimed.source_id,
                source.display_name AS source_name, claimed.conversation_key,
                claimed.representative_item_id, claimed.source_revision,
                linked_task.id AS linked_task_id,
                linked_task.title AS linked_task_title,
                linked_task.notes AS linked_task_notes,
                linked_task.assignee_name AS linked_task_assignee_name,
                COALESCE(assignees.names, '{}') AS assignee_options,
                selected_model.id AS processing_model_id,
                selected_effort.effort AS processing_reasoning_effort
             FROM claimed
             JOIN projects AS project ON project.id = claimed.project_id
             JOIN project_google_chat_sources AS source ON source.id = claimed.source_id
             LEFT JOIN LATERAL (
                SELECT task.id, task.title, task.notes, task.assignee_name
                FROM project_inflow_items AS item
                JOIN tasks AS task ON task.id = item.promoted_task_id
                WHERE item.source_id = claimed.source_id
                  AND (
                    (claimed.conversation_key LIKE 'thread:%'
                        AND item.provider_thread_name =
                            substr(claimed.conversation_key, 8))
                    OR
                    (claimed.conversation_key LIKE 'message:%'
                        AND item.provider_message_name =
                            substr(claimed.conversation_key, 9))
                  )
                ORDER BY item.updated_at DESC, item.id DESC
                LIMIT 1
             ) AS linked_task ON TRUE
             LEFT JOIN LATERAL (
                SELECT array_agg(DISTINCT mention.name ORDER BY mention.name) AS names
                FROM project_webhooks AS webhook
                CROSS JOIN LATERAL jsonb_each_text(
                    webhook.mention_directory -> 'users'
                ) AS mention(name, resource_name)
                WHERE webhook.project_id = claimed.project_id
                  AND webhook.provider = 'google_chat'
             ) AS assignees ON TRUE
             LEFT JOIN agent_preferences AS preference
                ON preference.user_id = claimed.user_id
             LEFT JOIN agent_models AS selected_model
                ON selected_model.id = preference.model_id
               AND selected_model.available = TRUE
             LEFT JOIN agent_models AS default_model
                ON default_model.is_default = TRUE AND default_model.available = TRUE
             LEFT JOIN agent_model_reasoning_efforts AS selected_effort
                ON selected_effort.model_id = COALESCE(selected_model.id, default_model.id)
               AND selected_effort.effort = preference.reasoning_effort",
        )
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let messages = analysis_messages(self, row.source_id, &row.conversation_key).await?;
        Ok(Some(ClaimedInflowAnalysis {
            id: row.id,
            user_id: row.user_id,
            project_id: row.project_id,
            project_title: row.project_title,
            source_id: row.source_id,
            source_name: row.source_name,
            conversation_key: row.conversation_key,
            representative_item_id: row.representative_item_id,
            source_revision: row.source_revision,
            messages,
            linked_task_id: row.linked_task_id,
            linked_task_title: row.linked_task_title,
            linked_task_notes: row.linked_task_notes,
            linked_task_assignee_name: row.linked_task_assignee_name,
            assignee_options: row.assignee_options,
            processing_model_id: row.processing_model_id,
            processing_reasoning_effort: row.processing_reasoning_effort,
        }))
    }

    /// Transitions a claimed analysis into active processing.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the lease inputs are
    /// invalid or the state transition cannot be stored.
    pub async fn start_inflow_analysis(
        &self,
        analysis_id: Uuid,
        runner_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        if !is_v7(analysis_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE project_inflow_analyses
             SET state = 'running',
                 claim_expires_at = NOW() + ($3 * INTERVAL '1 millisecond')
             WHERE id = $1 AND claim_owner = $2 AND state = 'claimed'
             RETURNING user_id, version",
        )
        .bind(analysis_id)
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        Ok(row.is_some())
    }

    /// Persists a structured result for the claimed source revision.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the result violates its
    /// contract, the assignee is unknown, or the atomic update cannot commit.
    pub async fn complete_inflow_analysis(
        &self,
        job: &ClaimedInflowAnalysis,
        runner_id: &str,
        result: &InflowAnalysisResult,
    ) -> Result<bool, StorageError> {
        result.validate()?;
        if !valid_runner_id(runner_id)
            || result
                .suggested_assignee_name
                .as_ref()
                .is_some_and(|name| !job.assignee_options.iter().any(|value| value == name))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current_revision = sqlx::query_scalar::<_, i32>(
            "SELECT source_revision
             FROM project_inflow_analyses
             WHERE id = $1 AND user_id = $2 AND claim_owner = $3
               AND state = 'running'
             FOR UPDATE",
        )
        .bind(job.id)
        .bind(job.user_id)
        .bind(runner_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(current_revision) = current_revision else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        if current_revision != job.source_revision {
            let version = sqlx::query_scalar::<_, i64>(
                "UPDATE project_inflow_analyses
                 SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL,
                     attempt_count = 0, error_code = NULL
                 WHERE id = $1 AND claim_owner = $2 AND state = 'running'
                 RETURNING version",
            )
            .bind(job.id)
            .bind(runner_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(classify)?;
            append_change(
                &mut transaction,
                job.user_id,
                "project_inflow_analysis",
                job.id,
                version,
            )
            .await?;
            transaction.commit().await.map_err(classify)?;
            return Ok(true);
        }
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE project_inflow_analyses
             SET state = 'ready', classification = $4, confidence = $5,
                 summary = $6, suggested_task_title = $7,
                 suggested_action_items = $8,
                 suggested_completion_criteria = $9,
                 suggested_assignee_name = $10, suggested_due_at = $11,
                 suggested_priority = $12, linked_task_id = $13,
                 analysis_model_id = $14, analysis_version = $15,
                 analyzed_revision = source_revision, analyzed_at = NOW(),
                 claim_owner = NULL, claim_expires_at = NULL,
                 error_code = NULL
             WHERE id = $1 AND user_id = $2 AND claim_owner = $3
               AND state = 'running'
             RETURNING version",
        )
        .bind(job.id)
        .bind(job.user_id)
        .bind(runner_id)
        .bind(classification_value(result.classification))
        .bind(result.confidence)
        .bind(result.summary.trim())
        .bind(trimmed_optional(result.suggested_task_title.as_deref()))
        .bind(trimmed_strings(&result.suggested_action_items))
        .bind(trimmed_optional(
            result.suggested_completion_criteria.as_deref(),
        ))
        .bind(trimmed_optional(result.suggested_assignee_name.as_deref()))
        .bind(result.suggested_due_at)
        .bind(result.suggested_priority)
        .bind(job.linked_task_id)
        .bind(job.processing_model_id.as_deref())
        .bind(ANALYSIS_VERSION)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            job.user_id,
            "project_inflow_analysis",
            job.id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Records a terminal worker failure or requeues a newer source revision.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when worker metadata is
    /// invalid or the state and sync event cannot commit together.
    pub async fn fail_inflow_analysis(
        &self,
        job: &ClaimedInflowAnalysis,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !valid_runner_id(runner_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (i32, i64)>(
            "UPDATE project_inflow_analyses
             SET state = CASE
                    WHEN source_revision = $4 THEN 'failed'
                    ELSE 'queued'
                 END,
                 claim_owner = NULL, claim_expires_at = NULL,
                 error_code = CASE
                    WHEN source_revision = $4 THEN $3
                    ELSE NULL
                 END,
                 attempt_count = CASE
                    WHEN source_revision = $4 THEN attempt_count
                    ELSE 0
                 END
             WHERE id = $1 AND claim_owner = $2
               AND state IN ('claimed', 'running')
             RETURNING source_revision, version",
        )
        .bind(job.id)
        .bind(runner_id)
        .bind(error_code)
        .bind(job.source_revision)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((_, version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        append_change(
            &mut transaction,
            job.user_id,
            "project_inflow_analysis",
            job.id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Requeues one failed analysis after an explicit user retry.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when identifiers or the
    /// expected item version are invalid, or the retry cannot commit.
    pub async fn retry_project_inflow_analysis(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        item_id: Uuid,
        expected_item_version: i64,
    ) -> Result<bool, StorageError> {
        if ![user_id, project_id, item_id].into_iter().all(is_v7) || expected_item_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE project_inflow_analyses AS analysis
             SET state = 'queued', attempt_count = 0, error_code = NULL
             FROM project_inflow_items AS item
             WHERE item.id = $3 AND item.user_id = $1 AND item.project_id = $2
               AND item.version = $4
               AND analysis.user_id = item.user_id
               AND analysis.project_id = item.project_id
               AND analysis.source_id = item.source_id
               AND analysis.conversation_key = COALESCE(
                    'thread:' || item.provider_thread_name,
                    'message:' || item.provider_message_name
               )
               AND analysis.state = 'failed'
             RETURNING analysis.id, analysis.version",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(item_id)
        .bind(expected_item_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((analysis_id, version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        append_change(
            &mut transaction,
            user_id,
            "project_inflow_analysis",
            analysis_id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Fails running analyses whose processing lease has expired.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the failure code is
    /// invalid or the recovered states and sync events cannot commit.
    pub async fn fail_expired_running_inflow_analyses(
        &self,
        error_code: &str,
    ) -> Result<u64, StorageError> {
        if !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let rows = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "UPDATE project_inflow_analyses
             SET state = 'failed', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = $1
             WHERE state = 'running' AND claim_expires_at < NOW()
             RETURNING id, user_id, version",
        )
        .bind(error_code)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        for (id, user_id, version) in &rows {
            append_change(
                &mut transaction,
                *user_id,
                "project_inflow_analysis",
                *id,
                *version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(rows.len() as u64)
    }
}

pub(crate) async fn queue_inflow_analysis_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    project_id: Uuid,
    source_id: Uuid,
    conversation_key: &str,
    representative_item_id: Uuid,
) -> Result<(), StorageError> {
    if ![user_id, project_id, source_id, representative_item_id]
        .into_iter()
        .all(is_v7)
        || conversation_key.trim().is_empty()
        || conversation_key.chars().count() > 2_048
    {
        return Err(StorageError::InvalidConfiguration);
    }
    let analysis_id = Uuid::now_v7();
    let (id, version) = sqlx::query_as::<_, (Uuid, i64)>(
        "INSERT INTO project_inflow_analyses (
            id, user_id, project_id, source_id, conversation_key,
            representative_item_id
         ) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (source_id, conversation_key) DO UPDATE
         SET representative_item_id = EXCLUDED.representative_item_id,
             source_revision = project_inflow_analyses.source_revision + 1,
             state = CASE
                WHEN project_inflow_analyses.state IN ('claimed', 'running')
                THEN project_inflow_analyses.state
                ELSE 'queued'
             END,
             classification = NULL, confidence = NULL, summary = NULL,
             suggested_task_title = NULL, suggested_action_items = '{}',
             suggested_completion_criteria = NULL,
             suggested_assignee_name = NULL, suggested_due_at = NULL,
             suggested_priority = NULL, analysis_model_id = NULL,
             analysis_version = NULL, analyzed_revision = NULL,
             analyzed_at = NULL, error_code = NULL,
             attempt_count = CASE
                WHEN project_inflow_analyses.state IN ('claimed', 'running')
                THEN project_inflow_analyses.attempt_count
                ELSE 0
             END
         RETURNING id, version",
    )
    .bind(analysis_id)
    .bind(user_id)
    .bind(project_id)
    .bind(source_id)
    .bind(conversation_key)
    .bind(representative_item_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)?;
    append_change(transaction, user_id, "project_inflow_analysis", id, version).await
}

async fn analyses(
    database: &Database,
    user_id: Uuid,
    project_id: Option<Uuid>,
) -> Result<Vec<ProjectInflowAnalysis>, StorageError> {
    if !is_v7(user_id) || project_id.is_some_and(|value| !is_v7(value)) {
        return Err(StorageError::InvalidConfiguration);
    }
    sqlx::query_as::<_, ProjectInflowAnalysisRow>(
        "SELECT id, project_id, source_id, conversation_key,
            representative_item_id, state, classification, confidence, summary,
            suggested_task_title, suggested_action_items,
            suggested_completion_criteria, suggested_assignee_name,
            suggested_due_at, suggested_priority, linked_task_id,
            error_code, version
         FROM project_inflow_analyses
         WHERE user_id = $1 AND ($2::UUID IS NULL OR project_id = $2)
         ORDER BY updated_at DESC, id DESC
         LIMIT 300",
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_all(database.pool())
    .await
    .map_err(classify)?
    .into_iter()
    .map(ProjectInflowAnalysis::try_from)
    .collect()
}

async fn analysis_messages(
    database: &Database,
    source_id: Uuid,
    conversation_key: &str,
) -> Result<Vec<InflowAnalysisMessage>, StorageError> {
    let rows = sqlx::query_as::<_, InflowAnalysisMessageRow>(
        "SELECT item.id,
            COALESCE(item.sender_name, (
                SELECT mention.name
                FROM project_webhooks AS webhook
                CROSS JOIN LATERAL jsonb_each_text(
                    webhook.mention_directory -> 'users'
                ) AS mention(name, resource_name)
                WHERE webhook.project_id = item.project_id
                  AND mention.resource_name = item.sender_provider_name
                ORDER BY webhook.created_at, mention.name
                LIMIT 1
            )) AS sender_name,
            COALESCE(
                item.sender_provider_name = CONCAT('users/', account.provider_subject),
                FALSE
            ) AS sent_by_owner,
            item.content_text, item.received_at
         FROM project_inflow_items AS item
         JOIN project_google_chat_sources AS source ON source.id = item.source_id
         JOIN google_chat_accounts AS account ON account.id = source.account_id
         WHERE item.source_id = $1
           AND (
                ($2 LIKE 'thread:%'
                    AND item.provider_thread_name = substr($2, 8))
                OR
                ($2 LIKE 'message:%'
                    AND item.provider_message_name = substr($2, 9))
           )
         ORDER BY item.received_at, item.id
         LIMIT 100",
    )
    .bind(source_id)
    .bind(conversation_key)
    .fetch_all(database.pool())
    .await
    .map_err(classify)?;
    Ok(rows
        .into_iter()
        .map(|row| InflowAnalysisMessage {
            id: row.id,
            sender_name: row.sender_name,
            sent_by_owner: row.sent_by_owner,
            content_text: row.content_text,
            received_at: row.received_at,
        })
        .collect())
}

fn parse_state(value: &str) -> Result<InflowAnalysisState, StorageError> {
    match value {
        "queued" => Ok(InflowAnalysisState::Queued),
        "claimed" => Ok(InflowAnalysisState::Claimed),
        "running" => Ok(InflowAnalysisState::Running),
        "ready" => Ok(InflowAnalysisState::Ready),
        "failed" => Ok(InflowAnalysisState::Failed),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_classification(value: &str) -> Result<InflowClassification, StorageError> {
    match value {
        "new_task" => Ok(InflowClassification::NewTask),
        "follow_up" => Ok(InflowClassification::FollowUp),
        "question" => Ok(InflowClassification::Question),
        "status_update" => Ok(InflowClassification::StatusUpdate),
        "noise" => Ok(InflowClassification::Noise),
        "duplicate" => Ok(InflowClassification::Duplicate),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

pub const fn classification_value(value: InflowClassification) -> &'static str {
    match value {
        InflowClassification::NewTask => "new_task",
        InflowClassification::FollowUp => "follow_up",
        InflowClassification::Question => "question",
        InflowClassification::StatusUpdate => "status_update",
        InflowClassification::Noise => "noise",
        InflowClassification::Duplicate => "duplicate",
    }
}

fn trimmed_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn trimmed_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn valid_text(value: &str, maximum: usize) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().count() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn claim_lease_millis(runner_id: &str, lease: Duration) -> Result<i64, StorageError> {
    if !valid_runner_id(runner_id) || lease.is_zero() {
        return Err(StorageError::InvalidConfiguration);
    }
    i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)
}

fn valid_runner_id(value: &str) -> bool {
    !value.is_empty() && value.chars().count() <= 200 && !value.chars().any(char::is_control)
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ERROR_CODE_BYTES
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{InflowAnalysisResult, InflowClassification};

    #[test]
    fn new_task_requires_structured_execution_details() {
        let result = InflowAnalysisResult {
            classification: InflowClassification::NewTask,
            confidence: 90,
            summary: "정산방식 표시를 추가해야 한다.".to_owned(),
            suggested_task_title: Some("거래내역 정산방식 표시 추가".to_owned()),
            suggested_action_items: vec!["거래내역 응답에 정산방식을 표시한다.".to_owned()],
            suggested_completion_criteria: Some(
                "거래내역에서 정산방식을 확인할 수 있다.".to_owned(),
            ),
            suggested_assignee_name: None,
            suggested_due_at: None,
            suggested_priority: Some(1),
        };
        assert!(result.validate().is_ok());
    }

    #[test]
    fn follow_up_cannot_leak_a_second_task_candidate() {
        let result = InflowAnalysisResult {
            classification: InflowClassification::FollowUp,
            confidence: 96,
            summary: "기존 업무의 확인 요청이다.".to_owned(),
            suggested_task_title: Some("다시 확인".to_owned()),
            suggested_action_items: Vec::new(),
            suggested_completion_criteria: None,
            suggested_assignee_name: None,
            suggested_due_at: None,
            suggested_priority: None,
        };
        assert!(result.validate().is_err());
    }
}

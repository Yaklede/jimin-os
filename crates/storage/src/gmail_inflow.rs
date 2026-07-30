//! Workspace-scoped Gmail assistant inbox.
//!
//! Provider messages remain immutable evidence. One candidate per account and
//! provider thread is analyzed under a lease, then explicitly promoted,
//! dismissed, or deferred by the owner.

use std::time::Duration;

use sqlx::{FromRow, Postgres, QueryBuilder};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::append_change,
    planning::{NewTask, Task, TaskStatus, queue_task_webhook_in_transaction},
};

const MAX_SUMMARY_CHARS: usize = 2_000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_DETAIL_CHARS: usize = 2_000;
const MAX_ASSIGNEE_CHARS: usize = 80;
const MAX_ACTION_ITEMS: usize = 8;
const MAX_RUNNER_BYTES: usize = 200;
const MAX_ERROR_BYTES: usize = 120;
const ANALYSIS_VERSION: &str = "gmail-inflow-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailInflowStatus {
    Attention,
    Pending,
    Promoted,
    Dismissed,
    Deferred,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailInflowAnalysisState {
    Queued,
    Claimed,
    Running,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailInflowClassification {
    NewTask,
    FollowUp,
    Question,
    StatusUpdate,
    Automated,
    Newsletter,
    Marketing,
    Noise,
    Duplicate,
}

impl GmailInflowClassification {
    #[must_use]
    pub const fn is_task(self) -> bool {
        matches!(self, Self::NewTask)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailInflowCandidate {
    pub id: Uuid,
    pub account_id: Uuid,
    pub account_email: String,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub workspace_scope: String,
    pub message_id: Uuid,
    pub provider_message_id: String,
    pub provider_thread_id: String,
    pub sender_name: Option<String>,
    pub sender_email: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub body_text: Option<String>,
    pub reference_links: Vec<String>,
    pub received_at: Option<OffsetDateTime>,
    pub analysis_state: GmailInflowAnalysisState,
    pub classification: Option<GmailInflowClassification>,
    pub confidence: Option<i16>,
    pub summary: Option<String>,
    pub suggested_task_title: Option<String>,
    pub suggested_action_items: Vec<String>,
    pub suggested_completion_criteria: Option<String>,
    pub suggested_assignee_name: Option<String>,
    pub suggested_due_at: Option<OffsetDateTime>,
    pub suggested_priority: Option<i16>,
    pub status: String,
    pub promoted_task_id: Option<Uuid>,
    pub deferred_until: Option<OffsetDateTime>,
    pub error_code: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GmailInflowCursor {
    pub created_at: OffsetDateTime,
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailInflowPage {
    pub items: Vec<GmailInflowCandidate>,
    pub next_cursor: Option<GmailInflowCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct GmailInflowMessage {
    pub id: Uuid,
    pub sender: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub body_text: Option<String>,
    pub reference_links: Vec<String>,
    pub received_at: Option<OffsetDateTime>,
    pub list_id: Option<String>,
    pub list_unsubscribe: bool,
    pub precedence: Option<String>,
    pub auto_submitted: Option<String>,
}

pub struct ClaimedGmailInflowAnalysis {
    pub id: Uuid,
    pub user_id: Uuid,
    pub account_id: Uuid,
    pub account_email: String,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub workspace_scope: String,
    pub provider_thread_id: String,
    pub source_revision: i32,
    pub messages: Vec<GmailInflowMessage>,
    pub processing_model_id: Option<String>,
    pub processing_reasoning_effort: Option<String>,
}

pub struct GmailInflowAnalysisResult {
    pub classification: GmailInflowClassification,
    pub confidence: i16,
    pub summary: String,
    pub suggested_task_title: Option<String>,
    pub suggested_action_items: Vec<String>,
    pub suggested_completion_criteria: Option<String>,
    pub suggested_assignee_name: Option<String>,
    pub suggested_due_at: Option<OffsetDateTime>,
    pub suggested_priority: Option<i16>,
}

pub struct PromoteGmailInflowCandidate {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub candidate_id: Uuid,
    pub expected_version: i64,
    pub project_id: Uuid,
    pub title: String,
    pub notes: Option<String>,
    pub assignee_name: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
}

#[derive(FromRow)]
struct GmailInflowCandidateRow {
    id: Uuid,
    account_id: Uuid,
    account_email: String,
    workspace_id: Uuid,
    workspace_name: String,
    workspace_scope: String,
    message_id: Uuid,
    provider_message_id: String,
    provider_thread_id: String,
    sender: Option<String>,
    subject: Option<String>,
    snippet: Option<String>,
    body_text: Option<String>,
    reference_links: Vec<String>,
    received_at: Option<OffsetDateTime>,
    analysis_state: String,
    classification: Option<String>,
    confidence: Option<i16>,
    summary: Option<String>,
    suggested_task_title: Option<String>,
    suggested_action_items: Vec<String>,
    suggested_completion_criteria: Option<String>,
    suggested_assignee_name: Option<String>,
    suggested_due_at: Option<OffsetDateTime>,
    suggested_priority: Option<i16>,
    decision_status: String,
    promoted_task_id: Option<Uuid>,
    deferred_until: Option<OffsetDateTime>,
    error_code: Option<String>,
    created_at: OffsetDateTime,
    version: i64,
}

#[derive(FromRow)]
struct ClaimedGmailInflowRow {
    id: Uuid,
    user_id: Uuid,
    account_id: Uuid,
    account_email: String,
    workspace_id: Uuid,
    workspace_name: String,
    workspace_scope: String,
    provider_thread_id: String,
    source_revision: i32,
    processing_model_id: Option<String>,
    processing_reasoning_effort: Option<String>,
}

impl GmailInflowAnalysisResult {
    /// Validates a structured classification before persistence.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration when bounded fields or the task contract
    /// do not match the selected classification.
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
        if self.classification == GmailInflowClassification::NewTask {
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

impl TryFrom<GmailInflowCandidateRow> for GmailInflowCandidate {
    type Error = StorageError;

    fn try_from(row: GmailInflowCandidateRow) -> Result<Self, Self::Error> {
        let (sender_name, sender_email) = parse_sender(row.sender.as_deref());
        Ok(Self {
            id: row.id,
            account_id: row.account_id,
            account_email: row.account_email,
            workspace_id: row.workspace_id,
            workspace_name: row.workspace_name,
            workspace_scope: row.workspace_scope,
            message_id: row.message_id,
            provider_message_id: row.provider_message_id,
            provider_thread_id: row.provider_thread_id,
            sender_name,
            sender_email,
            subject: row.subject,
            snippet: row.snippet,
            body_text: row.body_text,
            reference_links: row.reference_links,
            received_at: row.received_at,
            analysis_state: parse_analysis_state(&row.analysis_state)?,
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
            status: row.decision_status,
            promoted_task_id: row.promoted_task_id,
            deferred_until: row.deferred_until,
            error_code: row.error_code,
            version: row.version,
        })
    }
}

impl Database {
    /// Resolves the owned workspace of one candidate.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for malformed identifiers or a
    /// persistence error when the candidate cannot be read.
    pub async fn gmail_inflow_workspace_for_candidate(
        &self,
        user_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<Option<Uuid>, StorageError> {
        if !is_v7(user_id) || !is_v7(candidate_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_scalar(
            "SELECT workspace_id FROM gmail_inflow_candidates
             WHERE id = $1 AND user_id = $2",
        )
        .bind(candidate_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)
    }

    /// Lists candidates inside one explicit owned workspace. Attention returns
    /// only ready new-task analyses that have not been decided.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for a foreign workspace.
    pub async fn gmail_inflow_candidate_page(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        status: GmailInflowStatus,
        limit: i64,
        cursor: Option<GmailInflowCursor>,
    ) -> Result<GmailInflowPage, StorageError> {
        if !is_v7(user_id)
            || !is_v7(workspace_id)
            || !(1..=100).contains(&limit)
            || cursor.is_some_and(|value| !is_v7(value.id))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let owns_workspace = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2
             )",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(self.pool())
        .await
        .map_err(classify)?;
        if !owns_workspace {
            return Err(StorageError::InvalidConfiguration);
        }
        self.release_due_gmail_inflow_deferrals(user_id, workspace_id)
            .await?;
        let mut query = QueryBuilder::<Postgres>::new(candidate_select());
        query
            .push(" WHERE candidate.user_id = ")
            .push_bind(user_id)
            .push(" AND candidate.workspace_id = ")
            .push_bind(workspace_id)
            .push(" AND ")
            .push(status_predicate(status));
        if let Some(cursor) = cursor {
            query
                .push(" AND (candidate.created_at < ")
                .push_bind(cursor.created_at)
                .push(" OR (candidate.created_at = ")
                .push_bind(cursor.created_at)
                .push(" AND candidate.id < ")
                .push_bind(cursor.id)
                .push("))");
        }
        query
            .push(" ORDER BY candidate.created_at DESC, candidate.id DESC LIMIT ")
            .push_bind(limit + 1);
        let mut rows = query
            .build_query_as::<GmailInflowCandidateRow>()
            .fetch_all(self.pool())
            .await
            .map_err(classify)?;
        let has_more =
            rows.len() > usize::try_from(limit).map_err(|_| StorageError::InvalidConfiguration)?;
        if has_more {
            rows.pop();
        }
        let next_cursor = has_more
            .then(|| {
                rows.last().map(|row| GmailInflowCursor {
                    created_at: row.created_at,
                    id: row.id,
                })
            })
            .flatten();
        let items = rows
            .into_iter()
            .map(GmailInflowCandidate::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GmailInflowPage { items, next_cursor })
    }

    /// Loads one candidate owned by the user.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for malformed identifiers or a
    /// persistence error when the candidate cannot be read.
    pub async fn gmail_inflow_candidate(
        &self,
        user_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<Option<GmailInflowCandidate>, StorageError> {
        if !is_v7(user_id) || !is_v7(candidate_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut query = QueryBuilder::<Postgres>::new(candidate_select());
        query
            .push(" WHERE candidate.user_id = ")
            .push_bind(user_id)
            .push(" AND candidate.id = ")
            .push_bind(candidate_id);
        let row = query
            .build_query_as::<GmailInflowCandidateRow>()
            .fetch_optional(self.pool())
            .await
            .map_err(classify)?;
        row.map(GmailInflowCandidate::try_from).transpose()
    }

    /// Claims the oldest queued mailbox analysis without mixing workspaces.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when the lease or source messages cannot be
    /// loaded.
    pub async fn claim_next_gmail_inflow_analysis(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedGmailInflowAnalysis>, StorageError> {
        if !valid_runner(runner_id) || lease.is_zero() {
            return Err(StorageError::InvalidConfiguration);
        }
        let lease_millis =
            i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)?;
        let row = sqlx::query_as::<_, ClaimedGmailInflowRow>(
            "\
            WITH candidate AS (
                SELECT inflow.id
                FROM gmail_inflow_candidates AS inflow
                WHERE inflow.analysis_state = 'queued'
                  AND inflow.decision_status IN ('pending', 'promoted')
                ORDER BY inflow.created_at, inflow.id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            ), claimed AS (
                UPDATE gmail_inflow_candidates AS inflow
                SET analysis_state = 'claimed', claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE inflow.id = candidate.id
                RETURNING inflow.*
            )
            SELECT claimed.id, claimed.user_id, claimed.account_id,
                account.email AS account_email, claimed.workspace_id,
                workspace.name AS workspace_name, workspace.scope AS workspace_scope,
                claimed.provider_thread_id, claimed.source_revision,
                selected_model.id AS processing_model_id,
                selected_effort.effort AS processing_reasoning_effort
            FROM claimed
            JOIN gmail_accounts AS account
              ON account.id = claimed.account_id
             AND account.user_id = claimed.user_id
             AND account.workspace_id = claimed.workspace_id
            JOIN workspaces AS workspace
              ON workspace.id = claimed.workspace_id
             AND workspace.user_id = claimed.user_id
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
        let messages = sqlx::query_as::<_, GmailInflowMessage>(
            "\
            SELECT id, sender, subject, snippet, body_text, reference_links,
                received_at, list_id, list_unsubscribe, precedence, auto_submitted
            FROM (
                SELECT id, sender, subject, snippet, body_text, reference_links,
                    received_at, list_id, list_unsubscribe, precedence, auto_submitted
                FROM gmail_messages
                WHERE account_id = $1 AND workspace_id = $2
                  AND provider_thread_id = $3 AND provider_deleted_at IS NULL
                ORDER BY received_at DESC NULLS LAST, id DESC
                LIMIT 100
            ) AS recent
            ORDER BY received_at ASC NULLS FIRST, id ASC",
        )
        .bind(row.account_id)
        .bind(row.workspace_id)
        .bind(&row.provider_thread_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(Some(ClaimedGmailInflowAnalysis {
            id: row.id,
            user_id: row.user_id,
            account_id: row.account_id,
            account_email: row.account_email,
            workspace_id: row.workspace_id,
            workspace_name: row.workspace_name,
            workspace_scope: row.workspace_scope,
            provider_thread_id: row.provider_thread_id,
            source_revision: row.source_revision,
            messages,
            processing_model_id: row.processing_model_id,
            processing_reasoning_effort: row.processing_reasoning_effort,
        }))
    }

    /// Moves a claimed analysis into its running state and renews its lease.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for malformed lease input or a
    /// persistence error when the state transition cannot be written.
    pub async fn start_gmail_inflow_analysis(
        &self,
        analysis_id: Uuid,
        runner_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        if !is_v7(analysis_id) || !valid_runner(runner_id) || lease.is_zero() {
            return Err(StorageError::InvalidConfiguration);
        }
        let lease_millis =
            i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)?;
        let changed = sqlx::query(
            "UPDATE gmail_inflow_candidates
             SET analysis_state = 'running',
                 claim_expires_at = NOW() + ($3 * INTERVAL '1 millisecond')
             WHERE id = $1 AND claim_owner = $2 AND analysis_state = 'claimed'",
        )
        .bind(analysis_id)
        .bind(runner_id)
        .bind(lease_millis)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(changed.rows_affected() == 1)
    }

    /// Atomically stores a validated analysis result for the claimed source
    /// revision, or requeues the candidate when a newer revision arrived.
    ///
    /// # Errors
    ///
    /// Returns validation, lost persistence, or transaction errors.
    #[allow(
        clippy::if_not_else,
        reason = "The stale-revision guard is intentionally handled first so obsolete model output can never enter the ready-state update."
    )]
    pub async fn complete_gmail_inflow_analysis(
        &self,
        job: &ClaimedGmailInflowAnalysis,
        runner_id: &str,
        result: &GmailInflowAnalysisResult,
    ) -> Result<bool, StorageError> {
        result.validate()?;
        if !valid_runner(runner_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT source_revision, decision_status FROM gmail_inflow_candidates
             WHERE id = $1 AND user_id = $2 AND claim_owner = $3
               AND analysis_state = 'running' FOR UPDATE",
        )
        .bind(job.id)
        .bind(job.user_id)
        .bind(runner_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(current) = current else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        if current.0 != job.source_revision {
            sqlx::query(
                "UPDATE gmail_inflow_candidates
                 SET analysis_state = 'queued', claim_owner = NULL,
                     claim_expires_at = NULL, attempt_count = 0, error_code = NULL
                 WHERE id = $1",
            )
            .bind(job.id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        } else {
            let decision_status = if current.1 == "promoted" {
                "promoted"
            } else if result.classification.is_task()
                || result.classification == GmailInflowClassification::Duplicate
            {
                "pending"
            } else {
                "dismissed"
            };
            sqlx::query(
                "UPDATE gmail_inflow_candidates
                 SET analysis_state = 'ready', classification = $4,
                     confidence = $5, summary = $6, suggested_task_title = $7,
                     suggested_action_items = $8,
                     suggested_completion_criteria = $9,
                     suggested_assignee_name = $10, suggested_due_at = $11,
                     suggested_priority = $12, decision_status = $13,
                     deferred_until = NULL, analyzed_revision = source_revision,
                     analyzed_at = NOW(), analysis_model_id = $14,
                     analysis_version = $15, claim_owner = NULL,
                     claim_expires_at = NULL, error_code = NULL
                 WHERE id = $1 AND user_id = $2 AND claim_owner = $3
                   AND analysis_state = 'running'",
            )
            .bind(job.id)
            .bind(job.user_id)
            .bind(runner_id)
            .bind(classification_value(result.classification))
            .bind(result.confidence)
            .bind(result.summary.trim())
            .bind(trimmed(result.suggested_task_title.as_deref()))
            .bind(trimmed_vec(&result.suggested_action_items))
            .bind(trimmed(result.suggested_completion_criteria.as_deref()))
            .bind(trimmed(result.suggested_assignee_name.as_deref()))
            .bind(result.suggested_due_at)
            .bind(result.suggested_priority)
            .bind(decision_status)
            .bind(job.processing_model_id.as_deref())
            .bind(ANALYSIS_VERSION)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        let version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM gmail_inflow_candidates WHERE id = $1",
        )
        .bind(job.id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            job.user_id,
            "gmail_inflow_candidate",
            job.id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Fails the currently leased revision, or requeues it if source content
    /// changed while the analysis was running.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for unsafe runner/error values or a
    /// persistence error when the lease transition cannot be written.
    pub async fn fail_gmail_inflow_analysis(
        &self,
        job: &ClaimedGmailInflowAnalysis,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !valid_runner(runner_id) || !valid_error(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE gmail_inflow_candidates
             SET analysis_state = CASE
                    WHEN source_revision = $4 THEN 'failed' ELSE 'queued' END,
                 claim_owner = NULL, claim_expires_at = NULL,
                 error_code = CASE WHEN source_revision = $4 THEN $3 ELSE NULL END,
                 attempt_count = CASE
                    WHEN source_revision = $4 THEN attempt_count ELSE 0 END
             WHERE id = $1 AND claim_owner = $2
               AND analysis_state IN ('claimed', 'running')
             RETURNING version",
        )
        .bind(job.id)
        .bind(runner_id)
        .bind(error_code)
        .bind(job.source_revision)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(version) = version {
            append_change(
                &mut transaction,
                job.user_id,
                "gmail_inflow_candidate",
                job.id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(version.is_some())
    }

    /// Requeues an owned failed candidate using optimistic version matching.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for unsafe input or a persistence error
    /// when the retry transition cannot be written.
    pub async fn retry_gmail_inflow_analysis(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        candidate_id: Uuid,
        expected_version: i64,
    ) -> Result<bool, StorageError> {
        if ![user_id, workspace_id, candidate_id].into_iter().all(is_v7) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE gmail_inflow_candidates
             SET analysis_state = 'queued', attempt_count = 0, error_code = NULL
             WHERE id = $1 AND user_id = $2 AND workspace_id = $3
               AND version = $4 AND analysis_state = 'failed'
               AND decision_status IN ('pending', 'promoted')
             RETURNING version",
        )
        .bind(candidate_id)
        .bind(user_id)
        .bind(workspace_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(version) = version {
            append_change(
                &mut transaction,
                user_id,
                "gmail_inflow_candidate",
                candidate_id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(version.is_some())
    }

    /// Dismisses or defers an owned candidate using optimistic version
    /// matching.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for an invalid decision/time or a
    /// persistence error when the decision cannot be written.
    pub async fn decide_gmail_inflow_candidate(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        candidate_id: Uuid,
        expected_version: i64,
        decision: &str,
        revisit_at: Option<OffsetDateTime>,
    ) -> Result<bool, StorageError> {
        if ![user_id, workspace_id, candidate_id].into_iter().all(is_v7)
            || expected_version <= 0
            || !matches!(decision, "dismiss" | "defer")
            || (decision == "defer")
                != revisit_at.is_some_and(|value| {
                    value > OffsetDateTime::now_utc()
                        && value <= OffsetDateTime::now_utc() + time::Duration::days(365)
                })
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE gmail_inflow_candidates
             SET decision_status = $5, deferred_until = $6
             WHERE id = $1 AND user_id = $2 AND workspace_id = $3
               AND version = $4 AND decision_status IN ('pending', 'deferred')
             RETURNING version",
        )
        .bind(candidate_id)
        .bind(user_id)
        .bind(workspace_id)
        .bind(expected_version)
        .bind(if decision == "dismiss" {
            "dismissed"
        } else {
            "deferred"
        })
        .bind(revisit_at)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(version) = version {
            append_change(
                &mut transaction,
                user_id,
                "gmail_inflow_candidate",
                candidate_id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(version.is_some())
    }

    /// Creates the task and records the source decision in one transaction.
    ///
    /// # Errors
    ///
    /// Returns validation, ownership, version, or persistence errors. A
    /// mismatched optimistic version returns `Ok(false)`.
    #[allow(
        clippy::too_many_lines,
        reason = "Promotion deliberately keeps task creation, candidate linkage, webhook enqueueing, and change events in one auditable transaction."
    )]
    pub async fn promote_gmail_inflow_candidate(
        &self,
        command: &PromoteGmailInflowCandidate,
    ) -> Result<bool, StorageError> {
        let task = NewTask {
            id: command.candidate_id,
            user_id: command.user_id,
            project_id: Some(command.project_id),
            parent_task_id: None,
            title: command.title.clone(),
            notes: command.notes.clone(),
            assignee_name: command.assignee_name.clone(),
            priority: command.priority,
            due_at: command.due_at,
        };
        task.validate()?;
        if !is_v7(command.workspace_id) || command.expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let locked = sqlx::query_scalar::<_, Uuid>(
            "SELECT candidate.id
             FROM gmail_inflow_candidates AS candidate
             JOIN projects AS project
               ON project.id = $5 AND project.user_id = candidate.user_id
              AND project.workspace_id = candidate.workspace_id
              AND project.status <> 'completed'
             WHERE candidate.id = $1 AND candidate.user_id = $2
               AND candidate.workspace_id = $3 AND candidate.version = $4
               AND candidate.analysis_state = 'ready'
               AND candidate.classification = 'new_task'
               AND candidate.decision_status IN ('pending', 'deferred')
             FOR UPDATE OF candidate",
        )
        .bind(command.candidate_id)
        .bind(command.user_id)
        .bind(command.workspace_id)
        .bind(command.expected_version)
        .bind(command.project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if locked.is_none() {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        }
        let inserted = sqlx::query(
            "INSERT INTO tasks (
                id, user_id, project_id, parent_task_id, title, notes,
                assignee_name, status, priority, due_at
             ) VALUES ($1, $2, $3, NULL, $4, $5, $6, 'open', $7, $8)
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(task.id)
        .bind(task.user_id)
        .bind(task.project_id)
        .bind(task.title.trim())
        .bind(trimmed(task.notes.as_deref()))
        .bind(trimmed(task.assignee_name.as_deref()))
        .bind(task.priority)
        .bind(task.due_at)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        if inserted.rows_affected() != 1 {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        }
        let persisted = Task {
            id: task.id,
            project_id: task.project_id,
            parent_task_id: None,
            title: task.title.trim().to_owned(),
            notes: trimmed(task.notes.as_deref()).map(str::to_owned),
            assignee_name: trimmed(task.assignee_name.as_deref()).map(str::to_owned),
            status: TaskStatus::Open,
            priority: task.priority,
            due_at: task.due_at,
            completed_at: None,
            version: 1,
        };
        append_change(
            &mut transaction,
            command.user_id,
            "task",
            persisted.id,
            persisted.version,
        )
        .await?;
        queue_task_webhook_in_transaction(
            &mut transaction,
            command.user_id,
            &persisted,
            "task.created",
        )
        .await?;
        let candidate_version = sqlx::query_scalar::<_, i64>(
            "UPDATE gmail_inflow_candidates
             SET decision_status = 'promoted', promoted_project_id = $5,
                 promoted_task_id = $1, deferred_until = NULL
             WHERE id = $1 AND user_id = $2 AND workspace_id = $3
               AND version = $4
             RETURNING version",
        )
        .bind(command.candidate_id)
        .bind(command.user_id)
        .bind(command.workspace_id)
        .bind(command.expected_version)
        .bind(command.project_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(candidate_version) = candidate_version else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        append_change(
            &mut transaction,
            command.user_id,
            "gmail_inflow_candidate",
            command.candidate_id,
            candidate_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Recovers expired claimed/running analyses with a sanitized error code.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for unsafe error codes or a persistence
    /// error when recovery cannot be committed.
    pub async fn fail_expired_running_gmail_inflow_analyses(
        &self,
        error_code: &str,
    ) -> Result<u64, StorageError> {
        if !valid_error(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let changed = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "UPDATE gmail_inflow_candidates
             SET analysis_state = 'failed', claim_owner = NULL,
                 claim_expires_at = NULL, error_code = $1
             WHERE analysis_state IN ('claimed', 'running')
               AND claim_expires_at <= NOW()
             RETURNING id, user_id, version",
        )
        .bind(error_code)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        for (candidate_id, user_id, version) in &changed {
            append_change(
                &mut transaction,
                *user_id,
                "gmail_inflow_candidate",
                *candidate_id,
                *version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        u64::try_from(changed.len()).map_err(|_| StorageError::PersistenceUnavailable)
    }

    async fn release_due_gmail_inflow_deferrals(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let changed = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE gmail_inflow_candidates
             SET decision_status = 'pending', deferred_until = NULL
             WHERE user_id = $1 AND workspace_id = $2
               AND decision_status = 'deferred' AND deferred_until <= NOW()
             RETURNING id, version",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        for (candidate_id, version) in changed {
            append_change(
                &mut transaction,
                user_id,
                "gmail_inflow_candidate",
                candidate_id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(())
    }
}

pub(crate) async fn upsert_gmail_inflow_candidate(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
    account_id: Uuid,
    message_id: Uuid,
    provider_thread_id: &str,
    source_changed: bool,
) -> Result<(), StorageError> {
    if !source_changed {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO gmail_inflow_candidates (
            id, user_id, workspace_id, account_id, provider_thread_id,
            representative_message_id
         ) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (account_id, provider_thread_id) DO UPDATE
         SET representative_message_id = CASE
                WHEN COALESCE((
                    SELECT received_at FROM gmail_messages
                    WHERE id = EXCLUDED.representative_message_id
                ), '-infinity'::TIMESTAMPTZ)
                    > COALESCE((
                        SELECT received_at FROM gmail_messages
                        WHERE id = gmail_inflow_candidates.representative_message_id
                    ), '-infinity'::TIMESTAMPTZ)
                  OR (
                    COALESCE((
                        SELECT received_at FROM gmail_messages
                        WHERE id = EXCLUDED.representative_message_id
                    ), '-infinity'::TIMESTAMPTZ)
                        = COALESCE((
                            SELECT received_at FROM gmail_messages
                            WHERE id = gmail_inflow_candidates.representative_message_id
                        ), '-infinity'::TIMESTAMPTZ)
                    AND EXCLUDED.representative_message_id::TEXT
                        > gmail_inflow_candidates.representative_message_id::TEXT
                  )
                THEN EXCLUDED.representative_message_id
                ELSE gmail_inflow_candidates.representative_message_id
             END,
             source_revision = gmail_inflow_candidates.source_revision + 1,
             analysis_state = 'queued',
             decision_status = CASE
                WHEN gmail_inflow_candidates.decision_status = 'dismissed'
                THEN 'pending' ELSE gmail_inflow_candidates.decision_status END,
             claim_owner = NULL, claim_expires_at = NULL,
             attempt_count = 0, error_code = NULL
        ",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(workspace_id)
    .bind(account_id)
    .bind(provider_thread_id)
    .bind(message_id)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    Ok(())
}

fn candidate_select() -> &'static str {
    "SELECT candidate.id, candidate.account_id, account.email AS account_email,
        candidate.workspace_id, workspace.name AS workspace_name,
        workspace.scope AS workspace_scope,
        message.id AS message_id, message.provider_message_id,
        candidate.provider_thread_id, message.sender, message.subject,
        message.snippet, message.body_text, message.reference_links,
        message.received_at, candidate.analysis_state,
        candidate.classification, candidate.confidence, candidate.summary,
        candidate.suggested_task_title, candidate.suggested_action_items,
        candidate.suggested_completion_criteria,
        candidate.suggested_assignee_name, candidate.suggested_due_at,
        candidate.suggested_priority, candidate.decision_status,
        candidate.promoted_task_id, candidate.deferred_until,
        candidate.error_code, candidate.created_at, candidate.version
     FROM gmail_inflow_candidates AS candidate
     JOIN gmail_accounts AS account
       ON account.id = candidate.account_id
      AND account.user_id = candidate.user_id
      AND account.workspace_id = candidate.workspace_id
     JOIN workspaces AS workspace
       ON workspace.id = candidate.workspace_id
      AND workspace.user_id = candidate.user_id
     JOIN gmail_messages AS message
       ON message.id = candidate.representative_message_id
      AND message.account_id = candidate.account_id
      AND message.workspace_id = candidate.workspace_id"
}

const fn status_predicate(status: GmailInflowStatus) -> &'static str {
    match status {
        GmailInflowStatus::Attention => {
            "candidate.decision_status = 'pending'
             AND (
                (candidate.analysis_state = 'ready'
                    AND candidate.classification IN ('new_task', 'duplicate'))
                OR candidate.analysis_state = 'failed'
             )"
        }
        GmailInflowStatus::Pending => "candidate.decision_status = 'pending'",
        GmailInflowStatus::Promoted => "candidate.decision_status = 'promoted'",
        GmailInflowStatus::Dismissed => "candidate.decision_status = 'dismissed'",
        GmailInflowStatus::Deferred => {
            "candidate.decision_status = 'deferred' AND candidate.deferred_until > NOW()"
        }
        GmailInflowStatus::All => "TRUE",
    }
}

fn parse_analysis_state(value: &str) -> Result<GmailInflowAnalysisState, StorageError> {
    match value {
        "queued" => Ok(GmailInflowAnalysisState::Queued),
        "claimed" => Ok(GmailInflowAnalysisState::Claimed),
        "running" => Ok(GmailInflowAnalysisState::Running),
        "ready" => Ok(GmailInflowAnalysisState::Ready),
        "failed" => Ok(GmailInflowAnalysisState::Failed),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_classification(value: &str) -> Result<GmailInflowClassification, StorageError> {
    match value {
        "new_task" => Ok(GmailInflowClassification::NewTask),
        "follow_up" => Ok(GmailInflowClassification::FollowUp),
        "question" => Ok(GmailInflowClassification::Question),
        "status_update" => Ok(GmailInflowClassification::StatusUpdate),
        "automated" => Ok(GmailInflowClassification::Automated),
        "newsletter" => Ok(GmailInflowClassification::Newsletter),
        "marketing" => Ok(GmailInflowClassification::Marketing),
        "noise" => Ok(GmailInflowClassification::Noise),
        "duplicate" => Ok(GmailInflowClassification::Duplicate),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

const fn classification_value(value: GmailInflowClassification) -> &'static str {
    match value {
        GmailInflowClassification::NewTask => "new_task",
        GmailInflowClassification::FollowUp => "follow_up",
        GmailInflowClassification::Question => "question",
        GmailInflowClassification::StatusUpdate => "status_update",
        GmailInflowClassification::Automated => "automated",
        GmailInflowClassification::Newsletter => "newsletter",
        GmailInflowClassification::Marketing => "marketing",
        GmailInflowClassification::Noise => "noise",
        GmailInflowClassification::Duplicate => "duplicate",
    }
}

fn parse_sender(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return (None, None);
    };
    if let (Some(start), Some(end)) = (value.rfind('<'), value.rfind('>'))
        && start < end
    {
        let name = value[..start].trim().trim_matches('"');
        let email = value[start + 1..end].trim();
        return (
            (!name.is_empty()).then(|| name.to_owned()),
            (!email.is_empty()).then(|| email.to_owned()),
        );
    }
    if value.contains('@') {
        (None, Some(value.to_owned()))
    } else {
        (Some(value.to_owned()), None)
    }
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trimmed_vec(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= maximum
        && !value.chars().any(|character| character == '\0')
}

fn valid_runner(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_RUNNER_BYTES && !value.chars().any(char::is_control)
}

fn valid_error(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ERROR_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn classify(error: sqlx::Error) -> StorageError {
    match error {
        sqlx::Error::Database(database)
            if database.is_unique_violation()
                || database.is_check_violation()
                || database.is_foreign_key_violation() =>
        {
            StorageError::InvalidConfiguration
        }
        _ => StorageError::PersistenceUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_parser_separates_display_name_and_email() {
        assert_eq!(
            parse_sender(Some("\"Jimin\" <jimin@example.com>")),
            (
                Some("Jimin".to_owned()),
                Some("jimin@example.com".to_owned())
            )
        );
    }

    #[test]
    fn non_task_analysis_rejects_task_fields() {
        let result = GmailInflowAnalysisResult {
            classification: GmailInflowClassification::Newsletter,
            confidence: 99,
            summary: "정기 뉴스레터입니다.".to_owned(),
            suggested_task_title: Some("읽기".to_owned()),
            suggested_action_items: Vec::new(),
            suggested_completion_criteria: None,
            suggested_assignee_name: None,
            suggested_due_at: None,
            suggested_priority: None,
        };
        assert!(matches!(
            result.validate(),
            Err(StorageError::InvalidConfiguration)
        ));
    }
}

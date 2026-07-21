//! Owner-scoped meeting transcripts, AI analysis, and approval-gated actions.

use std::time::Duration;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

const MAX_TITLE_CHARS: usize = 200;
const MAX_TRANSCRIPT_CHARS: usize = 120_000;
const MAX_SUMMARY_CHARS: usize = 20_000;
const MAX_DETAIL_CHARS: usize = 4_000;
const MAX_EXCERPT_CHARS: usize = 2_000;
const MAX_ANALYSIS_ITEMS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingStatus {
    Queued,
    Analyzing,
    ReviewReady,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingActionKind {
    Task,
    Schedule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingActionStatus {
    Suggested,
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
    pub id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub project_title: Option<String>,
    pub title: String,
    pub transcript: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration_seconds: Option<i32>,
    pub status: MeetingStatus,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub risks: Vec<String>,
    pub follow_up: Option<String>,
    pub analyzed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingDecision {
    pub id: Uuid,
    pub content: String,
    pub rationale: Option<String>,
    pub source_excerpt: String,
    pub source_timestamp_seconds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingActionItem {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub kind: MeetingActionKind,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub starts_at: Option<OffsetDateTime>,
    pub ends_at: Option<OffsetDateTime>,
    pub time_zone: Option<String>,
    pub source_excerpt: String,
    pub confidence: i16,
    pub status: MeetingActionStatus,
    pub target_entity_id: Uuid,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingDetail {
    pub meeting: Meeting,
    pub decisions: Vec<MeetingDecision>,
    pub action_items: Vec<MeetingActionItem>,
}

pub struct NewMeeting {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub transcript: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration_seconds: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewMeetingDecision {
    pub id: Uuid,
    pub content: String,
    pub rationale: Option<String>,
    pub source_excerpt: String,
    pub source_timestamp_seconds: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewMeetingActionItem {
    pub id: Uuid,
    pub target_entity_id: Uuid,
    pub kind: MeetingActionKind,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
    pub starts_at: Option<OffsetDateTime>,
    pub ends_at: Option<OffsetDateTime>,
    pub time_zone: Option<String>,
    pub source_excerpt: String,
    pub confidence: i16,
}

pub struct MeetingAnalysisResult {
    pub summary: String,
    pub topics: Vec<String>,
    pub risks: Vec<String>,
    pub follow_up: Option<String>,
    pub decisions: Vec<NewMeetingDecision>,
    pub action_items: Vec<NewMeetingActionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedMeetingAnalysis {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub transcript: String,
    pub project_id: Option<Uuid>,
    pub project_title: Option<String>,
    pub started_at: Option<OffsetDateTime>,
    pub processing_model_id: Option<String>,
    pub processing_reasoning_effort: Option<String>,
}

#[derive(sqlx::FromRow)]
struct MeetingRow {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_title: Option<String>,
    title: String,
    transcript: String,
    started_at: Option<OffsetDateTime>,
    duration_seconds: Option<i32>,
    status: String,
    summary: Option<String>,
    topics: Vec<String>,
    risks: Vec<String>,
    follow_up: Option<String>,
    analyzed_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct MeetingDecisionRow {
    id: Uuid,
    content: String,
    rationale: Option<String>,
    source_excerpt: String,
    source_timestamp_seconds: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct MeetingActionItemRow {
    id: Uuid,
    meeting_id: Uuid,
    kind: String,
    project_id: Option<Uuid>,
    title: String,
    notes: Option<String>,
    priority: i16,
    due_at: Option<OffsetDateTime>,
    starts_at: Option<OffsetDateTime>,
    ends_at: Option<OffsetDateTime>,
    time_zone: Option<String>,
    source_excerpt: String,
    confidence: i16,
    status: String,
    target_entity_id: Uuid,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct ClaimedMeetingAnalysisRow {
    id: Uuid,
    meeting_id: Uuid,
    user_id: Uuid,
    title: String,
    transcript: String,
    project_id: Option<Uuid>,
    project_title: Option<String>,
    started_at: Option<OffsetDateTime>,
    processing_model_id: Option<String>,
    processing_reasoning_effort: Option<String>,
}

impl NewMeeting {
    /// Validates bounded transcript metadata before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        let duration_valid = self
            .duration_seconds
            .is_none_or(|seconds| (1..=43_200).contains(&seconds));
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_optional_id(self.workspace_id)
            || !valid_optional_id(self.project_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS)
            || !valid_body_text(&self.transcript, MAX_TRANSCRIPT_CHARS)
            || !duration_valid
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl MeetingAnalysisResult {
    /// Validates all model-derived content before it is persisted.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when model output is
    /// unbounded, incomplete, or internally inconsistent.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !valid_body_text(&self.summary, MAX_SUMMARY_CHARS)
            || self.topics.len() > MAX_ANALYSIS_ITEMS
            || self.risks.len() > MAX_ANALYSIS_ITEMS
            || self.decisions.len() > MAX_ANALYSIS_ITEMS
            || self.action_items.len() > MAX_ANALYSIS_ITEMS
            || !self
                .topics
                .iter()
                .chain(&self.risks)
                .all(|value| valid_body_text(value, MAX_DETAIL_CHARS))
            || !valid_optional_body_text(self.follow_up.as_deref(), MAX_DETAIL_CHARS)
            || !self
                .decisions
                .iter()
                .all(|decision| decision.validate().is_ok())
            || !self.action_items.iter().all(|item| item.validate().is_ok())
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl NewMeetingDecision {
    /// Validates one model-derived decision before it is included in a result.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the decision is
    /// incomplete or exceeds the persisted bounds.
    pub fn validate(&self) -> Result<(), StorageError> {
        (is_v7(self.id)
            && valid_body_text(&self.content, MAX_EXCERPT_CHARS)
            && valid_optional_body_text(self.rationale.as_deref(), MAX_EXCERPT_CHARS)
            && valid_body_text(&self.source_excerpt, MAX_EXCERPT_CHARS)
            && self.source_timestamp_seconds.is_none_or(|value| value >= 0))
        .then_some(())
        .ok_or(StorageError::InvalidConfiguration)
    }
}

impl NewMeetingActionItem {
    /// Validates one model-derived action candidate before it is included in a result.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the candidate is
    /// incomplete, out of bounds, or has an inconsistent schedule window.
    pub fn validate(&self) -> Result<(), StorageError> {
        let schedule_fields_valid = match self.kind {
            MeetingActionKind::Task => {
                self.starts_at.is_none() && self.ends_at.is_none() && self.time_zone.is_none()
            }
            MeetingActionKind::Schedule => {
                self.starts_at
                    .zip(self.ends_at)
                    .is_some_and(|(start, end)| end > start)
                    && self
                        .time_zone
                        .as_deref()
                        .is_some_and(|value| valid_text(value, 100))
            }
        };
        (is_v7(self.id)
            && is_v7(self.target_entity_id)
            && valid_optional_id(self.project_id)
            && valid_text(&self.title, MAX_TITLE_CHARS)
            && valid_optional_body_text(self.notes.as_deref(), MAX_DETAIL_CHARS)
            && (0..=3).contains(&self.priority)
            && valid_body_text(&self.source_excerpt, MAX_EXCERPT_CHARS)
            && (0..=100).contains(&self.confidence)
            && schedule_fields_valid)
            .then_some(())
            .ok_or(StorageError::InvalidConfiguration)
    }
}

impl TryFrom<MeetingRow> for Meeting {
    type Error = StorageError;

    fn try_from(row: MeetingRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            project_id: row.project_id,
            project_title: row.project_title,
            title: row.title,
            transcript: row.transcript,
            started_at: row.started_at,
            duration_seconds: row.duration_seconds,
            status: parse_meeting_status(&row.status)?,
            summary: row.summary,
            topics: row.topics,
            risks: row.risks,
            follow_up: row.follow_up,
            analyzed_at: row.analyzed_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}

impl TryFrom<MeetingDecisionRow> for MeetingDecision {
    type Error = StorageError;

    fn try_from(row: MeetingDecisionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            content: row.content,
            rationale: row.rationale,
            source_excerpt: row.source_excerpt,
            source_timestamp_seconds: row.source_timestamp_seconds,
        })
    }
}

impl TryFrom<MeetingActionItemRow> for MeetingActionItem {
    type Error = StorageError;

    fn try_from(row: MeetingActionItemRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            meeting_id: row.meeting_id,
            kind: parse_action_kind(&row.kind)?,
            project_id: row.project_id,
            title: row.title,
            notes: row.notes,
            priority: row.priority,
            due_at: row.due_at,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            time_zone: row.time_zone,
            source_excerpt: row.source_excerpt,
            confidence: row.confidence,
            status: parse_action_status(&row.status)?,
            target_entity_id: row.target_entity_id,
            version: row.version,
        })
    }
}

impl From<ClaimedMeetingAnalysisRow> for ClaimedMeetingAnalysis {
    fn from(row: ClaimedMeetingAnalysisRow) -> Self {
        Self {
            id: row.id,
            meeting_id: row.meeting_id,
            user_id: row.user_id,
            title: row.title,
            transcript: row.transcript,
            project_id: row.project_id,
            project_title: row.project_title,
            started_at: row.started_at,
            processing_model_id: row.processing_model_id,
            processing_reasoning_effort: row.processing_reasoning_effort,
        }
    }
}

impl Database {
    /// Creates an owner-scoped meeting and atomically queues its AI analysis.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, or persistence error.
    pub async fn create_meeting(&self, meeting: &NewMeeting) -> Result<Meeting, StorageError> {
        meeting.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if !meeting_scope_is_owned(
            &mut transaction,
            meeting.user_id,
            meeting.workspace_id,
            meeting.project_id,
        )
        .await?
        {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        }
        let row = sqlx::query_as::<_, MeetingRow>(
            "INSERT INTO meetings (
                id, user_id, workspace_id, project_id, title, transcript,
                started_at, duration_seconds
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, workspace_id, project_id,
                NULL::text AS project_title, title, transcript, started_at,
                duration_seconds, status, summary, topics, risks, follow_up,
                analyzed_at, created_at, updated_at, version",
        )
        .bind(meeting.id)
        .bind(meeting.user_id)
        .bind(meeting.workspace_id)
        .bind(meeting.project_id)
        .bind(meeting.title.trim())
        .bind(meeting.transcript.trim())
        .bind(meeting.started_at)
        .bind(meeting.duration_seconds)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let job_id = Uuid::now_v7();
        let job_version = sqlx::query_scalar::<_, i64>(
            "INSERT INTO meeting_analysis_jobs (id, meeting_id, user_id)
             VALUES ($1, $2, $3)
             RETURNING version",
        )
        .bind(job_id)
        .bind(meeting.id)
        .bind(meeting.user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            meeting.user_id,
            "meeting",
            meeting.id,
            row.version,
        )
        .await?;
        append_change(
            &mut transaction,
            meeting.user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Meeting::try_from(row)
    }

    /// Lists recent meeting summaries without transferring source transcripts.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn meetings_for_user(&self, user_id: Uuid) -> Result<Vec<Meeting>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, MeetingRow>(
            "SELECT meeting.id, meeting.workspace_id, meeting.project_id,
                project.title AS project_title, meeting.title, ''::text AS transcript,
                meeting.started_at, meeting.duration_seconds, meeting.status,
                meeting.summary, meeting.topics, meeting.risks, meeting.follow_up,
                meeting.analyzed_at, meeting.created_at, meeting.updated_at,
                meeting.version
             FROM meetings AS meeting
             LEFT JOIN projects AS project ON project.id = meeting.project_id
             WHERE meeting.user_id = $1
             ORDER BY meeting.created_at DESC, meeting.id DESC
             LIMIT 100",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?
        .into_iter()
        .map(Meeting::try_from)
        .collect()
    }

    /// Returns one meeting with every review item for its owner.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn meeting_detail_for_user(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
    ) -> Result<Option<MeetingDetail>, StorageError> {
        if !is_v7(user_id) || !is_v7(meeting_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, MeetingRow>(
            "SELECT meeting.id, meeting.workspace_id, meeting.project_id,
                project.title AS project_title, meeting.title, meeting.transcript,
                meeting.started_at, meeting.duration_seconds, meeting.status,
                meeting.summary, meeting.topics, meeting.risks, meeting.follow_up,
                meeting.analyzed_at, meeting.created_at, meeting.updated_at,
                meeting.version
             FROM meetings AS meeting
             LEFT JOIN projects AS project ON project.id = meeting.project_id
             WHERE meeting.user_id = $1 AND meeting.id = $2",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let decisions = sqlx::query_as::<_, MeetingDecisionRow>(
            "SELECT id, content, rationale, source_excerpt, source_timestamp_seconds
             FROM meeting_decisions
             WHERE meeting_id = $1
             ORDER BY created_at, id",
        )
        .bind(meeting_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?
        .into_iter()
        .map(MeetingDecision::try_from)
        .collect::<Result<Vec<_>, _>>()?;
        let action_items = meeting_action_rows(self, user_id, meeting_id)
            .await?
            .into_iter()
            .map(MeetingActionItem::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(MeetingDetail {
            meeting: Meeting::try_from(row)?,
            decisions,
            action_items,
        }))
    }

    /// Claims the oldest queued meeting analysis for this worker.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn claim_next_meeting_analysis(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedMeetingAnalysis>, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let row = sqlx::query_as::<_, ClaimedMeetingAnalysisRow>(
            "WITH recovered AS (
                UPDATE meeting_analysis_jobs
                SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL
                WHERE state = 'claimed' AND claim_expires_at < NOW()
             ), candidate AS (
                SELECT id FROM meeting_analysis_jobs
                WHERE state = 'queued'
                ORDER BY created_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             ), claimed AS (
                UPDATE meeting_analysis_jobs AS job
                SET state = 'claimed', claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE job.id = candidate.id
                RETURNING job.id, job.meeting_id, job.user_id
             )
             SELECT claimed.id, claimed.meeting_id, claimed.user_id,
                meeting.title, meeting.transcript, meeting.project_id,
                project.title AS project_title, meeting.started_at,
                selected_model.id AS processing_model_id,
                selected_effort.effort AS processing_reasoning_effort
             FROM claimed
             INNER JOIN meetings AS meeting ON meeting.id = claimed.meeting_id
             LEFT JOIN projects AS project ON project.id = meeting.project_id
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
        Ok(row.map(ClaimedMeetingAnalysis::from))
    }

    /// Marks a claimed meeting analysis as running before contacting Codex.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn start_meeting_analysis(
        &self,
        job_id: Uuid,
        runner_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        if !is_v7(job_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, Uuid, i64)>(
            "UPDATE meeting_analysis_jobs
             SET state = 'running', started_at = COALESCE(started_at, NOW()),
                 claim_expires_at = NOW() + ($3 * INTERVAL '1 millisecond')
             WHERE id = $1 AND claim_owner = $2 AND state = 'claimed'
             RETURNING user_id, meeting_id, id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((user_id, meeting_id, job_id, job_version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings SET status = 'analyzing'
             WHERE id = $1 AND user_id = $2 AND status = 'queued'
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Atomically stores validated AI analysis and releases it for review.
    ///
    /// # Errors
    ///
    /// Returns a validation, lease, ownership, or persistence error.
    #[allow(clippy::too_many_lines)] // One transaction keeps analysis rows and queue state atomic.
    pub async fn complete_meeting_analysis(
        &self,
        job: &ClaimedMeetingAnalysis,
        runner_id: &str,
        result: &MeetingAnalysisResult,
    ) -> Result<bool, StorageError> {
        result.validate()?;
        if !valid_runner_id(runner_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        if result
            .action_items
            .iter()
            .any(|item| item.project_id.is_some() && item.project_id != job.project_id)
        {
            return Err(StorageError::IdentityConflict);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let owned = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM meeting_analysis_jobs
                WHERE id = $1 AND meeting_id = $2 AND user_id = $3
                  AND claim_owner = $4 AND state = 'running'
            )",
        )
        .bind(job.id)
        .bind(job.meeting_id)
        .bind(job.user_id)
        .bind(runner_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        if !owned {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        }
        for decision in &result.decisions {
            sqlx::query(
                "INSERT INTO meeting_decisions (
                    id, meeting_id, content, rationale, source_excerpt,
                    source_timestamp_seconds
                 ) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(decision.id)
            .bind(job.meeting_id)
            .bind(decision.content.trim())
            .bind(trimmed_optional(decision.rationale.as_deref()))
            .bind(decision.source_excerpt.trim())
            .bind(decision.source_timestamp_seconds)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        for item in &result.action_items {
            sqlx::query(
                "INSERT INTO meeting_action_items (
                    id, meeting_id, kind, project_id, title, notes, priority,
                    due_at, starts_at, ends_at, time_zone, source_excerpt,
                    confidence, target_entity_id
                 ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
                 )",
            )
            .bind(item.id)
            .bind(job.meeting_id)
            .bind(action_kind_value(item.kind))
            .bind(item.project_id)
            .bind(item.title.trim())
            .bind(trimmed_optional(item.notes.as_deref()))
            .bind(item.priority)
            .bind(item.due_at)
            .bind(item.starts_at)
            .bind(item.ends_at)
            .bind(trimmed_optional(item.time_zone.as_deref()))
            .bind(item.source_excerpt.trim())
            .bind(item.confidence)
            .bind(item.target_entity_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings
             SET status = 'review_ready', summary = $3, topics = $4, risks = $5,
                 follow_up = $6, analyzed_at = NOW()
             WHERE id = $1 AND user_id = $2 AND status = 'analyzing'
             RETURNING version",
        )
        .bind(job.meeting_id)
        .bind(job.user_id)
        .bind(result.summary.trim())
        .bind(trimmed_strings(&result.topics))
        .bind(trimmed_strings(&result.risks))
        .bind(trimmed_optional(result.follow_up.as_deref()))
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let job_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meeting_analysis_jobs
             SET state = 'completed', claim_owner = NULL, claim_expires_at = NULL,
                 finished_at = NOW()
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
            "meeting",
            job.meeting_id,
            meeting_version,
        )
        .await?;
        append_change(
            &mut transaction,
            job.user_id,
            "meeting_analysis_job",
            job.id,
            job_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Fails a lease-owned analysis without exposing provider error details.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn fail_meeting_analysis(
        &self,
        job_id: Uuid,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id) || !valid_runner_id(runner_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "UPDATE meeting_analysis_jobs
             SET state = 'failed', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = $3, finished_at = NOW()
             WHERE id = $1 AND claim_owner = $2 AND state IN ('claimed', 'running')
             RETURNING user_id, meeting_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(error_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((user_id, meeting_id, job_version)) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(false);
        };
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings SET status = 'failed'
             WHERE id = $1 AND user_id = $2
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Queues a failed meeting analysis again for an explicit owner retry.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, state, or persistence error.
    pub async fn retry_meeting_analysis(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
    ) -> Result<Meeting, StorageError> {
        if !is_v7(user_id) || !is_v7(meeting_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let job_row = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE meeting_analysis_jobs AS job
             SET state = 'queued', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = NULL, started_at = NULL, finished_at = NULL
             FROM meetings AS meeting
             WHERE job.meeting_id = $2 AND job.user_id = $1
               AND meeting.id = job.meeting_id AND meeting.user_id = $1
               AND job.state = 'failed' AND meeting.status = 'failed'
               AND job.attempt_count < 8
             RETURNING job.id, job.version",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((job_id, job_version)) = job_row else {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        };
        let row = sqlx::query_as::<_, MeetingRow>(
            "UPDATE meetings AS meeting
             SET status = 'queued', summary = NULL, topics = '{}', risks = '{}',
                 follow_up = NULL, analyzed_at = NULL
             WHERE meeting.id = $2 AND meeting.user_id = $1
             RETURNING meeting.id, meeting.workspace_id, meeting.project_id,
                NULL::text AS project_title, meeting.title, meeting.transcript,
                meeting.started_at, meeting.duration_seconds, meeting.status,
                meeting.summary, meeting.topics, meeting.risks, meeting.follow_up,
                meeting.analyzed_at, meeting.created_at, meeting.updated_at,
                meeting.version",
        )
        .bind(user_id)
        .bind(meeting_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting_analysis_job",
            job_id,
            job_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Meeting::try_from(row)
    }

    /// Fails provider-started analyses whose lease expired after a restart.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn fail_expired_running_meeting_analyses(
        &self,
        error_code: &str,
    ) -> Result<usize, StorageError> {
        if !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, i64)>(
            "UPDATE meeting_analysis_jobs
             SET state = 'failed', claim_owner = NULL, claim_expires_at = NULL,
                 error_code = $1, finished_at = NOW()
             WHERE state = 'running' AND claim_expires_at < NOW()
             RETURNING id, user_id, meeting_id, version",
        )
        .bind(error_code)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        for (job_id, user_id, meeting_id, job_version) in &rows {
            let meeting_version = sqlx::query_scalar::<_, i64>(
                "UPDATE meetings SET status = 'failed'
                 WHERE id = $1 AND user_id = $2
                 RETURNING version",
            )
            .bind(meeting_id)
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(classify)?;
            append_change(
                &mut transaction,
                *user_id,
                "meeting_analysis_job",
                *job_id,
                *job_version,
            )
            .await?;
            append_change(
                &mut transaction,
                *user_id,
                "meeting",
                *meeting_id,
                meeting_version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(rows.len())
    }

    /// Returns one review item only when both meeting and item belong to owner.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error.
    pub async fn meeting_action_item_for_user(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
        item_id: Uuid,
    ) -> Result<Option<MeetingActionItem>, StorageError> {
        if !is_v7(user_id) || !is_v7(meeting_id) || !is_v7(item_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, MeetingActionItemRow>(
            "SELECT item.id, item.meeting_id, item.kind, item.project_id,
                item.title, item.notes, item.priority, item.due_at,
                item.starts_at, item.ends_at, item.time_zone,
                item.source_excerpt, item.confidence, item.status,
                item.target_entity_id, item.version
             FROM meeting_action_items AS item
             INNER JOIN meetings AS meeting ON meeting.id = item.meeting_id
             WHERE meeting.user_id = $1 AND meeting.id = $2 AND item.id = $3",
        )
        .bind(user_id)
        .bind(meeting_id)
        .bind(item_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?
        .map(MeetingActionItem::try_from)
        .transpose()
    }

    /// Records the owner's final decision after a target action succeeds.
    ///
    /// # Errors
    ///
    /// Returns a validation, conflict, or persistence error.
    pub async fn decide_meeting_action_item(
        &self,
        user_id: Uuid,
        meeting_id: Uuid,
        item_id: Uuid,
        decision: MeetingActionStatus,
    ) -> Result<MeetingActionItem, StorageError> {
        if !is_v7(user_id)
            || !is_v7(meeting_id)
            || !is_v7(item_id)
            || decision == MeetingActionStatus::Suggested
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, MeetingActionItemRow>(
            "UPDATE meeting_action_items AS item
             SET status = $4,
                 applied_at = CASE WHEN $4 = 'applied' THEN NOW() ELSE NULL END,
                 rejected_at = CASE WHEN $4 = 'rejected' THEN NOW() ELSE NULL END
             FROM meetings AS meeting
             WHERE item.id = $3 AND item.meeting_id = $2
               AND meeting.id = item.meeting_id AND meeting.user_id = $1
               AND item.status IN ('suggested', $4)
             RETURNING item.id, item.meeting_id, item.kind, item.project_id,
                item.title, item.notes, item.priority, item.due_at,
                item.starts_at, item.ends_at, item.time_zone,
                item.source_excerpt, item.confidence, item.status,
                item.target_entity_id, item.version",
        )
        .bind(user_id)
        .bind(meeting_id)
        .bind(item_id)
        .bind(action_status_value(decision))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(row) = row else {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::IdentityConflict);
        };
        append_change(
            &mut transaction,
            user_id,
            "meeting_action_item",
            item_id,
            row.version,
        )
        .await?;
        let meeting_version = sqlx::query_scalar::<_, i64>(
            "UPDATE meetings
             SET status = CASE WHEN EXISTS(
                    SELECT 1 FROM meeting_action_items
                    WHERE meeting_id = $1 AND status = 'suggested'
                 ) THEN 'review_ready' ELSE 'applied' END
             WHERE id = $1 AND user_id = $2 AND status IN ('review_ready', 'applied')
             RETURNING version",
        )
        .bind(meeting_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            user_id,
            "meeting",
            meeting_id,
            meeting_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        MeetingActionItem::try_from(row)
    }
}

async fn meeting_action_rows(
    database: &Database,
    user_id: Uuid,
    meeting_id: Uuid,
) -> Result<Vec<MeetingActionItemRow>, StorageError> {
    sqlx::query_as::<_, MeetingActionItemRow>(
        "SELECT item.id, item.meeting_id, item.kind, item.project_id,
            item.title, item.notes, item.priority, item.due_at,
            item.starts_at, item.ends_at, item.time_zone,
            item.source_excerpt, item.confidence, item.status,
            item.target_entity_id, item.version
         FROM meeting_action_items AS item
         INNER JOIN meetings AS meeting ON meeting.id = item.meeting_id
         WHERE meeting.user_id = $1 AND meeting.id = $2
         ORDER BY item.created_at, item.id",
    )
    .bind(user_id)
    .bind(meeting_id)
    .fetch_all(database.pool())
    .await
    .map_err(classify)
}

async fn meeting_scope_is_owned(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> Result<bool, StorageError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT
            ($2::uuid IS NULL OR EXISTS(
                SELECT 1 FROM workspaces WHERE id = $2 AND user_id = $1
            ))
            AND ($3::uuid IS NULL OR EXISTS(
                SELECT 1 FROM projects
                WHERE id = $3 AND user_id = $1
                  AND ($2::uuid IS NULL OR workspace_id = $2)
            ))",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(project_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

const fn action_kind_value(kind: MeetingActionKind) -> &'static str {
    match kind {
        MeetingActionKind::Task => "task",
        MeetingActionKind::Schedule => "schedule",
    }
}

const fn action_status_value(status: MeetingActionStatus) -> &'static str {
    match status {
        MeetingActionStatus::Suggested => "suggested",
        MeetingActionStatus::Applied => "applied",
        MeetingActionStatus::Rejected => "rejected",
    }
}

fn parse_meeting_status(value: &str) -> Result<MeetingStatus, StorageError> {
    match value {
        "queued" => Ok(MeetingStatus::Queued),
        "analyzing" => Ok(MeetingStatus::Analyzing),
        "review_ready" => Ok(MeetingStatus::ReviewReady),
        "applied" => Ok(MeetingStatus::Applied),
        "failed" => Ok(MeetingStatus::Failed),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_action_kind(value: &str) -> Result<MeetingActionKind, StorageError> {
    match value {
        "task" => Ok(MeetingActionKind::Task),
        "schedule" => Ok(MeetingActionKind::Schedule),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_action_status(value: &str) -> Result<MeetingActionStatus, StorageError> {
    match value {
        "suggested" => Ok(MeetingActionStatus::Suggested),
        "applied" => Ok(MeetingActionStatus::Applied),
        "rejected" => Ok(MeetingActionStatus::Rejected),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn claim_lease_millis(runner_id: &str, lease: Duration) -> Result<i64, StorageError> {
    if !valid_runner_id(runner_id) || lease.is_zero() {
        return Err(StorageError::InvalidConfiguration);
    }
    i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)
}

fn valid_runner_id(value: &str) -> bool {
    valid_text(value, 200)
}

fn valid_error_code(value: &str) -> bool {
    valid_text(value, 120)
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
}

fn valid_optional_id(value: Option<Uuid>) -> bool {
    value.is_none_or(is_v7)
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_optional_body_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| valid_body_text(value, maximum))
}

fn valid_text(value: &str, maximum: usize) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

fn valid_body_text(value: &str, maximum: usize) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().count() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn trimmed_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trimmed_strings(values: &[String]) -> Vec<String> {
    values.iter().map(|value| value.trim().to_owned()).collect()
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{
        MeetingActionKind, MeetingAnalysisResult, NewMeeting, NewMeetingActionItem,
        NewMeetingDecision,
    };
    use uuid::Uuid;

    #[test]
    fn meeting_input_requires_bounded_source_text() {
        let input = NewMeeting {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            workspace_id: None,
            project_id: None,
            title: "제품 회의".to_owned(),
            transcript: "지민: 출시 전 흐름을 다시 검토해요.\n담당자: 내일까지 확인할게요."
                .to_owned(),
            started_at: None,
            duration_seconds: Some(600),
        };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn schedule_suggestion_requires_a_complete_time_window() {
        let result = MeetingAnalysisResult {
            summary: "출시 전 검토 일정을 잡기로 했다.".to_owned(),
            topics: vec!["출시 준비".to_owned()],
            risks: Vec::new(),
            follow_up: None,
            decisions: vec![NewMeetingDecision {
                id: Uuid::now_v7(),
                content: "계약 등록 흐름을 재검토한다.".to_owned(),
                rationale: None,
                source_excerpt: "계약 등록 흐름을 다시 보죠.".to_owned(),
                source_timestamp_seconds: None,
            }],
            action_items: vec![NewMeetingActionItem {
                id: Uuid::now_v7(),
                target_entity_id: Uuid::now_v7(),
                kind: MeetingActionKind::Schedule,
                project_id: None,
                title: "계약 등록 검토".to_owned(),
                notes: None,
                priority: 1,
                due_at: None,
                starts_at: None,
                ends_at: None,
                time_zone: None,
                source_excerpt: "내일 다시 검토하죠.".to_owned(),
                confidence: 90,
            }],
        };
        assert!(result.validate().is_err());
    }
}

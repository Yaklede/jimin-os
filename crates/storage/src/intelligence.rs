//! Persistent P1 work-intelligence records.
//!
//! Recommendations are not tasks. They retain the assistant's reason,
//! expected effect, risk, and confidence until the owner makes an explicit
//! decision. Decision writes use optimistic concurrency and an idempotent
//! client mutation ID before later stages execute any suggested action.

use std::collections::BTreeMap;

use sqlx::{Postgres, Transaction};
use time::{OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::append_change,
    gmail::GmailMessage,
    goals::{GoalHealth, GoalOverview, GoalStatus},
    planning::{ScheduleEntry, Task},
    work::{Project, ProjectStatus},
};

const MAX_TITLE_CHARS: usize = 200;
const MAX_RATIONALE_CHARS: usize = 4_000;
const MAX_EFFECT_CHARS: usize = 2_000;
const MAX_REASON_CHARS: usize = 2_000;
const MAX_RECOMMENDATION_LIST: i64 = 50;
const MAX_EFFORT_MINUTES: i32 = 10_080;
const INSERT_RECOMMENDATION_SQL: &str = "
    INSERT INTO recommendations (
        id, user_id, workspace_id, project_id, goal_id, signal_id,
        title, rationale, expected_effect, risk_summary,
        confidence, urgency, impact, risk_level, effort_minutes,
        suggested_action_kind, suggested_entity_id, status, valid_until
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
        $11, $12, $13, $14, $15, $16, $17, 'pending', $18
    )
    RETURNING
        id, workspace_id, project_id, goal_id, signal_id,
        title, rationale, expected_effect, risk_summary,
        confidence, urgency, impact, risk_level, effort_minutes,
        suggested_action_kind, suggested_entity_id, status, valid_until, revisit_at,
        created_at, updated_at, version";
const SELECT_ACTIVE_RECOMMENDATIONS_SQL: &str = "
    SELECT
        id, workspace_id, project_id, goal_id, signal_id,
        title, rationale, expected_effect, risk_summary,
        confidence, urgency, impact, risk_level, effort_minutes,
        suggested_action_kind, suggested_entity_id, status, valid_until, revisit_at,
        created_at, updated_at, version
    FROM recommendations
    WHERE user_id = $1
      AND (
          status IN ('pending', 'analysis_requested')
          OR (status = 'deferred' AND revisit_at <= $2)
      )
      AND (valid_until IS NULL OR valid_until > $2)
    ORDER BY urgency DESC, impact DESC, confidence DESC, created_at DESC, id DESC
    LIMIT $3";
const SELECT_RECOMMENDATION_HISTORY_SQL: &str = "
    SELECT
        id, workspace_id, project_id, goal_id, signal_id,
        title, rationale, expected_effect, risk_summary,
        confidence, urgency, impact, risk_level, effort_minutes,
        suggested_action_kind, suggested_entity_id, status, valid_until, revisit_at,
        created_at, updated_at, version
    FROM recommendations
    WHERE user_id = $1
    ORDER BY updated_at DESC, created_at DESC, id DESC
    LIMIT $2";
const UPDATE_RECOMMENDATION_DECISION_SQL: &str = "
    UPDATE recommendations
    SET status = $4, revisit_at = $5
    WHERE id = $1 AND user_id = $2 AND version = $3
      AND status IN ('pending', 'deferred', 'analysis_requested')
    RETURNING
        id, workspace_id, project_id, goal_id, signal_id,
        title, rationale, expected_effect, risk_summary,
        confidence, urgency, impact, risk_level, effort_minutes,
        suggested_action_kind, suggested_entity_id, status, valid_until, revisit_at,
        created_at, updated_at, version";
const SELECT_RECOMMENDATION_BY_ID_SQL: &str = "
    SELECT
        id, workspace_id, project_id, goal_id, signal_id,
        title, rationale, expected_effect, risk_summary,
        confidence, urgency, impact, risk_level, effort_minutes,
        suggested_action_kind, suggested_entity_id, status, valid_until, revisit_at,
        created_at, updated_at, version
    FROM recommendations
    WHERE id = $1 AND user_id = $2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationStatus {
    Pending,
    Approved,
    Rejected,
    Deferred,
    AnalysisRequested,
    Executing,
    Executed,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestedActionKind {
    Review,
    CreateTask,
    UpdateTask,
    CreateSchedule,
    UpdateProject,
    RunWebhook,
    RequestAnalysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationDecision {
    Approve,
    Reject,
    Defer,
    RequestAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommendation {
    pub id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub goal_id: Option<Uuid>,
    pub signal_id: Option<Uuid>,
    pub title: String,
    pub rationale: String,
    pub expected_effect: String,
    pub risk_summary: Option<String>,
    pub confidence: i16,
    pub urgency: i16,
    pub impact: i16,
    pub risk_level: i16,
    pub effort_minutes: Option<i32>,
    pub suggested_action_kind: Option<SuggestedActionKind>,
    pub suggested_entity_id: Option<Uuid>,
    pub status: RecommendationStatus,
    pub valid_until: Option<OffsetDateTime>,
    pub revisit_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

pub struct NewRecommendation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub goal_id: Option<Uuid>,
    pub signal_id: Option<Uuid>,
    pub title: String,
    pub rationale: String,
    pub expected_effect: String,
    pub risk_summary: Option<String>,
    pub confidence: i16,
    pub urgency: i16,
    pub impact: i16,
    pub risk_level: i16,
    pub effort_minutes: Option<i32>,
    pub suggested_action_kind: Option<SuggestedActionKind>,
    pub suggested_entity_id: Option<Uuid>,
    pub valid_until: Option<OffsetDateTime>,
}

/// A schedule request that the assistant intentionally left unexecuted after
/// finding a conflict. It remains in the decision inbox until the same
/// conversation successfully creates or moves a schedule.
pub struct NewScheduleRequestConflict {
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub agent_job_id: Uuid,
    pub conflicting_schedule_id: Option<Uuid>,
    pub rationale: String,
    pub expected_effect: String,
    pub risk_summary: String,
    pub valid_until: OffsetDateTime,
}

pub struct DecideRecommendation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub recommendation_id: Uuid,
    pub decision: RecommendationDecision,
    pub reason: Option<String>,
    pub revisit_at: Option<OffsetDateTime>,
    pub expected_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecideRecommendationOutcome {
    Applied(Recommendation),
    Replayed(Recommendation),
    NotFound,
    VersionConflict,
}

#[derive(sqlx::FromRow)]
struct RecommendationRow {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    goal_id: Option<Uuid>,
    signal_id: Option<Uuid>,
    title: String,
    rationale: String,
    expected_effect: String,
    risk_summary: Option<String>,
    confidence: i16,
    urgency: i16,
    impact: i16,
    risk_level: i16,
    effort_minutes: Option<i32>,
    suggested_action_kind: Option<String>,
    suggested_entity_id: Option<Uuid>,
    status: String,
    valid_until: Option<OffsetDateTime>,
    revisit_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct DecisionReplayRow {
    recommendation_id: Uuid,
    decision: String,
    reason: Option<String>,
    revisit_at: Option<OffsetDateTime>,
    recommendation_version: i64,
}

struct WorkObservation {
    fingerprint: String,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    goal_id: Option<Uuid>,
    severity: i16,
    kind: &'static str,
    source_type: &'static str,
    source_entity_id: Option<Uuid>,
    suggested_action_kind: SuggestedActionKind,
    suggested_entity_id: Option<Uuid>,
    title: String,
    summary: String,
    expected_effect: String,
    risk_summary: Option<String>,
    confidence: i16,
    urgency: i16,
    impact: i16,
    risk_level: i16,
    effort_minutes: Option<i32>,
    valid_until: OffsetDateTime,
}

impl NewRecommendation {
    /// Validates identifiers, bounded copy, scores, and optional action data.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when any value falls
    /// outside the persisted contract.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !valid_optional_id(self.workspace_id)
            || !valid_optional_id(self.project_id)
            || !valid_optional_id(self.goal_id)
            || !valid_optional_id(self.signal_id)
            || !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || !valid_text(&self.rationale, MAX_RATIONALE_CHARS, false)
            || !valid_text(&self.expected_effect, MAX_EFFECT_CHARS, false)
            || !self
                .risk_summary
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_EFFECT_CHARS, false))
            || !(0..=100).contains(&self.confidence)
            || !(0..=3).contains(&self.urgency)
            || !(0..=3).contains(&self.impact)
            || !(0..=3).contains(&self.risk_level)
            || self
                .effort_minutes
                .is_some_and(|value| !(1..=MAX_EFFORT_MINUTES).contains(&value))
            || !valid_optional_id(self.suggested_entity_id)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl DecideRecommendation {
    /// Validates the owner decision and its optimistic-concurrency key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid UUIDs,
    /// versions, or unbounded decision copy.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !is_v7(self.recommendation_id)
            || self.expected_version <= 0
            || !self
                .reason
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_REASON_CHARS, true))
            || match self.decision {
                RecommendationDecision::Defer => self
                    .revisit_at
                    .is_none_or(|value| value <= OffsetDateTime::now_utc()),
                _ => self.revisit_at.is_some(),
            }
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl NewScheduleRequestConflict {
    fn validate(&self, now: OffsetDateTime) -> Result<(), StorageError> {
        if !is_v7(self.user_id)
            || !is_v7(self.conversation_id)
            || !is_v7(self.agent_job_id)
            || !valid_optional_id(self.conflicting_schedule_id)
            || !valid_text(&self.rationale, MAX_RATIONALE_CHARS, false)
            || !valid_text(&self.expected_effect, MAX_EFFECT_CHARS, false)
            || !valid_text(&self.risk_summary, MAX_EFFECT_CHARS, false)
            || self.valid_until <= now
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

impl TryFrom<RecommendationRow> for Recommendation {
    type Error = StorageError;

    fn try_from(row: RecommendationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            project_id: row.project_id,
            goal_id: row.goal_id,
            signal_id: row.signal_id,
            title: row.title,
            rationale: row.rationale,
            expected_effect: row.expected_effect,
            risk_summary: row.risk_summary,
            confidence: row.confidence,
            urgency: row.urgency,
            impact: row.impact,
            risk_level: row.risk_level,
            effort_minutes: row.effort_minutes,
            suggested_action_kind: row
                .suggested_action_kind
                .as_deref()
                .map(parse_suggested_action_kind)
                .transpose()?,
            suggested_entity_id: row.suggested_entity_id,
            status: parse_recommendation_status(&row.status)?,
            valid_until: row.valid_until,
            revisit_at: row.revisit_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: row.version,
        })
    }
}

impl Database {
    /// Returns owners whose active devices should receive independently
    /// refreshed work intelligence. The result contains identifiers only and
    /// is bounded for one worker pass.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when active owners cannot be listed.
    pub async fn active_work_brief_user_ids(&self) -> Result<Vec<Uuid>, StorageError> {
        sqlx::query_scalar(
            "SELECT DISTINCT users.id
             FROM users
             INNER JOIN devices ON devices.user_id = users.id
             WHERE devices.status = 'active'
             ORDER BY users.id
             LIMIT 100",
        )
        .fetch_all(self.pool())
        .await
        .map_err(classify)
    }

    /// Persists one server-generated recommendation after verifying every
    /// optional scope belongs to the same owner.
    ///
    /// # Errors
    ///
    /// Returns a validation, ownership, or persistence error when the record
    /// cannot be stored atomically.
    pub async fn create_recommendation(
        &self,
        recommendation: &NewRecommendation,
    ) -> Result<Recommendation, StorageError> {
        recommendation.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if !recommendation_scope_is_owned(&mut transaction, recommendation).await? {
            return Err(StorageError::IdentityConflict);
        }
        let row = sqlx::query_as::<_, RecommendationRow>(INSERT_RECOMMENDATION_SQL)
            .bind(recommendation.id)
            .bind(recommendation.user_id)
            .bind(recommendation.workspace_id)
            .bind(recommendation.project_id)
            .bind(recommendation.goal_id)
            .bind(recommendation.signal_id)
            .bind(recommendation.title.trim())
            .bind(recommendation.rationale.trim())
            .bind(recommendation.expected_effect.trim())
            .bind(trim_optional(recommendation.risk_summary.as_deref()))
            .bind(recommendation.confidence)
            .bind(recommendation.urgency)
            .bind(recommendation.impact)
            .bind(recommendation.risk_level)
            .bind(recommendation.effort_minutes)
            .bind(
                recommendation
                    .suggested_action_kind
                    .map(suggested_action_kind_value),
            )
            .bind(recommendation.suggested_entity_id)
            .bind(recommendation.valid_until)
            .fetch_one(&mut *transaction)
            .await
            .map_err(classify)?;
        append_change(
            &mut transaction,
            recommendation.user_id,
            "recommendation",
            recommendation.id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Recommendation::try_from(row)
    }

    /// Keeps one unresolved conversational schedule conflict in the decision
    /// inbox. Replays for the same agent job update the same signal and
    /// recommendation instead of creating duplicate reminders.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the conflict cannot be
    /// recorded atomically.
    pub async fn record_schedule_request_conflict(
        &self,
        conflict: &NewScheduleRequestConflict,
    ) -> Result<Recommendation, StorageError> {
        let now = OffsetDateTime::now_utc();
        conflict.validate(now)?;
        let fingerprint = format!(
            "agent:schedule-conflict:{}:{}",
            conflict.conversation_id, conflict.agent_job_id
        );
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let (signal_id, signal_version) = sqlx::query_as::<_, (Uuid, i64)>(
            "INSERT INTO intelligence_signals (
                id, user_id, kind, severity, title, summary, source_type,
                source_entity_id, fingerprint, status, observed_at, valid_until
             ) VALUES (
                $1, $2, 'schedule_conflict', 2, '일정 시간을 다시 정해 주세요',
                $3, 'manual', $4, $5, 'active', $6, $7
             )
             ON CONFLICT (user_id, fingerprint) WHERE status = 'active'
             DO UPDATE SET
                summary = EXCLUDED.summary,
                source_entity_id = EXCLUDED.source_entity_id,
                observed_at = EXCLUDED.observed_at,
                valid_until = EXCLUDED.valid_until
             RETURNING id, version",
        )
        .bind(Uuid::now_v7())
        .bind(conflict.user_id)
        .bind(conflict.rationale.trim())
        .bind(conflict.conflicting_schedule_id)
        .bind(&fingerprint)
        .bind(now)
        .bind(conflict.valid_until)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let row = sqlx::query_as::<_, RecommendationRow>(
            "INSERT INTO recommendations (
                id, user_id, signal_id, title, rationale, expected_effect,
                risk_summary, confidence, urgency, impact, risk_level,
                effort_minutes, suggested_action_kind, suggested_entity_id,
                status, valid_until
             ) VALUES (
                $1, $2, $3, '일정 시간을 다시 정해 주세요', $4, $5,
                $6, 100, 2, 2, 1, 5, 'review', $7, 'pending', $8
             )
             ON CONFLICT (signal_id) WHERE signal_id IS NOT NULL
             DO UPDATE SET
                rationale = EXCLUDED.rationale,
                expected_effect = EXCLUDED.expected_effect,
                risk_summary = EXCLUDED.risk_summary,
                suggested_entity_id = EXCLUDED.suggested_entity_id,
                status = CASE
                    WHEN recommendations.status IN ('pending', 'deferred', 'analysis_requested')
                        THEN 'pending'
                    ELSE recommendations.status
                END,
                revisit_at = NULL,
                valid_until = EXCLUDED.valid_until
             RETURNING
                id, workspace_id, project_id, goal_id, signal_id,
                title, rationale, expected_effect, risk_summary,
                confidence, urgency, impact, risk_level, effort_minutes,
                suggested_action_kind, suggested_entity_id, status, valid_until,
                revisit_at, created_at, updated_at, version",
        )
        .bind(Uuid::now_v7())
        .bind(conflict.user_id)
        .bind(signal_id)
        .bind(conflict.rationale.trim())
        .bind(conflict.expected_effect.trim())
        .bind(conflict.risk_summary.trim())
        .bind(conflict.conflicting_schedule_id)
        .bind(conflict.valid_until)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            conflict.user_id,
            "intelligence_signal",
            signal_id,
            signal_version,
        )
        .await?;
        append_change(
            &mut transaction,
            conflict.user_id,
            "recommendation",
            row.id,
            row.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Recommendation::try_from(row)
    }

    /// Returns the active decision inbox in server-owned priority order.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid owner or limit, or a
    /// persistence error when the inbox cannot be loaded.
    pub async fn active_recommendations_for_user(
        &self,
        user_id: Uuid,
        now: OffsetDateTime,
        limit: i64,
    ) -> Result<Vec<Recommendation>, StorageError> {
        if !is_v7(user_id) || !(1..=MAX_RECOMMENDATION_LIST).contains(&limit) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, RecommendationRow>(SELECT_ACTIVE_RECOMMENDATIONS_SQL)
            .bind(user_id)
            .bind(now)
            .bind(limit)
            .fetch_all(self.pool())
            .await
            .map_err(classify)?;
        rows.into_iter().map(Recommendation::try_from).collect()
    }

    /// Returns recent recommendation decisions and pending items together so
    /// the owner can review what the assistant proposed and how it ended.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid owner or limit, or a
    /// persistence error when history cannot be loaded.
    pub async fn recommendation_history_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Recommendation>, StorageError> {
        if !is_v7(user_id) || !(1..=MAX_RECOMMENDATION_LIST).contains(&limit) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, RecommendationRow>(SELECT_RECOMMENDATION_HISTORY_SQL)
            .bind(user_id)
            .bind(limit)
            .fetch_all(self.pool())
            .await
            .map_err(classify)?;
        rows.into_iter().map(Recommendation::try_from).collect()
    }

    /// Applies or idempotently replays one explicit owner decision. A safe
    /// review action is completed and audited in the same transaction; actions
    /// that mutate another domain remain approved for their dedicated executor.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the decision cannot be
    /// evaluated atomically. Domain conflicts are returned in the outcome.
    pub async fn decide_recommendation(
        &self,
        command: &DecideRecommendation,
    ) -> Result<DecideRecommendationOutcome, StorageError> {
        command.validate()?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        if let Some(existing) = decision_replay(&mut transaction, command).await? {
            let outcome = replayed_decision(&mut transaction, command, existing).await?;
            transaction.rollback().await.map_err(classify)?;
            return Ok(outcome);
        }

        let target_status = decision_target_status(command.decision);
        let row = sqlx::query_as::<_, RecommendationRow>(UPDATE_RECOMMENDATION_DECISION_SQL)
            .bind(command.recommendation_id)
            .bind(command.user_id)
            .bind(command.expected_version)
            .bind(target_status)
            .bind(command.revisit_at)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?;
        let Some(mut row) = row else {
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(
                    SELECT 1 FROM recommendations WHERE id = $1 AND user_id = $2
                 )",
            )
            .bind(command.recommendation_id)
            .bind(command.user_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(classify)?;
            transaction.rollback().await.map_err(classify)?;
            return Ok(if exists {
                DecideRecommendationOutcome::VersionConflict
            } else {
                DecideRecommendationOutcome::NotFound
            });
        };

        sqlx::query(
            "INSERT INTO recommendation_decisions (
                id, user_id, recommendation_id, decision, reason, revisit_at,
                recommendation_version
             ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.recommendation_id)
        .bind(decision_value(command.decision))
        .bind(trim_optional(command.reason.as_deref()))
        .bind(command.revisit_at)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        let action_result_id =
            execute_safe_approved_action(&mut transaction, command, &mut row).await?;
        append_change(
            &mut transaction,
            command.user_id,
            "recommendation",
            command.recommendation_id,
            row.version,
        )
        .await?;
        if let Some(result_id) = action_result_id {
            append_change(
                &mut transaction,
                command.user_id,
                "recommendation_action_result",
                result_id,
                1,
            )
            .await?;
        }
        append_change(
            &mut transaction,
            command.user_id,
            "recommendation_decision",
            command.id,
            1,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(DecideRecommendationOutcome::Applied(
            Recommendation::try_from(row)?,
        ))
    }

    /// Re-evaluates structured work state and refreshes the owner's active
    /// decision inbox without relying on title keyword matching.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when owned work cannot be
    /// evaluated and stored atomically.
    pub async fn refresh_work_brief(
        &self,
        user_id: Uuid,
        now: OffsetDateTime,
    ) -> Result<Vec<Recommendation>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let horizon = now
            .checked_add(time::Duration::days(2))
            .ok_or(StorageError::InvalidConfiguration)?;
        let tasks = self.open_tasks_for_user(user_id).await?;
        let projects = self.projects_for_user(user_id).await?;
        let schedules = self
            .schedule_entries_in_range(user_id, now, horizon)
            .await?;
        let goals = self.goal_overviews_for_user(user_id, now).await?;
        let inbox = self.recent_gmail_messages_for_user(user_id).await?;
        let observations = work_observations(&tasks, &projects, &schedules, &goals, &inbox, now);
        let fingerprints = observations
            .iter()
            .map(|observation| observation.fingerprint.clone())
            .collect::<Vec<_>>();
        let mut transaction = self.pool().begin().await.map_err(classify)?;

        for observation in &observations {
            let signal_id = upsert_work_signal(&mut transaction, user_id, observation, now).await?;
            if let Some(row) =
                insert_work_recommendation(&mut transaction, user_id, signal_id, observation)
                    .await?
            {
                append_change(
                    &mut transaction,
                    user_id,
                    "recommendation",
                    row.id,
                    row.version,
                )
                .await?;
            }
        }

        sqlx::query(
            "WITH resolved AS (
                UPDATE intelligence_signals
                SET status = 'resolved', resolved_at = $2
                WHERE user_id = $1 AND status = 'active'
                  AND fingerprint LIKE 'work:%'
                  AND NOT (fingerprint = ANY($3::text[]))
                RETURNING id
             )
             UPDATE recommendations
             SET status = 'expired', revisit_at = NULL
             WHERE signal_id IN (SELECT id FROM resolved)
               AND status IN ('pending', 'deferred', 'analysis_requested')",
        )
        .bind(user_id)
        .bind(now)
        .bind(&fingerprints)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;

        let rows = sqlx::query_as::<_, RecommendationRow>(SELECT_ACTIVE_RECOMMENDATIONS_SQL)
            .bind(user_id)
            .bind(now)
            .bind(5_i64)
            .fetch_all(&mut *transaction)
            .await
            .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        rows.into_iter().map(Recommendation::try_from).collect()
    }
}

async fn mark_review_recommendation_executed(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    recommendation_id: Uuid,
) -> Result<RecommendationRow, StorageError> {
    sqlx::query_as::<_, RecommendationRow>(
        "UPDATE recommendations
         SET status = 'executed', revisit_at = NULL
         WHERE id = $1 AND user_id = $2 AND status = 'approved'
         RETURNING
            id, workspace_id, project_id, goal_id, signal_id,
            title, rationale, expected_effect, risk_summary,
            confidence, urgency, impact, risk_level, effort_minutes,
            suggested_action_kind, suggested_entity_id, status, valid_until,
            revisit_at, created_at, updated_at, version",
    )
    .bind(recommendation_id)
    .bind(user_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

async fn execute_safe_approved_action(
    transaction: &mut Transaction<'_, Postgres>,
    command: &DecideRecommendation,
    recommendation: &mut RecommendationRow,
) -> Result<Option<Uuid>, StorageError> {
    if command.decision != RecommendationDecision::Approve
        || recommendation.suggested_action_kind.as_deref() != Some("review")
    {
        return Ok(None);
    }
    let completed_at = OffsetDateTime::now_utc();
    sqlx::query(
        "INSERT INTO recommendation_action_results (
            id, user_id, recommendation_id, action_type, entity_id,
            status, summary, expected_effect, actual_effect,
            started_at, completed_at
         ) VALUES (
            $1, $2, $3, 'review', $4, 'succeeded', $5, $6, $7, $8, $8
         )",
    )
    .bind(command.id)
    .bind(command.user_id)
    .bind(command.recommendation_id)
    .bind(recommendation.suggested_entity_id)
    .bind("추천 내용을 확인했어요.")
    .bind(&recommendation.expected_effect)
    .bind("사용자가 추천의 근거와 예상 효과를 확인했어요.")
    .bind(completed_at)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    *recommendation = mark_review_recommendation_executed(
        transaction,
        command.user_id,
        command.recommendation_id,
    )
    .await?;
    Ok(Some(command.id))
}

async fn upsert_work_signal(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    observation: &WorkObservation,
    now: OffsetDateTime,
) -> Result<Uuid, StorageError> {
    sqlx::query_scalar(
        "INSERT INTO intelligence_signals (
            id, user_id, workspace_id, project_id, goal_id, kind, severity,
            title, summary, source_type, source_entity_id, fingerprint,
            status, observed_at, valid_until
         ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
            'active', $13, $14
         )
         ON CONFLICT (user_id, fingerprint) WHERE status = 'active'
         DO UPDATE SET
            workspace_id = EXCLUDED.workspace_id,
            project_id = EXCLUDED.project_id,
            goal_id = EXCLUDED.goal_id,
            kind = EXCLUDED.kind,
            severity = EXCLUDED.severity,
            title = EXCLUDED.title,
            summary = EXCLUDED.summary,
            source_type = EXCLUDED.source_type,
            source_entity_id = EXCLUDED.source_entity_id,
            observed_at = EXCLUDED.observed_at,
            valid_until = EXCLUDED.valid_until
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(observation.workspace_id)
    .bind(observation.project_id)
    .bind(observation.goal_id)
    .bind(observation.kind)
    .bind(observation.severity)
    .bind(&observation.title)
    .bind(&observation.summary)
    .bind(observation.source_type)
    .bind(observation.source_entity_id)
    .bind(&observation.fingerprint)
    .bind(now)
    .bind(observation.valid_until)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

async fn insert_work_recommendation(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    signal_id: Uuid,
    observation: &WorkObservation,
) -> Result<Option<RecommendationRow>, StorageError> {
    sqlx::query_as::<_, RecommendationRow>(
        "INSERT INTO recommendations (
            id, user_id, workspace_id, project_id, goal_id, signal_id,
            title, rationale, expected_effect, risk_summary,
            confidence, urgency, impact, risk_level, effort_minutes,
            suggested_action_kind, suggested_entity_id, status, valid_until
         ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, 'pending', $18
         )
         ON CONFLICT (signal_id) WHERE signal_id IS NOT NULL DO NOTHING
         RETURNING
            id, workspace_id, project_id, goal_id, signal_id,
            title, rationale, expected_effect, risk_summary,
            confidence, urgency, impact, risk_level, effort_minutes,
            suggested_action_kind, suggested_entity_id, status, valid_until,
            revisit_at, created_at, updated_at, version",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(observation.workspace_id)
    .bind(observation.project_id)
    .bind(observation.goal_id)
    .bind(signal_id)
    .bind(&observation.title)
    .bind(&observation.summary)
    .bind(&observation.expected_effect)
    .bind(observation.risk_summary.as_deref())
    .bind(observation.confidence)
    .bind(observation.urgency)
    .bind(observation.impact)
    .bind(observation.risk_level)
    .bind(observation.effort_minutes)
    .bind(suggested_action_kind_value(
        observation.suggested_action_kind,
    ))
    .bind(observation.suggested_entity_id)
    .bind(observation.valid_until)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)
}

fn work_observations(
    tasks: &[Task],
    projects: &[Project],
    schedules: &[ScheduleEntry],
    goals: &[GoalOverview],
    inbox: &[GmailMessage],
    now: OffsetDateTime,
) -> Vec<WorkObservation> {
    let mut observations = Vec::new();
    if let Some(task) = priority_focus_task(tasks, goals, now) {
        let goal = active_goal_for_task(task, goals);
        observations.push(task_focus_observation(task, goal, now));
    }
    observations.extend(
        projects
            .iter()
            .filter(|project| project.status == ProjectStatus::Active)
            .filter_map(|project| project_attention_observation(project, now)),
    );
    if tasks.len() >= 5 {
        observations.push(workload_observation(tasks.len(), now));
    }
    observations.extend(assignee_workload_observations(tasks, now));
    observations.extend(schedule_observations(schedules, now));
    if let Some(observation) = tomorrow_work_observation(tasks, schedules, now) {
        observations.push(observation);
    }
    observations.extend(
        goals
            .iter()
            .filter(|overview| overview.goal.status == GoalStatus::Active)
            .filter_map(|goal| goal_observation(goal, now)),
    );
    if let Some(observation) = inbox_observation(inbox, now) {
        observations.push(observation);
    }
    observations
}

fn priority_focus_task<'a>(
    tasks: &'a [Task],
    goals: &[GoalOverview],
    now: OffsetDateTime,
) -> Option<&'a Task> {
    tasks.iter().min_by_key(|task| {
        let overdue = task.due_at.is_some_and(|due_at| due_at < now);
        let due_soon = task
            .due_at
            .is_some_and(|due_at| due_at <= now + time::Duration::days(1));
        let goal = active_goal_for_task(task, goals);
        let attention_rank = if overdue {
            0
        } else if due_soon {
            1
        } else if goal.is_some() {
            2
        } else {
            3
        };
        (
            attention_rank,
            goal.and_then(|overview| overview.goal.target_at)
                .map_or(i64::MAX, OffsetDateTime::unix_timestamp),
            std::cmp::Reverse(task.priority),
            task.due_at.map_or(i64::MAX, OffsetDateTime::unix_timestamp),
            task.id,
        )
    })
}

fn active_goal_for_task<'a>(task: &Task, goals: &'a [GoalOverview]) -> Option<&'a GoalOverview> {
    task.project_id.and_then(|project_id| {
        goals.iter().find(|overview| {
            overview.goal.status == GoalStatus::Active
                && overview.goal.project_id == Some(project_id)
        })
    })
}

fn task_focus_observation(
    task: &Task,
    goal: Option<&GoalOverview>,
    now: OffsetDateTime,
) -> WorkObservation {
    let overdue = task.due_at.is_some_and(|due_at| due_at < now);
    let due_soon = task
        .due_at
        .is_some_and(|due_at| due_at >= now && due_at <= now + time::Duration::days(1));
    let (title, summary, urgency, risk_level, risk_summary) = if overdue {
        (
            "기한이 지난 할 일을 먼저 확인하세요".to_owned(),
            format!("‘{}’의 기한이 지났어요.", task.title),
            3,
            2,
            Some("더 늦어지면 연결된 프로젝트 일정에도 영향을 줄 수 있어요.".to_owned()),
        )
    } else if due_soon {
        (
            "마감이 가까운 할 일을 먼저 확인하세요".to_owned(),
            format!("‘{}’의 마감이 하루 안으로 다가왔어요.", task.title),
            2,
            1,
            None,
        )
    } else if let Some(goal) = goal {
        (
            "목표에 연결된 할 일부터 시작하세요".to_owned(),
            format!(
                "‘{}’는 ‘{}’ 목표를 앞으로 진행시키는 다음 할 일이에요.",
                task.title, goal.goal.title
            ),
            1,
            0,
            None,
        )
    } else {
        (
            "우선순위가 높은 할 일부터 시작하세요".to_owned(),
            format!(
                "현재 열린 할 일 중 ‘{}’의 우선순위가 가장 높아요.",
                task.title
            ),
            1,
            0,
            None,
        )
    };
    WorkObservation {
        fingerprint: format!("work:task-focus:{}", task.id),
        workspace_id: goal.and_then(|overview| overview.goal.workspace_id),
        project_id: task.project_id,
        goal_id: goal.map(|overview| overview.goal.id),
        severity: urgency,
        kind: if overdue || due_soon {
            "task_deadline"
        } else {
            "opportunity"
        },
        source_type: "task",
        source_entity_id: Some(task.id),
        suggested_action_kind: SuggestedActionKind::Review,
        suggested_entity_id: Some(task.id),
        title,
        summary,
        expected_effect: goal.map_or_else(
            || "가장 중요한 한 가지에 먼저 집중해 작업 전환 비용을 줄일 수 있어요.".to_owned(),
            |overview| {
                format!(
                    "이 일을 끝내면 ‘{}’ 목표의 진행 근거가 바로 갱신돼요.",
                    overview.goal.title
                )
            },
        ),
        risk_summary,
        confidence: 96,
        urgency,
        impact: if goal.is_some() {
            task.priority.max(2)
        } else {
            task.priority.max(1)
        },
        risk_level,
        effort_minutes: None,
        valid_until: now + time::Duration::days(2),
    }
}

fn project_attention_observation(
    project: &Project,
    now: OffsetDateTime,
) -> Option<WorkObservation> {
    let (title, summary, severity, risk_level, risk_summary) = if project.overdue_task_count > 0 {
        (
            format!("{}의 늦어진 일을 먼저 정리하세요", project.title),
            format!(
                "진행률은 {}%이고 기한이 지난 실행 항목이 {}개 있어요.",
                project.progress_percent, project.overdue_task_count
            ),
            3,
            2,
            Some("늦어진 하위 일이 전체 프로젝트 일정에 영향을 줄 수 있어요.".to_owned()),
        )
    } else if project.risk_level >= 2 {
        (
            format!("{}의 위험 요소를 먼저 확인하세요", project.title),
            "프로젝트 위험도가 높게 설정되어 있어 진행 상태를 다시 확인할 필요가 있어요."
                .to_owned(),
            project.risk_level,
            project.risk_level,
            Some("확인하지 않으면 일정이나 범위 조정이 늦어질 수 있어요.".to_owned()),
        )
    } else if project.unassigned_task_count > 0 {
        (
            format!("{}의 담당자를 정하세요", project.title),
            format!(
                "담당자가 정해지지 않은 실행 항목이 {}개 있어요.",
                project.unassigned_task_count
            ),
            2,
            1,
            Some("담당자가 없으면 시작 여부와 진행 상태를 확인하기 어려워요.".to_owned()),
        )
    } else if project.progress_percent == 100 && project.total_task_count > 0 {
        (
            format!("{}의 완료 여부를 확인하세요", project.title),
            "실행 항목을 모두 마쳤어요. 프로젝트의 목적까지 달성했는지 확인할 차례예요.".to_owned(),
            1,
            0,
            None,
        )
    } else if project.open_task_count > 0 && project.next_action.is_none() {
        (
            format!("{}의 다음 행동을 정하세요", project.title),
            format!(
                "열린 할 일 {}개가 있지만 다음 행동이 정해지지 않았어요.",
                project.open_task_count
            ),
            1,
            0,
            None,
        )
    } else {
        return None;
    };
    Some(WorkObservation {
        fingerprint: format!("work:project-attention:{}", project.id),
        workspace_id: Some(project.workspace_id),
        project_id: Some(project.id),
        goal_id: None,
        severity,
        kind: "project_risk",
        source_type: "project",
        source_entity_id: Some(project.id),
        suggested_action_kind: SuggestedActionKind::Review,
        suggested_entity_id: Some(project.id),
        title,
        summary,
        expected_effect: "다음 행동을 분명히 해 프로젝트가 멈춰 있는 시간을 줄일 수 있어요."
            .to_owned(),
        risk_summary,
        confidence: 94,
        urgency: severity,
        impact: project.risk_level.max(1),
        risk_level,
        effort_minutes: Some(10),
        valid_until: now + time::Duration::days(7),
    })
}

fn workload_observation(task_count: usize, now: OffsetDateTime) -> WorkObservation {
    WorkObservation {
        fingerprint: "work:workload:open-tasks".to_owned(),
        workspace_id: None,
        project_id: None,
        goal_id: None,
        severity: 2,
        kind: "workload",
        source_type: "system",
        source_entity_id: None,
        suggested_action_kind: SuggestedActionKind::Review,
        suggested_entity_id: None,
        title: "열린 할 일의 범위를 줄여 보세요".to_owned(),
        summary: format!("현재 열린 할 일이 {task_count}개 있어 우선순위를 다시 정할 시점이에요."),
        expected_effect: "오늘 처리할 범위를 줄이면 중요한 일의 완료 가능성을 높일 수 있어요."
            .to_owned(),
        risk_summary: Some("모든 일을 동시에 시작하면 완료가 늦어질 수 있어요.".to_owned()),
        confidence: 92,
        urgency: 2,
        impact: 2,
        risk_level: 1,
        effort_minutes: Some(10),
        valid_until: now + time::Duration::days(2),
    }
}

fn assignee_workload_observations(tasks: &[Task], now: OffsetDateTime) -> Vec<WorkObservation> {
    let mut workload = BTreeMap::<&str, usize>::new();
    for task in tasks {
        if let Some(assignee) = task.assignee_name.as_deref() {
            *workload.entry(assignee).or_default() += 1;
        }
    }
    workload
        .into_iter()
        .filter(|(_, count)| *count >= 5)
        .take(3)
        .map(|(assignee, count)| WorkObservation {
            fingerprint: format!("work:assignee-workload:{assignee}"),
            workspace_id: None,
            project_id: None,
            goal_id: None,
            severity: 2,
            kind: "workload",
            source_type: "system",
            source_entity_id: None,
            suggested_action_kind: SuggestedActionKind::Review,
            suggested_entity_id: None,
            title: format!("{assignee}님의 업무량을 다시 확인하세요"),
            summary: format!("현재 담당한 열린 할 일이 {count}개 있어요."),
            expected_effect: "담당 업무를 나누거나 우선순위를 정하면 병목을 줄일 수 있어요."
                .to_owned(),
            risk_summary: Some(
                "한 사람에게 업무가 몰리면 프로젝트 일정이 함께 늦어질 수 있어요.".to_owned(),
            ),
            confidence: 98,
            urgency: 2,
            impact: 2,
            risk_level: 1,
            effort_minutes: Some(10),
            valid_until: now + time::Duration::days(2),
        })
        .collect()
}

fn schedule_observations(schedules: &[ScheduleEntry], now: OffsetDateTime) -> Vec<WorkObservation> {
    let mut observations = Vec::new();
    if let Some((first, second)) = schedules
        .windows(2)
        .find_map(|pair| (pair[1].starts_at < pair[0].ends_at).then_some((&pair[0], &pair[1])))
    {
        observations.push(WorkObservation {
            fingerprint: format!("work:schedule-conflict:{}:{}", first.id, second.id),
            workspace_id: None,
            project_id: None,
            goal_id: None,
            severity: 3,
            kind: "schedule_conflict",
            source_type: "schedule",
            source_entity_id: Some(first.id),
            suggested_action_kind: SuggestedActionKind::Review,
            suggested_entity_id: None,
            title: "겹치는 일정을 먼저 확인하세요".to_owned(),
            summary: format!("‘{}’와 ‘{}’ 일정이 겹쳐 있어요.", first.title, second.title),
            expected_effect: "겹친 시간을 미리 조정하면 다음 일정이 늦어지는 일을 줄일 수 있어요."
                .to_owned(),
            risk_summary: Some("이동 시간과 준비 시간을 함께 확인해 주세요.".to_owned()),
            confidence: 99,
            urgency: 3,
            impact: 2,
            risk_level: 2,
            effort_minutes: Some(5),
            valid_until: first.ends_at.max(second.ends_at),
        });
    }

    if let Some(next) = schedules
        .iter()
        .filter(|entry| entry.starts_at >= now)
        .min_by_key(|entry| (entry.starts_at, entry.id))
        .filter(|entry| entry.starts_at <= now + time::Duration::hours(2))
    {
        observations.push(WorkObservation {
            fingerprint: format!("work:schedule-upcoming:{}", next.id),
            workspace_id: None,
            project_id: None,
            goal_id: None,
            severity: 2,
            kind: "opportunity",
            source_type: "schedule",
            source_entity_id: Some(next.id),
            suggested_action_kind: SuggestedActionKind::Review,
            suggested_entity_id: None,
            title: format!("{} 일정을 준비할 시간이에요", next.title),
            summary: "두 시간 안에 시작하는 일정이 있어요.".to_owned(),
            expected_effect: "필요한 자료와 이동 시간을 지금 확인하면 여유 있게 시작할 수 있어요."
                .to_owned(),
            risk_summary: None,
            confidence: 98,
            urgency: 2,
            impact: 1,
            risk_level: 0,
            effort_minutes: Some(5),
            valid_until: next.ends_at,
        });
    }
    observations
}

fn tomorrow_work_observation(
    tasks: &[Task],
    schedules: &[ScheduleEntry],
    now: OffsetDateTime,
) -> Option<WorkObservation> {
    let korea_offset = UtcOffset::from_hms(9, 0, 0).ok()?;
    let korea_now = now.to_offset(korea_offset);
    if korea_now.hour() < 18 {
        return None;
    }
    let tomorrow = korea_now.date().next_day()?;
    let day_after = tomorrow.next_day()?;
    let tomorrow_start =
        PrimitiveDateTime::new(tomorrow, Time::MIDNIGHT).assume_offset(korea_offset);
    let tomorrow_end =
        PrimitiveDateTime::new(day_after, Time::MIDNIGHT).assume_offset(korea_offset);
    let task_count = tasks
        .iter()
        .filter(|task| {
            task.due_at
                .is_some_and(|due_at| due_at >= tomorrow_start && due_at < tomorrow_end)
        })
        .count();
    let schedule_count = schedules
        .iter()
        .filter(|entry| entry.starts_at >= tomorrow_start && entry.starts_at < tomorrow_end)
        .count();
    if task_count + schedule_count == 0 {
        return None;
    }
    Some(WorkObservation {
        fingerprint: format!("work:tomorrow-preview:{tomorrow}"),
        workspace_id: None,
        project_id: None,
        goal_id: None,
        severity: 1,
        kind: "opportunity",
        source_type: "system",
        source_entity_id: None,
        suggested_action_kind: SuggestedActionKind::Review,
        suggested_entity_id: None,
        title: "내일 일정과 할 일을 미리 확인하세요".to_owned(),
        summary: format!("내일 일정 {schedule_count}개와 마감할 일 {task_count}개가 있어요."),
        expected_effect: "오늘 마무리할 것과 내일 바로 시작할 일을 미리 나눌 수 있어요.".to_owned(),
        risk_summary: None,
        confidence: 100,
        urgency: 1,
        impact: 1,
        risk_level: 0,
        effort_minutes: Some(5),
        valid_until: tomorrow_end,
    })
}

fn goal_observation(overview: &GoalOverview, now: OffsetDateTime) -> Option<WorkObservation> {
    let goal = &overview.goal;
    let goal_title = truncate_chars(&goal.title, 140);
    let (title, summary, severity, risk_level, risk_summary) = match overview.health {
        GoalHealth::AtRisk if goal.target_at.is_some_and(|target_at| target_at < now) => (
            format!("{goal_title} 목표의 계획을 다시 확인하세요"),
            format!(
                "현재 진행률은 {}%이고, 목표 날짜가 지났어요.",
                overview.progress_percent
            ),
            3,
            2,
            Some("현재 범위나 목표 날짜를 그대로 둘지 결정해야 해요.".to_owned()),
        ),
        GoalHealth::AtRisk if overview.overdue_task_count > 0 => (
            format!("{goal_title} 목표의 늦어진 일을 확인하세요"),
            format!(
                "현재 진행률은 {}%이고, 기한이 지난 할 일이 {}개 있어요.",
                overview.progress_percent, overview.overdue_task_count
            ),
            2,
            1,
            Some("늦어진 일이 목표 날짜에 영향을 주는지 확인해 주세요.".to_owned()),
        ),
        GoalHealth::AtRisk => (
            format!("{goal_title} 목표의 남은 일을 확인하세요"),
            format!(
                "목표 날짜가 일주일 안으로 다가왔고 진행률은 {}%예요.",
                overview.progress_percent
            ),
            2,
            1,
            None,
        ),
        GoalHealth::NeedsPlan if goal.project_id.is_none() => (
            format!("{goal_title} 목표를 실행할 프로젝트를 연결하세요"),
            "목표는 진행 중이지만 연결된 프로젝트가 없어요.".to_owned(),
            1,
            0,
            None,
        ),
        GoalHealth::NeedsPlan => (
            format!("{goal_title} 목표의 다음 행동을 정하세요"),
            "연결된 프로젝트에 열린 할 일이나 다음 행동이 없어요.".to_owned(),
            1,
            0,
            None,
        ),
        GoalHealth::ReadyToComplete => (
            format!("{goal_title} 목표의 달성 여부를 확인하세요"),
            "연결된 할 일을 모두 마쳤어요. 원하는 결과까지 달성했는지 확인할 차례예요.".to_owned(),
            1,
            0,
            None,
        ),
        GoalHealth::OnTrack | GoalHealth::Paused | GoalHealth::Achieved => return None,
    };

    Some(WorkObservation {
        fingerprint: format!("work:goal-attention:{}", goal.id),
        workspace_id: goal.workspace_id,
        project_id: goal.project_id,
        goal_id: Some(goal.id),
        severity,
        kind: "project_risk",
        source_type: "manual",
        source_entity_id: Some(goal.id),
        suggested_action_kind: SuggestedActionKind::Review,
        suggested_entity_id: goal.project_id,
        title,
        summary,
        expected_effect: "목표와 실제 행동을 다시 맞추면 중요한 결과에 집중할 수 있어요."
            .to_owned(),
        risk_summary,
        confidence: 95,
        urgency: severity,
        impact: 2,
        risk_level,
        effort_minutes: Some(10),
        valid_until: now + time::Duration::days(7),
    })
}

fn truncate_chars(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.to_owned();
    }
    let mut truncated = value
        .chars()
        .take(maximum.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn inbox_observation(inbox: &[GmailMessage], now: OffsetDateTime) -> Option<WorkObservation> {
    let unread_count = inbox.iter().filter(|message| message.is_unread).count();
    if unread_count == 0 {
        return None;
    }
    let severity = if unread_count >= 5 { 2 } else { 1 };
    Some(WorkObservation {
        fingerprint: "work:inbox:unread".to_owned(),
        workspace_id: None,
        project_id: None,
        goal_id: None,
        severity,
        kind: "opportunity",
        source_type: "inbox",
        source_entity_id: None,
        suggested_action_kind: SuggestedActionKind::Review,
        suggested_entity_id: None,
        title: "읽지 않은 메일을 확인하세요".to_owned(),
        summary: format!("최근 받은 메일 중 읽지 않은 메일이 {unread_count}개 있어요."),
        expected_effect: "일정이나 프로젝트에 영향을 주는 요청을 놓치지 않을 수 있어요.".to_owned(),
        risk_summary: None,
        confidence: 90,
        urgency: severity,
        impact: 1,
        risk_level: 0,
        effort_minutes: Some(10),
        valid_until: now + time::Duration::days(1),
    })
}

async fn recommendation_scope_is_owned(
    transaction: &mut Transaction<'_, Postgres>,
    recommendation: &NewRecommendation,
) -> Result<bool, StorageError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT
            ($2::uuid IS NULL OR EXISTS(
                SELECT 1 FROM workspaces WHERE id = $2 AND user_id = $1
            ))
            AND ($3::uuid IS NULL OR EXISTS(
                SELECT 1 FROM projects WHERE id = $3 AND user_id = $1
            ))
            AND ($4::uuid IS NULL OR EXISTS(
                SELECT 1 FROM goals WHERE id = $4 AND user_id = $1
            ))
            AND ($5::uuid IS NULL OR EXISTS(
                SELECT 1 FROM intelligence_signals WHERE id = $5 AND user_id = $1
            ))",
    )
    .bind(recommendation.user_id)
    .bind(recommendation.workspace_id)
    .bind(recommendation.project_id)
    .bind(recommendation.goal_id)
    .bind(recommendation.signal_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

async fn decision_replay(
    transaction: &mut Transaction<'_, Postgres>,
    command: &DecideRecommendation,
) -> Result<Option<DecisionReplayRow>, StorageError> {
    sqlx::query_as::<_, DecisionReplayRow>(
        "SELECT recommendation_id, decision, reason, revisit_at, recommendation_version
         FROM recommendation_decisions
         WHERE id = $1 AND user_id = $2",
    )
    .bind(command.id)
    .bind(command.user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)
}

async fn replayed_decision(
    transaction: &mut Transaction<'_, Postgres>,
    command: &DecideRecommendation,
    existing: DecisionReplayRow,
) -> Result<DecideRecommendationOutcome, StorageError> {
    if existing.recommendation_id != command.recommendation_id
        || existing.decision != decision_value(command.decision)
        || trim_optional(existing.reason.as_deref()) != trim_optional(command.reason.as_deref())
        || existing.revisit_at != command.revisit_at
        || existing.recommendation_version != command.expected_version
    {
        return Ok(DecideRecommendationOutcome::VersionConflict);
    }
    let row = sqlx::query_as::<_, RecommendationRow>(SELECT_RECOMMENDATION_BY_ID_SQL)
        .bind(command.recommendation_id)
        .bind(command.user_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(classify)?;
    let Some(row) = row else {
        return Ok(DecideRecommendationOutcome::NotFound);
    };
    Ok(DecideRecommendationOutcome::Replayed(
        Recommendation::try_from(row)?,
    ))
}

const fn decision_target_status(decision: RecommendationDecision) -> &'static str {
    match decision {
        RecommendationDecision::Approve => "approved",
        RecommendationDecision::Reject => "rejected",
        RecommendationDecision::Defer => "deferred",
        RecommendationDecision::RequestAnalysis => "analysis_requested",
    }
}

const fn decision_value(decision: RecommendationDecision) -> &'static str {
    match decision {
        RecommendationDecision::Approve => "approve",
        RecommendationDecision::Reject => "reject",
        RecommendationDecision::Defer => "defer",
        RecommendationDecision::RequestAnalysis => "request_analysis",
    }
}

const fn suggested_action_kind_value(kind: SuggestedActionKind) -> &'static str {
    match kind {
        SuggestedActionKind::Review => "review",
        SuggestedActionKind::CreateTask => "create_task",
        SuggestedActionKind::UpdateTask => "update_task",
        SuggestedActionKind::CreateSchedule => "create_schedule",
        SuggestedActionKind::UpdateProject => "update_project",
        SuggestedActionKind::RunWebhook => "run_webhook",
        SuggestedActionKind::RequestAnalysis => "request_analysis",
    }
}

fn parse_suggested_action_kind(value: &str) -> Result<SuggestedActionKind, StorageError> {
    match value {
        "review" => Ok(SuggestedActionKind::Review),
        "create_task" => Ok(SuggestedActionKind::CreateTask),
        "update_task" => Ok(SuggestedActionKind::UpdateTask),
        "create_schedule" => Ok(SuggestedActionKind::CreateSchedule),
        "update_project" => Ok(SuggestedActionKind::UpdateProject),
        "run_webhook" => Ok(SuggestedActionKind::RunWebhook),
        "request_analysis" => Ok(SuggestedActionKind::RequestAnalysis),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_recommendation_status(value: &str) -> Result<RecommendationStatus, StorageError> {
    match value {
        "pending" => Ok(RecommendationStatus::Pending),
        "approved" => Ok(RecommendationStatus::Approved),
        "rejected" => Ok(RecommendationStatus::Rejected),
        "deferred" => Ok(RecommendationStatus::Deferred),
        "analysis_requested" => Ok(RecommendationStatus::AnalysisRequested),
        "executing" => Ok(RecommendationStatus::Executing),
        "executed" => Ok(RecommendationStatus::Executed),
        "failed" => Ok(RecommendationStatus::Failed),
        "expired" => Ok(RecommendationStatus::Expired),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn valid_optional_id(value: Option<Uuid>) -> bool {
    value.is_none_or(is_v7)
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_text(value: &str, maximum: usize, allow_blank: bool) -> bool {
    let value = value.trim();
    (allow_blank || !value.is_empty())
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

fn trim_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{
        DecideRecommendation, NewRecommendation, RecommendationDecision, SuggestedActionKind,
        project_attention_observation, tomorrow_work_observation,
    };
    use crate::{
        planning::{ScheduleEntry, ScheduleSource, ScheduleStatus, Task, TaskStatus},
        work::{Project, ProjectManagementMode, ProjectStatus},
    };
    use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
    use uuid::Uuid;

    fn valid_recommendation() -> NewRecommendation {
        NewRecommendation {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            workspace_id: None,
            project_id: None,
            goal_id: None,
            signal_id: None,
            title: "마감 위험을 먼저 줄이세요".to_owned(),
            rationale: "기한이 임박한 열린 일이 있습니다.".to_owned(),
            expected_effect: "프로젝트 지연 가능성을 줄입니다.".to_owned(),
            risk_summary: Some("다른 작업 시작이 늦어질 수 있습니다.".to_owned()),
            confidence: 92,
            urgency: 3,
            impact: 3,
            risk_level: 1,
            effort_minutes: Some(30),
            suggested_action_kind: Some(SuggestedActionKind::CreateTask),
            suggested_entity_id: None,
            valid_until: Some(OffsetDateTime::now_utc() + time::Duration::days(1)),
        }
    }

    #[test]
    fn recommendation_input_is_bounded() {
        assert!(valid_recommendation().validate().is_ok());

        let mut invalid = valid_recommendation();
        invalid.confidence = 101;
        assert!(invalid.validate().is_err());

        let mut invalid = valid_recommendation();
        invalid.rationale = "\n".to_owned();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn decision_requires_a_versioned_idempotency_key() {
        let command = DecideRecommendation {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            recommendation_id: Uuid::now_v7(),
            decision: RecommendationDecision::Approve,
            reason: Some("오늘 일정에 반영합니다.".to_owned()),
            revisit_at: None,
            expected_version: 1,
        };
        assert!(command.validate().is_ok());

        let invalid = DecideRecommendation {
            expected_version: 0,
            ..command
        };
        assert!(invalid.validate().is_err());

        let deferred = DecideRecommendation {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            recommendation_id: Uuid::now_v7(),
            decision: RecommendationDecision::Defer,
            reason: None,
            revisit_at: Some(OffsetDateTime::now_utc() + time::Duration::hours(1)),
            expected_version: 1,
        };
        assert!(deferred.validate().is_ok());
    }

    #[test]
    fn evening_brief_previews_tomorrow_in_korea_time() {
        let offset = UtcOffset::from_hms(9, 0, 0).expect("Korea offset");
        let now = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::July, 24).expect("valid date"),
            Time::from_hms(18, 0, 0).expect("valid time"),
        )
        .assume_offset(offset);
        let tomorrow_due = now + time::Duration::hours(16);
        let task = Task {
            id: Uuid::now_v7(),
            project_id: None,
            parent_task_id: None,
            title: "내일 마감할 일".to_owned(),
            notes: None,
            assignee_name: None,
            status: TaskStatus::Open,
            priority: 1,
            due_at: Some(tomorrow_due),
            completed_at: None,
            version: 1,
        };
        let schedule = ScheduleEntry {
            id: Uuid::now_v7(),
            title: "내일 회의".to_owned(),
            notes: None,
            starts_at: tomorrow_due + time::Duration::hours(1),
            ends_at: tomorrow_due + time::Duration::hours(2),
            time_zone: "Asia/Seoul".to_owned(),
            status: ScheduleStatus::Confirmed,
            source: ScheduleSource::Manual,
            editable: true,
            version: 1,
        };

        assert!(
            tomorrow_work_observation(
                std::slice::from_ref(&task),
                std::slice::from_ref(&schedule),
                now - time::Duration::hours(1),
            )
            .is_none()
        );
        let observation = tomorrow_work_observation(&[task], &[schedule], now)
            .expect("evening work should create a tomorrow preview");
        assert_eq!(observation.title, "내일 일정과 할 일을 미리 확인하세요");
        assert_eq!(
            observation.summary,
            "내일 일정 1개와 마감할 일 1개가 있어요."
        );
        assert_eq!(observation.urgency, 1);
    }

    #[test]
    fn project_attention_prioritizes_overdue_executable_work() {
        let project = Project {
            id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            title: "개인 운영체제".to_owned(),
            objective: Some("실행 흐름 완성".to_owned()),
            status: ProjectStatus::Active,
            management_mode: ProjectManagementMode::Completion,
            reporting_enabled: true,
            stale_threshold_days: 7,
            risk_level: 1,
            next_action: Some("늦어진 하위 일 처리".to_owned()),
            due_at: None,
            open_task_count: 3,
            total_task_count: 4,
            completed_task_count: 2,
            overdue_task_count: 1,
            unassigned_task_count: 0,
            progress_percent: 50,
            weekly_created_task_count: 0,
            weekly_completed_task_count: 0,
            backlog_delta: 0,
            stale_task_count: 0,
            average_cycle_time_hours: 0,
            on_time_completion_percent: None,
            version: 1,
        };
        let observation = project_attention_observation(&project, OffsetDateTime::now_utc())
            .expect("overdue work should need attention");
        assert!(observation.title.contains("늦어진 일"));
        assert!(observation.summary.contains("50%"));
        assert_eq!(observation.urgency, 3);
    }
}

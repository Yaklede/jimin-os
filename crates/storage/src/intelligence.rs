//! Persistent P1 work-intelligence records.
//!
//! Recommendations are not tasks. They retain the assistant's reason,
//! expected effect, risk, and confidence until the owner makes an explicit
//! decision. Decision writes use optimistic concurrency and an idempotent
//! client mutation ID before later stages execute any suggested action.

use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

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

    /// Applies or idempotently replays one explicit owner decision. Approval
    /// changes recommendation state only; action execution remains a separate
    /// audited stage.
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
        let Some(row) = row else {
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
        append_change(
            &mut transaction,
            command.user_id,
            "recommendation",
            command.recommendation_id,
            row.version,
        )
        .await?;
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
    };
    use time::OffsetDateTime;
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
}

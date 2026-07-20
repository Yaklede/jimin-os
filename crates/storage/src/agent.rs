//! Durable, server-owned agent conversation queues. The runtime claims these
//! jobs later; API requests never connect to Codex directly.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::{append_change, append_delete_change},
    calendar_mutation::{
        ScheduleCalendarMutationOperation, ScheduleCalendarMutationPayload,
        attach_schedule_to_active_primary_and_queue_create, queue_linked_schedule_mutation,
    },
    planning::TaskStatus,
    webhook::{project_event_payload, queue_project_event_in_transaction},
    work::ProjectStatus,
};

const MAX_CONTENT_CHARS: usize = 24_000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_RUNNER_ID_CHARS: usize = 200;
const MAX_AUTH_URL_CHARS: usize = 2_048;
const MAX_AUTH_USER_CODE_CHARS: usize = 256;
const MAX_MODEL_ID_CHARS: usize = 200;
const MAX_MODEL_NAME_CHARS: usize = 200;
const MAX_MODEL_DESCRIPTION_CHARS: usize = 2_000;
const MAX_REASONING_EFFORT_ID_CHARS: usize = 80;
const MAX_REASONING_EFFORT_DESCRIPTION_CHARS: usize = 1_000;
const MAX_REASONING_EFFORT_COUNT: usize = 16;
const MAX_MODEL_COUNT: usize = 128;
const MAX_PRESENTATION_ITEMS: usize = 32;
const MAX_PRESENTATION_SECTIONS: usize = 3;
const MAX_PRESENTATION_DETAIL_CHARS: usize = 500;
const MAX_PRESENTATION_TIMESTAMP_CHARS: usize = 80;
const MAX_AGENT_ACTIONS: usize = 32;
const MIN_CLAIM_LEASE: Duration = Duration::from_secs(5);
const MAX_CLAIM_LEASE: Duration = Duration::from_mins(5);

pub struct NewConversation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
}

impl NewConversation {
    /// Validates bounded client-visible conversation metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs or text.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !self
                .title
                .as_deref()
                .is_none_or(|title| valid_text(title, MAX_TITLE_CHARS, false))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

pub struct NewAgentTurn {
    pub job_id: Uuid,
    pub message_id: Uuid,
    pub client_message_id: Uuid,
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
}

/// A local planning action extracted from a conversational request. The API
/// decides whether a narrow deterministic action can execute immediately or
/// should remain in `waiting_approval` for an explicit owner decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAgentAction {
    CreateTask {
        title: String,
        due_at: Option<OffsetDateTime>,
    },
    CreateSchedule {
        title: String,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
        time_zone: String,
    },
}

/// One server-validated planning mutation selected by the managed assistant.
///
/// The worker supplies complete replacement values for updates from the
/// authenticated context. Storage validates ownership and optimistic versions
/// again, then commits the mutation with the final assistant message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentActionCommand {
    CreateTask {
        id: Uuid,
        project_id: Option<Uuid>,
        title: String,
        notes: Option<String>,
        priority: i16,
        due_at: Option<OffsetDateTime>,
    },
    UpdateTask {
        id: Uuid,
        project_id: Option<Uuid>,
        title: String,
        notes: Option<String>,
        priority: i16,
        due_at: Option<OffsetDateTime>,
        expected_version: i64,
    },
    SetTaskStatus {
        id: Uuid,
        status: TaskStatus,
        expected_version: i64,
    },
    CreateSchedule {
        id: Uuid,
        title: String,
        notes: Option<String>,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
        time_zone: String,
        allow_schedule_conflict: bool,
    },
    UpdateSchedule {
        id: Uuid,
        title: String,
        notes: Option<String>,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
        time_zone: String,
        expected_version: i64,
        allow_schedule_conflict: bool,
    },
    CancelSchedule {
        id: Uuid,
        expected_version: i64,
    },
    CreateProject {
        id: Uuid,
        workspace_id: Uuid,
        title: String,
        objective: Option<String>,
        risk_level: i16,
        next_action: Option<String>,
        due_at: Option<OffsetDateTime>,
    },
    UpdateProject {
        id: Uuid,
        title: String,
        objective: Option<String>,
        status: ProjectStatus,
        risk_level: i16,
        next_action: Option<String>,
        due_at: Option<OffsetDateTime>,
        expected_version: i64,
    },
    DeleteProject {
        id: Uuid,
        expected_version: i64,
    },
    SendWebhookMessage {
        id: Uuid,
        project_id: Uuid,
        webhook_id: Uuid,
        message: String,
    },
}

impl AgentActionCommand {
    #[allow(clippy::too_many_lines)] // Keep every action variant's storage boundary validation explicit and adjacent.
    fn validate(&self) -> Result<(), StorageError> {
        let valid_optional = |value: Option<&String>, maximum| {
            value.is_none_or(|value| valid_text(value, maximum, true))
        };
        match self {
            Self::CreateTask {
                id,
                project_id,
                title,
                notes,
                priority,
                ..
            } => {
                if is_v7(*id)
                    && project_id.is_none_or(is_v7)
                    && valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_optional(notes.as_ref(), 10_000)
                    && (0..=3).contains(priority)
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::UpdateTask {
                id,
                project_id,
                title,
                notes,
                priority,
                expected_version,
                ..
            } => {
                if is_v7(*id)
                    && project_id.is_none_or(is_v7)
                    && valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_optional(notes.as_ref(), 10_000)
                    && (0..=3).contains(priority)
                    && *expected_version > 0
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::SetTaskStatus {
                id,
                expected_version,
                ..
            }
            | Self::CancelSchedule {
                id,
                expected_version,
            }
            | Self::DeleteProject {
                id,
                expected_version,
            } => {
                if is_v7(*id) && *expected_version > 0 {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::CreateSchedule {
                id,
                title,
                notes,
                starts_at,
                ends_at,
                time_zone,
                ..
            } => {
                if is_v7(*id)
                    && valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_optional(notes.as_ref(), 10_000)
                    && valid_time_zone(time_zone)
                    && ends_at > starts_at
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::UpdateSchedule {
                id,
                title,
                notes,
                starts_at,
                ends_at,
                time_zone,
                expected_version,
                ..
            } => {
                if is_v7(*id)
                    && valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_optional(notes.as_ref(), 10_000)
                    && valid_time_zone(time_zone)
                    && ends_at > starts_at
                    && *expected_version > 0
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::CreateProject {
                id,
                workspace_id,
                title,
                objective,
                risk_level,
                next_action,
                ..
            } => {
                if is_v7(*id)
                    && is_v7(*workspace_id)
                    && valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_optional(objective.as_ref(), 10_000)
                    && valid_optional(next_action.as_ref(), 500)
                    && (0..=3).contains(risk_level)
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::UpdateProject {
                id,
                title,
                objective,
                risk_level,
                next_action,
                expected_version,
                ..
            } => {
                if is_v7(*id)
                    && valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_optional(objective.as_ref(), 10_000)
                    && valid_optional(next_action.as_ref(), 500)
                    && (0..=3).contains(risk_level)
                    && *expected_version > 0
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::SendWebhookMessage {
                id,
                project_id,
                webhook_id,
                message,
            } => {
                if is_v7(*id)
                    && is_v7(*project_id)
                    && is_v7(*webhook_id)
                    && valid_text(message, 1_800, false)
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
        }
    }

    const fn action_type(&self) -> &'static str {
        match self {
            Self::CreateTask { .. } => "create_task",
            Self::UpdateTask { .. }
            | Self::SetTaskStatus {
                status: TaskStatus::Open,
                ..
            } => "update_task",
            Self::SetTaskStatus {
                status: TaskStatus::Completed,
                ..
            } => "complete_task",
            Self::SetTaskStatus {
                status: TaskStatus::Cancelled,
                ..
            } => "cancel_task",
            Self::CreateSchedule { .. } => "create_schedule",
            Self::UpdateSchedule { .. } => "update_schedule",
            Self::CancelSchedule { .. } => "cancel_schedule",
            Self::CreateProject { .. } => "create_project",
            Self::UpdateProject { .. } => "update_project",
            Self::DeleteProject { .. } => "delete_project",
            Self::SendWebhookMessage { .. } => "send_webhook_message",
        }
    }

    const fn entity_id(&self) -> Uuid {
        match self {
            Self::CreateTask { id, .. }
            | Self::UpdateTask { id, .. }
            | Self::SetTaskStatus { id, .. }
            | Self::CreateSchedule { id, .. }
            | Self::UpdateSchedule { id, .. }
            | Self::CancelSchedule { id, .. }
            | Self::CreateProject { id, .. }
            | Self::UpdateProject { id, .. }
            | Self::DeleteProject { id, .. }
            | Self::SendWebhookMessage { id, .. } => *id,
        }
    }
}

impl PendingAgentAction {
    fn validate(&self) -> Result<(), StorageError> {
        match self {
            Self::CreateTask { title, .. } => {
                if valid_text(title, MAX_TITLE_CHARS, false) {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::CreateSchedule {
                title,
                starts_at,
                ends_at,
                time_zone,
            } => {
                if valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_time_zone(time_zone)
                    && ends_at > starts_at
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
        }
    }

    fn action_type(&self) -> &'static str {
        match self {
            Self::CreateTask { .. } => "create_task",
            Self::CreateSchedule { .. } => "create_schedule",
        }
    }

    fn title(&self) -> &str {
        match self {
            Self::CreateTask { title, .. } | Self::CreateSchedule { title, .. } => title,
        }
    }

    fn persistence_values(
        &self,
    ) -> (
        Option<OffsetDateTime>,
        Option<OffsetDateTime>,
        Option<OffsetDateTime>,
        Option<&str>,
    ) {
        match self {
            Self::CreateTask { due_at, .. } => (*due_at, None, None, None),
            Self::CreateSchedule {
                starts_at,
                ends_at,
                time_zone,
                ..
            } => (None, Some(*starts_at), Some(*ends_at), Some(time_zone)),
        }
    }

    fn completion_message(&self) -> String {
        match self {
            Self::CreateTask { title, .. } => format!("{title} 할 일을 추가했어요."),
            Self::CreateSchedule {
                title, starts_at, ..
            } => format!(
                "{:02}:{:02}에 {title} 일정을 등록했어요.",
                starts_at.hour(),
                starts_at.minute()
            ),
        }
    }

    fn decline_message(&self) -> String {
        match self {
            Self::CreateTask { title, .. } => format!("{title} 할 일 추가를 취소했어요."),
            Self::CreateSchedule { title, .. } => format!("{title} 일정 등록을 취소했어요."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAgentActionDecision {
    Approve,
    Decline,
}

impl NewAgentTurn {
    /// Validates a single bounded user turn before it is atomically queued.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs or text.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.job_id)
            || !is_v7(self.message_id)
            || !is_v7(self.client_message_id)
            || !is_v7(self.user_id)
            || !is_v7(self.conversation_id)
            || !valid_text(&self.content, MAX_CONTENT_CHARS, false)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub id: Uuid,
    pub title: Option<String>,
    pub status: ConversationStatus,
    pub last_message_at: Option<OffsetDateTime>,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedAgentTurn {
    pub job_id: Uuid,
    pub message_id: Uuid,
    pub conversation_id: Uuid,
    pub state: AgentJobState,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedAgentJob {
    pub id: Uuid,
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub input_message_id: Uuid,
    pub input_content: String,
    pub codex_thread_id: Option<String>,
    pub processing_model_id: Option<String>,
    pub processing_reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentReasoningEffort {
    pub id: String,
    pub description: String,
}

/// A validated model picker entry reported by the managed Codex runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentModelCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<AgentReasoningEffort>,
}

/// The available model choices and the user's optional pinned selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentModelSettings {
    pub models: Vec<AgentModelCatalogEntry>,
    pub selected_model_id: Option<String>,
    pub selected_reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentJobState {
    Queued,
    Claimed,
    Running,
    WaitingApproval,
    RetryWait,
    Completed,
    Failed,
    Cancelled,
    Declined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub role: ConversationMessageRole,
    pub content: String,
    pub presentation: Option<AssistantPresentation>,
    pub status: ConversationMessageStatus,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub version: i64,
}

/// A server-validated result surface selected by the assistant from entities
/// that were present in the authenticated turn context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantPresentation {
    pub kind: AssistantPresentationKind,
    pub title: String,
    pub items: Vec<AssistantPresentationItem>,
    #[serde(default)]
    pub layout: AssistantPresentationLayout,
    #[serde(default)]
    pub sections: Vec<AssistantPresentationSection>,
    #[serde(default)]
    pub focus_item_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantPresentationKind {
    Summary,
    Tasks,
    Schedule,
    Projects,
    Composite,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantPresentationLayout {
    #[default]
    Stack,
    Split,
    Focus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantPresentationSection {
    pub kind: AssistantPresentationSectionKind,
    pub title: String,
    pub view: AssistantPresentationView,
    pub item_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantPresentationSectionKind {
    Tasks,
    Schedule,
    Projects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantPresentationView {
    List,
    Checklist,
    Timeline,
    Cards,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantPresentationItem {
    Task {
        id: Uuid,
        #[serde(rename = "projectId")]
        project_id: Option<Uuid>,
        #[serde(rename = "projectTitle")]
        project_title: Option<String>,
        title: String,
        #[serde(default = "default_task_presentation_status")]
        status: String,
        priority: i16,
        #[serde(rename = "dueAt")]
        due_at: Option<String>,
    },
    Schedule {
        id: Uuid,
        title: String,
        #[serde(default = "default_schedule_presentation_status")]
        status: String,
        #[serde(rename = "startsAt")]
        starts_at: String,
        #[serde(rename = "endsAt")]
        ends_at: String,
        #[serde(rename = "timeZone")]
        time_zone: String,
    },
    Project {
        id: Uuid,
        #[serde(rename = "workspaceId")]
        workspace_id: Uuid,
        title: String,
        #[serde(default = "default_project_presentation_status")]
        status: String,
        objective: Option<String>,
        #[serde(rename = "nextAction")]
        next_action: Option<String>,
        #[serde(rename = "riskLevel")]
        risk_level: i16,
        #[serde(rename = "openTaskCount")]
        open_task_count: i64,
    },
}

fn default_task_presentation_status() -> String {
    "open".to_owned()
}

fn default_schedule_presentation_status() -> String {
    "confirmed".to_owned()
}

fn default_project_presentation_status() -> String {
    "active".to_owned()
}

impl AssistantPresentation {
    /// Rejects malformed or internally inconsistent presentation snapshots
    /// before they reach the durable message stream.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid data.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !valid_text(&self.title, MAX_TITLE_CHARS, false)
            || self.items.len() > MAX_PRESENTATION_ITEMS
            || self.sections.len() > MAX_PRESENTATION_SECTIONS
            || self.items.iter().any(|item| !valid_presentation_item(item))
        {
            return Err(StorageError::InvalidConfiguration);
        }

        let item_ids = self
            .items
            .iter()
            .map(presentation_item_id)
            .collect::<HashSet<_>>();
        if item_ids.len() != self.items.len() {
            return Err(StorageError::InvalidConfiguration);
        }

        if self.sections.is_empty() {
            let matches_kind = self
                .items
                .iter()
                .all(|item| presentation_item_matches_kind(item, self.kind));
            if (self.kind == AssistantPresentationKind::Summary && !self.items.is_empty())
                || matches!(self.kind, AssistantPresentationKind::Composite)
                || (self.kind != AssistantPresentationKind::Summary && !matches_kind)
                || self.focus_item_id.is_some_and(|id| !item_ids.contains(&id))
            {
                return Err(StorageError::InvalidConfiguration);
            }
            return Ok(());
        }

        let expected_kind = if self.sections.len() > 1 {
            AssistantPresentationKind::Composite
        } else {
            match self.sections[0].kind {
                AssistantPresentationSectionKind::Tasks => AssistantPresentationKind::Tasks,
                AssistantPresentationSectionKind::Schedule => AssistantPresentationKind::Schedule,
                AssistantPresentationSectionKind::Projects => AssistantPresentationKind::Projects,
            }
        };
        if self.kind != expected_kind {
            return Err(StorageError::InvalidConfiguration);
        }

        let mut section_kinds = HashSet::new();
        let mut referenced_ids = HashSet::new();
        for section in &self.sections {
            if !section_kinds.insert(section.kind)
                || !valid_text(&section.title, MAX_TITLE_CHARS, false)
                || section.item_ids.is_empty()
                || section.item_ids.len() > MAX_PRESENTATION_ITEMS
                || !presentation_view_matches_kind(section.view, section.kind)
            {
                return Err(StorageError::InvalidConfiguration);
            }
            for id in &section.item_ids {
                let Some(item) = self
                    .items
                    .iter()
                    .find(|item| presentation_item_id(item) == *id)
                else {
                    return Err(StorageError::InvalidConfiguration);
                };
                if !referenced_ids.insert(*id)
                    || !presentation_item_matches_section_kind(item, section.kind)
                {
                    return Err(StorageError::InvalidConfiguration);
                }
            }
        }
        if referenced_ids.len() != self.items.len()
            || self
                .focus_item_id
                .is_some_and(|id| !referenced_ids.contains(&id))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationMessageRole {
    User,
    Assistant,
    SystemEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationMessageStatus {
    Pending,
    Streaming,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentJob {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub state: AgentJobState,
    pub created_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub version: i64,
    pub pending_action: Option<PendingAgentAction>,
}

/// The safe, client-visible state of the managed `ChatGPT` sign-in ceremony.
/// OAuth access and refresh tokens remain in the Codex runtime and are never
/// represented in this persistence model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAuthentication {
    pub id: Uuid,
    pub state: AgentAuthenticationState,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentAuthenticationState {
    Requested,
    AwaitingAuthorization,
    Ready,
    Failed,
}

/// A request the agent runtime may turn into a Codex-managed device-code
/// login. It deliberately excludes the presentable code so the agent never
/// needs to log or cache it after persisting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedAgentAuthentication {
    pub id: Uuid,
    pub user_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct ConversationRow {
    id: Uuid,
    title: Option<String>,
    status: String,
    last_message_at: Option<OffsetDateTime>,
    version: i64,
}

impl TryFrom<ConversationRow> for Conversation {
    type Error = StorageError;

    fn try_from(row: ConversationRow) -> Result<Self, Self::Error> {
        let status = match row.status.as_str() {
            "active" => ConversationStatus::Active,
            "archived" => ConversationStatus::Archived,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            title: row.title,
            status,
            last_message_at: row.last_message_at,
            version: row.version,
        })
    }
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    input_message_id: Uuid,
    conversation_id: Uuid,
    state: String,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct ClaimedJobRow {
    id: Uuid,
    user_id: Uuid,
    conversation_id: Uuid,
    input_message_id: Uuid,
    input_content: String,
    codex_thread_id: Option<String>,
    processing_model_id: Option<String>,
    processing_reasoning_effort: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AgentModelRow {
    id: String,
    display_name: String,
    description: String,
    is_default: bool,
    default_reasoning_effort: String,
}

#[derive(sqlx::FromRow)]
struct AgentReasoningEffortRow {
    model_id: String,
    effort: String,
    description: String,
}

#[derive(sqlx::FromRow)]
struct AgentPreferenceRow {
    model_id: Option<String>,
    reasoning_effort: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ExistingAgentTurnRow {
    id: Uuid,
    input_message_id: Uuid,
    conversation_id: Uuid,
    state: String,
    version: i64,
    content: String,
}

#[derive(sqlx::FromRow)]
struct ConversationMessageRow {
    id: Uuid,
    role: String,
    content: String,
    presentation: Option<serde_json::Value>,
    status: String,
    created_at: OffsetDateTime,
    completed_at: Option<OffsetDateTime>,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct AgentJobReadRow {
    id: Uuid,
    conversation_id: Uuid,
    state: String,
    created_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
    version: i64,
    pending_action_type: Option<String>,
    pending_action_title: Option<String>,
    pending_action_due_at: Option<OffsetDateTime>,
    pending_action_starts_at: Option<OffsetDateTime>,
    pending_action_ends_at: Option<OffsetDateTime>,
    pending_action_time_zone: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PendingActionJobRow {
    conversation_id: Uuid,
    state: String,
    pending_action_type: Option<String>,
    pending_action_title: Option<String>,
    pending_action_due_at: Option<OffsetDateTime>,
    pending_action_starts_at: Option<OffsetDateTime>,
    pending_action_ends_at: Option<OffsetDateTime>,
    pending_action_time_zone: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AgentAuthenticationRow {
    id: Uuid,
    state: String,
    verification_url: Option<String>,
    user_code: Option<String>,
}

#[derive(sqlx::FromRow)]
struct RequestedAgentAuthenticationRow {
    id: Uuid,
    user_id: Uuid,
}

impl TryFrom<JobRow> for QueuedAgentTurn {
    type Error = StorageError;

    fn try_from(row: JobRow) -> Result<Self, Self::Error> {
        let state = parse_job_state(&row.state)?;
        Ok(Self {
            job_id: row.id,
            message_id: row.input_message_id,
            conversation_id: row.conversation_id,
            state,
            version: row.version,
        })
    }
}

impl From<ClaimedJobRow> for ClaimedAgentJob {
    fn from(row: ClaimedJobRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            conversation_id: row.conversation_id,
            input_message_id: row.input_message_id,
            input_content: row.input_content,
            codex_thread_id: row.codex_thread_id,
            processing_model_id: row.processing_model_id,
            processing_reasoning_effort: row.processing_reasoning_effort,
        }
    }
}

impl From<AgentModelRow> for AgentModelCatalogEntry {
    fn from(row: AgentModelRow) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name,
            description: row.description,
            is_default: row.is_default,
            default_reasoning_effort: row.default_reasoning_effort,
            supported_reasoning_efforts: Vec::new(),
        }
    }
}

impl TryFrom<ExistingAgentTurnRow> for QueuedAgentTurn {
    type Error = StorageError;

    fn try_from(row: ExistingAgentTurnRow) -> Result<Self, Self::Error> {
        Ok(Self {
            job_id: row.id,
            message_id: row.input_message_id,
            conversation_id: row.conversation_id,
            state: parse_job_state(&row.state)?,
            version: row.version,
        })
    }
}

impl TryFrom<ConversationMessageRow> for ConversationMessage {
    type Error = StorageError;

    fn try_from(row: ConversationMessageRow) -> Result<Self, Self::Error> {
        let role = match row.role.as_str() {
            "user" => ConversationMessageRole::User,
            "assistant" => ConversationMessageRole::Assistant,
            "system_event" => ConversationMessageRole::SystemEvent,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        let status = match row.status.as_str() {
            "pending" => ConversationMessageStatus::Pending,
            "streaming" => ConversationMessageStatus::Streaming,
            "completed" => ConversationMessageStatus::Completed,
            "failed" => ConversationMessageStatus::Failed,
            "cancelled" => ConversationMessageStatus::Cancelled,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            role,
            content: row.content,
            presentation: row
                .presentation
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| StorageError::PersistenceUnavailable)?,
            status,
            created_at: row.created_at,
            completed_at: row.completed_at,
            version: row.version,
        })
    }
}

impl TryFrom<AgentJobReadRow> for AgentJob {
    type Error = StorageError;

    fn try_from(row: AgentJobReadRow) -> Result<Self, Self::Error> {
        let pending_action = pending_action_from_fields(
            row.pending_action_type.as_deref(),
            row.pending_action_title,
            row.pending_action_due_at,
            row.pending_action_starts_at,
            row.pending_action_ends_at,
            row.pending_action_time_zone,
        )?;
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            state: parse_job_state(&row.state)?,
            created_at: row.created_at,
            finished_at: row.finished_at,
            version: row.version,
            pending_action,
        })
    }
}

impl TryFrom<AgentAuthenticationRow> for AgentAuthentication {
    type Error = StorageError;

    fn try_from(row: AgentAuthenticationRow) -> Result<Self, Self::Error> {
        let state = parse_agent_authentication_state(&row.state)?;
        let has_device_code = row.verification_url.is_some() && row.user_code.is_some();
        if matches!(state, AgentAuthenticationState::AwaitingAuthorization) != has_device_code {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(Self {
            id: row.id,
            state,
            verification_url: row.verification_url,
            user_code: row.user_code,
        })
    }
}

impl From<RequestedAgentAuthenticationRow> for RequestedAgentAuthentication {
    fn from(row: RequestedAgentAuthenticationRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
        }
    }
}

impl Database {
    /// Replaces the available portion of the runtime model catalog without
    /// deleting historical IDs that may still be referenced by preferences.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] when the runtime returns
    /// malformed or duplicate entries, and a classified persistence error for
    /// database failures.
    pub async fn replace_agent_model_catalog(
        &self,
        models: &[AgentModelCatalogEntry],
    ) -> Result<(), StorageError> {
        validate_agent_model_catalog(models)?;

        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        sqlx::query("UPDATE agent_models SET available = FALSE, is_default = FALSE")
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
        sqlx::query("DELETE FROM agent_model_reasoning_efforts")
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
        for model in models {
            sqlx::query(
                "\
                INSERT INTO agent_models (
                    id, display_name, description, is_default, available, synced_at,
                    default_reasoning_effort
                )
                VALUES ($1, $2, $3, $4, TRUE, NOW(), $5)
                ON CONFLICT (id) DO UPDATE
                SET display_name = EXCLUDED.display_name,
                    description = EXCLUDED.description,
                    is_default = EXCLUDED.is_default,
                    available = TRUE,
                    synced_at = NOW(),
                    default_reasoning_effort = EXCLUDED.default_reasoning_effort",
            )
            .bind(&model.id)
            .bind(&model.display_name)
            .bind(&model.description)
            .bind(model.is_default)
            .bind(&model.default_reasoning_effort)
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
            for (position, effort) in model.supported_reasoning_efforts.iter().enumerate() {
                sqlx::query(
                    "\
                    INSERT INTO agent_model_reasoning_efforts (
                        model_id, effort, description, position
                    ) VALUES ($1, $2, $3, $4)",
                )
                .bind(&model.id)
                .bind(&effort.id)
                .bind(&effort.description)
                .bind(i16::try_from(position).map_err(|_| StorageError::InvalidConfiguration)?)
                .execute(&mut *transaction)
                .await
                .map_err(|error| classify(&error))?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(())
    }

    /// Returns visible model choices and the user's pinned model, if one is
    /// still available. A missing selection means the Codex runtime default.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error for invalid identifiers or query
    /// failures.
    pub async fn agent_model_settings_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<AgentModelSettings, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut models = sqlx::query_as::<_, AgentModelRow>(
            "\
            SELECT id, display_name, description, is_default, default_reasoning_effort
            FROM agent_models
            WHERE available = TRUE AND default_reasoning_effort IS NOT NULL
            ORDER BY is_default DESC, display_name ASC, id ASC",
        )
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify(&error))?
        .into_iter()
        .map(AgentModelCatalogEntry::from)
        .collect::<Vec<_>>();
        let effort_rows = sqlx::query_as::<_, AgentReasoningEffortRow>(
            "\
            SELECT effort.model_id, effort.effort, effort.description
            FROM agent_model_reasoning_efforts AS effort
            INNER JOIN agent_models AS model ON model.id = effort.model_id
            WHERE model.available = TRUE
            ORDER BY effort.model_id, effort.position",
        )
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        let mut efforts_by_model = HashMap::<String, Vec<AgentReasoningEffort>>::new();
        for row in effort_rows {
            efforts_by_model
                .entry(row.model_id)
                .or_default()
                .push(AgentReasoningEffort {
                    id: row.effort,
                    description: row.description,
                });
        }
        for model in &mut models {
            model.supported_reasoning_efforts =
                efforts_by_model.remove(&model.id).unwrap_or_default();
        }
        let preference = sqlx::query_as::<_, AgentPreferenceRow>(
            "SELECT model_id, reasoning_effort FROM agent_preferences WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        let selected_model_id = preference
            .as_ref()
            .and_then(|preference| preference.model_id.as_ref())
            .filter(|model_id| models.iter().any(|model| &model.id == *model_id))
            .cloned();
        let effective_model = selected_model_id
            .as_ref()
            .and_then(|model_id| models.iter().find(|model| &model.id == model_id))
            .or_else(|| models.iter().find(|model| model.is_default));
        let selected_reasoning_effort = preference
            .and_then(|preference| preference.reasoning_effort)
            .filter(|selected| {
                effective_model.is_some_and(|model| {
                    model
                        .supported_reasoning_efforts
                        .iter()
                        .any(|effort| effort.id == *selected)
                })
            });
        Ok(AgentModelSettings {
            models,
            selected_model_id,
            selected_reasoning_effort,
        })
    }

    /// Pins one available model for future user turns, or clears the pin to
    /// follow the runtime default.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for an unknown model or
    /// malformed user identifier, and a classified persistence error otherwise.
    pub async fn set_agent_model_for_user(
        &self,
        user_id: Uuid,
        model_id: Option<&str>,
        reasoning_effort: Option<&str>,
    ) -> Result<AgentModelSettings, StorageError> {
        if !is_v7(user_id)
            || model_id.is_some_and(|id| !valid_text(id, MAX_MODEL_ID_CHARS, false))
            || reasoning_effort
                .is_some_and(|effort| !valid_text(effort, MAX_REASONING_EFFORT_ID_CHARS, false))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let effective_model_id = sqlx::query_scalar::<_, String>(
            "\
            SELECT id
            FROM agent_models
            WHERE available = TRUE
              AND (($1::TEXT IS NOT NULL AND id = $1) OR ($1::TEXT IS NULL AND is_default = TRUE))
            ORDER BY is_default DESC
            LIMIT 1",
        )
        .bind(model_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(effective_model_id) = effective_model_id else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Err(StorageError::InvalidConfiguration);
        };
        if let Some(reasoning_effort) = reasoning_effort {
            let supported = sqlx::query_scalar::<_, bool>(
                "\
                SELECT EXISTS(
                    SELECT 1 FROM agent_model_reasoning_efforts
                    WHERE model_id = $1 AND effort = $2
                )",
            )
            .bind(&effective_model_id)
            .bind(reasoning_effort)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
            if !supported {
                transaction
                    .rollback()
                    .await
                    .map_err(|error| classify(&error))?;
                return Err(StorageError::InvalidConfiguration);
            }
        }
        if model_id.is_some() || reasoning_effort.is_some() {
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO agent_preferences (user_id, model_id, reasoning_effort)
                VALUES ($1, $2, $3)
                ON CONFLICT (user_id) DO UPDATE
                SET model_id = EXCLUDED.model_id,
                    reasoning_effort = EXCLUDED.reasoning_effort,
                    version = agent_preferences.version + 1
                RETURNING version",
            )
            .bind(user_id)
            .bind(model_id)
            .bind(reasoning_effort)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
            append_change(
                &mut transaction,
                user_id,
                "agent_preference",
                user_id,
                version,
            )
            .await?;
        } else {
            sqlx::query("DELETE FROM agent_preferences WHERE user_id = $1")
                .bind(user_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| classify(&error))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        self.agent_model_settings_for_user(user_id).await
    }

    /// Returns the newest authentication state owned by this personal user.
    /// The row carries only presentation fields for the official device-code
    /// flow; Codex owns the actual OAuth credentials.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for a non-version-seven
    /// user identifier and a classified storage error when persistence fails.
    pub async fn agent_authentication_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AgentAuthentication>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, AgentAuthenticationRow>(
            "\
            SELECT id, state, verification_url, user_code
            FROM agent_auth_attempts
            WHERE user_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        row.map(AgentAuthentication::try_from).transpose()
    }

    /// Starts a personal sign-in request. A successful `ready` row is stable
    /// across agent restarts. An explicit new request supersedes an unfinished
    /// ceremony so a rejected or expired device code never traps the client.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed identifiers
    /// and a classified storage error when persistence fails.
    pub async fn request_agent_authentication(
        &self,
        user_id: Uuid,
        attempt_id: Uuid,
    ) -> Result<AgentAuthentication, StorageError> {
        if !is_v7(user_id) || !is_v7(attempt_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let ready = sqlx::query_as::<_, AgentAuthenticationRow>(
            "\
            SELECT id, state, verification_url, user_code
            FROM agent_auth_attempts
            WHERE user_id = $1
              AND state = 'ready'
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        if let Some(ready) = ready {
            let authentication = AgentAuthentication::try_from(ready)?;
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(authentication);
        }

        sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'failed',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL,
                error_code = 'agent_authentication_restarted',
                completed_at = NOW()
            WHERE user_id = $1
              AND state IN ('requested', 'awaiting_authorization')",
        )
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;

        let row = sqlx::query_as::<_, AgentAuthenticationRow>(
            "\
            INSERT INTO agent_auth_attempts (id, user_id, state)
            VALUES ($1, $2, 'requested')
            RETURNING id, state, verification_url, user_code",
        )
        .bind(attempt_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        AgentAuthentication::try_from(row)
    }

    /// Reports whether one runtime-owned ceremony is still the active request.
    /// The agent uses this to cancel an obsolete App Server login before
    /// issuing the replacement code requested by the user.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for a malformed attempt
    /// identifier and a classified storage error when persistence fails.
    pub async fn agent_authentication_is_awaiting(
        &self,
        attempt_id: Uuid,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM agent_auth_attempts
                WHERE id = $1 AND state = 'awaiting_authorization'
            )",
        )
        .bind(attempt_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| classify(&error))
    }

    /// Finds one requested ceremony for the single trusted agent process.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence fails.
    pub async fn next_requested_agent_authentication(
        &self,
    ) -> Result<Option<RequestedAgentAuthentication>, StorageError> {
        let row = sqlx::query_as::<_, RequestedAgentAuthenticationRow>(
            "\
            SELECT id, user_id
            FROM agent_auth_attempts
            WHERE state = 'requested'
            ORDER BY created_at ASC, id ASC
            LIMIT 1",
        )
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(row.map(RequestedAgentAuthentication::from))
    }

    /// A device-code ceremony is bound to one App Server process. If that
    /// process restarted before completion, discard the stale presentation
    /// code and let the agent issue a fresh official code.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence fails.
    pub async fn restart_pending_agent_authentication(&self) -> Result<(), StorageError> {
        sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'requested',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL
            WHERE state = 'awaiting_authorization'",
        )
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(())
    }

    /// A persisted ready marker is only valid while the managed Codex process
    /// can still read its `ChatGPT` account. When startup reports no account,
    /// clear that stale marker so the paired user can start a fresh ceremony.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence fails.
    pub async fn invalidate_ready_agent_authentication(&self) -> Result<(), StorageError> {
        sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'failed',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL,
                error_code = 'agent_authentication_required',
                completed_at = NOW()
            WHERE state = 'ready'",
        )
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(())
    }

    /// Stores only the URL and one-time user code returned by the managed
    /// runtime. Both values are intentionally bounded and never emitted by
    /// server logs.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed values and
    /// a classified storage error when persistence fails.
    pub async fn begin_agent_authentication(
        &self,
        attempt_id: Uuid,
        login_id: &str,
        verification_url: &str,
        user_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id)
            || !valid_external_id(login_id)
            || !valid_text(verification_url, MAX_AUTH_URL_CHARS, false)
            || !valid_text(user_code, MAX_AUTH_USER_CODE_CHARS, false)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'awaiting_authorization',
                login_id = $2,
                verification_url = $3,
                user_code = $4
            WHERE id = $1 AND state = 'requested'",
        )
        .bind(attempt_id)
        .bind(login_id)
        .bind(verification_url)
        .bind(user_code)
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(result.rows_affected() == 1)
    }

    /// Marks the specific owned ceremony complete after Codex reports a
    /// managed `ChatGPT` account. The device code is cleared immediately.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for a malformed attempt
    /// identifier and a classified storage error when persistence fails.
    pub async fn complete_agent_authentication(
        &self,
        attempt_id: Uuid,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'ready',
                verification_url = NULL,
                user_code = NULL,
                completed_at = NOW()
            WHERE id = $1 AND state = 'awaiting_authorization'",
        )
        .bind(attempt_id)
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(result.rows_affected() == 1)
    }

    /// Retains a short safe reason when the runtime cannot start the official
    /// sign-in ceremony. The device code is never retained on a failure path.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed values and
    /// a classified storage error when persistence fails.
    pub async fn fail_agent_authentication(
        &self,
        attempt_id: Uuid,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'failed',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL,
                error_code = $2,
                completed_at = NOW()
            WHERE id = $1 AND state IN ('requested', 'awaiting_authorization')",
        )
        .bind(attempt_id)
        .bind(error_code)
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(result.rows_affected() == 1)
    }

    /// Creates an active conversation and emits one sync upsert transactionally.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing conversation text.
    pub async fn create_conversation(
        &self,
        conversation: &NewConversation,
    ) -> Result<Conversation, StorageError> {
        conversation.validate()?;
        let user_id = conversation.user_id;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let normalized_title = conversation
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let inserted = sqlx::query_as::<_, ConversationRow>(
            "\
            INSERT INTO conversations (id, user_id, title)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO NOTHING
            RETURNING id, title, status, last_message_at, version",
        )
        .bind(conversation.id)
        .bind(conversation.user_id)
        .bind(normalized_title)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(row) = inserted else {
            let existing = sqlx::query_as::<_, ConversationRow>(
                "\
                SELECT id, title, status, last_message_at, version
                FROM conversations
                WHERE id = $1 AND user_id = $2",
            )
            .bind(conversation.id)
            .bind(conversation.user_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
            let Some(existing) = existing else {
                return Err(StorageError::IdentityConflict);
            };
            if existing.title.as_deref() != normalized_title {
                return Err(StorageError::IdentityConflict);
            }
            let conversation = Conversation::try_from(existing)?;
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(conversation);
        };
        let conversation = Conversation::try_from(row)?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            conversation.id,
            conversation.version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(conversation)
    }

    /// Lists the owning user's active conversations without their message text.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence is unavailable.
    pub async fn active_conversations_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Conversation>, StorageError> {
        let rows = sqlx::query_as::<_, ConversationRow>(
            "\
            SELECT id, title, status, last_message_at, version
            FROM conversations
            WHERE user_id = $1 AND status = 'active'
            ORDER BY last_message_at DESC NULLS LAST, created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        rows.into_iter().map(Conversation::try_from).collect()
    }

    /// Returns the ordered message history only when the conversation belongs
    /// to the supplied user. An inaccessible conversation is indistinguishable
    /// from a missing one.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing message text in logs.
    pub async fn conversation_messages_for_user(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<Vec<ConversationMessage>>, StorageError> {
        let owns_conversation = sqlx::query_scalar::<_, bool>(
            "\
            SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = $1 AND user_id = $2
            )",
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        if !owns_conversation {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, ConversationMessageRow>(
            "\
            SELECT id, role, content, presentation, status, created_at, completed_at, version
            FROM messages
            WHERE conversation_id = $1
            ORDER BY created_at ASC, id ASC",
        )
        .bind(conversation_id)
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        rows.into_iter()
            .map(ConversationMessage::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    /// Returns one agent job only when it belongs to the supplied user.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing job metadata for
    /// inaccessible conversations.
    pub async fn agent_job_for_user(
        &self,
        user_id: Uuid,
        job_id: Uuid,
    ) -> Result<Option<AgentJob>, StorageError> {
        let row = sqlx::query_as::<_, AgentJobReadRow>(
            "\
            SELECT id, conversation_id, state, created_at, finished_at, version,
                   pending_action_type, pending_action_title,
                   pending_action_due_at,
                   pending_action_starts_at, pending_action_ends_at,
                   pending_action_time_zone
            FROM agent_jobs
            WHERE id = $1 AND user_id = $2",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        row.map(AgentJob::try_from).transpose()
    }

    /// Returns the newest job in an owned conversation so a client that was
    /// restarted or opened on another device can restore its request state.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing another user's job.
    pub async fn latest_agent_job_for_conversation_for_user(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<AgentJob>, StorageError> {
        let row = sqlx::query_as::<_, AgentJobReadRow>(
            "\
            SELECT id, conversation_id, state, created_at, finished_at, version,
                   pending_action_type, pending_action_title,
                   pending_action_due_at,
                   pending_action_starts_at, pending_action_ends_at,
                   pending_action_time_zone
            FROM agent_jobs
            WHERE user_id = $1 AND conversation_id = $2
            ORDER BY created_at DESC, id DESC
            LIMIT 1",
        )
        .bind(user_id)
        .bind(conversation_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        row.map(AgentJob::try_from).transpose()
    }

    /// Atomically records the user message and a queued job for one owned,
    /// active conversation. The unique active-job index turns concurrent turns
    /// into an ordinary conflict rather than a competing provider call.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::IdentityConflict`] for an unknown/foreign or
    /// active-busy conversation and a classified error for storage failures.
    pub async fn enqueue_agent_turn(
        &self,
        turn: &NewAgentTurn,
    ) -> Result<QueuedAgentTurn, StorageError> {
        self.enqueue_agent_turn_inner(turn, None).await
    }

    /// Records a planning action as a conversation turn without executing it.
    /// The caller must later resolve the same job through the explicit
    /// approval operation below.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed action
    /// details and a classified storage error when persistence is unavailable.
    pub async fn enqueue_agent_action_turn(
        &self,
        turn: &NewAgentTurn,
        action: PendingAgentAction,
    ) -> Result<QueuedAgentTurn, StorageError> {
        action.validate()?;
        self.enqueue_agent_turn_inner(turn, Some(&action)).await
    }

    #[allow(clippy::too_many_lines)] // One transaction intentionally owns turn, job, and sync writes.
    async fn enqueue_agent_turn_inner(
        &self,
        turn: &NewAgentTurn,
        pending_action: Option<&PendingAgentAction>,
    ) -> Result<QueuedAgentTurn, StorageError> {
        turn.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        if !owns_active_conversation(&mut transaction, turn.user_id, turn.conversation_id).await? {
            return Err(StorageError::IdentityConflict);
        }

        let existing = existing_agent_turn(
            &mut transaction,
            turn.conversation_id,
            turn.client_message_id,
        )
        .await?;
        if let Some(existing) = existing {
            if existing.content != turn.content.trim() {
                return Err(StorageError::IdentityConflict);
            }
            let queued = QueuedAgentTurn::try_from(existing)?;
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(queued);
        }

        let inserted = sqlx::query(
            "\
            INSERT INTO messages (
                id, conversation_id, role, content, status, client_message_id
            ) VALUES ($1, $2, 'user', $3, 'completed', $4)",
        )
        .bind(turn.message_id)
        .bind(turn.conversation_id)
        .bind(turn.content.trim())
        .bind(turn.client_message_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        if inserted.rows_affected() != 1 {
            return Err(StorageError::PersistenceUnavailable);
        }

        let (
            action_type,
            action_title,
            action_due_at,
            action_starts_at,
            action_ends_at,
            action_time_zone,
        ) = pending_action.map_or((None, None, None, None, None, None), |action| {
            let (due_at, starts_at, ends_at, time_zone) = action.persistence_values();
            (
                Some(action.action_type()),
                Some(action.title()),
                due_at,
                starts_at,
                ends_at,
                time_zone,
            )
        });
        let row = sqlx::query_as::<_, JobRow>(
            "\
            INSERT INTO agent_jobs (
                id, user_id, conversation_id, input_message_id, state,
                pending_action_type, pending_action_title,
                pending_action_due_at,
                pending_action_starts_at, pending_action_ends_at,
                pending_action_time_zone
            ) VALUES (
                $1, $2, $3, $4,
                CASE WHEN $5::text IS NULL THEN 'queued' ELSE 'waiting_approval' END,
                $5, $6, $7, $8, $9, $10
            )
            RETURNING id, input_message_id, conversation_id, state, version",
        )
        .bind(turn.job_id)
        .bind(turn.user_id)
        .bind(turn.conversation_id)
        .bind(turn.message_id)
        .bind(action_type)
        .bind(action_title)
        .bind(action_due_at)
        .bind(action_starts_at)
        .bind(action_ends_at)
        .bind(action_time_zone)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET last_message_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(turn.conversation_id)
        .bind(turn.user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let queued = QueuedAgentTurn::try_from(row)?;
        append_change(
            &mut transaction,
            turn.user_id,
            "agent_job",
            queued.job_id,
            queued.version,
        )
        .await?;
        append_change(
            &mut transaction,
            turn.user_id,
            "conversation",
            turn.conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(queued)
    }

    /// Resolves one owner-visible planning proposal exactly once. Approval
    /// creates the local planning record and finalizes the conversation in the
    /// same transaction; decline records a clear conversation outcome without
    /// changing the personal plan.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs and a
    /// classified storage error when the action or its audit changes cannot be
    /// persisted.
    #[allow(clippy::too_many_lines)] // Approval must atomically cover plan, job, message, and sync writes.
    pub async fn resolve_agent_action(
        &self,
        user_id: Uuid,
        job_id: Uuid,
        decision: PendingAgentActionDecision,
    ) -> Result<bool, StorageError> {
        if !is_v7(user_id) || !is_v7(job_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, PendingActionJobRow>(
            "\
            SELECT conversation_id, state,
                   pending_action_type, pending_action_title,
                   pending_action_due_at,
                   pending_action_starts_at, pending_action_ends_at,
                   pending_action_time_zone
            FROM agent_jobs
            WHERE id = $1 AND user_id = $2
            FOR UPDATE",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(row) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        if parse_job_state(&row.state)? != AgentJobState::WaitingApproval {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        }
        let action = pending_action_from_fields(
            row.pending_action_type.as_deref(),
            row.pending_action_title,
            row.pending_action_due_at,
            row.pending_action_starts_at,
            row.pending_action_ends_at,
            row.pending_action_time_zone,
        )?
        .ok_or(StorageError::PersistenceUnavailable)?;

        let outcome = match decision {
            PendingAgentActionDecision::Approve => {
                persist_approved_agent_action(&mut transaction, user_id, &action).await?;
                action.completion_message()
            }
            PendingAgentActionDecision::Decline => action.decline_message(),
        };
        let state = match decision {
            PendingAgentActionDecision::Approve => "completed",
            PendingAgentActionDecision::Decline => "declined",
        };
        let job_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE agent_jobs
            SET state = $3,
                phase = NULL,
                pending_action_type = NULL,
                pending_action_title = NULL,
                pending_action_due_at = NULL,
                pending_action_starts_at = NULL,
                pending_action_ends_at = NULL,
                pending_action_time_zone = NULL,
                finished_at = NOW()
            WHERE id = $1 AND user_id = $2 AND state = 'waiting_approval'
            RETURNING version",
        )
        .bind(job_id)
        .bind(user_id)
        .bind(state)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(job_version) = job_version else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let assistant_message_id = Uuid::now_v7();
        let message_version = sqlx::query_scalar::<_, i64>(
            "\
            INSERT INTO messages (
                id, conversation_id, agent_job_id, role, content, status, completed_at
            ) VALUES ($1, $2, $3, 'assistant', $4, 'completed', NOW())
            RETURNING version",
        )
        .bind(assistant_message_id)
        .bind(row.conversation_id)
        .bind(job_id)
        .bind(outcome)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET last_message_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(row.conversation_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "message",
            assistant_message_id,
            message_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            row.conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Claims one queued turn for a named runner with a bounded lease.
    ///
    /// An expired `claimed` job is safe to recover because the runner has not
    /// persisted the transition that permits a provider turn yet. `running`
    /// jobs are deliberately not reclaimed automatically: their provider side
    /// effect needs an explicit recovery path.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing message content.
    pub async fn claim_next_agent_job(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedAgentJob>, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let row = sqlx::query_as::<_, ClaimedJobRow>(
            "\
            WITH recovered AS (
                UPDATE agent_jobs
                SET state = 'queued', phase = NULL, claim_owner = NULL, claim_expires_at = NULL
                WHERE state = 'claimed' AND claim_expires_at < NOW()
            ), candidate AS (
                SELECT id
                FROM agent_jobs
                WHERE state IN ('queued', 'retry_wait')
                ORDER BY created_at ASC, id ASC
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            ), claimed AS (
                UPDATE agent_jobs AS job
                SET state = 'claimed',
                    phase = 'preparing',
                    claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE job.id = candidate.id
                RETURNING job.id, job.user_id, job.conversation_id, job.input_message_id
            )
            SELECT job.id,
                   job.user_id,
                   job.conversation_id,
                   job.input_message_id,
                   input.content AS input_content,
                   conversation.codex_thread_id,
                   selected_model.id AS processing_model_id,
                   selected_effort.effort AS processing_reasoning_effort
            FROM claimed AS job
            INNER JOIN messages AS input ON input.id = job.input_message_id
            INNER JOIN conversations AS conversation ON conversation.id = job.conversation_id
            LEFT JOIN agent_preferences AS preference ON preference.user_id = job.user_id
            LEFT JOIN agent_models AS selected_model
                ON selected_model.id = preference.model_id AND selected_model.available = TRUE
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
        .map_err(|error| classify(&error))?;
        Ok(row.map(ClaimedAgentJob::from))
    }

    /// Safely finalizes expired work that was interrupted after the provider
    /// turn boundary. The turn is never requeued because the provider may have
    /// received it before the worker stopped. The deployed topology has one
    /// durable personal worker, so an expired lease proves no healthy worker
    /// remains responsible for this turn.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed error
    /// metadata, and a classified storage error when persistence fails.
    pub async fn fail_expired_running_agent_jobs(
        &self,
        error_code: &str,
    ) -> Result<usize, StorageError> {
        if !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let rows = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'failed',
                phase = NULL,
                claim_owner = NULL,
                claim_expires_at = NULL,
                error_code = $1,
                finished_at = NOW()
            WHERE state = 'running'
              AND claim_expires_at < NOW()
            RETURNING id, user_id, version",
        )
        .bind(error_code)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        for (job_id, user_id, version) in &rows {
            append_change(&mut transaction, *user_id, "agent_job", *job_id, *version).await?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(rows.len())
    }

    /// Marks a lease-owned job as running after its Codex thread is available,
    /// but before `turn/start` is sent.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing provider IDs in logs.
    pub async fn start_agent_job(
        &self,
        job_id: Uuid,
        runner_id: &str,
        codex_thread_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id) || !valid_runner_id(runner_id) || !valid_external_id(codex_thread_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'running',
                phase = 'starting_turn',
                codex_thread_id = $3,
                claim_expires_at = NOW() + ($4 * INTERVAL '1 millisecond'),
                started_at = COALESCE(started_at, NOW())
            WHERE id = $1 AND claim_owner = $2 AND state = 'claimed'
            RETURNING user_id, conversation_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(codex_thread_id)
        .bind(lease_millis)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, conversation_id, job_version)) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET codex_thread_id = COALESCE(codex_thread_id, $3)
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(conversation_id)
        .bind(user_id)
        .bind(codex_thread_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Persists the authoritative final assistant message and makes a lease-owned
    /// running job terminal in one transaction.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing assistant content.
    pub async fn complete_agent_job(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        content: &str,
        presentation: Option<&AssistantPresentation>,
    ) -> Result<bool, StorageError> {
        self.complete_agent_job_inner(
            job_id,
            runner_id,
            assistant_message_id,
            content,
            presentation,
            &[],
        )
        .await
    }

    /// Atomically commits one validated planning mutation with the final
    /// assistant response. A stale entity version or foreign entity rolls the
    /// whole transaction back, so the client never sees a success message for
    /// a change that did not happen.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::IdentityConflict`] for a stale or inaccessible
    /// target and classified persistence errors for unavailable storage.
    pub async fn complete_agent_job_with_action(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        content: &str,
        presentation: Option<&AssistantPresentation>,
        action: &AgentActionCommand,
    ) -> Result<bool, StorageError> {
        self.complete_agent_job_with_actions(
            job_id,
            runner_id,
            assistant_message_id,
            content,
            presentation,
            std::slice::from_ref(action),
        )
        .await
    }

    /// Atomically commits several validated planning mutations with one final
    /// assistant response. Every mutation succeeds or the complete turn rolls
    /// back, including its success message and audit rows.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for an empty, oversized,
    /// malformed, or duplicate-target batch. Returns
    /// [`StorageError::IdentityConflict`] when any target is stale or foreign.
    pub async fn complete_agent_job_with_actions(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        content: &str,
        presentation: Option<&AssistantPresentation>,
        actions: &[AgentActionCommand],
    ) -> Result<bool, StorageError> {
        if actions.is_empty() || actions.len() > MAX_AGENT_ACTIONS {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut entity_ids = HashSet::with_capacity(actions.len());
        for action in actions {
            action.validate()?;
            if !entity_ids.insert(action.entity_id()) {
                return Err(StorageError::InvalidConfiguration);
            }
        }
        self.complete_agent_job_inner(
            job_id,
            runner_id,
            assistant_message_id,
            content,
            presentation,
            actions,
        )
        .await
    }

    #[allow(clippy::too_many_lines)] // One transaction intentionally owns action, job, message, conversation, and sync writes.
    async fn complete_agent_job_inner(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        content: &str,
        presentation: Option<&AssistantPresentation>,
        actions: &[AgentActionCommand],
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id)
            || !is_v7(assistant_message_id)
            || !valid_runner_id(runner_id)
            || !valid_assistant_output(content)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        if let Some(presentation) = presentation {
            presentation.validate()?;
        }
        let presentation = presentation
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| StorageError::InvalidConfiguration)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let legacy_action = (actions.len() == 1).then(|| &actions[0]);
        let action_count =
            i16::try_from(actions.len()).map_err(|_| StorageError::InvalidConfiguration)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'completed',
                phase = NULL,
                claim_owner = NULL,
                claim_expires_at = NULL,
                executed_action_type = $3,
                executed_entity_id = $4,
                executed_at = CASE WHEN $3::text IS NULL THEN NULL ELSE NOW() END,
                executed_action_count = $5,
                finished_at = NOW()
            WHERE id = $1 AND claim_owner = $2 AND state = 'running'
            RETURNING user_id, conversation_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(legacy_action.map(AgentActionCommand::action_type))
        .bind(legacy_action.map(AgentActionCommand::entity_id))
        .bind(action_count)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, conversation_id, job_version)) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        for (index, action) in actions.iter().enumerate() {
            persist_agent_action(&mut transaction, user_id, action).await?;
            sqlx::query(
                "\
                INSERT INTO agent_job_action_executions (
                    job_id, action_index, action_type, entity_id
                ) VALUES ($1, $2, $3, $4)",
            )
            .bind(job_id)
            .bind(i16::try_from(index).map_err(|_| StorageError::InvalidConfiguration)?)
            .bind(action.action_type())
            .bind(action.entity_id())
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
        }
        if actions.iter().any(|action| {
            matches!(
                action,
                AgentActionCommand::CreateSchedule { .. }
                    | AgentActionCommand::UpdateSchedule { .. }
            )
        }) {
            resolve_conversation_schedule_conflicts(&mut transaction, user_id, conversation_id)
                .await?;
        }
        let message_version = sqlx::query_scalar::<_, i64>(
            "\
            INSERT INTO messages (
                id, conversation_id, agent_job_id, role, content, presentation, status, completed_at
            ) VALUES ($1, $2, $3, 'assistant', $4, $5, 'completed', NOW())
            ON CONFLICT (id) DO UPDATE
            SET content = EXCLUDED.content,
                presentation = EXCLUDED.presentation,
                status = 'completed',
                completed_at = NOW()
            WHERE messages.conversation_id = EXCLUDED.conversation_id
              AND messages.agent_job_id = EXCLUDED.agent_job_id
            RETURNING version",
        )
        .bind(assistant_message_id)
        .bind(conversation_id)
        .bind(job_id)
        .bind(content.trim())
        .bind(presentation)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET last_message_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "message",
            assistant_message_id,
            message_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Appends one safe assistant delta to the message visible to the
    /// conversation owner while the lease-owning turn is still running.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs,
    /// runner metadata, or assistant text, and a classified storage error when
    /// persistence fails.
    pub async fn append_agent_response_delta(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        delta: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id)
            || !is_v7(assistant_message_id)
            || !valid_runner_id(runner_id)
            || !valid_assistant_delta(delta)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let job = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET phase = 'streaming'
            WHERE id = $1 AND claim_owner = $2 AND state = 'running'
            RETURNING user_id, conversation_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, conversation_id, job_version)) = job else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let message_version = sqlx::query_scalar::<_, i64>(
            "\
            INSERT INTO messages (
                id, conversation_id, agent_job_id, role, content, status
            ) VALUES ($1, $2, $3, 'assistant', $4, 'streaming')
            ON CONFLICT (id) DO UPDATE
            SET content = messages.content || EXCLUDED.content,
                status = 'streaming',
                completed_at = NULL
            WHERE messages.conversation_id = EXCLUDED.conversation_id
              AND messages.agent_job_id = EXCLUDED.agent_job_id
              AND char_length(messages.content) + char_length(EXCLUDED.content) <= $5
            RETURNING version",
        )
        .bind(assistant_message_id)
        .bind(conversation_id)
        .bind(job_id)
        .bind(delta)
        .bind(i32::try_from(MAX_CONTENT_CHARS).map_err(|_| StorageError::InvalidConfiguration)?)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(message_version) = message_version else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Err(StorageError::PersistenceUnavailable);
        };
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "message",
            assistant_message_id,
            message_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Marks a lease-owned pre-provider or running job as failed using a
    /// sanitized error code only.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without retaining provider error text.
    pub async fn fail_agent_job(
        &self,
        job_id: Uuid,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id) || !valid_runner_id(runner_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, (Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'failed',
                phase = NULL,
                claim_owner = NULL,
                claim_expires_at = NULL,
                error_code = $3,
                finished_at = NOW()
            WHERE id = $1
              AND claim_owner = $2
              AND state IN ('claimed', 'running')
            RETURNING user_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(error_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, job_version)) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }
}

async fn resolve_conversation_schedule_conflicts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    conversation_id: Uuid,
) -> Result<(), StorageError> {
    let fingerprint_prefix = format!("agent:schedule-conflict:{conversation_id}:%");
    let recommendation_rows = sqlx::query_as::<_, (Uuid, i64)>(
        "UPDATE recommendations AS recommendation
         SET status = 'expired', revisit_at = NULL
         FROM intelligence_signals AS signal
         WHERE recommendation.signal_id = signal.id
           AND recommendation.user_id = $1
           AND signal.user_id = $1
           AND signal.fingerprint LIKE $2
           AND recommendation.status IN ('pending', 'deferred', 'analysis_requested')
         RETURNING recommendation.id, recommendation.version",
    )
    .bind(user_id)
    .bind(&fingerprint_prefix)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| classify(&error))?;
    let signal_rows = sqlx::query_as::<_, (Uuid, i64)>(
        "UPDATE intelligence_signals
         SET status = 'resolved', resolved_at = NOW()
         WHERE user_id = $1 AND status = 'active' AND fingerprint LIKE $2
         RETURNING id, version",
    )
    .bind(user_id)
    .bind(&fingerprint_prefix)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| classify(&error))?;
    for (recommendation_id, version) in recommendation_rows {
        append_change(
            transaction,
            user_id,
            "recommendation",
            recommendation_id,
            version,
        )
        .await?;
    }
    for (signal_id, version) in signal_rows {
        append_change(
            transaction,
            user_id,
            "intelligence_signal",
            signal_id,
            version,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // The exhaustive match keeps each atomic SQL mutation visibly tied to its action contract.
async fn persist_agent_action(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    action: &AgentActionCommand,
) -> Result<(), StorageError> {
    let (entity_type, entity_id, version) = match action {
        AgentActionCommand::CreateTask {
            id,
            project_id,
            title,
            notes,
            priority,
            due_at,
        } => {
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO tasks (
                    id, user_id, project_id, title, notes, status, priority, due_at
                )
                SELECT $1, $2, $3, $4, $5, 'open', $6, $7
                WHERE $3::uuid IS NULL OR EXISTS (
                    SELECT 1 FROM projects WHERE id = $3 AND user_id = $2
                )
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(project_id)
            .bind(title.trim())
            .bind(trim_optional_text(notes.as_deref()))
            .bind(priority)
            .bind(due_at)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            ("task", *id, version)
        }
        AgentActionCommand::UpdateTask {
            id,
            project_id,
            title,
            notes,
            priority,
            due_at,
            expected_version,
        } => {
            let version = sqlx::query_scalar::<_, i64>(
                "\
                UPDATE tasks
                SET project_id = $3, title = $4, notes = $5, priority = $6, due_at = $7
                WHERE id = $1 AND user_id = $2 AND status = 'open' AND version = $8
                  AND ($3::uuid IS NULL OR EXISTS (
                      SELECT 1 FROM projects WHERE id = $3 AND user_id = $2
                  ))
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(project_id)
            .bind(title.trim())
            .bind(trim_optional_text(notes.as_deref()))
            .bind(priority)
            .bind(due_at)
            .bind(expected_version)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            ("task", *id, version)
        }
        AgentActionCommand::SetTaskStatus {
            id,
            status,
            expected_version,
        } => {
            let (status, current_status) = match status {
                TaskStatus::Completed => ("completed", "open"),
                TaskStatus::Cancelled => ("cancelled", "open"),
                TaskStatus::Open => ("open", "completed"),
            };
            let version = sqlx::query_scalar::<_, i64>(
                "\
                UPDATE tasks
                SET status = $3,
                    completed_at = CASE WHEN $3 = 'completed' THEN NOW() ELSE NULL END
                WHERE id = $1 AND user_id = $2 AND status = $4 AND version = $5
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(status)
            .bind(current_status)
            .bind(expected_version)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            ("task", *id, version)
        }
        AgentActionCommand::CreateSchedule {
            id,
            title,
            notes,
            starts_at,
            ends_at,
            time_zone,
            ..
        } => {
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO schedule_entries (
                    id, user_id, title, notes, starts_at, ends_at, time_zone, source, status
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual', 'confirmed')
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .bind(trim_optional_text(notes.as_deref()))
            .bind(starts_at)
            .bind(ends_at)
            .bind(time_zone.trim())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?;
            attach_schedule_to_active_primary_and_queue_create(
                transaction,
                user_id,
                *id,
                version,
                &ScheduleCalendarMutationPayload {
                    title: title.trim().to_owned(),
                    notes: trim_optional_text(notes.as_deref()).map(str::to_owned),
                    starts_at: *starts_at,
                    ends_at: *ends_at,
                    time_zone: time_zone.trim().to_owned(),
                },
            )
            .await?;
            ("schedule_entry", *id, version)
        }
        AgentActionCommand::UpdateSchedule {
            id,
            title,
            notes,
            starts_at,
            ends_at,
            time_zone,
            expected_version,
            ..
        } => {
            let version = sqlx::query_scalar::<_, i64>(
                "\
                UPDATE schedule_entries
                SET title = $3, notes = $4, starts_at = $5, ends_at = $6, time_zone = $7
                WHERE id = $1 AND user_id = $2 AND source = 'manual'
                  AND status = 'confirmed' AND version = $8
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .bind(trim_optional_text(notes.as_deref()))
            .bind(starts_at)
            .bind(ends_at)
            .bind(time_zone.trim())
            .bind(expected_version)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            queue_linked_schedule_mutation(
                transaction,
                user_id,
                *id,
                version,
                ScheduleCalendarMutationOperation::Update,
                &ScheduleCalendarMutationPayload {
                    title: title.trim().to_owned(),
                    notes: trim_optional_text(notes.as_deref()).map(str::to_owned),
                    starts_at: *starts_at,
                    ends_at: *ends_at,
                    time_zone: time_zone.trim().to_owned(),
                },
            )
            .await?;
            ("schedule_entry", *id, version)
        }
        AgentActionCommand::CancelSchedule {
            id,
            expected_version,
        } => {
            let cancelled = sqlx::query_as::<
                _,
                (
                    i64,
                    String,
                    Option<String>,
                    OffsetDateTime,
                    OffsetDateTime,
                    String,
                ),
            >(
                "\
                UPDATE schedule_entries
                SET status = 'cancelled'
                WHERE id = $1 AND user_id = $2 AND source = 'manual'
                  AND status = 'confirmed' AND version = $3
                RETURNING version, title, notes, starts_at, ends_at, time_zone",
            )
            .bind(id)
            .bind(user_id)
            .bind(expected_version)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            let (version, title, notes, starts_at, ends_at, time_zone) = cancelled;
            queue_linked_schedule_mutation(
                transaction,
                user_id,
                *id,
                version,
                ScheduleCalendarMutationOperation::Delete,
                &ScheduleCalendarMutationPayload {
                    title,
                    notes,
                    starts_at,
                    ends_at,
                    time_zone,
                },
            )
            .await?;
            ("schedule_entry", *id, version)
        }
        AgentActionCommand::CreateProject {
            id,
            workspace_id,
            title,
            objective,
            risk_level,
            next_action,
            due_at,
        } => {
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO projects (
                    id, user_id, workspace_id, title, objective, status,
                    risk_level, next_action, due_at
                )
                SELECT $1, $2, workspaces.id, $4, $5, 'active', $6, $7, $8
                FROM workspaces
                WHERE workspaces.id = $3 AND workspaces.user_id = $2
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(workspace_id)
            .bind(title.trim())
            .bind(trim_optional_text(objective.as_deref()))
            .bind(risk_level)
            .bind(trim_optional_text(next_action.as_deref()))
            .bind(due_at)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            ("project", *id, version)
        }
        AgentActionCommand::UpdateProject {
            id,
            title,
            objective,
            status,
            risk_level,
            next_action,
            due_at,
            expected_version,
        } => {
            let status = match status {
                ProjectStatus::Active => "active",
                ProjectStatus::Paused => "paused",
                ProjectStatus::Completed => "completed",
            };
            let version = sqlx::query_scalar::<_, i64>(
                "\
                UPDATE projects
                SET title = $3, objective = $4, status = $5,
                    risk_level = $6, next_action = $7, due_at = $8
                WHERE id = $1 AND user_id = $2 AND version = $9
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .bind(trim_optional_text(objective.as_deref()))
            .bind(status)
            .bind(risk_level)
            .bind(trim_optional_text(next_action.as_deref()))
            .bind(due_at)
            .bind(expected_version)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            ("project", *id, version)
        }
        AgentActionCommand::DeleteProject {
            id,
            expected_version,
        } => {
            queue_agent_action_webhook(transaction, user_id, action).await?;
            let detached_tasks = sqlx::query_as::<_, (Uuid, i64)>(
                "\
                UPDATE tasks
                SET project_id = NULL
                WHERE user_id = $1 AND project_id = $2
                RETURNING id, version",
            )
            .bind(user_id)
            .bind(id)
            .fetch_all(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?;
            for (task_id, task_version) in detached_tasks {
                append_change(transaction, user_id, "task", task_id, task_version).await?;
            }
            let version = sqlx::query_scalar::<_, i64>(
                "\
                DELETE FROM projects
                WHERE id = $1 AND user_id = $2 AND version = $3
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(expected_version)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            append_delete_change(transaction, user_id, "project", *id, version).await?;
            return Ok(());
        }
        AgentActionCommand::SendWebhookMessage {
            id,
            project_id,
            webhook_id,
            message,
        } => {
            let payload = serde_json::json!({
                "event": "chat.message",
                "projectId": project_id,
                "message": message.trim(),
                "occurredAt": OffsetDateTime::now_utc().format(&Rfc3339).ok(),
            });
            let queued = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO webhook_deliveries (
                    id, user_id, project_id, webhook_id, provider,
                    destination_ciphertext, destination_nonce,
                    event_type, payload, status
                )
                SELECT $1, $2, webhook.project_id, webhook.id, webhook.provider,
                    webhook.destination_ciphertext, webhook.destination_nonce,
                    'chat.message', $5, 'queued'
                FROM project_webhooks AS webhook
                INNER JOIN projects AS project ON project.id = webhook.project_id
                WHERE webhook.id = $4 AND webhook.project_id = $3
                  AND project.user_id = $2 AND webhook.enabled = TRUE
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(project_id)
            .bind(webhook_id)
            .bind(payload)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?
            .ok_or(StorageError::IdentityConflict)?;
            let _ = queued;
            return Ok(());
        }
    };
    queue_agent_action_webhook(transaction, user_id, action).await?;
    append_change(transaction, user_id, entity_type, entity_id, version).await
}

async fn queue_agent_action_webhook(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    action: &AgentActionCommand,
) -> Result<(), StorageError> {
    let (project_id, event_type, entity_id) = match action {
        AgentActionCommand::CreateTask {
            id,
            project_id: Some(project_id),
            ..
        } => (Some(*project_id), "task.created", *id),
        AgentActionCommand::UpdateTask {
            id,
            project_id: Some(project_id),
            ..
        } => (Some(*project_id), "task.updated", *id),
        AgentActionCommand::SetTaskStatus { id, status, .. } => {
            let project_id: Option<Uuid> =
                sqlx::query_scalar("SELECT project_id FROM tasks WHERE id = $1 AND user_id = $2")
                    .bind(id)
                    .bind(user_id)
                    .fetch_optional(&mut **transaction)
                    .await
                    .map_err(|error| classify(&error))?
                    .flatten();
            let event_type = match status {
                TaskStatus::Completed => "task.completed",
                TaskStatus::Open => "task.restored",
                TaskStatus::Cancelled => "task.deleted",
            };
            (project_id, event_type, *id)
        }
        AgentActionCommand::UpdateProject { id, .. } => (Some(*id), "project.updated", *id),
        AgentActionCommand::DeleteProject { id, .. } => (Some(*id), "project.deleted", *id),
        _ => (None, "", action.entity_id()),
    };
    if let Some(project_id) = project_id {
        let payload = project_event_payload(event_type, project_id, entity_id)?;
        queue_project_event_in_transaction(transaction, user_id, project_id, event_type, &payload)
            .await?;
    }
    Ok(())
}

fn trim_optional_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

async fn persist_approved_agent_action(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    action: &PendingAgentAction,
) -> Result<(), StorageError> {
    match action {
        PendingAgentAction::CreateTask { title, due_at } => {
            let id = Uuid::now_v7();
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO tasks (id, user_id, title, notes, status, priority, due_at)
                VALUES ($1, $2, $3, NULL, 'open', 1, $4)
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .bind(due_at)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?;
            append_change(transaction, user_id, "task", id, version).await
        }
        PendingAgentAction::CreateSchedule {
            title,
            starts_at,
            ends_at,
            time_zone,
        } => {
            let id = Uuid::now_v7();
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO schedule_entries (
                    id, user_id, title, notes, starts_at, ends_at, time_zone, source, status
                ) VALUES ($1, $2, $3, NULL, $4, $5, $6, 'manual', 'confirmed')
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .bind(starts_at)
            .bind(ends_at)
            .bind(time_zone.trim())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?;
            attach_schedule_to_active_primary_and_queue_create(
                transaction,
                user_id,
                id,
                version,
                &ScheduleCalendarMutationPayload {
                    title: title.trim().to_owned(),
                    notes: None,
                    starts_at: *starts_at,
                    ends_at: *ends_at,
                    time_zone: time_zone.trim().to_owned(),
                },
            )
            .await?;
            append_change(transaction, user_id, "schedule_entry", id, version).await
        }
    }
}

async fn existing_agent_turn(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: Uuid,
    client_message_id: Uuid,
) -> Result<Option<ExistingAgentTurnRow>, StorageError> {
    sqlx::query_as::<_, ExistingAgentTurnRow>(
        "\
        SELECT job.id,
               job.input_message_id,
               job.conversation_id,
               job.state,
               job.version,
               message.content
        FROM messages AS message
        INNER JOIN agent_jobs AS job ON job.input_message_id = message.id
        WHERE message.conversation_id = $1
          AND message.client_message_id = $2",
    )
    .bind(conversation_id)
    .bind(client_message_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| classify(&error))
}

async fn owns_active_conversation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    conversation_id: Uuid,
) -> Result<bool, StorageError> {
    sqlx::query_scalar::<_, bool>(
        "\
        SELECT EXISTS(
            SELECT 1 FROM conversations
            WHERE id = $1 AND user_id = $2 AND status = 'active'
        )",
    )
    .bind(conversation_id)
    .bind(user_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| classify(&error))
}

fn parse_job_state(value: &str) -> Result<AgentJobState, StorageError> {
    match value {
        "queued" => Ok(AgentJobState::Queued),
        "claimed" => Ok(AgentJobState::Claimed),
        "running" => Ok(AgentJobState::Running),
        "waiting_approval" => Ok(AgentJobState::WaitingApproval),
        "retry_wait" => Ok(AgentJobState::RetryWait),
        "completed" => Ok(AgentJobState::Completed),
        "failed" => Ok(AgentJobState::Failed),
        "cancelled" => Ok(AgentJobState::Cancelled),
        "declined" => Ok(AgentJobState::Declined),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_agent_authentication_state(value: &str) -> Result<AgentAuthenticationState, StorageError> {
    match value {
        "requested" => Ok(AgentAuthenticationState::Requested),
        "awaiting_authorization" => Ok(AgentAuthenticationState::AwaitingAuthorization),
        "ready" => Ok(AgentAuthenticationState::Ready),
        "failed" => Ok(AgentAuthenticationState::Failed),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn pending_action_from_fields(
    action_type: Option<&str>,
    title: Option<String>,
    due_at: Option<OffsetDateTime>,
    starts_at: Option<OffsetDateTime>,
    ends_at: Option<OffsetDateTime>,
    time_zone: Option<String>,
) -> Result<Option<PendingAgentAction>, StorageError> {
    match (action_type, title, due_at, starts_at, ends_at, time_zone) {
        (None, None, None, None, None, None) => Ok(None),
        (Some("create_task"), Some(title), due_at, None, None, None) => {
            let action = PendingAgentAction::CreateTask { title, due_at };
            action.validate()?;
            Ok(Some(action))
        }
        (
            Some("create_schedule"),
            Some(title),
            None,
            Some(starts_at),
            Some(ends_at),
            Some(time_zone),
        ) => {
            let action = PendingAgentAction::CreateSchedule {
                title,
                starts_at,
                ends_at,
                time_zone,
            };
            action.validate()?;
            Ok(Some(action))
        }
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn validate_agent_model_catalog(models: &[AgentModelCatalogEntry]) -> Result<(), StorageError> {
    if models.is_empty() || models.len() > MAX_MODEL_COUNT {
        return Err(StorageError::InvalidConfiguration);
    }
    let mut ids = HashSet::with_capacity(models.len());
    let mut default_count = 0usize;
    for model in models {
        let mut effort_ids = HashSet::with_capacity(model.supported_reasoning_efforts.len());
        if !valid_text(&model.id, MAX_MODEL_ID_CHARS, false)
            || !valid_text(&model.display_name, MAX_MODEL_NAME_CHARS, false)
            || !valid_text(&model.description, MAX_MODEL_DESCRIPTION_CHARS, true)
            || !valid_text(
                &model.default_reasoning_effort,
                MAX_REASONING_EFFORT_ID_CHARS,
                false,
            )
            || model.supported_reasoning_efforts.is_empty()
            || model.supported_reasoning_efforts.len() > MAX_REASONING_EFFORT_COUNT
            || !ids.insert(model.id.as_str())
        {
            return Err(StorageError::InvalidConfiguration);
        }
        for effort in &model.supported_reasoning_efforts {
            if !valid_text(&effort.id, MAX_REASONING_EFFORT_ID_CHARS, false)
                || !valid_text(
                    &effort.description,
                    MAX_REASONING_EFFORT_DESCRIPTION_CHARS,
                    true,
                )
                || !effort_ids.insert(effort.id.as_str())
            {
                return Err(StorageError::InvalidConfiguration);
            }
        }
        if !effort_ids.contains(model.default_reasoning_effort.as_str()) {
            return Err(StorageError::InvalidConfiguration);
        }
        default_count += usize::from(model.is_default);
    }
    if default_count != 1 {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn valid_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.trim().is_empty())
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

fn valid_time_zone(value: &str) -> bool {
    valid_text(value, 80, false)
}

fn presentation_item_id(item: &AssistantPresentationItem) -> Uuid {
    match item {
        AssistantPresentationItem::Task { id, .. }
        | AssistantPresentationItem::Schedule { id, .. }
        | AssistantPresentationItem::Project { id, .. } => *id,
    }
}

fn presentation_item_matches_kind(
    item: &AssistantPresentationItem,
    kind: AssistantPresentationKind,
) -> bool {
    matches!(
        (kind, item),
        (
            AssistantPresentationKind::Tasks,
            AssistantPresentationItem::Task { .. }
        ) | (
            AssistantPresentationKind::Schedule,
            AssistantPresentationItem::Schedule { .. }
        ) | (
            AssistantPresentationKind::Projects,
            AssistantPresentationItem::Project { .. }
        )
    )
}

fn presentation_item_matches_section_kind(
    item: &AssistantPresentationItem,
    kind: AssistantPresentationSectionKind,
) -> bool {
    matches!(
        (kind, item),
        (
            AssistantPresentationSectionKind::Tasks,
            AssistantPresentationItem::Task { .. }
        ) | (
            AssistantPresentationSectionKind::Schedule,
            AssistantPresentationItem::Schedule { .. }
        ) | (
            AssistantPresentationSectionKind::Projects,
            AssistantPresentationItem::Project { .. }
        )
    )
}

fn presentation_view_matches_kind(
    view: AssistantPresentationView,
    kind: AssistantPresentationSectionKind,
) -> bool {
    matches!(
        (kind, view),
        (
            AssistantPresentationSectionKind::Tasks,
            AssistantPresentationView::List | AssistantPresentationView::Checklist
        ) | (
            AssistantPresentationSectionKind::Schedule,
            AssistantPresentationView::List | AssistantPresentationView::Timeline
        ) | (
            AssistantPresentationSectionKind::Projects,
            AssistantPresentationView::List | AssistantPresentationView::Cards
        )
    )
}

fn valid_presentation_item(item: &AssistantPresentationItem) -> bool {
    match item {
        AssistantPresentationItem::Task {
            id,
            project_id,
            project_title,
            title,
            status,
            priority,
            due_at,
        } => {
            is_v7(*id)
                && project_id.is_none_or(is_v7)
                && project_title
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_TITLE_CHARS, false))
                && valid_text(title, MAX_TITLE_CHARS, false)
                && matches!(status.as_str(), "open" | "completed" | "cancelled")
                && (0..=3).contains(priority)
                && due_at
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_PRESENTATION_TIMESTAMP_CHARS, false))
        }
        AssistantPresentationItem::Schedule {
            id,
            title,
            status,
            starts_at,
            ends_at,
            time_zone,
        } => {
            is_v7(*id)
                && valid_text(title, MAX_TITLE_CHARS, false)
                && matches!(status.as_str(), "confirmed" | "cancelled")
                && valid_text(starts_at, MAX_PRESENTATION_TIMESTAMP_CHARS, false)
                && valid_text(ends_at, MAX_PRESENTATION_TIMESTAMP_CHARS, false)
                && valid_time_zone(time_zone)
        }
        AssistantPresentationItem::Project {
            id,
            workspace_id,
            title,
            status,
            objective,
            next_action,
            risk_level,
            open_task_count,
        } => {
            is_v7(*id)
                && is_v7(*workspace_id)
                && valid_text(title, MAX_TITLE_CHARS, false)
                && matches!(
                    status.as_str(),
                    "active" | "paused" | "completed" | "removed"
                )
                && objective
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_PRESENTATION_DETAIL_CHARS, true))
                && next_action
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_PRESENTATION_DETAIL_CHARS, true))
                && (0..=3).contains(risk_level)
                && *open_task_count >= 0
        }
    }
}

fn valid_assistant_output(value: &str) -> bool {
    !value.trim().is_empty() && valid_assistant_delta(value)
}

fn valid_assistant_delta(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_CONTENT_CHARS
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn valid_runner_id(value: &str) -> bool {
    valid_text(value, MAX_RUNNER_ID_CHARS, false)
}

fn valid_external_id(value: &str) -> bool {
    valid_text(value, MAX_RUNNER_ID_CHARS, false)
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.')
        })
}

fn claim_lease_millis(runner_id: &str, lease: Duration) -> Result<i64, StorageError> {
    if !valid_runner_id(runner_id) || !(MIN_CLAIM_LEASE..=MAX_CLAIM_LEASE).contains(&lease) {
        return Err(StorageError::InvalidConfiguration);
    }
    i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)
}

fn classify(error: &sqlx::Error) -> StorageError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23505")
    {
        return StorageError::IdentityConflict;
    }
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{
        AgentActionCommand, AssistantPresentation, AssistantPresentationItem,
        AssistantPresentationKind, AssistantPresentationLayout, AssistantPresentationSection,
        AssistantPresentationSectionKind, AssistantPresentationView, NewAgentTurn, NewConversation,
    };
    use crate::{StorageError, planning::TaskStatus};
    use uuid::Uuid;

    #[test]
    fn conversation_rejects_invalid_title_or_identifier() {
        let invalid = NewConversation {
            id: Uuid::nil(),
            user_id: Uuid::now_v7(),
            title: Some(" ".to_owned()),
        };
        assert!(matches!(
            invalid.validate(),
            Err(StorageError::InvalidConfiguration)
        ));
    }

    #[test]
    fn agent_turn_rejects_blank_or_non_v7_identifiers() {
        let invalid = NewAgentTurn {
            job_id: Uuid::now_v7(),
            message_id: Uuid::now_v7(),
            client_message_id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            conversation_id: Uuid::nil(),
            content: " ".to_owned(),
        };
        assert!(matches!(
            invalid.validate(),
            Err(StorageError::InvalidConfiguration)
        ));
    }

    #[test]
    fn agent_action_accepts_reopening_a_completed_task() {
        let action = AgentActionCommand::SetTaskStatus {
            id: Uuid::now_v7(),
            status: TaskStatus::Open,
            expected_version: 2,
        };

        assert!(action.validate().is_ok());
        assert_eq!(action.action_type(), "update_task");
    }

    #[test]
    fn assistant_presentation_accepts_a_verified_composite_surface() {
        let task_id = Uuid::now_v7();
        let schedule_id = Uuid::now_v7();
        let presentation = AssistantPresentation {
            kind: AssistantPresentationKind::Composite,
            title: "오늘의 실행 계획".to_owned(),
            items: vec![
                AssistantPresentationItem::Task {
                    id: task_id,
                    project_id: None,
                    project_title: None,
                    title: "회의록 정리".to_owned(),
                    status: "open".to_owned(),
                    priority: 2,
                    due_at: None,
                },
                AssistantPresentationItem::Schedule {
                    id: schedule_id,
                    title: "주간 회의".to_owned(),
                    status: "confirmed".to_owned(),
                    starts_at: "2026-07-13T04:00:00Z".to_owned(),
                    ends_at: "2026-07-13T05:00:00Z".to_owned(),
                    time_zone: "Asia/Seoul".to_owned(),
                },
            ],
            layout: AssistantPresentationLayout::Focus,
            sections: vec![
                AssistantPresentationSection {
                    kind: AssistantPresentationSectionKind::Tasks,
                    title: "먼저 할 일".to_owned(),
                    view: AssistantPresentationView::Checklist,
                    item_ids: vec![task_id],
                },
                AssistantPresentationSection {
                    kind: AssistantPresentationSectionKind::Schedule,
                    title: "예정된 일정".to_owned(),
                    view: AssistantPresentationView::Timeline,
                    item_ids: vec![schedule_id],
                },
            ],
            focus_item_id: Some(task_id),
        };

        assert!(presentation.validate().is_ok());
    }

    #[test]
    fn assistant_presentation_rejects_a_section_with_the_wrong_item_type() {
        let task_id = Uuid::now_v7();
        let presentation = AssistantPresentation {
            kind: AssistantPresentationKind::Schedule,
            title: "오늘 일정".to_owned(),
            items: vec![AssistantPresentationItem::Task {
                id: task_id,
                project_id: None,
                project_title: None,
                title: "회의록 정리".to_owned(),
                status: "open".to_owned(),
                priority: 2,
                due_at: None,
            }],
            layout: AssistantPresentationLayout::Split,
            sections: vec![AssistantPresentationSection {
                kind: AssistantPresentationSectionKind::Schedule,
                title: "예정된 일정".to_owned(),
                view: AssistantPresentationView::Timeline,
                item_ids: vec![task_id],
            }],
            focus_item_id: Some(task_id),
        };

        assert!(matches!(
            presentation.validate(),
            Err(StorageError::InvalidConfiguration)
        ));
    }

    #[test]
    fn legacy_presentation_items_receive_safe_default_statuses() {
        let task_id = Uuid::now_v7();
        let item: AssistantPresentationItem = serde_json::from_value(serde_json::json!({
            "type": "task",
            "id": task_id,
            "projectId": null,
            "projectTitle": null,
            "title": "기존 할 일",
            "priority": 1,
            "dueAt": null
        }))
        .expect("legacy task should remain readable");

        assert!(matches!(
            item,
            AssistantPresentationItem::Task { id, ref status, .. }
                if id == task_id && status == "open"
        ));
    }
}

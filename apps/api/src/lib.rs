pub mod auth;
pub mod calendar_oauth;
pub mod config;
pub mod google_chat_oauth;
mod meetings;
pub mod probe;
pub mod push;
mod voice_command;
pub mod webhook;

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    future::Future,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use jimin_application::{ApplicationError, DeviceSession, SessionService};
use jimin_domain::{ClientPlatform, DeviceRegistration};
use jimin_observability::{RequestId, request_context};
use jimin_storage::{
    Database, EXPECTED_SCHEMA_VERSION, Readiness, StorageError,
    agent::{
        AgentAuthentication, AgentAuthenticationState, AgentJob, AgentJobState,
        AgentModelCatalogEntry, AgentModelSettings, AgentReasoningEffort, AssistantPresentation,
        AssistantPresentationItem, AssistantPresentationKind, AssistantPresentationLayout,
        AssistantPresentationSection, AssistantPresentationSectionKind, AssistantPresentationView,
        Conversation, ConversationMessage, ConversationMessageRole, ConversationMessageStatus,
        ConversationStatus, NewAgentTurn, NewConversation, PendingAgentAction,
        PendingAgentActionDecision, QueuedAgentTurn,
    },
    auth::{Device, DeviceStatus, Profile},
    calendar::{
        CalendarAccount, CalendarAccountStatus, CreateCalendarOAuthAuthorization,
        DisconnectCalendarAccountOutcome,
    },
    goals::{GoalHealth, GoalNextActionKind, GoalOverview, GoalStatus, GoalUpdate, NewGoal},
    google_chat::{
        CreateGoogleChatOAuthAuthorization, GoogleChatAccount, GoogleChatAccountStatus,
        GoogleChatCompletionDelivery, GoogleChatSourceSyncConnection, NewProjectGoogleChatSource,
        ProjectGoogleChatSource, ProjectInflowItem, ProjectInflowStatus, PromoteProjectInflowItem,
    },
    intelligence::{
        DecideRecommendation, DecideRecommendationOutcome, Recommendation, RecommendationDecision,
        RecommendationStatus, SuggestedActionKind,
    },
    planning::{
        DeleteTaskOutcome, NewScheduleEntry, NewTask, ScheduleEntry, ScheduleEntryUpdate,
        ScheduleSource, ScheduleStatus, Task, TaskStatus, TaskUpdate,
    },
    sync::SyncChange,
    webhook::{
        GoogleChatMentionDirectory, NewProjectWebhook, ProjectWebhook, ProjectWebhookUpdate,
        RetryWebhookDeliveryOutcome, WebhookDelivery, WebhookDestinationUpdate,
        WebhookMentionDirectoryUpdate, WebhookProvider,
    },
    work::{
        DeleteProjectOutcome, NewProject, Project, ProjectStatus, ProjectUpdate, Workspace,
        WorkspaceScope,
    },
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::{
    Duration as TimeDuration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339,
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::warn;
use utoipa::{OpenApi, ToSchema};

use crate::{
    calendar_oauth::{CalendarOAuthError, CalendarOAuthRuntime, storage_failure_code},
    google_chat_oauth::{GoogleChatOAuthError, GoogleChatOAuthRuntime},
    voice_command::{VoiceCommand, VoiceCommandError, VoiceTaskScope},
};

#[async_trait]
pub trait ReadinessProbe: Send + Sync {
    async fn check(&self, expected_schema_version: i64) -> Readiness;
}

#[async_trait]
impl ReadinessProbe for Database {
    async fn check(&self, expected_schema_version: i64) -> Readiness {
        self.readiness(expected_schema_version).await
    }
}

#[derive(Clone)]
pub struct ApiState {
    build_sha: Arc<str>,
    configuration_ready: bool,
    database: Option<Arc<dyn ReadinessProbe>>,
    expected_schema_version: i64,
    trusted_network: bool,
    authentication: Option<Arc<auth::Authentication>>,
    pairing: Option<Arc<PairingRuntime>>,
    planning: Option<Database>,
    calendar_oauth: Option<Arc<CalendarOAuthRuntime>>,
    google_chat_oauth: Option<Arc<GoogleChatOAuthRuntime>>,
    webhook: Option<Arc<webhook::WebhookRuntime>>,
    push: Option<Arc<push::PushRuntime>>,
    agent: Option<Database>,
}

impl ApiState {
    #[must_use]
    pub fn new(
        build_sha: impl Into<Arc<str>>,
        configuration_ready: bool,
        database: Option<Arc<dyn ReadinessProbe>>,
    ) -> Self {
        Self {
            build_sha: build_sha.into(),
            configuration_ready,
            database,
            expected_schema_version: EXPECTED_SCHEMA_VERSION,
            trusted_network: false,
            authentication: None,
            pairing: None,
            planning: None,
            calendar_oauth: None,
            google_chat_oauth: None,
            webhook: None,
            push: None,
            agent: None,
        }
    }

    #[must_use]
    pub fn with_authentication(mut self, authentication: auth::Authentication) -> Self {
        self.authentication = Some(Arc::new(authentication));
        self
    }

    /// Enables the private-network bootstrap route. Deployment ingress must
    /// restrict the API to the owner's VPN before this flag is set.
    #[must_use]
    pub fn with_trusted_network(mut self, trusted_network: bool) -> Self {
        self.trusted_network = trusted_network;
        self
    }

    #[must_use]
    const fn trusted_network(&self) -> bool {
        self.trusted_network
    }

    #[must_use]
    pub(crate) fn authentication(&self) -> Option<&Arc<auth::Authentication>> {
        self.authentication.as_ref()
    }

    #[must_use]
    pub fn with_pairing(mut self, pairing: PairingRuntime) -> Self {
        self.pairing = Some(Arc::new(pairing));
        self
    }

    #[must_use]
    fn pairing(&self) -> Option<&Arc<PairingRuntime>> {
        self.pairing.as_ref()
    }

    #[must_use]
    pub fn with_planning(mut self, planning: Database) -> Self {
        self.planning = Some(planning);
        self
    }

    #[must_use]
    fn planning(&self) -> Option<&Database> {
        self.planning.as_ref()
    }

    #[must_use]
    pub fn with_calendar_oauth(mut self, calendar_oauth: CalendarOAuthRuntime) -> Self {
        self.calendar_oauth = Some(Arc::new(calendar_oauth));
        self
    }

    #[must_use]
    pub fn with_google_chat_oauth(mut self, runtime: GoogleChatOAuthRuntime) -> Self {
        self.google_chat_oauth = Some(Arc::new(runtime));
        self
    }

    #[must_use]
    pub fn with_webhook_runtime(mut self, runtime: webhook::WebhookRuntime) -> Self {
        self.webhook = Some(Arc::new(runtime));
        self
    }

    fn webhook(&self) -> Option<&Arc<webhook::WebhookRuntime>> {
        self.webhook.as_ref()
    }

    #[must_use]
    pub fn with_push_runtime(mut self, runtime: push::PushRuntime) -> Self {
        self.push = Some(Arc::new(runtime));
        self
    }

    #[must_use]
    pub(crate) fn push(&self) -> Option<&Arc<push::PushRuntime>> {
        self.push.as_ref()
    }

    #[must_use]
    fn calendar_oauth(&self) -> Option<&Arc<CalendarOAuthRuntime>> {
        self.calendar_oauth.as_ref()
    }

    fn google_chat_oauth(&self) -> Option<&Arc<GoogleChatOAuthRuntime>> {
        self.google_chat_oauth.as_ref()
    }

    #[must_use]
    pub fn with_agent(mut self, agent: Database) -> Self {
        self.agent = Some(agent);
        self
    }

    #[must_use]
    fn agent(&self) -> Option<&Database> {
        self.agent.as_ref()
    }
}

pub struct PairingRuntime {
    sessions: SessionService,
}

impl PairingRuntime {
    #[must_use]
    pub fn new(sessions: SessionService) -> Self {
        Self { sessions }
    }

    async fn provision_trusted_network_device(
        &self,
        device: DeviceRegistration,
        request_id: uuid::Uuid,
    ) -> Result<DeviceSession, ApplicationError> {
        let pairing = self.sessions.issue_device_pairing().await?;
        self.sessions
            .consume_device_pairing(pairing.token().serialized().clone(), device, request_id)
            .await
    }
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LiveStatus {
    Ok,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReadyStatus {
    Ready,
    NotReady,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Ok,
    Error,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveHealthResponse {
    status: LiveStatus,
    service: &'static str,
    build_sha: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessChecks {
    configuration: CheckStatus,
    database: CheckStatus,
    migrations: CheckStatus,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadyHealthResponse {
    status: ReadyStatus,
    checks: ReadinessChecks,
    schema_version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MeResponse {
    id: uuid::Uuid,
    email: Option<String>,
    display_name: Option<String>,
    time_zone: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceResponse {
    id: uuid::Uuid,
    platform: String,
    name: String,
    app_version: String,
    os_version: Option<String>,
    status: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListResponse {
    items: Vec<DeviceResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEntryResponse {
    id: uuid::Uuid,
    title: String,
    notes: Option<String>,
    starts_at: String,
    ends_at: String,
    time_zone: String,
    status: String,
    source: String,
    editable: bool,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleListResponse {
    items: Vec<ScheduleEntryResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    id: uuid::Uuid,
    project_id: Option<uuid::Uuid>,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    status: String,
    priority: i16,
    due_at: Option<String>,
    completed_at: Option<String>,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskListResponse {
    items: Vec<TaskResponse>,
    next_cursor: Option<String>,
}

/// A personal or company work scope owned by the signed-in user.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    id: uuid::Uuid,
    scope: String,
    name: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResponse {
    items: Vec<WorkspaceResponse>,
    next_cursor: Option<String>,
}

/// The current operational state of one project.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    title: String,
    objective: Option<String>,
    status: String,
    risk_level: i16,
    next_action: Option<String>,
    due_at: Option<String>,
    open_task_count: i64,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListResponse {
    items: Vec<ProjectResponse>,
    next_cursor: Option<String>,
}

/// A desired outcome that gives projects and daily work a clear direction.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalResponse {
    id: uuid::Uuid,
    workspace_id: Option<uuid::Uuid>,
    project_id: Option<uuid::Uuid>,
    title: String,
    desired_outcome: String,
    status: String,
    target_at: Option<String>,
    project_title: Option<String>,
    progress_percent: i16,
    total_task_count: i64,
    open_task_count: i64,
    completed_task_count: i64,
    completed_last_seven_days: i64,
    overdue_task_count: i64,
    health: String,
    next_action: Option<GoalNextActionResponse>,
    created_at: String,
    updated_at: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalNextActionResponse {
    kind: String,
    id: Option<uuid::Uuid>,
    title: String,
    due_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalListResponse {
    items: Vec<GoalResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWebhookResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    provider: String,
    destination_label: String,
    mention_directory: WebhookMentionDirectory,
    events: Vec<String>,
    enabled: bool,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWebhookListResponse {
    items: Vec<ProjectWebhookResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDeliveryResponse {
    id: uuid::Uuid,
    webhook_id: uuid::Uuid,
    event_type: String,
    status: String,
    attempt_count: i32,
    response_code: Option<i32>,
    error_code: Option<String>,
    created_at: String,
    delivered_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDeliveryListResponse {
    items: Vec<WebhookDeliveryResponse>,
    next_cursor: Option<String>,
}

/// Server-owned read model for the real planning data shown on the daily home.
///
/// The snapshot deliberately excludes provider-shaped placeholders: a future
/// connected source is added only when its own persistence and sync contract
/// exists.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HomeSnapshotResponse {
    schedule: Vec<ScheduleEntryResponse>,
    tasks: Vec<TaskResponse>,
    due_tasks: Vec<TaskResponse>,
    inflow: Vec<ProjectInflowItemResponse>,
    recent_inflow: Vec<ProjectInflowItemResponse>,
    recommendations: Vec<RecommendationResponse>,
}

/// One prioritized action proposal generated from the owner's current context.
/// A recommendation is read-only until the owner records an explicit decision.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationResponse {
    id: uuid::Uuid,
    workspace_id: Option<uuid::Uuid>,
    project_id: Option<uuid::Uuid>,
    goal_id: Option<uuid::Uuid>,
    signal_id: Option<uuid::Uuid>,
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
    suggested_entity_id: Option<uuid::Uuid>,
    status: String,
    valid_until: Option<String>,
    revisit_at: Option<String>,
    created_at: String,
    updated_at: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationListResponse {
    items: Vec<RecommendationResponse>,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationDecisionKind {
    Approve,
    Reject,
    Defer,
    RequestAnalysis,
}

#[derive(Debug, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecommendationDecisionRequest {
    client_mutation_id: uuid::Uuid,
    decision: RecommendationDecisionKind,
    reason: Option<String>,
    revisit_at: Option<String>,
    expected_version: i64,
}

/// Safe Google Calendar connection state. Provider credentials and identifiers
/// never leave the server.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendarConnectionResponse {
    available: bool,
    status: String,
    email: Option<String>,
    granted_scopes: Vec<String>,
    last_successful_sync_at: Option<String>,
    last_error_code: Option<String>,
    reauth_required: bool,
    version: Option<i64>,
}

/// A platform-bound request to begin Calendar consent. The server owns the
/// Google client profile and callback URL; the client supplies no OAuth URL or
/// provider credential.
#[derive(Debug, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartGoogleCalendarAuthorizationRequest {
    client_kind: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartGoogleCalendarAuthorizationResponse {
    authorization_id: uuid::Uuid,
    authorization_url: String,
    expires_at: String,
}

#[derive(Debug, Deserialize)]
struct GoogleCalendarCallbackQuery {
    state: String,
    code: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChatAccountResponse {
    id: uuid::Uuid,
    email: String,
    status: String,
    last_successful_sync_at: Option<String>,
    last_error_code: Option<String>,
    reauth_required: bool,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChatAccountListResponse {
    available: bool,
    items: Vec<GoogleChatAccountResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChatSpaceResponse {
    name: String,
    display_name: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChatSpaceListResponse {
    items: Vec<GoogleChatSpaceResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGoogleChatSourceResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    account_id: uuid::Uuid,
    account_email: String,
    space_name: String,
    display_name: String,
    enabled: bool,
    acknowledge_with_reaction: bool,
    last_successful_sync_at: Option<String>,
    last_error_code: Option<String>,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGoogleChatSourceListResponse {
    items: Vec<ProjectGoogleChatSourceResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The read model exposes independent sender, acknowledgement, assignment, reaction, and reply facets to shared clients."
)]
pub struct ProjectInflowItemResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    project_name: String,
    source_id: uuid::Uuid,
    source_name: String,
    sender_name: Option<String>,
    sent_by_owner: bool,
    content_text: String,
    suggested_task_title: String,
    suggested_task_notes: String,
    message_count: usize,
    first_received_at: String,
    received_at: String,
    messages: Vec<ProjectInflowMessageResponse>,
    status: String,
    promoted_task_id: Option<uuid::Uuid>,
    acknowledged: bool,
    completion_status: String,
    completion_reaction_completed: bool,
    completion_reply_completed: bool,
    completion_error_code: Option<String>,
    completion_attempt_count: i32,
    assignee_options: Vec<String>,
    notifiable_assignee_names: Vec<String>,
    assignee_notification_available: bool,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInflowMessageResponse {
    sender_name: Option<String>,
    sent_by_owner: bool,
    content_text: String,
    received_at: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInflowItemListResponse {
    items: Vec<ProjectInflowItemResponse>,
}

#[derive(Debug, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateProjectGoogleChatSourceRequest {
    account_id: uuid::Uuid,
    space_name: String,
    display_name: String,
    acknowledge_with_reaction: bool,
    #[serde(default)]
    import_history: bool,
}

#[derive(Debug, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DeleteVersionedConnectionQuery {
    expected_version: i64,
}

#[derive(Debug, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectInflowDecisionRequest {
    decision: String,
    expected_version: i64,
    title: Option<String>,
    notes: Option<String>,
    assignee_name: Option<String>,
    priority: Option<i16>,
    due_at: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProjectInflowListQuery {
    status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationResponse {
    id: uuid::Uuid,
    title: Option<String>,
    status: String,
    last_message_at: Option<String>,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationListResponse {
    items: Vec<ConversationResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedAgentTurnResponse {
    job_id: uuid::Uuid,
    message_id: uuid::Uuid,
    conversation_id: uuid::Uuid,
    state: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessageResponse {
    id: uuid::Uuid,
    role: String,
    content: String,
    presentation: Option<AssistantPresentationResponse>,
    status: String,
    created_at: String,
    completed_at: Option<String>,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantPresentationResponse {
    kind: String,
    title: String,
    items: Vec<AssistantPresentationItemResponse>,
    layout: String,
    sections: Vec<AssistantPresentationSectionResponse>,
    focus_item_id: Option<uuid::Uuid>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantPresentationSectionResponse {
    kind: String,
    title: String,
    view: String,
    item_ids: Vec<uuid::Uuid>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AssistantPresentationItemResponse {
    Task {
        id: uuid::Uuid,
        project_id: Option<uuid::Uuid>,
        project_title: Option<String>,
        assignee_name: Option<String>,
        title: String,
        status: String,
        priority: i16,
        due_at: Option<String>,
    },
    Schedule {
        id: uuid::Uuid,
        title: String,
        status: String,
        starts_at: String,
        ends_at: String,
        time_zone: String,
    },
    Project {
        id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        title: String,
        status: String,
        objective: Option<String>,
        next_action: Option<String>,
        risk_level: i16,
        open_task_count: i64,
    },
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessageListResponse {
    items: Vec<ConversationMessageResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentJobResponse {
    id: uuid::Uuid,
    conversation_id: uuid::Uuid,
    state: String,
    created_at: String,
    finished_at: Option<String>,
    version: i64,
    pending_action: Option<PendingAgentActionResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingAgentActionResponse {
    kind: String,
    title: String,
    due_at: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationStreamSnapshot {
    messages: Vec<ConversationMessageResponse>,
    job: Option<AgentJobResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentAuthenticationResponse {
    state: String,
    verification_url: Option<String>,
    user_code: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentModelResponse {
    id: String,
    display_name: String,
    description: String,
    is_default: bool,
    default_reasoning_effort: String,
    supported_reasoning_efforts: Vec<AgentReasoningEffortResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentReasoningEffortResponse {
    id: String,
    description: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentModelSettingsResponse {
    items: Vec<AgentModelResponse>,
    selected_model_id: Option<String>,
    selected_reasoning_effort: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum VoiceCommandKind {
    ScheduleListed,
    ScheduleCreated,
    TasksListed,
    TaskCreated,
    NeedsDetails,
    ContinueConversation,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum VoiceCommandDestination {
    Home,
    Calendar,
    Conversation,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum VoiceCommandItemType {
    Task,
    Schedule,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct VoiceCommandItemResponse {
    item_type: VoiceCommandItemType,
    id: uuid::Uuid,
    title: String,
    due_at: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    priority: Option<i16>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct VoiceCommandResponse {
    kind: VoiceCommandKind,
    message: String,
    destination: VoiceCommandDestination,
    items: Vec<VoiceCommandItemResponse>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSessionResponse {
    access_token: String,
    access_token_expires_at: String,
    refresh_token: String,
    user: MeResponse,
    device: DeviceResponse,
    sync_cursor: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SyncChangesQuery {
    after: i64,
    limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SyncChangeResponse {
    sequence: String,
    entity_type: String,
    entity_id: uuid::Uuid,
    operation: String,
    entity_version: i64,
    changed_at: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SyncChangeListResponse {
    items: Vec<SyncChangeResponse>,
    next_cursor: String,
    current_cursor: String,
    has_more: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SyncStreamQuery {
    after: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCursorEvent {
    cursor: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RefreshSessionRequest {
    #[schema(value_type = String)]
    refresh_token: SecretString,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DeviceRegistrationRequest {
    installation_id: uuid::Uuid,
    #[schema(value_type = String)]
    platform: ClientPlatform,
    name: String,
    app_version: String,
    os_version: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ScheduleRangeQuery {
    from: String,
    to: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecommendationListQuery {
    limit: Option<i64>,
    scope: Option<RecommendationListScope>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecommendationListScope {
    Active,
    All,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DisconnectGoogleCalendarQuery {
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateScheduleRequest {
    client_mutation_id: Option<uuid::Uuid>,
    title: String,
    notes: Option<String>,
    starts_at: String,
    ends_at: String,
    time_zone: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateScheduleRequest {
    title: String,
    notes: Option<String>,
    starts_at: String,
    ends_at: String,
    time_zone: String,
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DeleteScheduleRequest {
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateTaskRequest {
    project_id: Option<uuid::Uuid>,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    priority: i16,
    due_at: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateTaskRequest {
    project_id: Option<uuid::Uuid>,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    status: String,
    priority: i16,
    due_at: Option<String>,
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateProjectRequest {
    workspace_id: uuid::Uuid,
    title: String,
    objective: Option<String>,
    risk_level: i16,
    next_action: Option<String>,
    due_at: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateProjectRequest {
    title: String,
    objective: Option<String>,
    status: String,
    risk_level: i16,
    next_action: Option<String>,
    due_at: Option<String>,
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DeleteProjectRequest {
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateGoalRequest {
    workspace_id: Option<uuid::Uuid>,
    project_id: Option<uuid::Uuid>,
    title: String,
    desired_outcome: String,
    target_at: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateGoalRequest {
    workspace_id: Option<uuid::Uuid>,
    project_id: Option<uuid::Uuid>,
    title: String,
    desired_outcome: String,
    status: String,
    target_at: Option<String>,
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateProjectWebhookRequest {
    provider: String,
    url: String,
    events: Vec<String>,
    mention_directory: Option<WebhookMentionDirectory>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateProjectWebhookRequest {
    provider: String,
    destination_mode: String,
    url: Option<String>,
    events: Vec<String>,
    enabled: bool,
    expected_version: i64,
    mention_directory: Option<WebhookMentionDirectory>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct WebhookMentionDirectory {
    users: BTreeMap<String, String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DeleteProjectWebhookRequest {
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SendWebhookMessageRequest {
    message: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProjectListQuery {
    workspace_id: uuid::Uuid,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TaskListQuery {
    project_id: Option<uuid::Uuid>,
    status: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct VoiceCommandRequest {
    client_mutation_id: Option<uuid::Uuid>,
    text: String,
    reference_at: String,
    time_zone: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateConversationRequest {
    client_conversation_id: uuid::Uuid,
    title: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateAgentTurnRequest {
    client_message_id: uuid::Uuid,
    input: Vec<AgentTurnInput>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResolveAgentActionRequest {
    decision: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateAgentModelRequest {
    model_id: Option<String>,
    reasoning_effort: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AgentTurnInput {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CompleteTaskRequest {
    expected_version: i64,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DeleteTaskRequest {
    expected_version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
    request_id: String,
    retryable: bool,
    details: BTreeMap<String, serde_json::Value>,
}

pub(crate) fn error_response(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    request_id: RequestId,
    retryable: bool,
) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            error: ErrorBody {
                code,
                message,
                request_id: request_id.to_string(),
                retryable,
                details: BTreeMap::new(),
            },
        }),
    )
        .into_response()
}

#[derive(OpenApi)]
#[openapi(
    paths(
        trusted_network_session,
        refresh_session,
        list_sync_changes,
        stream_sync_changes,
        list_schedule_entries,
        get_google_calendar_connection,
        disconnect_google_calendar,
        start_google_calendar_authorization,
        complete_google_calendar_authorization,
        sync_google_calendar,
        list_google_chat_connections,
        start_google_chat_authorization,
        delete_google_chat_connection,
        list_google_chat_spaces,
        list_project_google_chat_sources,
        create_project_google_chat_source,
        delete_project_google_chat_source,
        sync_project_google_chat_source,
        list_project_inflow_items,
        decide_project_inflow_item,
        get_home_snapshot,
        refresh_work_brief,
        list_recommendations,
        decide_recommendation,
        create_schedule_entry,
        update_schedule_entry,
        delete_schedule_entry,
        list_workspaces,
        list_goals,
        create_goal,
        update_goal,
        list_projects,
        create_project,
        update_project,
        delete_project,
        list_project_webhooks,
        create_project_webhook,
        update_project_webhook,
        delete_project_webhook,
        test_project_webhook,
        send_webhook_message,
        list_webhook_deliveries,
        retry_webhook_delivery,
        list_open_tasks,
        create_task,
        get_task,
        update_task,
        delete_task,
        complete_task,
        execute_voice_command,
        list_conversations,
        create_conversation,
        list_conversation_messages,
        stream_conversation_updates,
        get_latest_conversation_job,
        create_agent_turn,
        get_agent_job,
        resolve_agent_action,
        get_agent_authentication,
        request_agent_authentication,
        get_agent_model_settings,
        update_agent_model_settings,
        meetings::list_meetings,
        meetings::create_meeting,
        meetings::get_meeting,
        meetings::reanalyze_meeting,
        meetings::decide_meeting_action,
        live,
        ready,
        me,
        devices,
        push::get_push_registration,
        push::register_push_token,
        push::delete_push_registration
    ),
    components(schemas(
        LiveStatus,
        ReadyStatus,
        CheckStatus,
        LiveHealthResponse,
        ReadinessChecks,
        ReadyHealthResponse,
        MeResponse,
        DeviceResponse,
        DeviceListResponse,
        DeviceSessionResponse,
        SyncChangeResponse,
        SyncChangeListResponse,
        DeviceRegistrationRequest,
        CreateScheduleRequest,
        ScheduleEntryResponse,
        ScheduleListResponse,
        GoogleCalendarConnectionResponse,
        StartGoogleCalendarAuthorizationRequest,
        StartGoogleCalendarAuthorizationResponse,
        GoogleChatAccountResponse,
        GoogleChatAccountListResponse,
        GoogleChatSpaceResponse,
        GoogleChatSpaceListResponse,
        ProjectGoogleChatSourceResponse,
        ProjectGoogleChatSourceListResponse,
        ProjectInflowMessageResponse,
        ProjectInflowItemResponse,
        ProjectInflowItemListResponse,
        CreateProjectGoogleChatSourceRequest,
        DeleteVersionedConnectionQuery,
        ProjectInflowDecisionRequest,
        ProjectInflowListQuery,
        TaskResponse,
        TaskListResponse,
        WorkspaceResponse,
        WorkspaceListResponse,
        ProjectResponse,
        ProjectListResponse,
        ProjectWebhookResponse,
        ProjectWebhookListResponse,
        WebhookDeliveryResponse,
        WebhookDeliveryListResponse,
        VoiceCommandKind,
        VoiceCommandDestination,
        VoiceCommandItemType,
        VoiceCommandItemResponse,
        VoiceCommandResponse,
        HomeSnapshotResponse,
        RecommendationResponse,
        RecommendationListResponse,
        RecommendationDecisionKind,
        RecommendationDecisionRequest,
        ConversationResponse,
        ConversationListResponse,
        QueuedAgentTurnResponse,
        ConversationMessageResponse,
        AssistantPresentationResponse,
        AssistantPresentationSectionResponse,
        AssistantPresentationItemResponse,
        ConversationMessageListResponse,
        AgentJobResponse,
        PendingAgentActionResponse,
        AgentAuthenticationResponse,
        AgentModelResponse,
        AgentReasoningEffortResponse,
        AgentModelSettingsResponse,
        CreateConversationRequest,
        UpdateScheduleRequest,
        DeleteScheduleRequest,
        CreateProjectRequest,
        CreateTaskRequest,
        UpdateProjectRequest,
        DeleteProjectRequest,
        CreateProjectWebhookRequest,
        UpdateProjectWebhookRequest,
        DeleteProjectWebhookRequest,
        WebhookMentionDirectory,
        SendWebhookMessageRequest,
        UpdateTaskRequest,
        DeleteTaskRequest,
        CreateAgentTurnRequest,
        ResolveAgentActionRequest,
        UpdateAgentModelRequest,
        AgentTurnInput,
        ProjectListQuery,
        TaskListQuery,
        CompleteTaskRequest,
        VoiceCommandRequest,
        meetings::CreateMeetingRequest,
        meetings::DecideMeetingActionRequest,
        meetings::MeetingActionDecision,
        meetings::MeetingResponse,
        meetings::MeetingListResponse,
        meetings::MeetingListItemResponse,
        meetings::MeetingDetailResponse,
        meetings::MeetingDecisionResponse,
        meetings::MeetingActionItemResponse,
        meetings::MeetingStatusResponse,
        meetings::MeetingActionKindResponse,
        meetings::MeetingActionStatusResponse,
        push::PushRegistrationResponse,
        push::RegisterPushTokenRequest
    )),
    tags((name = "health", description = "Process and dependency health"))
)]
struct ApiDoc;

#[must_use]
pub fn openapi_document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[allow(clippy::too_many_lines)] // The router is an auditable registry of public API surfaces.
pub fn router(state: ApiState) -> Router {
    let router = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .merge(
            calendar_router()
                .merge(google_chat_router())
                .merge(push_router())
                .merge(sync_router()),
        )
        .route("/v1/auth/refresh", axum::routing::post(refresh_session))
        .route(
            "/v1/access/session",
            axum::routing::post(trusted_network_session),
        )
        .route(
            "/v1/schedule-entries",
            get(list_schedule_entries).post(create_schedule_entry),
        )
        .route(
            "/v1/schedule-entries/{schedule_entry_id}",
            axum::routing::put(update_schedule_entry).delete(delete_schedule_entry),
        )
        .route("/v1/home", get(get_home_snapshot))
        .route("/v1/briefs/work/refresh", post(refresh_work_brief))
        .route("/v1/recommendations", get(list_recommendations))
        .route(
            "/v1/recommendations/{recommendation_id}/decisions",
            post(decide_recommendation),
        )
        .route("/v1/workspaces", get(list_workspaces))
        .merge(goal_router())
        .route("/v1/projects", get(list_projects).post(create_project))
        .route(
            "/v1/projects/{project_id}",
            axum::routing::put(update_project).delete(delete_project),
        )
        .merge(webhook_router())
        .route("/v1/tasks", get(list_open_tasks).post(create_task))
        .route(
            "/v1/tasks/{task_id}",
            get(get_task).put(update_task).delete(delete_task),
        )
        .route(
            "/v1/tasks/{task_id}/complete",
            axum::routing::post(complete_task),
        )
        .route(
            "/v1/assistant/voice-commands",
            axum::routing::post(execute_voice_command),
        )
        .route(
            "/v1/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/v1/conversations/{conversation_id}/turns",
            axum::routing::post(create_agent_turn),
        )
        .route(
            "/v1/conversations/{conversation_id}/messages",
            get(list_conversation_messages),
        )
        .route(
            "/v1/conversations/{conversation_id}/stream",
            get(stream_conversation_updates),
        )
        .route(
            "/v1/conversations/{conversation_id}/jobs/latest",
            get(get_latest_conversation_job),
        )
        .route(
            "/v1/agent/authentication",
            get(get_agent_authentication).post(request_agent_authentication),
        )
        .route(
            "/v1/agent/models",
            get(get_agent_model_settings).put(update_agent_model_settings),
        )
        .route("/v1/agent/jobs/{job_id}", get(get_agent_job))
        .route(
            "/v1/agent/jobs/{job_id}/approval",
            axum::routing::post(resolve_agent_action),
        )
        .route("/v1/me", get(me))
        .route("/v1/devices", get(devices))
        .merge(meetings::routes());

    let allowed_origins = allowed_client_origins(state.trusted_network());

    router
        .fallback(not_found)
        .with_state(state)
        .layer(
            CorsLayer::new()
                // The desktop and mobile WebViews use fixed Tauri origins.
                // A loopback-only trusted-network deployment additionally
                // permits the local Vite dev server for desktop app testing.
                // Do not widen this to arbitrary web origins: this API accepts
                // bearer tokens from the installed personal client.
                .allow_origin(allowed_origins)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                ]),
        )
        .layer(middleware::from_fn(request_context))
}

fn webhook_router() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/projects/{project_id}/webhooks",
            get(list_project_webhooks).post(create_project_webhook),
        )
        .route(
            "/v1/projects/{project_id}/webhooks/{webhook_id}",
            axum::routing::put(update_project_webhook).delete(delete_project_webhook),
        )
        .route(
            "/v1/projects/{project_id}/webhooks/{webhook_id}/test",
            post(test_project_webhook),
        )
        .route(
            "/v1/projects/{project_id}/webhooks/{webhook_id}/messages",
            post(send_webhook_message),
        )
        .route(
            "/v1/projects/{project_id}/webhook-deliveries",
            get(list_webhook_deliveries),
        )
        .route(
            "/v1/projects/{project_id}/webhook-deliveries/{delivery_id}/retry",
            post(retry_webhook_delivery),
        )
}

fn push_router() -> Router<ApiState> {
    Router::new().route(
        "/v1/push/registration",
        get(push::get_push_registration)
            .put(push::register_push_token)
            .delete(push::delete_push_registration),
    )
}

fn sync_router() -> Router<ApiState> {
    Router::new()
        .route("/v1/sync/changes", get(list_sync_changes))
        .route("/v1/sync/stream", get(stream_sync_changes))
}

fn goal_router() -> Router<ApiState> {
    Router::new()
        .route("/v1/goals", get(list_goals).post(create_goal))
        .route("/v1/goals/{goal_id}", axum::routing::put(update_goal))
}

fn calendar_router() -> Router<ApiState> {
    Router::new()
        .route(
            "/oauth/google/calendar/callback",
            get(complete_google_calendar_authorization),
        )
        .route(
            "/v1/calendar/connections/google",
            get(get_google_calendar_connection).delete(disconnect_google_calendar),
        )
        .route(
            "/v1/calendar/connections/google/authorizations",
            post(start_google_calendar_authorization),
        )
        .route(
            "/v1/calendar/connections/google/sync",
            post(sync_google_calendar),
        )
}

fn google_chat_router() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/google-chat/connections",
            get(list_google_chat_connections),
        )
        .route(
            "/v1/google-chat/connections/authorizations",
            post(start_google_chat_authorization),
        )
        .route(
            "/v1/google-chat/connections/{account_id}",
            axum::routing::delete(delete_google_chat_connection),
        )
        .route(
            "/v1/google-chat/connections/{account_id}/spaces",
            get(list_google_chat_spaces),
        )
        .route(
            "/v1/projects/{project_id}/google-chat-sources",
            get(list_project_google_chat_sources).post(create_project_google_chat_source),
        )
        .route(
            "/v1/projects/{project_id}/google-chat-sources/{source_id}",
            axum::routing::delete(delete_project_google_chat_source),
        )
        .route(
            "/v1/projects/{project_id}/google-chat-sources/{source_id}/sync",
            post(sync_project_google_chat_source),
        )
        .route(
            "/v1/projects/{project_id}/inflow",
            get(list_project_inflow_items),
        )
        .route(
            "/v1/projects/{project_id}/inflow/{item_id}/decision",
            post(decide_project_inflow_item),
        )
}

fn allowed_client_origins(trusted_network: bool) -> Vec<HeaderValue> {
    let mut origins = vec![
        HeaderValue::from_static("tauri://localhost"),
        HeaderValue::from_static("http://tauri.localhost"),
        HeaderValue::from_static("https://tauri.localhost"),
    ];
    if trusted_network {
        origins.extend([
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://127.0.0.1:1420"),
        ]);
    }
    origins
}

/// Serves the router until the supplied shutdown future resolves.
///
/// # Errors
///
/// Returns the listener error produced while accepting or serving a connection.
pub async fn serve_with_shutdown<F>(
    listener: TcpListener,
    app: Router,
    shutdown: F,
) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
}

const CALENDAR_SYNC_INITIAL_DELAY: Duration = Duration::from_secs(30);
const CALENDAR_SYNC_INTERVAL: Duration = Duration::from_mins(5);
const CALENDAR_MUTATION_INTERVAL: Duration = Duration::from_secs(2);
const GOOGLE_CHAT_SYNC_INITIAL_DELAY: Duration = Duration::from_secs(20);
const GOOGLE_CHAT_SYNC_INTERVAL: Duration = Duration::from_mins(1);

/// Starts the single-process Google Calendar reconciliation loop when both
/// storage and provider configuration are available. The loop processes
/// accounts sequentially to avoid a provider burst and never logs owner IDs,
/// credentials, sync tokens, or event content.
#[must_use]
pub fn spawn_calendar_sync_worker(state: &ApiState) -> Option<tokio::task::JoinHandle<()>> {
    let planning = state.planning()?.clone();
    let calendar_oauth = Arc::clone(state.calendar_oauth()?);
    Some(tokio::spawn(async move {
        tokio::time::sleep(CALENDAR_SYNC_INITIAL_DELAY).await;
        loop {
            if let Ok(identities) = planning.active_calendar_sync_identities().await {
                for identity in identities {
                    if let Err(error) = synchronize_google_calendar(
                        &planning,
                        &calendar_oauth,
                        identity.account_id,
                        identity.user_id,
                    )
                    .await
                    {
                        let _ = planning
                            .mark_calendar_sync_failure(
                                identity.account_id,
                                identity.user_id,
                                error.failure_code(),
                            )
                            .await;
                        warn!(
                            event = "calendar.periodic_sync_failed",
                            error_code = error.failure_code(),
                            retryable = error.retryable()
                        );
                    }
                }
            } else {
                warn!(
                    event = "calendar.periodic_sync_deferred",
                    error_code = "storage.persistence_unavailable"
                );
            }
            tokio::time::sleep(CALENDAR_SYNC_INTERVAL).await;
        }
    }))
}

/// Periodically imports new messages from every enabled project Chat source.
/// Sources are processed sequentially to keep provider usage bounded.
#[must_use]
pub fn spawn_google_chat_sync_worker(state: &ApiState) -> Option<tokio::task::JoinHandle<()>> {
    let planning = state.planning()?.clone();
    let runtime = Arc::clone(state.google_chat_oauth()?);
    Some(tokio::spawn(async move {
        tokio::time::sleep(GOOGLE_CHAT_SYNC_INITIAL_DELAY).await;
        loop {
            if let Ok(source_ids) = planning.active_google_chat_source_ids().await {
                for source_id in source_ids {
                    if let Err(error) =
                        synchronize_google_chat_source(&planning, &runtime, source_id, None).await
                    {
                        let _ = planning
                            .mark_google_chat_source_failure(
                                source_id,
                                error.failure_code(),
                                error.reauth_required(),
                            )
                            .await;
                        warn!(
                            event = "google_chat.periodic_sync_failed",
                            error_code = error.failure_code(),
                            retryable = error.retryable()
                        );
                    }
                }
            } else {
                warn!(
                    event = "google_chat.periodic_sync_deferred",
                    error_code = "storage.persistence_unavailable"
                );
            }
            tokio::time::sleep(GOOGLE_CHAT_SYNC_INTERVAL).await;
        }
    }))
}

/// Starts the durable Google mutation loop. A database lease is acquired
/// before every provider call, so restart and multi-process recovery cannot
/// dispatch the same journal row concurrently.
#[must_use]
pub fn spawn_calendar_mutation_worker(state: &ApiState) -> Option<tokio::task::JoinHandle<()>> {
    let planning = state.planning()?.clone();
    let calendar_oauth = Arc::clone(state.calendar_oauth()?);
    let worker_id = format!("calendar-mutation-{}", uuid::Uuid::now_v7());
    Some(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        loop {
            let _ = planning
                .resolve_unavailable_schedule_calendar_mutations()
                .await;
            if let Ok(mutations) = planning
                .claim_schedule_calendar_mutations(&worker_id, 1)
                .await
            {
                for mutation in mutations {
                    let connection = match planning
                        .begin_schedule_calendar_mutation_dispatch(
                            mutation.id,
                            mutation.attempt_count,
                            &worker_id,
                        )
                        .await
                    {
                        Ok(Some(connection)) => connection,
                        Ok(None) => continue,
                        Err(_) => {
                            let _ = planning
                                .fail_schedule_calendar_mutation(
                                    mutation.id,
                                    mutation.attempt_count,
                                    &worker_id,
                                    "calendar.provider_unavailable",
                                    true,
                                )
                                .await;
                            continue;
                        }
                    };
                    let result = calendar_oauth
                        .dispatch_schedule_calendar_mutation(&connection, &mutation)
                        .await;
                    match result {
                        Ok(provider_etag) => {
                            let _ = planning
                                .complete_schedule_calendar_mutation(
                                    mutation.id,
                                    mutation.attempt_count,
                                    &worker_id,
                                    provider_etag.as_deref(),
                                )
                                .await;
                        }
                        Err(error) => {
                            let _ = planning
                                .fail_schedule_calendar_mutation(
                                    mutation.id,
                                    mutation.attempt_count,
                                    &worker_id,
                                    error.failure_code(),
                                    error.retryable(),
                                )
                                .await;
                            warn!(
                                event = "calendar.mutation_failed",
                                error_code = error.failure_code(),
                                retryable = error.retryable(),
                                attempt = mutation.attempt_count
                            );
                        }
                    }
                }
            } else {
                warn!(
                    event = "calendar.mutation_deferred",
                    error_code = "storage.persistence_unavailable"
                );
            }
            tokio::time::sleep(CALENDAR_MUTATION_INTERVAL).await;
        }
    }))
}

/// Starts the durable project-webhook delivery loop. Claims are bounded and
/// each failure is persisted with exponential backoff before another claim.
#[must_use]
pub fn spawn_webhook_delivery_worker(state: &ApiState) -> Option<tokio::task::JoinHandle<()>> {
    let planning = state.planning()?.clone();
    let runtime = Arc::clone(state.webhook()?);
    let worker_id = format!("webhook-delivery-{}", uuid::Uuid::now_v7());
    Some(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        loop {
            if let Ok(deliveries) = planning.claim_webhook_deliveries(&worker_id, 10).await {
                for delivery in deliveries {
                    match runtime.deliver(&delivery).await {
                        Ok(result) => {
                            let _ = planning
                                .complete_webhook_delivery(
                                    delivery.id,
                                    &worker_id,
                                    delivery.attempt_count,
                                    result.response_code,
                                )
                                .await;
                        }
                        Err(error) => {
                            let response_code = match error {
                                webhook::WebhookRuntimeError::Rejected(code) => Some(code),
                                _ => None,
                            };
                            let _ = planning
                                .fail_webhook_delivery(
                                    delivery.id,
                                    &worker_id,
                                    delivery.attempt_count,
                                    response_code,
                                    error.code(),
                                )
                                .await;
                            warn!(
                                event = "webhook.delivery_failed",
                                error_code = error.code(),
                                attempt = delivery.attempt_count
                            );
                        }
                    }
                }
            } else {
                warn!(
                    event = "webhook.delivery_deferred",
                    error_code = "storage.persistence_unavailable"
                );
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }))
}

/// Starts the durable FCM reminder loop when storage and Firebase credentials
/// are both available. Reminder content and registration tokens are never
/// included in logs.
#[must_use]
pub fn spawn_push_delivery_worker(state: &ApiState) -> Option<tokio::task::JoinHandle<()>> {
    let planning = state.planning()?.clone();
    let runtime = Arc::clone(state.push()?);
    let worker_id = format!("push-delivery-{}", uuid::Uuid::now_v7());
    Some(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        loop {
            if planning
                .queue_due_push_reminders(OffsetDateTime::now_utc())
                .await
                .is_err()
            {
                warn!(
                    event = "push.reconciliation_deferred",
                    error_code = "storage.persistence_unavailable"
                );
            } else if let Ok(deliveries) = planning.claim_push_deliveries(&worker_id, 10).await {
                for delivery in deliveries {
                    match runtime.deliver(&delivery).await {
                        Ok(response_code) => {
                            let _ = planning
                                .complete_push_delivery(
                                    delivery.id,
                                    &worker_id,
                                    delivery.attempt_count,
                                    response_code,
                                )
                                .await;
                        }
                        Err(error) => {
                            let _ = planning
                                .fail_push_delivery(
                                    delivery.id,
                                    &worker_id,
                                    delivery.attempt_count,
                                    error.response_code(),
                                    error.code(),
                                    error.retryable(),
                                    error.invalidates_token(),
                                )
                                .await;
                            warn!(
                                event = "push.delivery_failed",
                                error_code = error.code(),
                                retryable = error.retryable(),
                                attempt = delivery.attempt_count
                            );
                        }
                    }
                }
            } else {
                warn!(
                    event = "push.delivery_deferred",
                    error_code = "storage.persistence_unavailable"
                );
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }))
}

#[utoipa::path(
    get,
    path = "/health/live",
    tag = "health",
    responses((status = 200, description = "The API event loop is responding", body = LiveHealthResponse))
)]
async fn live(State(state): State<ApiState>) -> Json<LiveHealthResponse> {
    Json(LiveHealthResponse {
        status: LiveStatus::Ok,
        service: "api",
        build_sha: state.build_sha.to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "The API is ready to receive traffic", body = ReadyHealthResponse),
        (status = 503, description = "A required dependency is not ready", body = ReadyHealthResponse)
    )
)]
async fn ready(State(state): State<ApiState>) -> (StatusCode, Json<ReadyHealthResponse>) {
    let configuration = if state.configuration_ready {
        CheckStatus::Ok
    } else {
        CheckStatus::Error
    };

    let storage_readiness = match &state.database {
        Some(database) if state.configuration_ready => {
            database.check(state.expected_schema_version).await
        }
        _ => Readiness::DatabaseUnavailable,
    };

    let (database, migrations) = match storage_readiness {
        Readiness::Ready { .. } => (CheckStatus::Ok, CheckStatus::Ok),
        Readiness::DatabaseUnavailable => (CheckStatus::Error, CheckStatus::Error),
        Readiness::SchemaUnavailable | Readiness::SchemaMismatch { .. } => {
            (CheckStatus::Ok, CheckStatus::Error)
        }
    };

    let is_ready = configuration == CheckStatus::Ok
        && database == CheckStatus::Ok
        && migrations == CheckStatus::Ok;
    let response = ReadyHealthResponse {
        status: if is_ready {
            ReadyStatus::Ready
        } else {
            ReadyStatus::NotReady
        },
        checks: ReadinessChecks {
            configuration,
            database,
            migrations,
        },
        schema_version: state.expected_schema_version,
    };

    (
        if is_ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(response),
    )
}

#[utoipa::path(
    get,
    path = "/v1/me",
    tag = "identity",
    responses(
        (status = 200, description = "Current authenticated profile", body = MeResponse),
        (status = 401, description = "Session is absent, invalid, or expired"),
        (status = 503, description = "Authentication storage is temporarily unavailable")
    )
)]
async fn me(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    request: Request,
) -> Response {
    let principal = match auth::authenticate(&state, request.headers()).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(authentication) = state.authentication() else {
        return auth::AuthenticationFailure::Unavailable.into_response(request_id);
    };
    let profile = match authentication
        .repository()
        .profile_for_user(principal.identity().user_id())
        .await
    {
        Ok(Some(profile)) => profile,
        Ok(None) => return auth::AuthenticationFailure::Unauthorized.into_response(request_id),
        Err(_) => return auth::AuthenticationFailure::Unavailable.into_response(request_id),
    };
    Json(me_response(profile)).into_response()
}

#[utoipa::path(
    get,
    path = "/v1/devices",
    tag = "identity",
    responses(
        (status = 200, description = "Devices owned by the current user", body = DeviceListResponse),
        (status = 401, description = "Session is absent, invalid, or expired"),
        (status = 503, description = "Authentication storage is temporarily unavailable")
    )
)]
async fn devices(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    request: Request,
) -> Response {
    let principal = match auth::authenticate(&state, request.headers()).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(authentication) = state.authentication() else {
        return auth::AuthenticationFailure::Unavailable.into_response(request_id);
    };
    let Ok(devices) = authentication
        .repository()
        .devices_for_user(principal.identity().user_id())
        .await
    else {
        return auth::AuthenticationFailure::Unavailable.into_response(request_id);
    };
    Json(DeviceListResponse {
        items: devices.into_iter().map(device_response).collect(),
        next_cursor: None,
    })
    .into_response()
}

#[utoipa::path(
    post,
    path = "/v1/briefs/work/refresh",
    tag = "intelligence",
    responses((status = 200, body = RecommendationListResponse), (status = 401), (status = 503))
)]
async fn refresh_work_brief(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let recommendations = match planning
        .refresh_work_brief(principal.identity().user_id(), OffsetDateTime::now_utc())
        .await
    {
        Ok(recommendations) => recommendations,
        Err(error) => return storage_error_response(&error, request_id),
    };
    let Ok(items) = recommendations
        .into_iter()
        .map(recommendation_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    Json(RecommendationListResponse { items }).into_response()
}

#[utoipa::path(
    get,
    path = "/v1/schedule-entries",
    tag = "planning",
    params(("from" = String, Query), ("to" = String, Query)),
    responses((status = 200, body = ScheduleListResponse), (status = 400), (status = 401), (status = 503))
)]
async fn list_schedule_entries(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    axum::extract::Query(query): axum::extract::Query<ScheduleRangeQuery>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let (Ok(from), Ok(to)) = (
        OffsetDateTime::parse(&query.from, &Rfc3339),
        OffsetDateTime::parse(&query.to, &Rfc3339),
    ) else {
        return invalid_request_response(request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .schedule_entries_in_range(principal.identity().user_id(), from, to)
        .await
    {
        Ok(entries) => match entries
            .into_iter()
            .map(schedule_entry_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(ScheduleListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_google_schedule_entry(
    _state: &ApiState,
    planning: &Database,
    user_id: uuid::Uuid,
    target: jimin_storage::calendar::PrimaryCalendarMutationTarget,
    body: &CreateScheduleRequest,
    starts_at: OffsetDateTime,
    ends_at: OffsetDateTime,
    request_id: RequestId,
) -> Response {
    match planning
        .create_schedule_entry_with_calendar_outbox(
            &NewScheduleEntry {
                id: body.client_mutation_id.unwrap_or_else(uuid::Uuid::now_v7),
                user_id,
                title: body.title.clone(),
                notes: body.notes.clone(),
                starts_at,
                ends_at,
                time_zone: body.time_zone.clone(),
            },
            &target,
        )
        .await
    {
        Ok(entry) => match schedule_entry_response(entry) {
            Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/home",
    tag = "home",
    params(("from" = String, Query), ("to" = String, Query)),
    responses((status = 200, body = HomeSnapshotResponse), (status = 400), (status = 401), (status = 503))
)]
async fn get_home_snapshot(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    axum::extract::Query(query): axum::extract::Query<ScheduleRangeQuery>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let (Ok(from), Ok(to)) = (
        OffsetDateTime::parse(&query.from, &Rfc3339),
        OffsetDateTime::parse(&query.to, &Rfc3339),
    ) else {
        return invalid_request_response(request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let Some(deadline_boundary) = to.checked_add(TimeDuration::days(1)) else {
        return invalid_request_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let (schedule, tasks, due_tasks, recommendations, inflow_items, webhooks) = match tokio::try_join!(
        planning.schedule_entries_in_range(user_id, from, to),
        planning.home_tasks_for_user(user_id, to),
        planning.deadline_tasks_for_user(user_id, deadline_boundary),
        planning.active_recommendations_for_user(user_id, OffsetDateTime::now_utc(), 5),
        planning.pending_project_inflow_for_user(user_id),
        planning.user_project_webhooks(user_id),
    ) {
        Ok(values) => values,
        Err(error) => return storage_error_response(&error, request_id),
    };
    let Ok(schedule) = schedule
        .into_iter()
        .map(schedule_entry_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    let Ok(tasks) = tasks
        .into_iter()
        .map(task_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    let Ok(due_tasks) = due_tasks
        .into_iter()
        .map(task_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    let Ok(recommendations) = recommendations
        .into_iter()
        .map(recommendation_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    let contexts = inflow_assignment_contexts(webhooks);
    let Ok(mut inflow) = group_project_inflow_candidates(inflow_items)
        .into_iter()
        .map(project_inflow_item_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    for item in &mut inflow {
        apply_inflow_assignment_context(item, &contexts);
    }
    Json(HomeSnapshotResponse {
        schedule,
        tasks,
        due_tasks,
        inflow,
        // Handled Chat decisions live in project history. Preserve the
        // response field for installed clients without putting completed
        // source messages back on the attention-focused home screen.
        recent_inflow: Vec::new(),
        recommendations,
    })
    .into_response()
}

#[utoipa::path(
    get,
    path = "/v1/recommendations",
    tag = "intelligence",
    params(
        ("limit" = Option<i64>, Query, description = "Maximum recommendations, 1 to 50"),
        ("scope" = Option<String>, Query, description = "active or all")
    ),
    responses((status = 200, body = RecommendationListResponse), (status = 400), (status = 401), (status = 503))
)]
async fn list_recommendations(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    Query(query): Query<RecommendationListQuery>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let limit = query.limit.unwrap_or(20);
    let recommendations = match query.scope.unwrap_or(RecommendationListScope::Active) {
        RecommendationListScope::Active => {
            planning
                .active_recommendations_for_user(user_id, OffsetDateTime::now_utc(), limit)
                .await
        }
        RecommendationListScope::All => {
            planning
                .recommendation_history_for_user(user_id, limit)
                .await
        }
    };
    let recommendations = match recommendations {
        Ok(recommendations) => recommendations,
        Err(error) => return storage_error_response(&error, request_id),
    };
    let Ok(items) = recommendations
        .into_iter()
        .map(recommendation_response)
        .collect::<Result<Vec<_>, _>>()
    else {
        return unavailable_response(request_id);
    };
    Json(RecommendationListResponse { items }).into_response()
}

#[utoipa::path(
    post,
    path = "/v1/recommendations/{recommendation_id}/decisions",
    tag = "intelligence",
    params(("recommendation_id" = uuid::Uuid, Path)),
    request_body = RecommendationDecisionRequest,
    responses((status = 200, body = RecommendationResponse), (status = 400), (status = 401), (status = 404), (status = 409), (status = 503))
)]
async fn decide_recommendation(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    Path(recommendation_id): Path<uuid::Uuid>,
    headers: HeaderMap,
    Json(body): Json<RecommendationDecisionRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let revisit_at = match body.revisit_at.as_deref() {
        Some(value) => match OffsetDateTime::parse(value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    let outcome = planning
        .decide_recommendation(&DecideRecommendation {
            id: body.client_mutation_id,
            user_id: principal.identity().user_id(),
            recommendation_id,
            decision: match body.decision {
                RecommendationDecisionKind::Approve => RecommendationDecision::Approve,
                RecommendationDecisionKind::Reject => RecommendationDecision::Reject,
                RecommendationDecisionKind::Defer => RecommendationDecision::Defer,
                RecommendationDecisionKind::RequestAnalysis => {
                    RecommendationDecision::RequestAnalysis
                }
            },
            reason: body.reason,
            revisit_at,
            expected_version: body.expected_version,
        })
        .await;
    let recommendation = match outcome {
        Ok(
            DecideRecommendationOutcome::Applied(recommendation)
            | DecideRecommendationOutcome::Replayed(recommendation),
        ) => recommendation,
        Ok(DecideRecommendationOutcome::NotFound) => {
            return recommendation_not_found_response(request_id);
        }
        Ok(DecideRecommendationOutcome::VersionConflict) => {
            return recommendation_conflict_response(request_id);
        }
        Err(error) => return storage_error_response(&error, request_id),
    };
    match recommendation_response(recommendation) {
        Ok(response) => Json(response).into_response(),
        Err(()) => unavailable_response(request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/schedule-entries",
    tag = "planning",
    request_body = CreateScheduleRequest,
    responses((status = 201, body = ScheduleEntryResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_schedule_entry(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<CreateScheduleRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    if body
        .client_mutation_id
        .is_some_and(|id| id.get_version_num() != 7)
    {
        return invalid_request_response(request_id);
    }
    let (Ok(starts_at), Ok(ends_at)) = (
        OffsetDateTime::parse(&body.starts_at, &Rfc3339),
        OffsetDateTime::parse(&body.ends_at, &Rfc3339),
    ) else {
        return invalid_request_response(request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .primary_calendar_mutation_target(principal.identity().user_id())
        .await
    {
        Ok(Some(target)) => {
            return create_google_schedule_entry(
                &state,
                planning,
                principal.identity().user_id(),
                target,
                &body,
                starts_at,
                ends_at,
                request_id,
            )
            .await;
        }
        Ok(None) => {}
        Err(error) => return storage_error_response(&error, request_id),
    }
    match planning
        .create_schedule_entry(&NewScheduleEntry {
            id: body.client_mutation_id.unwrap_or_else(uuid::Uuid::now_v7),
            user_id: principal.identity().user_id(),
            title: body.title,
            notes: body.notes,
            starts_at,
            ends_at,
            time_zone: body.time_zone,
        })
        .await
    {
        Ok(entry) => match schedule_entry_response(entry) {
            Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/schedule-entries/{schedule_entry_id}",
    tag = "planning",
    params(("schedule_entry_id" = String, Path)),
    request_body = UpdateScheduleRequest,
    responses((status = 200, body = ScheduleEntryResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn update_schedule_entry(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(schedule_entry_id): Path<uuid::Uuid>,
    Json(body): Json<UpdateScheduleRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let (Ok(starts_at), Ok(ends_at)) = (
        OffsetDateTime::parse(&body.starts_at, &Rfc3339),
        OffsetDateTime::parse(&body.ends_at, &Rfc3339),
    ) else {
        return invalid_request_response(request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .update_schedule_entry(&ScheduleEntryUpdate {
            id: schedule_entry_id,
            user_id: principal.identity().user_id(),
            title: body.title.clone(),
            notes: body.notes.clone(),
            starts_at,
            ends_at,
            time_zone: body.time_zone.clone(),
            expected_version: body.expected_version,
        })
        .await
    {
        Ok(Some(entry)) => match schedule_entry_response(entry) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => {
            update_google_schedule_entry(
                &state,
                planning,
                principal.identity().user_id(),
                schedule_entry_id,
                &body,
                starts_at,
                ends_at,
                request_id,
            )
            .await
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/schedule-entries/{schedule_entry_id}",
    tag = "planning",
    params(("schedule_entry_id" = String, Path)),
    request_body = DeleteScheduleRequest,
    responses((status = 204), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn delete_schedule_entry(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(schedule_entry_id): Path<uuid::Uuid>,
    Json(body): Json<DeleteScheduleRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .cancel_schedule_entry(
            principal.identity().user_id(),
            schedule_entry_id,
            body.expected_version,
        )
        .await
    {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None) => {
            delete_google_schedule_entry(
                &state,
                planning,
                principal.identity().user_id(),
                schedule_entry_id,
                body.expected_version,
                request_id,
            )
            .await
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[allow(clippy::too_many_arguments)]
async fn update_google_schedule_entry(
    state: &ApiState,
    planning: &Database,
    user_id: uuid::Uuid,
    schedule_entry_id: uuid::Uuid,
    body: &UpdateScheduleRequest,
    starts_at: OffsetDateTime,
    ends_at: OffsetDateTime,
    request_id: RequestId,
) -> Response {
    let target = match planning
        .calendar_event_mutation_target(user_id, schedule_entry_id, body.expected_version)
        .await
    {
        Ok(Some(target)) => target,
        Ok(None) => return schedule_conflict_response(request_id),
        Err(error) => return storage_error_response(&error, request_id),
    };
    let Some(calendar_oauth) = state.calendar_oauth() else {
        return calendar_oauth_error_response(CalendarOAuthError::Configuration, request_id);
    };
    let connection = match planning
        .calendar_sync_connection(target.account_id, user_id)
        .await
    {
        Ok(Some(connection)) => connection,
        Ok(None) => return schedule_conflict_response(request_id),
        Err(error) => return storage_error_response(&error, request_id),
    };
    let mutation = jimin_google::GoogleCalendarEventMutation {
        title: body.title.clone(),
        description: body.notes.clone(),
        start: starts_at,
        end: ends_at,
        time_zone: body.time_zone.clone(),
    };
    if let Err(error) = calendar_oauth
        .update_calendar_event(&connection, &target, mutation)
        .await
    {
        return calendar_oauth_error_response(error, request_id);
    }
    if let Err(error) =
        synchronize_google_calendar(planning, calendar_oauth, target.account_id, user_id).await
    {
        return calendar_oauth_error_response(error, request_id);
    }
    match planning
        .schedule_entry_by_id(user_id, schedule_entry_id)
        .await
    {
        Ok(Some(entry)) => match schedule_entry_response(entry) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => schedule_conflict_response(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn delete_google_schedule_entry(
    state: &ApiState,
    planning: &Database,
    user_id: uuid::Uuid,
    schedule_entry_id: uuid::Uuid,
    expected_version: i64,
    request_id: RequestId,
) -> Response {
    let target = match planning
        .calendar_event_mutation_target(user_id, schedule_entry_id, expected_version)
        .await
    {
        Ok(Some(target)) => target,
        Ok(None) => return schedule_conflict_response(request_id),
        Err(error) => return storage_error_response(&error, request_id),
    };
    let Some(calendar_oauth) = state.calendar_oauth() else {
        return calendar_oauth_error_response(CalendarOAuthError::Configuration, request_id);
    };
    let connection = match planning
        .calendar_sync_connection(target.account_id, user_id)
        .await
    {
        Ok(Some(connection)) => connection,
        Ok(None) => return schedule_conflict_response(request_id),
        Err(error) => return storage_error_response(&error, request_id),
    };
    if let Err(error) = calendar_oauth
        .delete_calendar_event(&connection, &target)
        .await
    {
        return calendar_oauth_error_response(error, request_id);
    }
    if let Err(error) =
        synchronize_google_calendar(planning, calendar_oauth, target.account_id, user_id).await
    {
        return calendar_oauth_error_response(error, request_id);
    }
    StatusCode::NO_CONTENT.into_response()
}

fn schedule_conflict_response(request_id: RequestId) -> Response {
    error_response(
        StatusCode::CONFLICT,
        "schedule.version_conflict",
        "일정이 다른 곳에서 변경됐어요. 최신 상태를 확인한 뒤 다시 시도해 주세요.",
        request_id,
        false,
    )
}

fn recommendation_not_found_response(request_id: RequestId) -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "recommendation.not_found",
        "제안을 찾을 수 없어요. 최신 브리핑을 다시 확인해 주세요.",
        request_id,
        false,
    )
}

fn recommendation_conflict_response(request_id: RequestId) -> Response {
    error_response(
        StatusCode::CONFLICT,
        "recommendation.version_conflict",
        "제안 상태가 이미 변경됐어요. 최신 브리핑을 다시 확인해 주세요.",
        request_id,
        false,
    )
}

#[utoipa::path(
    get,
    path = "/v1/goals",
    tag = "work",
    responses((status = 200, body = GoalListResponse), (status = 401), (status = 503))
)]
async fn list_goals(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .goal_overviews_for_user(
            principal.identity().user_id(),
            time::OffsetDateTime::now_utc(),
        )
        .await
    {
        Ok(goals) => match goals
            .into_iter()
            .map(goal_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(GoalListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/goals",
    tag = "work",
    request_body = CreateGoalRequest,
    responses((status = 201, body = GoalResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_goal(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<CreateGoalRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Ok(target_at) = parse_optional_timestamp(body.target_at) else {
        return invalid_request_response(request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .create_goal(&NewGoal {
            id: uuid::Uuid::now_v7(),
            user_id: principal.identity().user_id(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            title: body.title,
            desired_outcome: body.desired_outcome,
            target_at,
        })
        .await
    {
        Ok(goal) => match planning
            .goal_overview_for_user(
                principal.identity().user_id(),
                goal.id,
                time::OffsetDateTime::now_utc(),
            )
            .await
        {
            Ok(Some(overview)) => match goal_response(overview) {
                Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
                Err(()) => unavailable_response(request_id),
            },
            Ok(None) | Err(_) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/goals/{goal_id}",
    tag = "work",
    params(("goal_id" = String, Path)),
    request_body = UpdateGoalRequest,
    responses((status = 200, body = GoalResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn update_goal(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(goal_id): Path<uuid::Uuid>,
    Json(body): Json<UpdateGoalRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let status = match body.status.as_str() {
        "active" => GoalStatus::Active,
        "paused" => GoalStatus::Paused,
        "achieved" => GoalStatus::Achieved,
        "cancelled" => GoalStatus::Cancelled,
        _ => return invalid_request_response(request_id),
    };
    let Ok(target_at) = parse_optional_timestamp(body.target_at) else {
        return invalid_request_response(request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .update_goal(&GoalUpdate {
            id: goal_id,
            user_id: principal.identity().user_id(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            title: body.title,
            desired_outcome: body.desired_outcome,
            status,
            target_at,
            expected_version: body.expected_version,
        })
        .await
    {
        Ok(Some(goal)) => match planning
            .goal_overview_for_user(
                principal.identity().user_id(),
                goal.id,
                time::OffsetDateTime::now_utc(),
            )
            .await
        {
            Ok(Some(overview)) => match goal_response(overview) {
                Ok(response) => Json(response).into_response(),
                Err(()) => unavailable_response(request_id),
            },
            Ok(None) | Err(_) => unavailable_response(request_id),
        },
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "goal.version_conflict",
            "목표가 다른 곳에서 변경됐어요. 최신 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/workspaces",
    tag = "work",
    responses((status = 200, body = WorkspaceListResponse), (status = 401), (status = 503))
)]
async fn list_workspaces(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .workspaces_for_user(principal.identity().user_id())
        .await
    {
        Ok(workspaces) => Json(WorkspaceListResponse {
            items: workspaces.into_iter().map(workspace_response).collect(),
            next_cursor: None,
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/projects",
    tag = "work",
    params(("workspaceId" = String, Query)),
    responses((status = 200, body = ProjectListResponse), (status = 400), (status = 401), (status = 503))
)]
async fn list_projects(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    axum::extract::Query(query): axum::extract::Query<ProjectListQuery>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .projects_for_workspace(principal.identity().user_id(), query.workspace_id)
        .await
    {
        Ok(projects) => match projects
            .into_iter()
            .map(project_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(ProjectListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects",
    tag = "work",
    request_body = CreateProjectRequest,
    responses((status = 201, body = ProjectResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_project(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let due_at = match body.due_at {
        Some(value) => match OffsetDateTime::parse(&value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match planning
        .create_project(&NewProject {
            id: uuid::Uuid::now_v7(),
            user_id,
            workspace_id: body.workspace_id,
            title: body.title,
            objective: body.objective,
            risk_level: body.risk_level,
            next_action: body.next_action,
            due_at,
        })
        .await
    {
        Ok(project) => match project_response(project) {
            Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/projects/{project_id}",
    tag = "work",
    params(("project_id" = String, Path)),
    request_body = UpdateProjectRequest,
    responses((status = 200, body = ProjectResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn update_project(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
    Json(body): Json<UpdateProjectRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let due_at = match body.due_at {
        Some(value) => match OffsetDateTime::parse(&value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    let status = match body.status.as_str() {
        "active" => ProjectStatus::Active,
        "paused" => ProjectStatus::Paused,
        "completed" => ProjectStatus::Completed,
        _ => return invalid_request_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match planning
        .update_project(&ProjectUpdate {
            id: project_id,
            user_id,
            title: body.title,
            objective: body.objective,
            status,
            risk_level: body.risk_level,
            next_action: body.next_action,
            due_at,
            expected_version: body.expected_version,
        })
        .await
    {
        Ok(Some(project)) => match project_response(project) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "project.version_conflict",
            "프로젝트가 다른 곳에서 변경됐어요. 최신 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/projects/{project_id}",
    tag = "work",
    params(("project_id" = String, Path)),
    request_body = DeleteProjectRequest,
    responses((status = 204), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn delete_project(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
    Json(body): Json<DeleteProjectRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .delete_project(
            principal.identity().user_id(),
            project_id,
            body.expected_version,
        )
        .await
    {
        Ok(DeleteProjectOutcome::Deleted | DeleteProjectOutcome::AlreadyAbsent) => {
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(DeleteProjectOutcome::VersionConflict) => error_response(
            StatusCode::CONFLICT,
            "project.version_conflict",
            "프로젝트가 다른 곳에서 변경됐어요. 최신 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/projects/{project_id}/webhooks",
    tag = "work",
    params(("project_id" = String, Path)),
    responses((status = 200, body = ProjectWebhookListResponse), (status = 401), (status = 503))
)]
async fn list_project_webhooks(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .project_webhooks(principal.identity().user_id(), project_id)
        .await
    {
        Ok(items) => Json(ProjectWebhookListResponse {
            items: items.into_iter().map(project_webhook_response).collect(),
            next_cursor: None,
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/webhooks",
    tag = "work",
    params(("project_id" = String, Path)),
    request_body = CreateProjectWebhookRequest,
    responses((status = 201, body = ProjectWebhookResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_project_webhook(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
    Json(body): Json<CreateProjectWebhookRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(provider) = managed_webhook_provider(&body.provider) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "webhook.provider_invalid",
            "Google Chat 또는 Discord를 선택해 주세요.",
            request_id,
            false,
        );
    };
    let Some(mention_directory) =
        google_chat_mention_directory(provider, body.mention_directory.unwrap_or_default())
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "webhook.mention_directory_invalid",
            "입력한 JSON 형식이나 Google Chat 사용자 ID가 올바르지 않아요. 내용을 고친 뒤 다시 저장해 주세요.",
            request_id,
            false,
        );
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let Some(runtime) = state.webhook() else {
        return unavailable_response(request_id);
    };
    let webhook_id = uuid::Uuid::now_v7();
    let Ok(destination) =
        runtime.encrypt_destination(webhook_id, provider, &SecretString::from(body.url))
    else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "webhook.url_invalid",
            "선택한 서비스에서 발급한 웹훅 주소를 확인해 주세요.",
            request_id,
            false,
        );
    };
    match planning
        .create_project_webhook(&NewProjectWebhook {
            id: webhook_id,
            user_id: principal.identity().user_id(),
            project_id,
            provider,
            destination,
            destination_hint: webhook_destination_label(provider),
            mention_directory,
            events: body.events,
        })
        .await
    {
        Ok(webhook) => {
            (StatusCode::CREATED, Json(project_webhook_response(webhook))).into_response()
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/projects/{project_id}/webhooks/{webhook_id}",
    tag = "work",
    params(("project_id" = String, Path), ("webhook_id" = String, Path)),
    request_body = UpdateProjectWebhookRequest,
    responses((status = 200, body = ProjectWebhookResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn update_project_webhook(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, webhook_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    Json(body): Json<UpdateProjectWebhookRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(provider) = managed_webhook_provider(&body.provider) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "webhook.provider_invalid",
            "Google Chat 또는 Discord를 선택해 주세요.",
            request_id,
            false,
        );
    };
    let mention_directory = match body.mention_directory {
        None => WebhookMentionDirectoryUpdate::Keep,
        Some(value) => {
            let Some(directory) = google_chat_mention_directory(provider, value) else {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "webhook.mention_directory_invalid",
                    "입력한 JSON 형식이나 Google Chat 사용자 ID가 올바르지 않아요. 내용을 고친 뒤 다시 저장해 주세요.",
                    request_id,
                    false,
                );
            };
            WebhookMentionDirectoryUpdate::Replace(directory)
        }
    };
    let destination = match (body.destination_mode.as_str(), body.url) {
        ("keep", None) => WebhookDestinationUpdate::Keep,
        ("replace", Some(value)) if !value.trim().is_empty() => {
            let Some(runtime) = state.webhook() else {
                return unavailable_response(request_id);
            };
            match runtime.encrypt_destination(webhook_id, provider, &SecretString::from(value)) {
                Ok(secret) => WebhookDestinationUpdate::Replace {
                    provider,
                    secret,
                    hint: webhook_destination_label(provider),
                },
                Err(_) => {
                    return error_response(
                        StatusCode::BAD_REQUEST,
                        "webhook.url_invalid",
                        "선택한 서비스에서 발급한 웹훅 주소를 확인해 주세요.",
                        request_id,
                        false,
                    );
                }
            }
        }
        _ => return invalid_request_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .update_project_webhook(&ProjectWebhookUpdate {
            id: webhook_id,
            user_id: principal.identity().user_id(),
            project_id,
            provider,
            events: body.events,
            enabled: body.enabled,
            destination,
            mention_directory,
            expected_version: body.expected_version,
        })
        .await
    {
        Ok(Some(webhook)) => Json(project_webhook_response(webhook)).into_response(),
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "webhook.version_conflict",
            "웹훅 설정이 변경됐어요. 다시 불러온 뒤 저장해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/projects/{project_id}/webhooks/{webhook_id}",
    tag = "work",
    params(("project_id" = String, Path), ("webhook_id" = String, Path)),
    request_body = DeleteProjectWebhookRequest,
    responses((status = 204), (status = 401), (status = 409), (status = 503))
)]
async fn delete_project_webhook(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, webhook_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    Json(body): Json<DeleteProjectWebhookRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .delete_project_webhook(
            principal.identity().user_id(),
            project_id,
            webhook_id,
            body.expected_version,
        )
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error_response(
            StatusCode::CONFLICT,
            "webhook.version_conflict",
            "웹훅 설정이 변경됐어요. 다시 불러온 뒤 삭제해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/webhooks/{webhook_id}/test",
    tag = "work",
    params(("project_id" = String, Path), ("webhook_id" = String, Path)),
    responses((status = 202), (status = 401), (status = 409), (status = 503))
)]
async fn test_project_webhook(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, webhook_id)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let payload = webhook_payload("webhook.test", project_id, None);
    match planning
        .queue_webhook_test(
            principal.identity().user_id(),
            project_id,
            webhook_id,
            &payload,
        )
        .await
    {
        Ok(Some(_)) => StatusCode::ACCEPTED.into_response(),
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "webhook.unavailable",
            "웹훅 설정을 다시 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/webhooks/{webhook_id}/messages",
    tag = "work",
    params(("project_id" = String, Path), ("webhook_id" = String, Path)),
    request_body = SendWebhookMessageRequest,
    responses((status = 202), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn send_webhook_message(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, webhook_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    Json(body): Json<SendWebhookMessageRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let message = body.message.trim();
    if message.is_empty() || message.chars().count() > 1_800 {
        return error_response(
            StatusCode::BAD_REQUEST,
            "webhook.message_invalid",
            "보낼 메시지를 1,800자 이내로 적어 주세요.",
            request_id,
            false,
        );
    }
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .queue_webhook_message(
            principal.identity().user_id(),
            project_id,
            webhook_id,
            message,
        )
        .await
    {
        Ok(Some(_)) => StatusCode::ACCEPTED.into_response(),
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "webhook.unavailable",
            "연결을 사용할 수 없어요. 웹훅 설정과 전송 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/projects/{project_id}/webhook-deliveries",
    tag = "work",
    params(("project_id" = String, Path)),
    responses((status = 200, body = WebhookDeliveryListResponse), (status = 401), (status = 503))
)]
async fn list_webhook_deliveries(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .webhook_delivery_history(principal.identity().user_id(), project_id)
        .await
    {
        Ok(items) => match items
            .into_iter()
            .map(webhook_delivery_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(WebhookDeliveryListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/webhook-deliveries/{delivery_id}/retry",
    tag = "work",
    params(("project_id" = String, Path), ("delivery_id" = String, Path)),
    responses((status = 202), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn retry_webhook_delivery(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, delivery_id)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .retry_webhook_delivery(principal.identity().user_id(), project_id, delivery_id)
        .await
    {
        Ok(RetryWebhookDeliveryOutcome::Queued | RetryWebhookDeliveryOutcome::AlreadyQueued) => {
            StatusCode::ACCEPTED.into_response()
        }
        Ok(RetryWebhookDeliveryOutcome::Conflict) => error_response(
            StatusCode::CONFLICT,
            "webhook.delivery_not_retryable",
            "이미 전송 중이거나 전송을 마친 요청이에요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/tasks",
    tag = "planning",
    params(
        ("projectId" = Option<String>, Query),
        ("status" = Option<String>, Query, description = "Use completed for global completion history or all with a project to include completed work")
    ),
    responses((status = 200, body = TaskListResponse), (status = 400), (status = 401), (status = 503))
)]
async fn list_open_tasks(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    axum::extract::Query(query): axum::extract::Query<TaskListQuery>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let result = match (query.project_id, query.status.as_deref()) {
        (Some(project_id), Some("all")) => planning.tasks_for_project(user_id, project_id).await,
        (Some(project_id), None | Some("open")) => {
            planning.open_tasks_for_project(user_id, project_id).await
        }
        (None, None | Some("open")) => planning.open_tasks_for_user(user_id).await,
        (None, Some("completed")) => planning.completed_tasks_for_user(user_id).await,
        _ => return invalid_request_response(request_id),
    };
    match result {
        Ok(tasks) => match tasks
            .into_iter()
            .map(task_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(TaskListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/tasks",
    tag = "planning",
    request_body = CreateTaskRequest,
    responses((status = 201, body = TaskResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_task(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<CreateTaskRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let due_at = match body.due_at {
        Some(value) => match OffsetDateTime::parse(&value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match planning
        .create_task(&NewTask {
            id: uuid::Uuid::now_v7(),
            user_id,
            project_id: body.project_id,
            title: body.title,
            notes: body.notes,
            assignee_name: body.assignee_name,
            priority: body.priority,
            due_at,
        })
        .await
    {
        Ok(task) => match task_response(task) {
            Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/tasks/{task_id}",
    tag = "planning",
    params(("task_id" = String, Path)),
    responses((status = 200, body = TaskResponse), (status = 401), (status = 404), (status = 503))
)]
async fn get_task(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(task_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .task_for_user(principal.identity().user_id(), task_id)
        .await
    {
        Ok(Some(task)) => match task_response(task) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "task.not_found",
            "이 할 일을 찾지 못했어요. 목록을 새로 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/tasks/{task_id}",
    tag = "planning",
    params(("task_id" = String, Path)),
    request_body = UpdateTaskRequest,
    responses((status = 200, body = TaskResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn update_task(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(task_id): Path<uuid::Uuid>,
    Json(body): Json<UpdateTaskRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let due_at = match body.due_at {
        Some(value) => match OffsetDateTime::parse(&value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    let status = match body.status.as_str() {
        "open" => TaskStatus::Open,
        "completed" => TaskStatus::Completed,
        "cancelled" => TaskStatus::Cancelled,
        _ => return invalid_request_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match planning
        .update_task(&TaskUpdate {
            id: task_id,
            user_id,
            project_id: body.project_id,
            title: body.title,
            notes: body.notes,
            assignee_name: body.assignee_name,
            status,
            priority: body.priority,
            due_at,
            expected_version: body.expected_version,
        })
        .await
    {
        Ok(Some(task)) => match task_response(task) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "task.version_conflict",
            "할 일이 다른 기기에서 변경되었어요. 최신 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/tasks/{task_id}",
    tag = "planning",
    params(("task_id" = String, Path)),
    request_body = DeleteTaskRequest,
    responses((status = 204), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn delete_task(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(task_id): Path<uuid::Uuid>,
    Json(body): Json<DeleteTaskRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .delete_task(
            principal.identity().user_id(),
            task_id,
            body.expected_version,
        )
        .await
    {
        Ok(
            DeleteTaskOutcome::Deleted
            | DeleteTaskOutcome::AlreadyDeleted
            | DeleteTaskOutcome::AlreadyAbsent,
        ) => StatusCode::NO_CONTENT.into_response(),
        Ok(DeleteTaskOutcome::VersionConflict) => error_response(
            StatusCode::CONFLICT,
            "task.version_conflict",
            "할 일이 다른 기기에서 변경되었어요. 최신 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/assistant/voice-commands",
    tag = "assistant",
    request_body = VoiceCommandRequest,
    responses((status = 200, body = VoiceCommandResponse), (status = 201, body = VoiceCommandResponse), (status = 400), (status = 401), (status = 503))
)]
async fn execute_voice_command(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<VoiceCommandRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    if body
        .client_mutation_id
        .is_some_and(|id| id.get_version_num() != 7)
    {
        return invalid_request_response(request_id);
    }
    let Ok(reference_at) = OffsetDateTime::parse(&body.reference_at, &Rfc3339) else {
        return invalid_request_response(request_id);
    };
    let command = match voice_command::interpret(&body.text, reference_at, &body.time_zone) {
        Ok(command) => command,
        Err(VoiceCommandError::InvalidInput) => return invalid_request_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let client_mutation_id = body.client_mutation_id.unwrap_or_else(uuid::Uuid::now_v7);
    let calendar_target = match planning.primary_calendar_mutation_target(user_id).await {
        Ok(target) => target,
        Err(error) => return storage_error_response(&error, request_id),
    };

    handle_voice_command(
        planning,
        user_id,
        command,
        body.time_zone,
        calendar_target.as_ref(),
        client_mutation_id,
        request_id,
    )
    .await
}

async fn handle_voice_command(
    planning: &Database,
    user_id: uuid::Uuid,
    command: VoiceCommand,
    time_zone: String,
    calendar_target: Option<&jimin_storage::calendar::PrimaryCalendarMutationTarget>,
    client_mutation_id: uuid::Uuid,
    request_id: RequestId,
) -> Response {
    match command {
        VoiceCommand::ListSchedule {
            label,
            starts_at,
            ends_at,
        } => list_voice_schedule(planning, user_id, label, starts_at, ends_at, request_id).await,
        VoiceCommand::CreateSchedule {
            label,
            title,
            starts_at,
            ends_at,
        } => {
            create_voice_schedule(
                planning,
                user_id,
                VoiceScheduleInput {
                    label,
                    title,
                    starts_at,
                    ends_at,
                    time_zone,
                },
                calendar_target,
                client_mutation_id,
                request_id,
            )
            .await
        }
        VoiceCommand::ListTasks { scope } => {
            list_voice_tasks(planning, user_id, scope, request_id).await
        }
        VoiceCommand::CreateTask {
            label,
            title,
            due_at,
        } => {
            create_voice_task(
                planning,
                user_id,
                label,
                title,
                due_at,
                client_mutation_id,
                request_id,
            )
            .await
        }
        VoiceCommand::NeedsScheduleDetails => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::NeedsDetails,
            message: "일정 이름과 시간을 함께 말해 주세요. 예: 내일 오후 3시에 치과 일정 등록해 줘"
                .to_owned(),
            destination: VoiceCommandDestination::Conversation,
            items: Vec::new(),
        })
        .into_response(),
        VoiceCommand::NeedsTaskDetails => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::NeedsDetails,
            message: "추가할 할 일을 함께 말해 주세요. 예: 할 일에 장보기 추가해 줘".to_owned(),
            destination: VoiceCommandDestination::Conversation,
            items: Vec::new(),
        })
        .into_response(),
        VoiceCommand::ContinueConversation => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::ContinueConversation,
            message: "일정이나 할 일 외의 요청은 대화에서 이어서 도와드릴게요.".to_owned(),
            destination: VoiceCommandDestination::Conversation,
            items: Vec::new(),
        })
        .into_response(),
    }
}

struct VoiceScheduleInput {
    label: &'static str,
    title: String,
    starts_at: OffsetDateTime,
    ends_at: OffsetDateTime,
    time_zone: String,
}

async fn list_voice_schedule(
    planning: &Database,
    user_id: uuid::Uuid,
    label: &str,
    starts_at: OffsetDateTime,
    ends_at: OffsetDateTime,
    request_id: RequestId,
) -> Response {
    match planning
        .schedule_entries_in_range(user_id, starts_at, ends_at)
        .await
    {
        Ok(entries) => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::ScheduleListed,
            message: schedule_list_message(label, &entries),
            destination: VoiceCommandDestination::Calendar,
            items: entries.iter().map(voice_schedule_item).collect(),
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn create_voice_schedule(
    planning: &Database,
    user_id: uuid::Uuid,
    input: VoiceScheduleInput,
    calendar_target: Option<&jimin_storage::calendar::PrimaryCalendarMutationTarget>,
    client_mutation_id: uuid::Uuid,
    request_id: RequestId,
) -> Response {
    let VoiceScheduleInput {
        label,
        title,
        starts_at,
        ends_at,
        time_zone,
    } = input;
    let entry = NewScheduleEntry {
        id: client_mutation_id,
        user_id,
        title: title.clone(),
        notes: None,
        starts_at,
        ends_at,
        time_zone,
    };
    let created = match calendar_target {
        Some(target) => {
            planning
                .create_schedule_entry_with_calendar_outbox(&entry, target)
                .await
        }
        None => planning.create_schedule_entry(&entry).await,
    };
    match created {
        Ok(entry) => {
            let item = voice_schedule_item(&entry);
            (
                StatusCode::CREATED,
                Json(VoiceCommandResponse {
                    kind: VoiceCommandKind::ScheduleCreated,
                    message: format!(
                        "{label} {:02}:{:02}에 {title} 일정을 등록했어요.",
                        entry.starts_at.hour(),
                        entry.starts_at.minute(),
                    ),
                    destination: VoiceCommandDestination::Calendar,
                    items: vec![item],
                }),
            )
                .into_response()
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn list_voice_tasks(
    planning: &Database,
    user_id: uuid::Uuid,
    scope: VoiceTaskScope,
    request_id: RequestId,
) -> Response {
    let (label, destination, result) = match scope {
        VoiceTaskScope::All => (
            None,
            VoiceCommandDestination::Home,
            planning.open_tasks_for_user(user_id).await,
        ),
        VoiceTaskScope::Today { label, ends_at } => (
            Some(label),
            VoiceCommandDestination::Home,
            planning.home_tasks_for_user(user_id, ends_at).await,
        ),
        VoiceTaskScope::Dated {
            label,
            starts_at,
            ends_at,
        } => (
            Some(label),
            VoiceCommandDestination::Calendar,
            planning.open_tasks_for_user(user_id).await.map(|tasks| {
                tasks
                    .into_iter()
                    .filter(|task| {
                        task.due_at
                            .is_some_and(|due_at| due_at >= starts_at && due_at < ends_at)
                    })
                    .collect()
            }),
        ),
    };
    match result {
        Ok(tasks) => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::TasksListed,
            message: task_list_message(label, &tasks),
            destination,
            items: tasks.iter().map(voice_task_item).collect(),
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn create_voice_task(
    planning: &Database,
    user_id: uuid::Uuid,
    label: Option<&'static str>,
    title: String,
    due_at: Option<OffsetDateTime>,
    client_mutation_id: uuid::Uuid,
    request_id: RequestId,
) -> Response {
    match planning
        .create_task_idempotently(&NewTask {
            id: client_mutation_id,
            user_id,
            project_id: None,
            title: title.clone(),
            notes: None,
            assignee_name: None,
            priority: 1,
            due_at,
        })
        .await
    {
        Ok(task) => {
            let destination = match label {
                Some("내일" | "모레") => VoiceCommandDestination::Calendar,
                Some(_) | None => VoiceCommandDestination::Home,
            };
            let subject =
                label.map_or_else(|| "할 일".to_owned(), |value| format!("{value} 할 일"));
            (
                StatusCode::CREATED,
                Json(VoiceCommandResponse {
                    kind: VoiceCommandKind::TaskCreated,
                    message: format!("{subject}에 추가했어요: {title}"),
                    destination,
                    items: vec![voice_task_item(&task)],
                }),
            )
                .into_response()
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

fn schedule_list_message(label: &str, entries: &[ScheduleEntry]) -> String {
    match entries {
        [] => format!("{label} 일정은 없어요."),
        [_] => format!("{label} 일정은 1개예요."),
        _ => format!("{label} 일정은 {}개예요.", entries.len()),
    }
}

fn task_list_message(label: Option<&str>, tasks: &[Task]) -> String {
    let subject = label.map_or("열린 할 일", |value| match value {
        "오늘" => "오늘 할 일",
        "내일" => "내일 할 일",
        "모레" => "모레 할 일",
        _ => "할 일",
    });
    match tasks {
        [] => format!("{subject}이 없어요."),
        [_] => format!("{subject}은 1개예요."),
        _ => format!("{subject}은 {}개예요.", tasks.len()),
    }
}

fn voice_task_item(task: &Task) -> VoiceCommandItemResponse {
    VoiceCommandItemResponse {
        item_type: VoiceCommandItemType::Task,
        id: task.id,
        title: task.title.clone(),
        due_at: task.due_at.and_then(|value| value.format(&Rfc3339).ok()),
        starts_at: None,
        ends_at: None,
        priority: Some(task.priority),
    }
}

fn voice_schedule_item(entry: &ScheduleEntry) -> VoiceCommandItemResponse {
    VoiceCommandItemResponse {
        item_type: VoiceCommandItemType::Schedule,
        id: entry.id,
        title: entry.title.clone(),
        due_at: None,
        starts_at: entry.starts_at.format(&Rfc3339).ok(),
        ends_at: entry.ends_at.format(&Rfc3339).ok(),
        priority: None,
    }
}

#[utoipa::path(
    post,
    path = "/v1/tasks/{task_id}/complete",
    tag = "planning",
    params(("task_id" = String, Path)),
    request_body = CompleteTaskRequest,
    responses((status = 200, body = TaskResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn complete_task(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(task_id): Path<uuid::Uuid>,
    Json(body): Json<CompleteTaskRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match planning
        .complete_task(user_id, task_id, body.expected_version)
        .await
    {
        Ok(Some(task)) => match task_response(task) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => error_response(
            StatusCode::CONFLICT,
            "task.version_conflict",
            "할 일이 다른 기기에서 변경되었어요. 최신 상태를 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/conversations",
    tag = "agent",
    responses((status = 200, body = ConversationListResponse), (status = 401), (status = 503))
)]
async fn list_conversations(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .active_conversations_for_user(principal.identity().user_id())
        .await
    {
        Ok(conversations) => match conversations
            .into_iter()
            .map(conversation_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(ConversationListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/conversations",
    tag = "agent",
    request_body = CreateConversationRequest,
    responses((status = 201, body = ConversationResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_conversation(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<CreateConversationRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .create_conversation(&NewConversation {
            id: body.client_conversation_id,
            user_id: principal.identity().user_id(),
            title: body.title,
        })
        .await
    {
        Ok(conversation) => match conversation_response(conversation) {
            Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/conversations/{conversation_id}/messages",
    tag = "agent",
    params(("conversation_id" = String, Path)),
    responses((status = 200, body = ConversationMessageListResponse), (status = 401), (status = 404), (status = 503))
)]
async fn list_conversation_messages(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(conversation_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .conversation_messages_for_user(principal.identity().user_id(), conversation_id)
        .await
    {
        Ok(Some(messages)) => match messages
            .into_iter()
            .map(conversation_message_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(ConversationMessageListResponse {
                items,
                next_cursor: None,
            })
            .into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => agent_not_found_response(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/conversations/{conversation_id}/stream",
    tag = "agent",
    params(("conversation_id" = String, Path)),
    responses((status = 200, description = "Authenticated server-sent conversation snapshots"), (status = 401), (status = 404), (status = 503))
)]
async fn stream_conversation_updates(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(conversation_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent().cloned() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match conversation_stream_snapshot(&agent, user_id, conversation_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return agent_not_found_response(request_id),
        Err(error) => return storage_error_response(&error, request_id),
    }

    let stream = futures_util::stream::unfold(
        ConversationStreamState {
            agent,
            user_id,
            conversation_id,
            last_fingerprint: None,
            close_after_event: false,
        },
        |mut stream_state| async move {
            if stream_state.close_after_event {
                return None;
            }
            loop {
                let Ok(Some(snapshot)) = conversation_stream_snapshot(
                    &stream_state.agent,
                    stream_state.user_id,
                    stream_state.conversation_id,
                )
                .await
                else {
                    return None;
                };
                let fingerprint = conversation_stream_fingerprint(&snapshot);
                let terminal = snapshot
                    .job
                    .as_ref()
                    .is_none_or(|job| agent_job_response_is_terminal(&job.state));
                if stream_state.last_fingerprint.as_deref() != Some(fingerprint.as_str()) {
                    let Ok(data) = serde_json::to_string(&snapshot) else {
                        return None;
                    };
                    stream_state.last_fingerprint = Some(fingerprint);
                    stream_state.close_after_event = terminal;
                    return Some((
                        Ok::<Event, Infallible>(Event::default().event("snapshot").data(data)),
                        stream_state,
                    ));
                }
                if terminal {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
        },
    );
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("keep-alive"),
        )
        .into_response()
}

struct ConversationStreamState {
    agent: Database,
    user_id: uuid::Uuid,
    conversation_id: uuid::Uuid,
    last_fingerprint: Option<String>,
    close_after_event: bool,
}

async fn conversation_stream_snapshot(
    agent: &Database,
    user_id: uuid::Uuid,
    conversation_id: uuid::Uuid,
) -> Result<Option<ConversationStreamSnapshot>, StorageError> {
    let Some(messages) = agent
        .conversation_messages_for_user(user_id, conversation_id)
        .await?
    else {
        return Ok(None);
    };
    let messages = messages
        .into_iter()
        .map(conversation_message_response)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|()| StorageError::PersistenceUnavailable)?;
    let job = agent
        .latest_agent_job_for_conversation_for_user(user_id, conversation_id)
        .await?
        .map(|job| agent_job_response(&job))
        .transpose()
        .map_err(|()| StorageError::PersistenceUnavailable)?;
    Ok(Some(ConversationStreamSnapshot { messages, job }))
}

fn conversation_stream_fingerprint(snapshot: &ConversationStreamSnapshot) -> String {
    let message_versions = snapshot
        .messages
        .iter()
        .map(|message| format!("{}:{}:{}", message.id, message.version, message.status))
        .collect::<Vec<_>>()
        .join(",");
    let job = snapshot.job.as_ref().map_or_else(
        || "none".to_owned(),
        |job| format!("{}:{}:{}", job.id, job.version, job.state),
    );
    format!("{job}|{message_versions}")
}

fn agent_job_response_is_terminal(state: &str) -> bool {
    matches!(state, "completed" | "failed" | "cancelled" | "declined")
}

#[utoipa::path(
    get,
    path = "/v1/conversations/{conversation_id}/jobs/latest",
    tag = "agent",
    params(("conversation_id" = String, Path)),
    responses((status = 200, body = AgentJobResponse), (status = 204, description = "The conversation has no AI request yet"), (status = 401), (status = 503))
)]
async fn get_latest_conversation_job(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(conversation_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .latest_agent_job_for_conversation_for_user(principal.identity().user_id(), conversation_id)
        .await
    {
        Ok(Some(job)) => match agent_job_response(&job) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/conversations/{conversation_id}/turns",
    tag = "agent",
    params(("conversation_id" = String, Path)),
    request_body = CreateAgentTurnRequest,
    responses((status = 202, body = QueuedAgentTurnResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn create_agent_turn(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(conversation_id): Path<uuid::Uuid>,
    Json(body): Json<CreateAgentTurnRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    let mut input = body.input;
    if input.len() != 1 {
        return invalid_request_response(request_id);
    }
    let Some(input) = input.pop() else {
        return invalid_request_response(request_id);
    };
    if input.kind != "text" {
        return invalid_request_response(request_id);
    }

    let turn = NewAgentTurn {
        job_id: uuid::Uuid::now_v7(),
        message_id: uuid::Uuid::now_v7(),
        client_message_id: body.client_message_id,
        user_id: principal.identity().user_id(),
        conversation_id,
        content: input.text,
    };
    let queued = enqueue_conversation_turn(agent, &turn).await;
    match queued {
        Ok(queued) => (
            StatusCode::ACCEPTED,
            Json(queued_agent_turn_response(&queued)),
        )
            .into_response(),
        Err(StorageError::IdentityConflict) => error_response(
            StatusCode::CONFLICT,
            "conversation.unavailable",
            "이 대화는 다른 요청을 처리 중이에요. 잠시 후 다시 시도해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

/// Queues every conversational request for semantic interpretation by the
/// managed assistant. Planning mutations are selected through its structured
/// action contract and committed atomically by the worker, rather than by a
/// separate text-matching shortcut at the HTTP boundary.
async fn enqueue_conversation_turn(
    agent: &Database,
    turn: &NewAgentTurn,
) -> Result<QueuedAgentTurn, StorageError> {
    agent.enqueue_agent_turn(turn).await
}

#[utoipa::path(
    get,
    path = "/v1/agent/authentication",
    tag = "agent",
    responses((status = 200, body = AgentAuthenticationResponse), (status = 401), (status = 503))
)]
async fn get_agent_authentication(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .agent_authentication_for_user(principal.identity().user_id())
        .await
    {
        Ok(authentication) => no_store_json(agent_authentication_response(authentication)),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/agent/authentication",
    tag = "agent",
    responses((status = 202, body = AgentAuthenticationResponse), (status = 401), (status = 503))
)]
async fn request_agent_authentication(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .request_agent_authentication(principal.identity().user_id(), uuid::Uuid::now_v7())
        .await
    {
        Ok(authentication) => {
            let mut response = no_store_json(agent_authentication_response(Some(authentication)));
            *response.status_mut() = StatusCode::ACCEPTED;
            response
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/agent/models",
    tag = "agent",
    responses((status = 200, body = AgentModelSettingsResponse), (status = 401), (status = 503))
)]
async fn get_agent_model_settings(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .agent_model_settings_for_user(principal.identity().user_id())
        .await
    {
        Ok(settings) => no_store_json(agent_model_settings_response(settings)),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/agent/models",
    tag = "agent",
    request_body = UpdateAgentModelRequest,
    responses((status = 200, body = AgentModelSettingsResponse), (status = 400), (status = 401), (status = 503))
)]
async fn update_agent_model_settings(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(request): Json<UpdateAgentModelRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .set_agent_model_for_user(
            principal.identity().user_id(),
            request.model_id.as_deref(),
            request.reasoning_effort.as_deref(),
        )
        .await
    {
        Ok(settings) => no_store_json(agent_model_settings_response(settings)),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/agent/jobs/{job_id}",
    tag = "agent",
    params(("job_id" = String, Path)),
    responses((status = 200, body = AgentJobResponse), (status = 401), (status = 404), (status = 503))
)]
async fn get_agent_job(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(job_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    match agent
        .agent_job_for_user(principal.identity().user_id(), job_id)
        .await
    {
        Ok(Some(job)) => match agent_job_response(&job) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => agent_not_found_response(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/agent/jobs/{job_id}/approval",
    tag = "agent",
    params(("job_id" = String, Path)),
    request_body = ResolveAgentActionRequest,
    responses((status = 200, body = AgentJobResponse), (status = 400), (status = 401), (status = 409), (status = 404), (status = 503))
)]
async fn resolve_agent_action(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(job_id): Path<uuid::Uuid>,
    Json(body): Json<ResolveAgentActionRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let decision = match body.decision.as_str() {
        "approve" => PendingAgentActionDecision::Approve,
        "decline" => PendingAgentActionDecision::Decline,
        _ => return invalid_request_response(request_id),
    };
    let Some(agent) = state.agent() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match agent.resolve_agent_action(user_id, job_id, decision).await {
        Ok(true) => match agent.agent_job_for_user(user_id, job_id).await {
            Ok(Some(job)) => match agent_job_response(&job) {
                Ok(response) => Json(response).into_response(),
                Err(()) => unavailable_response(request_id),
            },
            Ok(None) => agent_not_found_response(request_id),
            Err(error) => storage_error_response(&error, request_id),
        },
        Ok(false) => error_response(
            StatusCode::CONFLICT,
            "agent.action_unavailable",
            "이 요청은 이미 처리되었거나 실행할 수 없어요. 대화를 다시 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/access/session",
    tag = "identity",
    request_body = DeviceRegistrationRequest,
    responses(
        (status = 200, description = "Private-network device session created without an interactive pairing step", body = DeviceSessionResponse),
        (status = 400, description = "Device metadata is invalid"),
        (status = 404, description = "Private-network access is not enabled for this deployment"),
        (status = 503, description = "Authentication service is temporarily unavailable")
    )
)]
async fn trusted_network_session(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    Json(request): Json<DeviceRegistrationRequest>,
) -> Response {
    if !state.trusted_network() {
        return not_found_response(request_id);
    }
    let Some(pairing) = state.pairing() else {
        return unavailable_response(request_id);
    };
    let Ok(device) = DeviceRegistration::new(
        request.installation_id,
        request.platform,
        request.name,
        request.app_version,
        request.os_version,
    ) else {
        return invalid_request_response(request_id);
    };
    let session = match pairing
        .provision_trusted_network_device(device, uuid::Uuid::now_v7())
        .await
    {
        Ok(session) => session,
        Err(error) => return application_error_response(&error, request_id),
    };
    match device_session_response(&session) {
        Ok(response) => no_store_json(response),
        Err(()) => unavailable_response(request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/calendar/connections/google",
    tag = "calendar",
    responses(
        (status = 200, body = GoogleCalendarConnectionResponse),
        (status = 401),
        (status = 503)
    )
)]
async fn get_google_calendar_connection(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };

    match planning
        .calendar_account_for_user(principal.identity().user_id())
        .await
    {
        Ok(account) => Json(calendar_connection_response(
            account,
            state.calendar_oauth().is_some(),
        ))
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/calendar/connections/google",
    tag = "calendar",
    params(("expectedVersion" = i64, Query)),
    responses(
        (status = 204, description = "Google Calendar connection and cached provider data were removed"),
        (status = 400),
        (status = 401),
        (status = 409),
        (status = 503)
    )
)]
async fn disconnect_google_calendar(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Query(query): Query<DisconnectGoogleCalendarQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let connection = match planning
        .disconnect_calendar_account(principal.identity().user_id(), query.expected_version)
        .await
    {
        Ok(DisconnectCalendarAccountOutcome::Disconnected(connection)) => connection,
        Ok(DisconnectCalendarAccountOutcome::AlreadyDisconnected) => {
            return StatusCode::NO_CONTENT.into_response();
        }
        Ok(DisconnectCalendarAccountOutcome::VersionConflict) => {
            return error_response(
                StatusCode::CONFLICT,
                "calendar.connection_changed",
                "Google Calendar 연결 상태가 달라졌어요. 다시 확인한 뒤 연결을 해제해 주세요.",
                request_id,
                false,
            );
        }
        Ok(DisconnectCalendarAccountOutcome::MutationInProgress) => {
            return error_response(
                StatusCode::CONFLICT,
                "calendar.mutation_in_progress",
                "Google Calendar에 일정을 반영하고 있어요. 잠시 후 연결 해제를 다시 시도해 주세요.",
                request_id,
                true,
            );
        }
        Err(error) => return storage_error_response(&error, request_id),
    };
    if let (Some(calendar_oauth), Some(connection)) = (state.calendar_oauth(), connection.as_ref())
        && calendar_oauth
            .revoke_calendar_connection(connection)
            .await
            .is_err()
    {
        warn!(
            event = "calendar_provider_revoke_failed",
            "Google Calendar provider revocation failed after local purge"
        );
    }
    StatusCode::NO_CONTENT.into_response()
}

#[utoipa::path(
    post,
    path = "/v1/calendar/connections/google/authorizations",
    tag = "calendar",
    request_body = StartGoogleCalendarAuthorizationRequest,
    responses(
        (status = 201, body = StartGoogleCalendarAuthorizationResponse),
        (status = 400),
        (status = 401),
        (status = 503)
    )
)]
async fn start_google_calendar_authorization(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(request): Json<StartGoogleCalendarAuthorizationRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(client_kind) = parse_client_platform(&request.client_kind) else {
        return invalid_request_response(request_id);
    };
    let Some(calendar_oauth) = state.calendar_oauth() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "calendar.configuration_missing",
            "Google Calendar 연결을 아직 준비하고 있어요.",
            request_id,
            false,
        );
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let force_consent = match planning
        .calendar_account_for_user(principal.identity().user_id())
        .await
    {
        Ok(None) => true,
        Ok(Some(account)) => matches!(
            account.status,
            CalendarAccountStatus::ReauthRequired | CalendarAccountStatus::Revoked
        ),
        Err(error) => return storage_error_response(&error, request_id),
    };
    let authorization_id = uuid::Uuid::now_v7();
    let authorization =
        match calendar_oauth.begin_authorization(authorization_id, client_kind, force_consent) {
            Ok(authorization) => authorization,
            Err(error) => return calendar_oauth_error_response(error, request_id),
        };
    let command = CreateCalendarOAuthAuthorization {
        id: authorization_id,
        user_id: principal.identity().user_id(),
        session_id: principal.identity().session_id(),
        device_id: principal.identity().device_id(),
        state_verifier: authorization.state_verifier,
        pkce_verifier: authorization.pkce_verifier,
        client_kind,
        expires_at: authorization.expires_at,
    };
    let persisted = match planning.create_calendar_oauth_authorization(&command).await {
        Ok(persisted) => persisted,
        Err(error) => return storage_error_response(&error, request_id),
    };
    let Ok(expires_at) = persisted.expires_at.format(&Rfc3339) else {
        return unavailable_response(request_id);
    };
    (
        StatusCode::CREATED,
        Json(StartGoogleCalendarAuthorizationResponse {
            authorization_id: persisted.id,
            authorization_url: authorization.authorization_url,
            expires_at,
        }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/oauth/google/calendar/callback",
    tag = "calendar",
    params(
        ("state" = String, Query),
        ("code" = Option<String>, Query),
        ("error" = Option<String>, Query)
    ),
    responses((status = 200), (status = 400), (status = 503))
)]
async fn complete_google_calendar_authorization(
    State(state): State<ApiState>,
    Query(query): Query<GoogleCalendarCallbackQuery>,
) -> Response {
    let Some(planning) = state.planning() else {
        return calendar_callback_page(
            StatusCode::SERVICE_UNAVAILABLE,
            "연결을 완료하지 못했어요",
            "잠시 후 앱에서 다시 시도해 주세요.",
        );
    };
    if let Some(calendar_oauth) = state.calendar_oauth() {
        match planning
            .claim_calendar_oauth_authorization(&calendar_oauth.state_verifier(&query.state))
            .await
        {
            Ok(Some(claimed)) => {
                return finish_google_calendar_authorization(
                    planning,
                    calendar_oauth,
                    claimed,
                    query,
                )
                .await;
            }
            Ok(None) => {}
            Err(_) => {
                return calendar_callback_page(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "연결을 완료하지 못했어요",
                    "잠시 후 앱에서 다시 시도해 주세요.",
                );
            }
        }
    }
    if let Some(google_chat_oauth) = state.google_chat_oauth() {
        match planning
            .claim_google_chat_oauth_authorization(&google_chat_oauth.state_verifier(&query.state))
            .await
        {
            Ok(Some(claimed)) => {
                return finish_google_chat_authorization(
                    planning,
                    google_chat_oauth,
                    claimed,
                    query,
                )
                .await;
            }
            Ok(None) => {}
            Err(_) => {
                return calendar_callback_page(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "연결을 완료하지 못했어요",
                    "잠시 후 앱에서 다시 시도해 주세요.",
                );
            }
        }
    }
    calendar_callback_page(
        StatusCode::BAD_REQUEST,
        "연결을 완료하지 못했어요",
        "연결 시간이 지났거나 이미 처리된 요청이에요. 앱에서 다시 연결해 주세요.",
    )
}

async fn finish_google_calendar_authorization(
    planning: &Database,
    calendar_oauth: &CalendarOAuthRuntime,
    claimed: jimin_storage::calendar::ClaimedCalendarOAuthAuthorization,
    query: GoogleCalendarCallbackQuery,
) -> Response {
    if query.error.is_some() || query.code.is_none() {
        let _ = planning
            .fail_calendar_oauth_authorization(claimed.id, "calendar.authorization_failed")
            .await;
        return calendar_callback_page(
            StatusCode::BAD_REQUEST,
            "연결을 완료하지 못했어요",
            "Google Calendar 권한이 허용되지 않았어요. 앱에서 다시 연결해 주세요.",
        );
    }
    let code = SecretString::from(query.code.unwrap_or_default());
    let authorization_id = claimed.id;
    let completion = calendar_oauth.complete_authorization(claimed, code).await;
    let command = match completion {
        Ok(command) => command,
        Err(error) => {
            let failure_code = error.authorization_failure_code();
            warn!(
                error_code = failure_code,
                "Google Calendar OAuth callback failed before account persistence"
            );
            let _ = planning
                .fail_calendar_oauth_authorization(authorization_id, failure_code)
                .await;
            return calendar_callback_error_page(error);
        }
    };
    let user_id = command.user_id;
    let account = match planning
        .complete_calendar_oauth_authorization(&command)
        .await
    {
        Ok(account) => account,
        Err(error) => {
            let failure_code = storage_failure_code(&error);
            warn!(
                error_code = failure_code,
                "Google Calendar OAuth callback failed during account persistence"
            );
            let _ = planning
                .fail_calendar_oauth_authorization(authorization_id, failure_code)
                .await;
            return calendar_callback_page(
                if matches!(
                    error,
                    StorageError::PersistenceUnavailable | StorageError::MigrationUnavailable
                ) {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::BAD_REQUEST
                },
                "연결을 완료하지 못했어요",
                "앱에서 Google Calendar 연결을 다시 시도해 주세요.",
            );
        }
    };
    finish_initial_calendar_sync(planning, calendar_oauth, account.id, user_id).await
}

async fn finish_google_chat_authorization(
    planning: &Database,
    runtime: &GoogleChatOAuthRuntime,
    claimed: jimin_storage::google_chat::ClaimedGoogleChatOAuthAuthorization,
    query: GoogleCalendarCallbackQuery,
) -> Response {
    if query.error.is_some() || query.code.is_none() {
        let _ = planning
            .fail_google_chat_oauth_authorization(claimed.id, "google_chat.authorization_rejected")
            .await;
        return calendar_callback_page(
            StatusCode::BAD_REQUEST,
            "회사 Google 계정을 연결하지 못했어요",
            "권한을 허용한 뒤 앱에서 다시 연결해 주세요.",
        );
    }
    let authorization_id = claimed.id;
    let completion = runtime
        .complete_authorization(claimed, SecretString::from(query.code.unwrap_or_default()))
        .await;
    let command = match completion {
        Ok(command) => command,
        Err(error) => {
            let _ = planning
                .fail_google_chat_oauth_authorization(authorization_id, error.failure_code())
                .await;
            return google_chat_callback_error_page(error);
        }
    };
    match planning
        .complete_google_chat_oauth_authorization(&command)
        .await
    {
        Ok(_) => calendar_callback_page(
            StatusCode::OK,
            "회사 Google 계정을 연결했어요",
            "이제 프로젝트에서 확인할 Chat 공간을 선택해 주세요.",
        ),
        Err(error) => {
            let _ = planning
                .fail_google_chat_oauth_authorization(
                    authorization_id,
                    "google_chat.persistence_failed",
                )
                .await;
            calendar_callback_page(
                if matches!(error, StorageError::PersistenceUnavailable) {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::BAD_REQUEST
                },
                "회사 Google 계정을 연결하지 못했어요",
                "앱에서 다시 연결해 주세요.",
            )
        }
    }
}

async fn finish_initial_calendar_sync(
    planning: &Database,
    calendar_oauth: &CalendarOAuthRuntime,
    account_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Response {
    match synchronize_google_calendar(planning, calendar_oauth, account_id, user_id).await {
        Ok(()) => calendar_callback_page(
            StatusCode::OK,
            "Google Calendar를 연결했어요",
            "일정을 불러왔어요. 이제 앱으로 돌아가도 됩니다.",
        ),
        Err(error) => {
            let _ = planning
                .mark_calendar_sync_failure(account_id, user_id, error.failure_code())
                .await;
            if error.is_connection_sync_failure() {
                calendar_callback_page(
                    StatusCode::OK,
                    "Google Calendar를 연결했어요",
                    "연결은 마쳤지만 일정을 아직 불러오지 못했어요. 앱에서 다시 가져와 주세요.",
                )
            } else {
                calendar_callback_error_page(error)
            }
        }
    }
}

#[utoipa::path(
    post,
    path = "/v1/calendar/connections/google/sync",
    tag = "calendar",
    responses(
        (status = 200, body = GoogleCalendarConnectionResponse),
        (status = 401),
        (status = 409),
        (status = 503)
    )
)]
async fn sync_google_calendar(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let Some(calendar_oauth) = state.calendar_oauth() else {
        return calendar_oauth_error_response(CalendarOAuthError::Configuration, request_id);
    };
    let user_id = principal.identity().user_id();
    let account = match planning.calendar_account_for_user(user_id).await {
        Ok(Some(account))
            if matches!(
                account.status,
                CalendarAccountStatus::Active | CalendarAccountStatus::Error
            ) =>
        {
            account
        }
        Ok(Some(_)) => {
            return error_response(
                StatusCode::CONFLICT,
                "calendar.connection_needs_attention",
                "Google Calendar 연결을 다시 확인해 주세요.",
                request_id,
                false,
            );
        }
        Ok(None) => {
            return error_response(
                StatusCode::CONFLICT,
                "calendar.connection_missing",
                "먼저 Google Calendar를 연결해 주세요.",
                request_id,
                false,
            );
        }
        Err(error) => return storage_error_response(&error, request_id),
    };
    match synchronize_google_calendar(planning, calendar_oauth, account.id, user_id).await {
        Ok(()) => match planning.calendar_account_for_user(user_id).await {
            Ok(connection) => Json(calendar_connection_response(connection, true)).into_response(),
            Err(error) => storage_error_response(&error, request_id),
        },
        Err(error) => {
            let _ = planning
                .mark_calendar_sync_failure(account.id, user_id, error.failure_code())
                .await;
            calendar_oauth_error_response(error, request_id)
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/google-chat/connections",
    tag = "google-chat",
    responses((status = 200, body = GoogleChatAccountListResponse), (status = 401), (status = 503))
)]
async fn list_google_chat_connections(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .google_chat_accounts_for_user(principal.identity().user_id())
        .await
    {
        Ok(accounts) => no_store_json(GoogleChatAccountListResponse {
            available: state.google_chat_oauth().is_some(),
            items: accounts
                .into_iter()
                .map(google_chat_account_response)
                .collect(),
        }),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/google-chat/connections/authorizations",
    tag = "google-chat",
    request_body = StartGoogleCalendarAuthorizationRequest,
    responses((status = 201, body = StartGoogleCalendarAuthorizationResponse), (status = 400), (status = 401), (status = 503))
)]
async fn start_google_chat_authorization(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(request): Json<StartGoogleCalendarAuthorizationRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(client_kind) = parse_client_platform(&request.client_kind) else {
        return invalid_request_response(request_id);
    };
    let Some(runtime) = state.google_chat_oauth() else {
        return google_chat_oauth_error_response(GoogleChatOAuthError::Configuration, request_id);
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let authorization_id = uuid::Uuid::now_v7();
    let authorization = match runtime.begin_authorization(authorization_id, client_kind) {
        Ok(authorization) => authorization,
        Err(error) => return google_chat_oauth_error_response(error, request_id),
    };
    let command = CreateGoogleChatOAuthAuthorization {
        id: authorization_id,
        user_id: principal.identity().user_id(),
        session_id: principal.identity().session_id(),
        device_id: principal.identity().device_id(),
        state_verifier: authorization.state_verifier,
        pkce_verifier: authorization.pkce_verifier,
        client_kind,
        expires_at: authorization.expires_at,
    };
    if let Err(error) = planning
        .create_google_chat_oauth_authorization(&command)
        .await
    {
        return storage_error_response(&error, request_id);
    }
    let Ok(expires_at) = authorization.expires_at.format(&Rfc3339) else {
        return unavailable_response(request_id);
    };
    (
        StatusCode::CREATED,
        Json(StartGoogleCalendarAuthorizationResponse {
            authorization_id,
            authorization_url: authorization.authorization_url,
            expires_at,
        }),
    )
        .into_response()
}

#[utoipa::path(
    delete,
    path = "/v1/google-chat/connections/{account_id}",
    tag = "google-chat",
    params(("account_id" = String, Path), ("expected_version" = i64, Query)),
    responses((status = 204), (status = 401), (status = 409), (status = 503))
)]
async fn delete_google_chat_connection(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(account_id): Path<uuid::Uuid>,
    Query(query): Query<DeleteVersionedConnectionQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let revocation_connection = match planning
        .google_chat_account_connection(user_id, account_id)
        .await
    {
        Ok(connection) => connection,
        Err(error) => return storage_error_response(&error, request_id),
    };
    match planning
        .delete_google_chat_account(user_id, account_id, query.expected_version)
        .await
    {
        Ok(true) => {
            if let (Some(runtime), Some(connection)) =
                (state.google_chat_oauth(), revocation_connection.as_ref())
            {
                let _ = runtime.revoke_account(connection).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => error_response(
            StatusCode::CONFLICT,
            "google_chat.connection_changed",
            "회사 Google 계정 상태가 달라졌어요. 다시 확인한 뒤 연결을 해제해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/google-chat/connections/{account_id}/spaces",
    tag = "google-chat",
    params(("account_id" = String, Path)),
    responses((status = 200, body = GoogleChatSpaceListResponse), (status = 401), (status = 404), (status = 503))
)]
async fn list_google_chat_spaces(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(account_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let Some(runtime) = state.google_chat_oauth() else {
        return google_chat_oauth_error_response(GoogleChatOAuthError::Configuration, request_id);
    };
    let connection = match planning
        .google_chat_account_connection(principal.identity().user_id(), account_id)
        .await
    {
        Ok(Some(connection)) => connection,
        Ok(None) => return not_found_response(request_id),
        Err(error) => return storage_error_response(&error, request_id),
    };
    match runtime.list_spaces(&connection).await {
        Ok(spaces) => no_store_json(GoogleChatSpaceListResponse {
            items: spaces
                .into_iter()
                .map(|space| GoogleChatSpaceResponse {
                    name: space.name,
                    display_name: space.display_name,
                })
                .collect(),
        }),
        Err(error) => google_chat_oauth_error_response(error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/projects/{project_id}/google-chat-sources",
    tag = "google-chat",
    params(("project_id" = String, Path)),
    responses((status = 200, body = ProjectGoogleChatSourceListResponse), (status = 401), (status = 503))
)]
async fn list_project_google_chat_sources(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .project_google_chat_sources(principal.identity().user_id(), project_id)
        .await
    {
        Ok(items) => no_store_json(ProjectGoogleChatSourceListResponse {
            items: items
                .into_iter()
                .filter_map(project_google_chat_source_response)
                .collect(),
        }),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/google-chat-sources",
    tag = "google-chat",
    params(("project_id" = String, Path)),
    request_body = CreateProjectGoogleChatSourceRequest,
    responses((status = 201, body = ProjectGoogleChatSourceResponse), (status = 400), (status = 401), (status = 503))
)]
async fn create_project_google_chat_source(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
    Json(request): Json<CreateProjectGoogleChatSourceRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let command = NewProjectGoogleChatSource {
        id: uuid::Uuid::now_v7(),
        user_id: principal.identity().user_id(),
        project_id,
        account_id: request.account_id,
        space_name: request.space_name,
        display_name: request.display_name,
        acknowledge_with_reaction: request.acknowledge_with_reaction,
        import_history: request.import_history,
    };
    match planning.create_project_google_chat_source(&command).await {
        Ok(source) => match project_google_chat_source_response(source) {
            Some(response) => (StatusCode::CREATED, Json(response)).into_response(),
            None => unavailable_response(request_id),
        },
        Err(StorageError::InvalidConfiguration) => invalid_request_response(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/projects/{project_id}/google-chat-sources/{source_id}",
    tag = "google-chat",
    params(
        ("project_id" = String, Path),
        ("source_id" = String, Path),
        ("expected_version" = i64, Query)
    ),
    responses((status = 204), (status = 401), (status = 409), (status = 503))
)]
async fn delete_project_google_chat_source(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, source_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    Query(query): Query<DeleteVersionedConnectionQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .delete_project_google_chat_source(
            principal.identity().user_id(),
            project_id,
            source_id,
            query.expected_version,
        )
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error_response(
            StatusCode::CONFLICT,
            "google_chat.source_changed",
            "연결된 Chat 공간 상태가 달라졌어요. 다시 확인해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/google-chat-sources/{source_id}/sync",
    tag = "google-chat",
    params(("project_id" = String, Path), ("source_id" = String, Path)),
    responses((status = 200, body = ProjectGoogleChatSourceListResponse), (status = 401), (status = 409), (status = 503))
)]
async fn sync_project_google_chat_source(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, source_id)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let Some(runtime) = state.google_chat_oauth() else {
        return google_chat_oauth_error_response(GoogleChatOAuthError::Configuration, request_id);
    };
    match synchronize_google_chat_source(
        planning,
        runtime,
        source_id,
        Some((principal.identity().user_id(), project_id)),
    )
    .await
    {
        Ok(()) => match planning
            .project_google_chat_sources(principal.identity().user_id(), project_id)
            .await
        {
            Ok(items) => no_store_json(ProjectGoogleChatSourceListResponse {
                items: items
                    .into_iter()
                    .filter_map(project_google_chat_source_response)
                    .collect(),
            }),
            Err(error) => storage_error_response(&error, request_id),
        },
        Err(error) => google_chat_oauth_error_response(error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/projects/{project_id}/inflow",
    tag = "google-chat",
    params(("project_id" = String, Path), ("status" = Option<String>, Query)),
    responses((status = 200, body = ProjectInflowItemListResponse), (status = 400), (status = 401), (status = 503))
)]
async fn list_project_inflow_items(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<uuid::Uuid>,
    Query(query): Query<ProjectInflowListQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let status = match query.status.as_deref() {
        None | Some("all") => None,
        Some("pending") => Some(ProjectInflowStatus::Pending),
        Some("promoted") => Some(ProjectInflowStatus::Promoted),
        Some("dismissed") => Some(ProjectInflowStatus::Dismissed),
        Some(_) => return invalid_request_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    match tokio::try_join!(
        planning.project_inflow_items(user_id, project_id, status),
        planning.project_webhooks(user_id, project_id),
    ) {
        Ok((items, webhooks)) => {
            let contexts = inflow_assignment_contexts(webhooks);
            let items = group_project_inflow_candidates(items)
                .into_iter()
                .map(project_inflow_item_response)
                .collect::<Result<Vec<_>, _>>()
                .map(|mut items| {
                    for item in &mut items {
                        apply_inflow_assignment_context(item, &contexts);
                    }
                    items
                });
            match items {
                Ok(items) => no_store_json(ProjectInflowItemListResponse { items }),
                Err(()) => unavailable_response(request_id),
            }
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/projects/{project_id}/inflow/{item_id}/decision",
    tag = "google-chat",
    params(("project_id" = String, Path), ("item_id" = String, Path)),
    request_body = ProjectInflowDecisionRequest,
    responses((status = 200, body = ProjectInflowItemResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
async fn decide_project_inflow_item(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project_id, item_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    Json(request): Json<ProjectInflowDecisionRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let result =
        apply_project_inflow_decision(planning, user_id, project_id, item_id, &request).await;
    match result {
        Ok(Some(mut item)) => {
            if matches!(request.decision.as_str(), "promote" | "retry_completion")
                && let Some(runtime) = state.google_chat_oauth()
                && let Ok(Some(connection)) = planning
                    .google_chat_source_sync_connection(item.source_id)
                    .await
            {
                if let Err(error) =
                    deliver_google_chat_completions(planning, runtime, &connection, Some(item.id))
                        .await
                {
                    warn!(
                        event = "google_chat.completion_delivery_deferred",
                        source_id = %item.source_id,
                        error_code = error.failure_code(),
                        "Google Chat completion delivery will be retried"
                    );
                }
                if let Ok(items) = planning
                    .project_inflow_items(user_id, project_id, Some(ProjectInflowStatus::Promoted))
                    .await
                    && let Some(refreshed) =
                        items.into_iter().find(|candidate| candidate.id == item.id)
                {
                    item = refreshed;
                }
            }
            match project_inflow_item_response(single_project_inflow_candidate(item)) {
                Ok(response) => no_store_json(response),
                Err(()) => unavailable_response(request_id),
            }
        }
        Ok(_) => {
            let (code, message) = if request.decision == "retry_completion" {
                (
                    "project.inflow_completion_changed",
                    "반영 상태가 바뀌었어요. 들어오는 업무를 다시 불러온 뒤 재시도해 주세요.",
                )
            } else {
                (
                    "project.inflow_changed",
                    "이 항목은 이미 처리되었어요. 들어오는 업무를 다시 불러와 주세요.",
                )
            };
            error_response(StatusCode::CONFLICT, code, message, request_id, false)
        }
        Err(StorageError::InvalidConfiguration) => invalid_request_response(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn apply_project_inflow_decision(
    planning: &Database,
    user_id: uuid::Uuid,
    project_id: uuid::Uuid,
    item_id: uuid::Uuid,
    request: &ProjectInflowDecisionRequest,
) -> Result<Option<ProjectInflowItem>, StorageError> {
    match request.decision.as_str() {
        "dismiss" => {
            planning
                .dismiss_project_inflow_item(user_id, project_id, item_id, request.expected_version)
                .await
        }
        "promote" => {
            let Some(title) = request.title.as_deref() else {
                return Err(StorageError::InvalidConfiguration);
            };
            let due_at = request
                .due_at
                .as_deref()
                .map(|value| OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ()))
                .transpose()
                .map_err(|()| StorageError::InvalidConfiguration)?;
            planning
                .promote_project_inflow_item(&PromoteProjectInflowItem {
                    user_id,
                    project_id,
                    item_id,
                    expected_version: request.expected_version,
                    task_id: uuid::Uuid::now_v7(),
                    title: title.to_owned(),
                    notes: request
                        .notes
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                    assignee_name: request
                        .assignee_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                    priority: request.priority.unwrap_or(1),
                    due_at,
                })
                .await
        }
        "retry_completion" => {
            planning
                .retry_project_inflow_completion(
                    user_id,
                    project_id,
                    item_id,
                    request.expected_version,
                )
                .await
        }
        _ => Err(StorageError::InvalidConfiguration),
    }
}

async fn synchronize_google_chat_source(
    planning: &Database,
    runtime: &GoogleChatOAuthRuntime,
    source_id: uuid::Uuid,
    expected_owner: Option<(uuid::Uuid, uuid::Uuid)>,
) -> Result<(), GoogleChatOAuthError> {
    let connection = planning
        .google_chat_source_sync_connection(source_id)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?
        .ok_or(GoogleChatOAuthError::ProviderRejected)?;
    if expected_owner.is_some_and(|(user_id, project_id)| {
        connection.user_id != user_id || connection.project_id != project_id
    }) {
        return Err(GoogleChatOAuthError::ProviderRejected);
    }
    let messages = runtime
        .list_source_messages(&connection, expected_owner.is_some())
        .await?;
    let acknowledgements = planning
        .apply_google_chat_messages(&connection, &messages)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
    if connection.acknowledge_with_reaction && !acknowledgements.is_empty() {
        let names = acknowledgements
            .iter()
            .map(|item| item.provider_message_name.clone())
            .collect::<Vec<_>>();
        if let Ok(outcomes) = runtime.acknowledge_messages(&connection, &names).await {
            for (item, acknowledged) in acknowledgements.iter().zip(outcomes) {
                if acknowledged {
                    let _ = planning
                        .mark_google_chat_inflow_acknowledged(connection.user_id, item.inflow_id)
                        .await;
                }
            }
        }
    }
    deliver_google_chat_completions(planning, runtime, &connection, None).await?;
    Ok(())
}

async fn deliver_google_chat_completions(
    planning: &Database,
    runtime: &GoogleChatOAuthRuntime,
    connection: &GoogleChatSourceSyncConnection,
    inflow_id: Option<uuid::Uuid>,
) -> Result<(), GoogleChatOAuthError> {
    let deliveries = planning
        .pending_google_chat_completion_deliveries(connection.source_id, inflow_id, 20)
        .await
        .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
    for delivery in deliveries {
        let reply = google_chat_completion_reply(&delivery);
        let outcome = runtime
            .deliver_completion(connection, &delivery, &reply)
            .await;
        planning
            .record_google_chat_completion_delivery(
                &delivery,
                outcome.reaction_completed,
                outcome.reply_completed,
                outcome.failure_code,
            )
            .await
            .map_err(|_| GoogleChatOAuthError::ProviderUnavailable)?;
        if let Some(error_code) = outcome.failure_code {
            warn!(
                event = "google_chat.completion_delivery_failed",
                source_id = %connection.source_id,
                attempt = delivery.attempt_count + 1,
                error_code,
                "Google Chat completion delivery is incomplete"
            );
        }
    }
    Ok(())
}

fn google_chat_completion_reply(delivery: &GoogleChatCompletionDelivery) -> String {
    let assignee = delivery.assignee_name.as_deref().unwrap_or("정하지 않음");
    let due_at = delivery
        .due_at
        .map_or_else(|| "정하지 않음".to_owned(), format_google_chat_due_at);
    format!(
        "✅ Jimin OS에서 할 일로 정리했어요.\n할 일: {}\n담당자: {assignee}\n마감일: {due_at}",
        delivery.task_title
    )
}

fn format_google_chat_due_at(value: OffsetDateTime) -> String {
    let korea_offset = UtcOffset::from_hms(9, 0, 0).expect("Korea offset should be valid");
    let value = value.to_offset(korea_offset);
    format!(
        "{}년 {}월 {}일 {:02}:{:02}",
        value.year(),
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute()
    )
}

async fn synchronize_google_calendar(
    planning: &Database,
    calendar_oauth: &CalendarOAuthRuntime,
    account_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), CalendarOAuthError> {
    let connection = planning
        .calendar_sync_connection(account_id, user_id)
        .await
        .map_err(|_| CalendarOAuthError::ProviderUnavailable)?
        .ok_or(CalendarOAuthError::ProviderUnavailable)?;
    let entries = calendar_oauth
        .initial_calendar_list_sync(&connection)
        .await?;
    planning
        .apply_calendar_list_sync(account_id, user_id, &entries)
        .await
        .map_err(|_| CalendarOAuthError::ProviderUnavailable)?;
    let targets = planning
        .calendar_sync_targets(account_id, user_id)
        .await
        .map_err(|_| CalendarOAuthError::ProviderUnavailable)?;
    let batches = calendar_oauth
        .calendar_event_sync(&connection, &targets)
        .await?;
    for batch in batches {
        if batch.is_full_sync {
            planning
                .apply_calendar_event_full_sync(
                    account_id,
                    user_id,
                    batch.calendar_id,
                    &batch.events,
                    &batch.next_sync_token,
                )
                .await
                .map_err(|_| CalendarOAuthError::ProviderUnavailable)?;
        } else {
            planning
                .apply_calendar_event_incremental_sync(
                    account_id,
                    user_id,
                    batch.calendar_id,
                    &batch.events,
                    &batch.next_sync_token,
                )
                .await
                .map_err(|_| CalendarOAuthError::ProviderUnavailable)?;
        }
    }
    match calendar_oauth.initial_gmail_inbox_sync(&connection).await {
        Ok(Some(messages)) => {
            if planning
                .apply_gmail_inbox_sync(user_id, &messages)
                .await
                .is_err()
            {
                let _ = planning
                    .mark_gmail_sync_failure(user_id, "gmail.provider_unavailable")
                    .await;
            }
        }
        Ok(None) => {}
        Err(error) => {
            let _ = planning
                .mark_gmail_sync_failure(user_id, error.failure_code())
                .await;
        }
    }
    Ok(())
}

fn calendar_oauth_error_response(error: CalendarOAuthError, request_id: RequestId) -> Response {
    let (status, code, message) = match error {
        CalendarOAuthError::Configuration => (
            StatusCode::SERVICE_UNAVAILABLE,
            "calendar.configuration_missing",
            "Google Calendar 연결을 아직 준비하고 있어요.",
        ),
        CalendarOAuthError::ProviderUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "calendar.provider_unavailable",
            "Google Calendar에 연결할 수 없어요. 잠시 후 다시 시도해 주세요.",
        ),
        CalendarOAuthError::SyncDataInvalid => (
            StatusCode::BAD_GATEWAY,
            "calendar.sync_data_invalid",
            "일부 Google Calendar 일정을 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
        ),
        CalendarOAuthError::Conflict => (
            StatusCode::CONFLICT,
            "calendar.event_conflict",
            "Google Calendar에서 일정이 먼저 변경됐어요. 최신 상태를 확인해 주세요.",
        ),
        CalendarOAuthError::EventNotFound => (
            StatusCode::CONFLICT,
            "calendar.event_not_found",
            "Google Calendar에서 일정을 찾을 수 없어요. 일정을 새로고침해 주세요.",
        ),
        CalendarOAuthError::EventRejected => (
            StatusCode::BAD_REQUEST,
            "calendar.event_rejected",
            "Google Calendar에 반영할 수 없는 일정이에요. 내용을 확인해 주세요.",
        ),
        CalendarOAuthError::IdentityMismatch => (
            StatusCode::FORBIDDEN,
            "calendar.account_mismatch",
            "로그인한 Google 계정을 확인한 뒤 다시 연결해 주세요.",
        ),
        CalendarOAuthError::InvalidCallback
        | CalendarOAuthError::ProviderRejected
        | CalendarOAuthError::RequiredScopeMissing
        | CalendarOAuthError::Encryption => (
            StatusCode::BAD_REQUEST,
            "calendar.authorization_failed",
            "Google Calendar 연결을 다시 진행해 주세요.",
        ),
    };
    error_response(status, code, message, request_id, error.retryable())
}

fn calendar_callback_error_page(error: CalendarOAuthError) -> Response {
    let message = match error {
        CalendarOAuthError::ProviderUnavailable => {
            "Google Calendar에 연결할 수 없어요. 잠시 후 앱에서 다시 시도해 주세요."
        }
        CalendarOAuthError::SyncDataInvalid => {
            "일부 Google Calendar 일정을 불러오지 못했어요. 앱에서 다시 가져와 주세요."
        }
        CalendarOAuthError::Conflict => {
            "Google Calendar에서 일정이 변경됐어요. 앱에서 새로고침한 뒤 다시 시도해 주세요."
        }
        CalendarOAuthError::EventNotFound => {
            "Google Calendar에서 일정을 찾을 수 없어요. 앱에서 새로고침해 주세요."
        }
        CalendarOAuthError::EventRejected => {
            "Google Calendar에 반영할 수 없는 일정이에요. 내용을 확인해 주세요."
        }
        CalendarOAuthError::IdentityMismatch => {
            "Jimin OS에 로그인한 계정과 같은 Google 계정으로 다시 연결해 주세요."
        }
        CalendarOAuthError::RequiredScopeMissing => {
            "필요한 Calendar 권한이 허용되지 않았어요. 앱에서 다시 연결해 주세요."
        }
        CalendarOAuthError::Configuration
        | CalendarOAuthError::InvalidCallback
        | CalendarOAuthError::ProviderRejected
        | CalendarOAuthError::Encryption => "앱에서 Google Calendar 연결을 다시 시도해 주세요.",
    };
    calendar_callback_page(
        if error.retryable() {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_REQUEST
        },
        "연결을 완료하지 못했어요",
        message,
    )
}

fn google_chat_oauth_error_response(
    error: GoogleChatOAuthError,
    request_id: RequestId,
) -> Response {
    let (status, message) = match error {
        GoogleChatOAuthError::Configuration => (
            StatusCode::SERVICE_UNAVAILABLE,
            "회사 Google 계정 연결을 아직 준비하고 있어요.",
        ),
        GoogleChatOAuthError::ProviderUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Google Chat에 연결할 수 없어요. 잠시 후 다시 시도해 주세요.",
        ),
        GoogleChatOAuthError::RequiredScopeMissing => (
            StatusCode::FORBIDDEN,
            "프로젝트 메시지를 확인할 권한이 부족해요. 회사 계정을 다시 연결해 주세요.",
        ),
        GoogleChatOAuthError::InvalidCallback
        | GoogleChatOAuthError::ProviderRejected
        | GoogleChatOAuthError::Encryption => (
            StatusCode::BAD_REQUEST,
            "회사 Google 계정 연결을 다시 진행해 주세요.",
        ),
    };
    error_response(
        status,
        error.failure_code(),
        message,
        request_id,
        error.retryable(),
    )
}

fn google_chat_callback_error_page(error: GoogleChatOAuthError) -> Response {
    let message = match error {
        GoogleChatOAuthError::ProviderUnavailable => {
            "Google Chat에 연결할 수 없어요. 잠시 후 앱에서 다시 시도해 주세요."
        }
        GoogleChatOAuthError::RequiredScopeMissing => {
            "Chat 공간과 메시지를 확인할 권한을 허용한 뒤 다시 연결해 주세요."
        }
        GoogleChatOAuthError::Configuration
        | GoogleChatOAuthError::InvalidCallback
        | GoogleChatOAuthError::ProviderRejected
        | GoogleChatOAuthError::Encryption => "앱에서 회사 Google 계정 연결을 다시 시도해 주세요.",
    };
    calendar_callback_page(
        if error.retryable() {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_REQUEST
        },
        "회사 Google 계정을 연결하지 못했어요",
        message,
    )
}

fn calendar_callback_page(status: StatusCode, title: &str, message: &str) -> Response {
    let page = format!(
        "<!doctype html><html lang=\"ko\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title><body><main><h1>{title}</h1><p>{message}</p></main></body></html>"
    );
    let mut response = (status, page).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}

fn parse_client_platform(value: &str) -> Option<ClientPlatform> {
    match value {
        "macos" => Some(ClientPlatform::Macos),
        "ios" => Some(ClientPlatform::Ios),
        "android" => Some(ClientPlatform::Android),
        _ => None,
    }
}

fn calendar_connection_response(
    account: Option<CalendarAccount>,
    available: bool,
) -> GoogleCalendarConnectionResponse {
    let Some(account) = account else {
        return GoogleCalendarConnectionResponse {
            available,
            status: "not_connected".to_owned(),
            email: None,
            granted_scopes: Vec::new(),
            last_successful_sync_at: None,
            last_error_code: None,
            reauth_required: false,
            version: None,
        };
    };
    let status = match account.status {
        CalendarAccountStatus::Connecting => "connecting",
        CalendarAccountStatus::Active => "active",
        CalendarAccountStatus::ReauthRequired => "reauth_required",
        CalendarAccountStatus::Revoking => "revoking",
        CalendarAccountStatus::Revoked => "revoked",
        CalendarAccountStatus::Error => "error",
    };
    GoogleCalendarConnectionResponse {
        available,
        status: status.to_owned(),
        email: Some(account.email),
        granted_scopes: account.granted_scopes,
        last_successful_sync_at: account
            .last_successful_sync_at
            .map(|value| value.format(&Rfc3339).unwrap_or_default()),
        last_error_code: account.last_error_code,
        reauth_required: account.status == CalendarAccountStatus::ReauthRequired,
        version: Some(account.version),
    }
}

fn google_chat_account_response(account: GoogleChatAccount) -> GoogleChatAccountResponse {
    let write_scope_missing =
        !GoogleChatOAuthRuntime::completion_scope_granted(&account.granted_scopes);
    let status = match account.status {
        GoogleChatAccountStatus::Connecting => "connecting",
        GoogleChatAccountStatus::Active if write_scope_missing => "reauth_required",
        GoogleChatAccountStatus::Active => "active",
        GoogleChatAccountStatus::ReauthRequired => "reauth_required",
        GoogleChatAccountStatus::Revoking => "revoking",
        GoogleChatAccountStatus::Revoked => "revoked",
        GoogleChatAccountStatus::Error => "error",
    };
    GoogleChatAccountResponse {
        id: account.id,
        email: account.email,
        status: status.to_owned(),
        last_successful_sync_at: account
            .last_successful_sync_at
            .and_then(|value| value.format(&Rfc3339).ok()),
        last_error_code: if write_scope_missing {
            Some("google_chat.write_scope_missing".to_owned())
        } else {
            account.last_error_code
        },
        reauth_required: account.status == GoogleChatAccountStatus::ReauthRequired
            || write_scope_missing,
        version: account.version,
    }
}

fn project_google_chat_source_response(
    source: ProjectGoogleChatSource,
) -> Option<ProjectGoogleChatSourceResponse> {
    Some(ProjectGoogleChatSourceResponse {
        id: source.id,
        project_id: source.project_id,
        account_id: source.account_id,
        account_email: source.account_email,
        space_name: source.space_name,
        display_name: source.display_name,
        enabled: source.enabled,
        acknowledge_with_reaction: source.acknowledge_with_reaction,
        last_successful_sync_at: source
            .last_successful_sync_at
            .map(|value| value.format(&Rfc3339))
            .transpose()
            .ok()?,
        last_error_code: source.last_error_code,
        version: source.version,
    })
}

struct ProjectInflowCandidate {
    representative: ProjectInflowItem,
    focus: ProjectInflowItem,
    messages: Vec<ProjectInflowItem>,
}

#[derive(Default)]
struct InflowAssignmentContext {
    names: BTreeSet<String>,
    notifiable_names: BTreeSet<String>,
}

fn inflow_assignment_contexts(
    webhooks: Vec<ProjectWebhook>,
) -> BTreeMap<uuid::Uuid, InflowAssignmentContext> {
    let mut contexts: BTreeMap<uuid::Uuid, InflowAssignmentContext> = BTreeMap::new();
    for webhook in webhooks
        .into_iter()
        .filter(|webhook| webhook.provider == WebhookProvider::GoogleChat)
    {
        let context = contexts.entry(webhook.project_id).or_default();
        let names = webhook
            .mention_directory
            .users
            .into_keys()
            .collect::<BTreeSet<_>>();
        context.names.extend(names.iter().cloned());
        if webhook.enabled && webhook.events.iter().any(|event| event == "task.created") {
            context.notifiable_names.extend(names);
        }
    }
    contexts
}

fn apply_inflow_assignment_context(
    response: &mut ProjectInflowItemResponse,
    contexts: &BTreeMap<uuid::Uuid, InflowAssignmentContext>,
) {
    let Some(context) = contexts.get(&response.project_id) else {
        return;
    };
    response.assignee_options = context.names.iter().cloned().collect();
    response.notifiable_assignee_names = context.notifiable_names.iter().cloned().collect();
    response.assignee_notification_available = !context.notifiable_names.is_empty();
}

fn single_project_inflow_candidate(item: ProjectInflowItem) -> ProjectInflowCandidate {
    ProjectInflowCandidate {
        representative: item.clone(),
        focus: item.clone(),
        messages: vec![item],
    }
}

fn group_project_inflow_candidates(items: Vec<ProjectInflowItem>) -> Vec<ProjectInflowCandidate> {
    let mut groups =
        BTreeMap::<(uuid::Uuid, String, &'static str, Option<uuid::Uuid>), Vec<_>>::new();
    for item in items {
        let group = item.provider_thread_name.clone().map_or_else(
            || format!("message:{}", item.id),
            |thread| format!("thread:{thread}"),
        );
        let status = project_inflow_status_name(item.status);
        groups
            .entry((item.source_id, group, status, item.promoted_task_id))
            .or_default()
            .push(item);
    }

    let mut candidates = groups
        .into_values()
        .filter_map(|mut messages| {
            messages.sort_by_key(|item| (item.received_at, item.id));
            let focus = messages
                .iter()
                .filter(|item| !item.sent_by_owner)
                .max_by_key(|item| {
                    (
                        inflow_actionability_score(&item.content_text),
                        item.content_text.chars().count(),
                    )
                })?
                .clone();
            let representative = messages.last()?.clone();
            if representative.status == ProjectInflowStatus::Pending
                && inflow_actionability_score(&focus.content_text) < 2
            {
                return None;
            }
            Some(ProjectInflowCandidate {
                representative,
                focus,
                messages,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.representative.received_at));
    candidates
}

fn project_inflow_status_name(status: ProjectInflowStatus) -> &'static str {
    match status {
        ProjectInflowStatus::Pending => "pending",
        ProjectInflowStatus::Promoted => "promoted",
        ProjectInflowStatus::Dismissed => "dismissed",
    }
}

fn inflow_actionability_score(content: &str) -> u8 {
    let normalized = content.trim().to_lowercase();
    let meaningful_chars = normalized
        .chars()
        .filter(|character| character.is_alphanumeric())
        .count();
    if meaningful_chars < 4 {
        return 0;
    }
    let compact_follow_up = meaningful_chars < 18
        && !normalized.contains('?')
        && !normalized.contains('？')
        && ["확인일자", "리마인드", "리체크", "먼저", "금일", "메모"]
            .iter()
            .any(|term| normalized.contains(term));
    if compact_follow_up {
        return 0;
    }
    let request_terms = [
        "요청",
        "확인",
        "검토",
        "개발",
        "수정",
        "오류",
        "안되",
        "안돼",
        "해야",
        "필요",
        "가능",
        "전달",
        "등록",
        "처리",
        "일정",
        "문의",
        "부탁",
        "언제",
        "얼마나",
        "request",
        "check",
        "review",
        "fix",
        "issue",
        "error",
        "need",
        "please",
    ];
    let mut score = 0;
    if request_terms.iter().any(|term| normalized.contains(term)) {
        score += 3;
    }
    if normalized.contains('?') || normalized.contains('？') {
        score += 2;
    }
    if meaningful_chars >= 24 {
        score += 1;
    }
    score
}

fn suggested_inflow_task_title(content: &str) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = normalized
        .trim_start_matches("혹시 ")
        .trim_start_matches("혹시")
        .trim();
    let lowered = normalized.to_lowercase();
    let task_like_title = if lowered.contains("개발")
        && (lowered.contains("얼마나 걸")
            || lowered.contains("소요 시간")
            || lowered.contains("예상 일정"))
    {
        Some("개발 범위와 예상 일정 확인")
    } else if (lowered.contains("안되") || lowered.contains("안돼") || lowered.contains("오류"))
        && (lowered.contains("확인") || lowered.contains("처리") || lowered.contains("대응"))
    {
        Some("문제 원인 확인 및 대응")
    } else if lowered.contains("안내") && (lowered.contains("해야") || lowered.contains("필요"))
    {
        Some("안내할 내용 확인 및 전달")
    } else if lowered.contains("전달") && (lowered.contains("확실") || lowered.contains("정확"))
    {
        Some("전달할 내용과 기준 확인")
    } else {
        None
    };
    if let Some(title) = task_like_title {
        return title.to_owned();
    }
    let title = normalized
        .trim_end_matches(['.', '?', '!', '。', '？'])
        .chars()
        .take(100)
        .collect::<String>();
    if title.chars().count() >= 4 {
        title
    } else {
        "대화 내용 확인".to_owned()
    }
}

fn suggested_inflow_task_notes(title: &str, messages: &[ProjectInflowItem]) -> String {
    let normalized_messages = messages
        .iter()
        .map(|item| item.content_text.to_lowercase())
        .collect::<Vec<_>>();
    let combined = normalized_messages.join("\n");
    let mut points = Vec::new();
    let mut seen = BTreeSet::new();

    if combined.contains("qr")
        && combined.contains("결제")
        && (combined.contains("배송") || combined.contains("통지") || combined.contains("통보"))
    {
        add_unique_inflow_summary_point(
            &mut points,
            &mut seen,
            "QR 결제 거래·배송 정보 통지 연동 범위를 확인합니다.",
        );
    }
    if combined.contains("가이드") {
        add_unique_inflow_summary_point(
            &mut points,
            &mut seen,
            "제공된 연동 가이드와 테스트 조건을 확인합니다.",
        );
    }
    if combined.contains("url") && combined.contains("회신") {
        add_unique_inflow_summary_point(
            &mut points,
            &mut seen,
            "거래 통지 수신 URL을 개발하고 연동처에 회신합니다.",
        );
    }
    if combined.contains("거래내역")
        && (combined.contains("목적")
            || combined.contains("쌓")
            || combined.contains("저장")
            || combined.contains("보는"))
    {
        add_unique_inflow_summary_point(
            &mut points,
            &mut seen,
            "거래내역 조회·저장 목적과 처리 범위를 정합니다.",
        );
    }
    if combined.contains("테스트 페이지") || combined.contains("테스트 환경") {
        add_unique_inflow_summary_point(
            &mut points,
            &mut seen,
            "제공된 테스트 환경에서 확인 가능한 기능과 검증 방법을 점검합니다.",
        );
    }

    if points.is_empty() {
        for message in messages {
            for line in message
                .content_text
                .split(['\n', '\r'])
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                if sensitive_inflow_summary_line(line) {
                    add_unique_inflow_summary_point(
                        &mut points,
                        &mut seen,
                        "제공된 테스트 환경과 계정으로 연동 동작을 확인합니다.",
                    );
                    continue;
                }
                let Some(line) = concise_inflow_summary_line(line) else {
                    continue;
                };
                add_unique_inflow_summary_point(&mut points, &mut seen, &line);
            }
        }
    }
    if points.is_empty() {
        add_unique_inflow_summary_point(
            &mut points,
            &mut seen,
            "요청 범위와 필요한 결과를 확인합니다.",
        );
    }

    format!(
        "업무 목적\n{}\n\n확인할 내용\n{}\n\n완료 기준\n요청 범위를 확정하고 처리 결과를 관계자에게 공유합니다.",
        title.trim(),
        points
            .into_iter()
            .map(|point| format!("- {point}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn add_unique_inflow_summary_point(
    points: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    value: &str,
) {
    if points.len() >= 5 {
        return;
    }
    let value = value.trim();
    if !value.is_empty() && seen.insert(value.to_owned()) {
        points.push(value.to_owned());
    }
}

fn sensitive_inflow_summary_line(line: &str) -> bool {
    let normalized = line.to_lowercase();
    [
        "비밀번호",
        "password",
        "passwd",
        "테스트 계정",
        "계정 정보",
        "access token",
        "api key",
        "secret",
    ]
    .iter()
    .any(|term| normalized.contains(term))
}

fn concise_inflow_summary_line(line: &str) -> Option<String> {
    let mut value = line
        .trim()
        .trim_start_matches("보낸 사람 정보 없음:")
        .trim_start_matches("안녕하세요.")
        .trim_start_matches("안녕하세요,")
        .trim()
        .trim_start_matches(['-', '*', '•'])
        .trim()
        .to_owned();
    for suffix in [
        "확인 부탁드립니다.",
        "확인 부탁드립니다",
        "부탁드립니다.",
        "부탁드립니다",
    ] {
        if value.ends_with(suffix) {
            value.truncate(value.len() - suffix.len());
            value = value.trim_end().to_owned();
        }
    }
    if value.starts_with("URL:")
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value
            .chars()
            .filter(|character| character.is_alphanumeric())
            .count()
            < 8
    {
        return None;
    }
    let mut value = value.chars().take(140).collect::<String>();
    if !value.ends_with(['.', '!', '?', '요', '다']) {
        value.push('.');
    }
    Some(value)
}

fn project_inflow_item_response(
    candidate: ProjectInflowCandidate,
) -> Result<ProjectInflowItemResponse, ()> {
    let ProjectInflowCandidate {
        representative,
        focus,
        messages,
    } = candidate;
    let first_received_at = messages.first().ok_or(())?.received_at;
    let acknowledged = messages.iter().all(|item| item.acknowledged_at.is_some());
    let completion = messages
        .iter()
        .find(|item| item.completion_requested_at.is_some());
    let completion_reaction_completed =
        completion.is_some_and(|item| item.completion_reaction_at.is_some());
    let completion_reply_completed =
        completion.is_some_and(|item| item.completion_reply_at.is_some());
    let completion_error_code =
        completion.and_then(|item| item.completion_delivery_error_code.clone());
    let completion_attempt_count =
        completion.map_or(0, |item| item.completion_delivery_attempt_count);
    let completion_status = if completion.is_none() {
        "not_requested"
    } else if completion_reaction_completed && completion_reply_completed {
        "sent"
    } else if completion_error_code.is_some() {
        "failed"
    } else {
        "pending"
    };
    let message_count = messages.len();
    let suggested_task_title = suggested_inflow_task_title(&focus.content_text);
    let suggested_task_notes =
        suggested_inflow_task_notes(&suggested_task_title, messages.as_slice());
    let messages = messages
        .into_iter()
        .map(|item| {
            Ok(ProjectInflowMessageResponse {
                sender_name: item
                    .sent_by_owner
                    .then(|| "나".to_owned())
                    .or(item.sender_name),
                sent_by_owner: item.sent_by_owner,
                content_text: item.content_text,
                received_at: item.received_at.format(&Rfc3339).map_err(|_| ())?,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    Ok(ProjectInflowItemResponse {
        id: focus.id,
        project_id: representative.project_id,
        project_name: representative.project_name,
        source_id: representative.source_id,
        source_name: representative.source_name,
        sender_name: focus
            .sent_by_owner
            .then(|| "나".to_owned())
            .or(focus.sender_name),
        sent_by_owner: focus.sent_by_owner,
        suggested_task_title,
        suggested_task_notes,
        content_text: focus.content_text,
        message_count,
        first_received_at: first_received_at.format(&Rfc3339).map_err(|_| ())?,
        received_at: representative
            .received_at
            .format(&Rfc3339)
            .map_err(|_| ())?,
        messages,
        status: project_inflow_status_name(representative.status).to_owned(),
        promoted_task_id: representative.promoted_task_id,
        acknowledged,
        completion_status: completion_status.to_owned(),
        completion_reaction_completed,
        completion_reply_completed,
        completion_error_code,
        completion_attempt_count,
        assignee_options: Vec::new(),
        notifiable_assignee_names: Vec::new(),
        assignee_notification_available: false,
        version: focus.version,
    })
}

#[utoipa::path(
    get,
    path = "/v1/sync/changes",
    tag = "sync",
    params(
        ("after" = i64, Query, description = "Last fully applied sync sequence"),
        ("limit" = Option<i64>, Query, description = "Page size from 1 through 200")
    ),
    responses(
        (status = 200, body = SyncChangeListResponse),
        (status = 400),
        (status = 401),
        (status = 503)
    )
)]
async fn list_sync_changes(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Query(query): Query<SyncChangesQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    let limit = query.limit.unwrap_or(100);
    if query.after < 0 || !(1..=200).contains(&limit) {
        return invalid_request_response(request_id);
    }

    match database
        .sync_changes_for_user(principal.identity().user_id(), query.after, limit)
        .await
    {
        Ok(page) => {
            let items = page
                .items
                .into_iter()
                .map(sync_change_response)
                .collect::<Result<Vec<_>, _>>();
            match items {
                Ok(items) => no_store_json(SyncChangeListResponse {
                    items,
                    next_cursor: page.next_cursor.to_string(),
                    current_cursor: page.current_cursor.to_string(),
                    has_more: page.has_more,
                }),
                Err(()) => unavailable_response(request_id),
            }
        }
        Err(StorageError::InvalidConfiguration) => invalid_request_response(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/sync/stream",
    tag = "sync",
    params(("after" = i64, Query, description = "Last fully applied sync sequence")),
    responses(
        (status = 200, description = "Authenticated server-sent sync cursor updates"),
        (status = 400),
        (status = 401),
        (status = 503)
    )
)]
async fn stream_sync_changes(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Query(query): Query<SyncStreamQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    if query.after < 0 {
        return invalid_request_response(request_id);
    }
    let Some(database) = state.planning().cloned() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();

    let stream = futures_util::stream::unfold(
        SyncStreamState {
            database,
            user_id,
            last_cursor: query.after,
        },
        |mut stream_state| async move {
            loop {
                let Ok(cursor) = stream_state
                    .database
                    .current_sync_cursor_for_user(stream_state.user_id)
                    .await
                else {
                    return None;
                };
                if cursor > stream_state.last_cursor {
                    let Ok(data) = serde_json::to_string(&SyncCursorEvent {
                        cursor: cursor.to_string(),
                    }) else {
                        return None;
                    };
                    stream_state.last_cursor = cursor;
                    return Some((
                        Ok::<Event, Infallible>(Event::default().event("cursor").data(data)),
                        stream_state,
                    ));
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        },
    );
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("keep-alive"),
        )
        .into_response()
}

struct SyncStreamState {
    database: Database,
    user_id: uuid::Uuid,
    last_cursor: i64,
}

fn sync_change_response(change: SyncChange) -> Result<SyncChangeResponse, ()> {
    Ok(SyncChangeResponse {
        sequence: change.sequence.to_string(),
        entity_type: change.entity_type,
        entity_id: change.entity_id,
        operation: change.operation,
        entity_version: change.entity_version,
        changed_at: change.changed_at.format(&Rfc3339).map_err(|_| ())?,
    })
}

#[utoipa::path(
    post,
    path = "/v1/auth/refresh",
    tag = "identity",
    responses(
        (status = 200, description = "Refresh token rotated into a new Jimin OS device session", body = DeviceSessionResponse),
        (status = 401, description = "Refresh token is invalid, expired, or reused"),
        (status = 503, description = "Authentication service is temporarily unavailable")
    )
)]
async fn refresh_session(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    Json(request): Json<RefreshSessionRequest>,
) -> Response {
    let Some(pairing) = state.pairing() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service.temporarily_unavailable",
            "잠시 후 다시 시도해 주세요.",
            request_id,
            true,
        );
    };
    let session = match pairing
        .sessions
        .refresh(request.refresh_token, uuid::Uuid::now_v7())
        .await
    {
        Ok(session) => session,
        Err(error) => return application_error_response(&error, request_id),
    };
    match device_session_response(&session) {
        Ok(response) => no_store_json(response),
        Err(()) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service.temporarily_unavailable",
            "잠시 후 다시 시도해 주세요.",
            request_id,
            true,
        ),
    }
}

fn device_session_response(session: &DeviceSession) -> Result<DeviceSessionResponse, ()> {
    let expires_at = OffsetDateTime::from(session.access_token().expires_at())
        .format(&Rfc3339)
        .map_err(|_| ())?;
    let sync_cursor = session.sync_cursor().ok_or(())?.to_string();
    Ok(DeviceSessionResponse {
        access_token: session.access_token().token().expose_secret().to_owned(),
        access_token_expires_at: expires_at,
        refresh_token: session
            .refresh_token()
            .serialized()
            .expose_secret()
            .to_owned(),
        user: me_response(session.profile().clone()),
        device: device_response(session.device().clone()),
        sync_cursor,
    })
}

fn invalid_request_response(request_id: RequestId) -> Response {
    error_response(
        StatusCode::BAD_REQUEST,
        "request.invalid",
        "입력한 내용을 다시 확인해 주세요.",
        request_id,
        false,
    )
}

fn parse_optional_timestamp(value: Option<String>) -> Result<Option<OffsetDateTime>, ()> {
    value
        .map(|value| OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| ()))
        .transpose()
}

fn application_error_response(error: &ApplicationError, request_id: RequestId) -> Response {
    match error {
        ApplicationError::InvalidIdentity | ApplicationError::InvalidSessionLifetime => {
            invalid_request_response(request_id)
        }
        ApplicationError::PairingRejected => error_response(
            StatusCode::UNAUTHORIZED,
            "auth.pairing_rejected",
            "개인 서버 연결을 다시 확인해 주세요.",
            request_id,
            false,
        ),
        ApplicationError::SessionExpired => {
            auth::AuthenticationFailure::Unauthorized.into_response(request_id)
        }
        ApplicationError::RefreshReused => error_response(
            StatusCode::UNAUTHORIZED,
            "auth.refresh_reused",
            "보안을 위해 다시 로그인해 주세요.",
            request_id,
            false,
        ),
        ApplicationError::Storage(_) | ApplicationError::AccessToken(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service.temporarily_unavailable",
            "잠시 후 다시 시도해 주세요.",
            request_id,
            true,
        ),
    }
}

fn no_store_json<T>(payload: T) -> Response
where
    T: Serialize,
{
    let mut response = Json(payload).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

fn unavailable_response(request_id: RequestId) -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "service.temporarily_unavailable",
        "잠시 후 다시 시도해 주세요.",
        request_id,
        true,
    )
}

fn agent_not_found_response(request_id: RequestId) -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "agent.not_found",
        "대화 정보를 찾을 수 없어요. 대화 목록을 다시 확인해 주세요.",
        request_id,
        false,
    )
}

fn storage_error_response(error: &StorageError, request_id: RequestId) -> Response {
    match error {
        StorageError::InvalidConfiguration | StorageError::IdentityConflict => {
            invalid_request_response(request_id)
        }
        StorageError::MigrationUnavailable | StorageError::PersistenceUnavailable => {
            unavailable_response(request_id)
        }
    }
}

fn schedule_entry_response(entry: ScheduleEntry) -> Result<ScheduleEntryResponse, ()> {
    Ok(ScheduleEntryResponse {
        id: entry.id,
        title: entry.title,
        notes: entry.notes,
        starts_at: entry.starts_at.format(&Rfc3339).map_err(|_| ())?,
        ends_at: entry.ends_at.format(&Rfc3339).map_err(|_| ())?,
        time_zone: entry.time_zone,
        status: match entry.status {
            ScheduleStatus::Confirmed => "confirmed".to_owned(),
            ScheduleStatus::Cancelled => "cancelled".to_owned(),
        },
        source: match entry.source {
            ScheduleSource::Manual => "manual".to_owned(),
            ScheduleSource::GoogleCalendar => "google_calendar".to_owned(),
        },
        editable: entry.editable,
        version: entry.version,
    })
}

fn task_response(task: Task) -> Result<TaskResponse, ()> {
    Ok(TaskResponse {
        id: task.id,
        project_id: task.project_id,
        title: task.title,
        notes: task.notes,
        assignee_name: task.assignee_name,
        status: match task.status {
            TaskStatus::Open => "open".to_owned(),
            TaskStatus::Completed => "completed".to_owned(),
            TaskStatus::Cancelled => "cancelled".to_owned(),
        },
        priority: task.priority,
        due_at: task
            .due_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        completed_at: task
            .completed_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        version: task.version,
    })
}

fn recommendation_response(recommendation: Recommendation) -> Result<RecommendationResponse, ()> {
    Ok(RecommendationResponse {
        id: recommendation.id,
        workspace_id: recommendation.workspace_id,
        project_id: recommendation.project_id,
        goal_id: recommendation.goal_id,
        signal_id: recommendation.signal_id,
        title: recommendation.title,
        rationale: recommendation.rationale,
        expected_effect: recommendation.expected_effect,
        risk_summary: recommendation.risk_summary,
        confidence: recommendation.confidence,
        urgency: recommendation.urgency,
        impact: recommendation.impact,
        risk_level: recommendation.risk_level,
        effort_minutes: recommendation.effort_minutes,
        suggested_action_kind: recommendation
            .suggested_action_kind
            .map(suggested_action_kind_name)
            .map(str::to_owned),
        suggested_entity_id: recommendation.suggested_entity_id,
        status: recommendation_status_name(recommendation.status).to_owned(),
        valid_until: recommendation
            .valid_until
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        revisit_at: recommendation
            .revisit_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        created_at: recommendation.created_at.format(&Rfc3339).map_err(|_| ())?,
        updated_at: recommendation.updated_at.format(&Rfc3339).map_err(|_| ())?,
        version: recommendation.version,
    })
}

const fn recommendation_status_name(status: RecommendationStatus) -> &'static str {
    match status {
        RecommendationStatus::Pending => "pending",
        RecommendationStatus::Approved => "approved",
        RecommendationStatus::Rejected => "rejected",
        RecommendationStatus::Deferred => "deferred",
        RecommendationStatus::AnalysisRequested => "analysis_requested",
        RecommendationStatus::Executing => "executing",
        RecommendationStatus::Executed => "executed",
        RecommendationStatus::Failed => "failed",
        RecommendationStatus::Expired => "expired",
    }
}

const fn suggested_action_kind_name(kind: SuggestedActionKind) -> &'static str {
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

fn workspace_response(workspace: Workspace) -> WorkspaceResponse {
    WorkspaceResponse {
        id: workspace.id,
        scope: match workspace.scope {
            WorkspaceScope::Personal => "personal".to_owned(),
            WorkspaceScope::Company => "company".to_owned(),
        },
        name: workspace.name,
        version: workspace.version,
    }
}

fn goal_response(overview: GoalOverview) -> Result<GoalResponse, ()> {
    let goal = overview.goal;
    Ok(GoalResponse {
        id: goal.id,
        workspace_id: goal.workspace_id,
        project_id: goal.project_id,
        title: goal.title,
        desired_outcome: goal.desired_outcome,
        status: match goal.status {
            GoalStatus::Active => "active".to_owned(),
            GoalStatus::Paused => "paused".to_owned(),
            GoalStatus::Achieved => "achieved".to_owned(),
            GoalStatus::Cancelled => "cancelled".to_owned(),
        },
        target_at: goal
            .target_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        project_title: overview.project_title,
        progress_percent: overview.progress_percent,
        total_task_count: overview.total_task_count,
        open_task_count: overview.open_task_count,
        completed_task_count: overview.completed_task_count,
        completed_last_seven_days: overview.completed_last_seven_days,
        overdue_task_count: overview.overdue_task_count,
        health: match overview.health {
            GoalHealth::OnTrack => "on_track",
            GoalHealth::AtRisk => "at_risk",
            GoalHealth::NeedsPlan => "needs_plan",
            GoalHealth::ReadyToComplete => "ready_to_complete",
            GoalHealth::Paused => "paused",
            GoalHealth::Achieved => "achieved",
        }
        .to_owned(),
        next_action: overview
            .next_action
            .map(|action| {
                Ok::<GoalNextActionResponse, ()>(GoalNextActionResponse {
                    kind: match action.kind {
                        GoalNextActionKind::Task => "task",
                        GoalNextActionKind::Project => "project",
                    }
                    .to_owned(),
                    id: action.id,
                    title: action.title,
                    due_at: action
                        .due_at
                        .map(|value| value.format(&Rfc3339).map_err(|_| ()))
                        .transpose()?,
                })
            })
            .transpose()?,
        created_at: goal.created_at.format(&Rfc3339).map_err(|_| ())?,
        updated_at: goal.updated_at.format(&Rfc3339).map_err(|_| ())?,
        version: goal.version,
    })
}

fn project_response(project: Project) -> Result<ProjectResponse, ()> {
    Ok(ProjectResponse {
        id: project.id,
        workspace_id: project.workspace_id,
        title: project.title,
        objective: project.objective,
        status: match project.status {
            ProjectStatus::Active => "active".to_owned(),
            ProjectStatus::Paused => "paused".to_owned(),
            ProjectStatus::Completed => "completed".to_owned(),
        },
        risk_level: project.risk_level,
        next_action: project.next_action,
        due_at: project
            .due_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        open_task_count: project.open_task_count,
        version: project.version,
    })
}

fn project_webhook_response(webhook: ProjectWebhook) -> ProjectWebhookResponse {
    ProjectWebhookResponse {
        id: webhook.id,
        project_id: webhook.project_id,
        provider: webhook.provider.as_str().to_owned(),
        destination_label: webhook.destination_hint,
        mention_directory: WebhookMentionDirectory {
            users: webhook.mention_directory.users,
        },
        events: webhook.events,
        enabled: webhook.enabled,
        version: webhook.version,
    }
}

fn webhook_delivery_response(delivery: WebhookDelivery) -> Result<WebhookDeliveryResponse, ()> {
    Ok(WebhookDeliveryResponse {
        id: delivery.id,
        webhook_id: delivery.webhook_id,
        event_type: delivery.event_type,
        status: delivery.status,
        attempt_count: delivery.attempt_count,
        response_code: delivery.response_code,
        error_code: delivery.last_error_code,
        created_at: delivery.created_at.format(&Rfc3339).map_err(|_| ())?,
        delivered_at: delivery
            .delivered_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
    })
}

fn managed_webhook_provider(value: &str) -> Option<WebhookProvider> {
    WebhookProvider::parse(value)
}

fn google_chat_mention_directory(
    provider: WebhookProvider,
    directory: WebhookMentionDirectory,
) -> Option<GoogleChatMentionDirectory> {
    let directory = GoogleChatMentionDirectory {
        users: directory.users,
    };
    directory.is_valid_for(provider).then_some(directory)
}

fn webhook_destination_label(provider: WebhookProvider) -> String {
    match provider {
        WebhookProvider::GoogleChat => "Google Chat 공간".to_owned(),
        WebhookProvider::Discord => "Discord 채널".to_owned(),
    }
}

fn webhook_payload(
    event_type: &str,
    project_id: uuid::Uuid,
    entity_id: Option<uuid::Uuid>,
) -> serde_json::Value {
    serde_json::json!({
        "event": event_type,
        "projectId": project_id,
        "entityId": entity_id,
        "occurredAt": OffsetDateTime::now_utc().format(&Rfc3339).ok(),
    })
}

fn conversation_response(conversation: Conversation) -> Result<ConversationResponse, ()> {
    Ok(ConversationResponse {
        id: conversation.id,
        title: conversation.title,
        status: match conversation.status {
            ConversationStatus::Active => "active".to_owned(),
            ConversationStatus::Archived => "archived".to_owned(),
        },
        last_message_at: conversation
            .last_message_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        version: conversation.version,
    })
}

fn queued_agent_turn_response(queued: &QueuedAgentTurn) -> QueuedAgentTurnResponse {
    QueuedAgentTurnResponse {
        job_id: queued.job_id,
        message_id: queued.message_id,
        conversation_id: queued.conversation_id,
        state: agent_job_state_name(queued.state).to_owned(),
    }
}

fn conversation_message_response(
    message: ConversationMessage,
) -> Result<ConversationMessageResponse, ()> {
    Ok(ConversationMessageResponse {
        id: message.id,
        role: match message.role {
            ConversationMessageRole::User => "user".to_owned(),
            ConversationMessageRole::Assistant => "assistant".to_owned(),
            ConversationMessageRole::SystemEvent => "system_event".to_owned(),
        },
        content: message.content,
        presentation: message.presentation.map(assistant_presentation_response),
        status: match message.status {
            ConversationMessageStatus::Pending => "pending".to_owned(),
            ConversationMessageStatus::Streaming => "streaming".to_owned(),
            ConversationMessageStatus::Completed => "completed".to_owned(),
            ConversationMessageStatus::Failed => "failed".to_owned(),
            ConversationMessageStatus::Cancelled => "cancelled".to_owned(),
        },
        created_at: message.created_at.format(&Rfc3339).map_err(|_| ())?,
        completed_at: message
            .completed_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        version: message.version,
    })
}

fn assistant_presentation_response(
    presentation: AssistantPresentation,
) -> AssistantPresentationResponse {
    AssistantPresentationResponse {
        kind: match presentation.kind {
            AssistantPresentationKind::Summary => "summary",
            AssistantPresentationKind::Tasks => "tasks",
            AssistantPresentationKind::Schedule => "schedule",
            AssistantPresentationKind::Projects => "projects",
            AssistantPresentationKind::Composite => "composite",
        }
        .to_owned(),
        title: presentation.title,
        items: presentation
            .items
            .into_iter()
            .map(|item| match item {
                AssistantPresentationItem::Task {
                    id,
                    project_id,
                    project_title,
                    assignee_name,
                    title,
                    status,
                    priority,
                    due_at,
                } => AssistantPresentationItemResponse::Task {
                    id,
                    project_id,
                    project_title,
                    assignee_name,
                    title,
                    status,
                    priority,
                    due_at,
                },
                AssistantPresentationItem::Schedule {
                    id,
                    title,
                    status,
                    starts_at,
                    ends_at,
                    time_zone,
                } => AssistantPresentationItemResponse::Schedule {
                    id,
                    title,
                    status,
                    starts_at,
                    ends_at,
                    time_zone,
                },
                AssistantPresentationItem::Project {
                    id,
                    workspace_id,
                    title,
                    status,
                    objective,
                    next_action,
                    risk_level,
                    open_task_count,
                } => AssistantPresentationItemResponse::Project {
                    id,
                    workspace_id,
                    title,
                    status,
                    objective,
                    next_action,
                    risk_level,
                    open_task_count,
                },
            })
            .collect(),
        layout: match presentation.layout {
            AssistantPresentationLayout::Stack => "stack",
            AssistantPresentationLayout::Split => "split",
            AssistantPresentationLayout::Focus => "focus",
        }
        .to_owned(),
        sections: presentation
            .sections
            .into_iter()
            .map(assistant_presentation_section_response)
            .collect(),
        focus_item_id: presentation.focus_item_id,
    }
}

fn assistant_presentation_section_response(
    section: AssistantPresentationSection,
) -> AssistantPresentationSectionResponse {
    AssistantPresentationSectionResponse {
        kind: match section.kind {
            AssistantPresentationSectionKind::Tasks => "tasks",
            AssistantPresentationSectionKind::Schedule => "schedule",
            AssistantPresentationSectionKind::Projects => "projects",
        }
        .to_owned(),
        title: section.title,
        view: match section.view {
            AssistantPresentationView::List => "list",
            AssistantPresentationView::Checklist => "checklist",
            AssistantPresentationView::Timeline => "timeline",
            AssistantPresentationView::Cards => "cards",
        }
        .to_owned(),
        item_ids: section.item_ids,
    }
}

fn agent_job_response(job: &AgentJob) -> Result<AgentJobResponse, ()> {
    Ok(AgentJobResponse {
        id: job.id,
        conversation_id: job.conversation_id,
        state: agent_job_state_name(job.state).to_owned(),
        created_at: job.created_at.format(&Rfc3339).map_err(|_| ())?,
        finished_at: job
            .finished_at
            .map(|value| value.format(&Rfc3339).map_err(|_| ()))
            .transpose()?,
        version: job.version,
        pending_action: job
            .pending_action
            .as_ref()
            .map(pending_agent_action_response)
            .transpose()?,
    })
}

fn pending_agent_action_response(
    action: &PendingAgentAction,
) -> Result<PendingAgentActionResponse, ()> {
    match action {
        PendingAgentAction::CreateTask { title, due_at } => Ok(PendingAgentActionResponse {
            kind: "create_task".to_owned(),
            title: title.clone(),
            due_at: due_at
                .map(|value| value.format(&Rfc3339).map_err(|_| ()))
                .transpose()?,
            starts_at: None,
            ends_at: None,
        }),
        PendingAgentAction::CreateSchedule {
            title,
            starts_at,
            ends_at,
            ..
        } => Ok(PendingAgentActionResponse {
            kind: "create_schedule".to_owned(),
            title: title.clone(),
            due_at: None,
            starts_at: Some(starts_at.format(&Rfc3339).map_err(|_| ())?),
            ends_at: Some(ends_at.format(&Rfc3339).map_err(|_| ())?),
        }),
    }
}

fn agent_authentication_response(
    authentication: Option<AgentAuthentication>,
) -> AgentAuthenticationResponse {
    let Some(authentication) = authentication else {
        return AgentAuthenticationResponse {
            state: "needs_login".to_owned(),
            verification_url: None,
            user_code: None,
        };
    };
    AgentAuthenticationResponse {
        state: match authentication.state {
            AgentAuthenticationState::Requested => "requested",
            AgentAuthenticationState::AwaitingAuthorization => "awaiting_authorization",
            AgentAuthenticationState::Ready => "ready",
            AgentAuthenticationState::Failed => "failed",
        }
        .to_owned(),
        verification_url: authentication.verification_url,
        user_code: authentication.user_code,
    }
}

fn agent_model_settings_response(settings: AgentModelSettings) -> AgentModelSettingsResponse {
    AgentModelSettingsResponse {
        items: settings
            .models
            .into_iter()
            .map(agent_model_response)
            .collect(),
        selected_model_id: settings.selected_model_id,
        selected_reasoning_effort: settings.selected_reasoning_effort,
    }
}

fn agent_model_response(model: AgentModelCatalogEntry) -> AgentModelResponse {
    AgentModelResponse {
        id: model.id,
        display_name: model.display_name,
        description: model.description,
        is_default: model.is_default,
        default_reasoning_effort: model.default_reasoning_effort,
        supported_reasoning_efforts: model
            .supported_reasoning_efforts
            .into_iter()
            .map(agent_reasoning_effort_response)
            .collect(),
    }
}

fn agent_reasoning_effort_response(effort: AgentReasoningEffort) -> AgentReasoningEffortResponse {
    AgentReasoningEffortResponse {
        id: effort.id,
        description: effort.description,
    }
}

const fn agent_job_state_name(state: AgentJobState) -> &'static str {
    match state {
        AgentJobState::Queued => "queued",
        AgentJobState::Claimed => "claimed",
        AgentJobState::Running => "running",
        AgentJobState::WaitingApproval => "waiting_approval",
        AgentJobState::RetryWait => "retry_wait",
        AgentJobState::Completed => "completed",
        AgentJobState::Failed => "failed",
        AgentJobState::Cancelled => "cancelled",
        AgentJobState::Declined => "declined",
    }
}

fn me_response(profile: Profile) -> MeResponse {
    MeResponse {
        id: profile.id,
        email: profile.email,
        display_name: profile.display_name,
        time_zone: profile.time_zone,
        version: profile.version,
    }
}

fn device_response(device: Device) -> DeviceResponse {
    DeviceResponse {
        id: device.id,
        platform: device.platform.as_str().to_owned(),
        name: device.name,
        app_version: device.app_version,
        os_version: device.os_version,
        status: match device.status {
            DeviceStatus::Active => "active".to_owned(),
            DeviceStatus::Revoked => "revoked".to_owned(),
        },
        version: device.version,
    }
}

async fn not_found(Extension(request_id): Extension<RequestId>) -> Response {
    not_found_response(request_id)
}

fn not_found_response(request_id: RequestId) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorEnvelope {
            error: ErrorBody {
                code: "request.not_found",
                message: "주소를 확인하고 다시 시도해 주세요.",
                request_id: request_id.to_string(),
                retryable: false,
                details: BTreeMap::new(),
            },
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, SystemTime},
    };

    use axum::{body::Body, http::Request};
    use ed25519_dalek::{
        SigningKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey},
    };
    use http_body_util::BodyExt;
    use jimin_auth::{
        AccessTokenIssuer, AccessTokenSettings, AccessTokenVerifier, SessionIdentity,
    };
    use pkcs8::LineEnding;
    use secrecy::{ExposeSecret, SecretString};
    use tokio::{sync::oneshot, time::timeout};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;

    struct FakeProbe(Readiness);

    #[test]
    fn calendar_connection_state_exposes_server_availability_without_credentials() {
        let unavailable = calendar_connection_response(None, false);
        assert!(!unavailable.available);
        assert_eq!(unavailable.status, "not_connected");
        assert_eq!(unavailable.email, None);

        let available = calendar_connection_response(None, true);
        assert!(available.available);
        assert_eq!(available.status, "not_connected");
    }

    #[test]
    fn voice_command_response_serializes_structured_result_items() {
        let item_id =
            Uuid::parse_str("019f68cb-9400-7000-8000-000000000000").expect("item ID should parse");
        let response = VoiceCommandResponse {
            kind: VoiceCommandKind::TasksListed,
            message: "오늘 할 일은 1개예요.".to_owned(),
            destination: VoiceCommandDestination::Home,
            items: vec![VoiceCommandItemResponse {
                item_type: VoiceCommandItemType::Task,
                id: item_id,
                title: "계약서 검토".to_owned(),
                due_at: Some("2026-07-15T18:00:00+09:00".to_owned()),
                starts_at: None,
                ends_at: None,
                priority: Some(2),
            }],
        };

        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(value["kind"], "tasks_listed");
        assert_eq!(value["destination"], "home");
        assert_eq!(value["items"][0]["itemType"], "task");
        assert_eq!(value["items"][0]["id"], item_id.to_string());
        assert_eq!(value["items"][0]["title"], "계약서 검토");
        assert_eq!(value["items"][0]["dueAt"], "2026-07-15T18:00:00+09:00");
        assert!(value["items"][0]["startsAt"].is_null());
        assert!(value["items"][0]["endsAt"].is_null());
        assert_eq!(value["items"][0]["priority"], 2);
    }

    #[async_trait]
    impl ReadinessProbe for FakeProbe {
        async fn check(&self, _expected_schema_version: i64) -> Readiness {
            self.0
        }
    }

    struct FakeAuthRepository {
        active: bool,
        profile: Option<Profile>,
    }

    #[async_trait]
    impl auth::AuthRepository for FakeAuthRepository {
        async fn session_is_active(
            &self,
            _identity: jimin_auth::SessionIdentity,
        ) -> Result<bool, jimin_storage::StorageError> {
            Ok(self.active)
        }

        async fn profile_for_user(
            &self,
            _user_id: Uuid,
        ) -> Result<Option<Profile>, jimin_storage::StorageError> {
            Ok(self.profile.clone())
        }

        async fn devices_for_user(
            &self,
            _user_id: Uuid,
        ) -> Result<Vec<Device>, jimin_storage::StorageError> {
            Ok(Vec::new())
        }
    }

    fn signed_auth_state(active: bool) -> (ApiState, String, Profile) {
        let user_id = Uuid::now_v7();
        let device_id = Uuid::now_v7();
        let session_id = Uuid::now_v7();
        let signing_key = SigningKey::from_bytes(&[13_u8; 32]);
        let private_key = SecretString::from(
            signing_key
                .to_pkcs8_pem(LineEnding::LF)
                .expect("test private key should encode")
                .to_string(),
        );
        let public_key = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("test public key should encode");
        let settings =
            AccessTokenSettings::new("https://jimin-os.test", "m1-test", Duration::from_mins(5))
                .expect("settings should be valid");
        let token = AccessTokenIssuer::from_ed25519_pem(settings, &private_key)
            .expect("private key should load")
            .issue(
                SessionIdentity::new(user_id, session_id, device_id, Uuid::now_v7())
                    .expect("session identity should be valid"),
                SystemTime::now(),
            )
            .expect("access token should issue");
        let verifier = AccessTokenVerifier::from_ed25519_pems(
            "https://jimin-os.test",
            [("m1-test".to_owned(), public_key.clone())],
        )
        .expect("public key should load");
        let profile = Profile {
            id: user_id,
            email: Some("owner@example.test".to_owned()),
            display_name: Some("Owner".to_owned()),
            time_zone: "Asia/Seoul".to_owned(),
            version: 1,
        };
        let state =
            ApiState::new("test-sha", false, None).with_authentication(auth::Authentication::new(
                verifier,
                Arc::new(FakeAuthRepository {
                    active,
                    profile: Some(profile.clone()),
                }),
            ));

        (state, token.token().expose_secret().to_owned(), profile)
    }

    #[tokio::test]
    async fn liveness_does_not_depend_on_database_readiness() {
        let state = ApiState::new("test-sha", false, None);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should be readable")
            .to_bytes();
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("health body should be JSON");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["service"], "api");
        assert_eq!(value["buildSha"], "test-sha");
    }

    #[tokio::test]
    async fn readiness_reports_only_non_sensitive_check_states() {
        let state = ApiState::new(
            "test-sha",
            true,
            Some(Arc::new(FakeProbe(Readiness::SchemaUnavailable))),
        );
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should be readable")
            .to_bytes();
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("health body should be JSON");
        assert_eq!(value["status"], "notReady");
        assert_eq!(value["checks"]["configuration"], "ok");
        assert_eq!(value["checks"]["database"], "ok");
        assert_eq!(value["checks"]["migrations"], "error");
        assert!(value.get("error").is_none());
    }

    #[tokio::test]
    async fn readiness_is_healthy_only_for_the_expected_schema() {
        let state = ApiState::new(
            "test-sha",
            true,
            Some(Arc::new(FakeProbe(Readiness::Ready {
                schema_version: EXPECTED_SCHEMA_VERSION,
            }))),
        );
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn profile_endpoint_requires_a_live_signed_session() {
        let (state, token, profile) = signed_auth_state(true);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should be readable")
            .to_bytes();
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("profile body should be JSON");
        assert_eq!(value["id"], profile.id.to_string());
        assert_eq!(value["email"], serde_json::json!(profile.email));
    }

    #[tokio::test]
    async fn profile_endpoint_rejects_revoked_or_missing_bearer_sessions() {
        let (inactive_state, token, _) = signed_auth_state(false);
        let inactive = router(inactive_state)
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(inactive.status(), StatusCode::UNAUTHORIZED);

        let (state, _, _) = signed_auth_state(true);
        let missing = router(state)
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "The OpenAPI route registry assertion intentionally keeps the full public surface in one ordered contract."
    )]
    fn openapi_paths_match_the_health_router_contract() {
        let document = openapi_document();
        let paths: Vec<_> = document.paths.paths.keys().map(String::as_str).collect();

        assert_eq!(
            paths,
            [
                "/health/live",
                "/health/ready",
                "/oauth/google/calendar/callback",
                "/v1/access/session",
                "/v1/agent/authentication",
                "/v1/agent/jobs/{job_id}",
                "/v1/agent/jobs/{job_id}/approval",
                "/v1/agent/models",
                "/v1/assistant/voice-commands",
                "/v1/auth/refresh",
                "/v1/briefs/work/refresh",
                "/v1/calendar/connections/google",
                "/v1/calendar/connections/google/authorizations",
                "/v1/calendar/connections/google/sync",
                "/v1/conversations",
                "/v1/conversations/{conversation_id}/jobs/latest",
                "/v1/conversations/{conversation_id}/messages",
                "/v1/conversations/{conversation_id}/stream",
                "/v1/conversations/{conversation_id}/turns",
                "/v1/devices",
                "/v1/goals",
                "/v1/goals/{goal_id}",
                "/v1/google-chat/connections",
                "/v1/google-chat/connections/authorizations",
                "/v1/google-chat/connections/{account_id}",
                "/v1/google-chat/connections/{account_id}/spaces",
                "/v1/home",
                "/v1/me",
                "/v1/meetings",
                "/v1/meetings/{meeting_id}",
                "/v1/meetings/{meeting_id}/action-items/{item_id}/decisions",
                "/v1/meetings/{meeting_id}/reanalyze",
                "/v1/projects",
                "/v1/projects/{project_id}",
                "/v1/projects/{project_id}/google-chat-sources",
                "/v1/projects/{project_id}/google-chat-sources/{source_id}",
                "/v1/projects/{project_id}/google-chat-sources/{source_id}/sync",
                "/v1/projects/{project_id}/inflow",
                "/v1/projects/{project_id}/inflow/{item_id}/decision",
                "/v1/projects/{project_id}/webhook-deliveries",
                "/v1/projects/{project_id}/webhook-deliveries/{delivery_id}/retry",
                "/v1/projects/{project_id}/webhooks",
                "/v1/projects/{project_id}/webhooks/{webhook_id}",
                "/v1/projects/{project_id}/webhooks/{webhook_id}/messages",
                "/v1/projects/{project_id}/webhooks/{webhook_id}/test",
                "/v1/push/registration",
                "/v1/recommendations",
                "/v1/recommendations/{recommendation_id}/decisions",
                "/v1/schedule-entries",
                "/v1/schedule-entries/{schedule_entry_id}",
                "/v1/sync/changes",
                "/v1/sync/stream",
                "/v1/tasks",
                "/v1/tasks/{task_id}",
                "/v1/tasks/{task_id}/complete",
                "/v1/workspaces"
            ]
        );
        assert!(
            document.paths.paths["/v1/projects/{project_id}"]
                .delete
                .is_some()
        );
        assert!(document.paths.paths["/v1/tasks/{task_id}"].get.is_some());
        assert!(document.paths.paths["/v1/tasks/{task_id}"].delete.is_some());
        assert!(
            document.paths.paths["/v1/projects/{project_id}/webhooks/{webhook_id}"]
                .put
                .is_some()
        );
        assert!(
            document.paths.paths
                ["/v1/projects/{project_id}/webhook-deliveries/{delivery_id}/retry"]
                .post
                .is_some()
        );
        for path in [
            "/v1/goals",
            "/v1/schedule-entries",
            "/v1/tasks",
            "/v1/tasks/{task_id}/complete",
            "/v1/recommendations/{recommendation_id}/decisions",
            "/v1/google-chat/connections/authorizations",
            "/v1/projects/{project_id}/google-chat-sources",
            "/v1/projects/{project_id}/inflow/{item_id}/decision",
        ] {
            assert!(
                document.paths.paths[path]
                    .post
                    .as_ref()
                    .and_then(|operation| operation.request_body.as_ref())
                    .is_some(),
                "{path} must publish its JSON request contract",
            );
        }
        assert!(
            document.paths.paths["/v1/goals/{goal_id}"]
                .put
                .as_ref()
                .and_then(|operation| operation.request_body.as_ref())
                .is_some(),
            "goal updates must publish their JSON request contract",
        );
    }

    #[tokio::test]
    async fn sync_endpoints_require_a_live_signed_session() {
        for uri in ["/v1/sync/changes?after=0", "/v1/sync/stream?after=0"] {
            let (state, _, _) = signed_auth_state(true);
            let response = router(state)
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request should be valid"),
                )
                .await
                .expect("handler should respond");

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn conversation_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/conversations")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"clientConversationId":"019f68cb-9400-7000-8000-000000000000","title":null}"#,
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn home_endpoint_requires_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/v1/home?from=2026-07-12T00%3A00%3A00Z&to=2026-07-13T00%3A00%3A00Z")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn recommendation_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let refresh_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/briefs/work/refresh")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(refresh_response.status(), StatusCode::UNAUTHORIZED);

        let list_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/recommendations")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(list_response.status(), StatusCode::UNAUTHORIZED);

        let decision_response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/recommendations/{}/decisions",
                        uuid::Uuid::now_v7()
                    ))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"clientMutationId":"{}","decision":"approve","reason":null,"expectedVersion":1}}"#,
                        uuid::Uuid::now_v7()
                    )))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(decision_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn work_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let goal_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/goals")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(goal_response.status(), StatusCode::UNAUTHORIZED);

        let workspace_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/workspaces")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(workspace_response.status(), StatusCode::UNAUTHORIZED);

        let project_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/projects?workspaceId=019f68cb-9400-7000-8000-000000000000")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(project_response.status(), StatusCode::UNAUTHORIZED);

        let project_update_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/v1/projects/019f68cb-9400-7000-8000-000000000001")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "title": "개인 운영체제",
                            "objective": null,
                            "status": "active",
                            "riskLevel": 0,
                            "nextAction": null,
                            "dueAt": null,
                            "expectedVersion": 1
                        })
                        .to_string(),
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(project_update_response.status(), StatusCode::UNAUTHORIZED);

        let schedule_update_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/v1/schedule-entries/019f68cb-9400-7000-8000-000000000003")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "title": "병원 방문",
                            "notes": null,
                            "startsAt": "2026-07-14T08:00:00Z",
                            "endsAt": "2026-07-14T09:00:00Z",
                            "timeZone": "Asia/Seoul",
                            "expectedVersion": 1
                        })
                        .to_string(),
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(schedule_update_response.status(), StatusCode::UNAUTHORIZED);

        let task_update_response = router(state)
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/v1/tasks/019f68cb-9400-7000-8000-000000000002")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "projectId": null,
                            "title": "계약서 검토",
                            "notes": null,
                            "status": "open",
                            "priority": 1,
                            "dueAt": null,
                            "expectedVersion": 1
                        })
                        .to_string(),
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");
        assert_eq!(task_update_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn delete_work_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        for request in [
            Request::builder()
                .method("DELETE")
                .uri("/v1/projects/019f68cb-9400-7000-8000-000000000001")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"expectedVersion":1}"#))
                .expect("request should be valid"),
            Request::builder()
                .method("DELETE")
                .uri("/v1/tasks/019f68cb-9400-7000-8000-000000000002")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"expectedVersion":1}"#))
                .expect("request should be valid"),
        ] {
            let response = router(state.clone())
                .oneshot(request)
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn webhook_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let project_id = "019f68cb-9400-7000-8000-000000000001";
        let webhook_id = "019f68cb-9400-7000-8000-000000000002";
        let delivery_id = "019f68cb-9400-7000-8000-000000000003";
        for request in [
            Request::builder()
                .uri(format!("/v1/projects/{project_id}/webhooks"))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri(format!("/v1/projects/{project_id}/webhooks"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "provider": "discord",
                        "url": "https://discord.com/api/webhooks/123/private",
                        "events": ["task.created"]
                    })
                    .to_string(),
                ))
                .expect("request should be valid"),
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/projects/{project_id}/webhooks/{webhook_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "provider": "discord",
                        "destinationMode": "keep",
                        "url": null,
                        "events": ["task.created"],
                        "enabled": true,
                        "expectedVersion": 1
                    })
                    .to_string(),
                ))
                .expect("request should be valid"),
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/projects/{project_id}/webhooks/{webhook_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"expectedVersion":1}"#))
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/projects/{project_id}/webhooks/{webhook_id}/messages"
                ))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message":"배포가 완료됐어요."}"#))
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/projects/{project_id}/webhooks/{webhook_id}/test"
                ))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .uri(format!("/v1/projects/{project_id}/webhook-deliveries"))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/projects/{project_id}/webhook-deliveries/{delivery_id}/retry"
                ))
                .body(Body::empty())
                .expect("request should be valid"),
        ] {
            let response = router(state.clone())
                .oneshot(request)
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn webhook_update_rejects_ambiguous_destination_mutations_before_storage() {
        let (state, token, _) = signed_auth_state(true);
        for (provider, destination_mode, url) in [
            ("discord", "replace", serde_json::Value::Null),
            (
                "discord",
                "keep",
                serde_json::json!("https://discord.com/api/webhooks/123/private"),
            ),
            ("discord", "unknown", serde_json::Value::Null),
            ("generic", "keep", serde_json::Value::Null),
        ] {
            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .method("PUT")
                        .uri("/v1/projects/019f68cb-9400-7000-8000-000000000001/webhooks/019f68cb-9400-7000-8000-000000000002")
                        .header("authorization", format!("Bearer {token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({
                                "provider": provider,
                                "destinationMode": destination_mode,
                                "url": url,
                                "events": ["task.created"],
                                "enabled": true,
                                "expectedVersion": 1
                            })
                            .to_string(),
                        ))
                        .expect("request should be valid"),
                )
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn webhook_response_never_exposes_destination_secrets() {
        let value = serde_json::to_value(project_webhook_response(ProjectWebhook {
            id: uuid::Uuid::now_v7(),
            project_id: uuid::Uuid::now_v7(),
            provider: WebhookProvider::Discord,
            destination_hint: "Discord 채널".to_owned(),
            mention_directory: GoogleChatMentionDirectory::default(),
            events: vec!["task.created".to_owned()],
            enabled: true,
            version: 1,
        }))
        .expect("webhook response should serialize");
        assert_eq!(value["provider"], "discord");
        assert_eq!(value["destinationLabel"], "Discord 채널");
        assert_eq!(value["mentionDirectory"]["users"], serde_json::json!({}));
        assert!(value.get("hasAuthentication").is_none());
        assert!(value.get("url").is_none());
        assert!(value.get("authorization").is_none());
        assert!(value.get("authHeaderCiphertext").is_none());
        assert!(value.get("authHeaderNonce").is_none());
    }

    #[test]
    fn inflow_assignment_context_only_marks_names_on_active_task_webhooks() {
        let project_id = uuid::Uuid::now_v7();
        let webhook = |name: &str, event: &str, enabled: bool| ProjectWebhook {
            id: uuid::Uuid::now_v7(),
            project_id,
            provider: WebhookProvider::GoogleChat,
            destination_hint: "Google Chat".to_owned(),
            mention_directory: GoogleChatMentionDirectory {
                users: [(name.to_owned(), "users/123456789012345678901".to_owned())]
                    .into_iter()
                    .collect(),
            },
            events: vec![event.to_owned()],
            enabled,
            version: 1,
        };
        let contexts = inflow_assignment_contexts(vec![
            webhook("김담당", "task.created", true),
            webhook("박담당", "project.updated", true),
            webhook("이담당", "task.created", false),
        ]);
        let context = contexts
            .get(&project_id)
            .expect("project assignment context should exist");

        assert_eq!(
            context.names.iter().cloned().collect::<Vec<_>>(),
            vec!["김담당", "박담당", "이담당"]
        );
        assert_eq!(
            context.notifiable_names.iter().cloned().collect::<Vec<_>>(),
            vec!["김담당"]
        );
    }

    #[test]
    fn google_chat_mention_directory_rejects_invalid_ids_and_discord_entries() {
        let valid = WebhookMentionDirectory {
            users: [(
                "홍길동".to_owned(),
                "users/123456789012345678901".to_owned(),
            )]
            .into_iter()
            .collect(),
        };
        assert!(
            google_chat_mention_directory(WebhookProvider::GoogleChat, valid.clone()).is_some()
        );
        assert!(google_chat_mention_directory(WebhookProvider::Discord, valid).is_none());
        assert!(
            google_chat_mention_directory(
                WebhookProvider::GoogleChat,
                WebhookMentionDirectory {
                    users: [("홍길동".to_owned(), "123456789012345678901".to_owned())]
                        .into_iter()
                        .collect(),
                },
            )
            .is_none()
        );
    }

    #[tokio::test]
    async fn calendar_connection_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        for request in [
            Request::builder()
                .uri("/v1/calendar/connections/google")
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri("/v1/calendar/connections/google/sync")
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("DELETE")
                .uri("/v1/calendar/connections/google?expectedVersion=1")
                .body(Body::empty())
                .expect("request should be valid"),
        ] {
            let response = router(state.clone())
                .oneshot(request)
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[test]
    fn inflow_candidates_group_threads_and_hide_reaction_only_messages() {
        let project_id = Uuid::now_v7();
        let source_id = Uuid::now_v7();
        let thread_name = Some("spaces/company/threads/request-1".to_owned());
        let make_item =
            |content: &str, received_at: OffsetDateTime, provider_thread_name: Option<String>| {
                ProjectInflowItem {
                    id: Uuid::now_v7(),
                    project_id,
                    project_name: "비스킷링크".to_owned(),
                    source_id,
                    source_name: "PAYMENTS CS".to_owned(),
                    provider_thread_name,
                    sender_provider_name: Some("users/223456789012345678901".to_owned()),
                    sender_name: Some("업무 담당자".to_owned()),
                    sent_by_owner: false,
                    content_text: content.to_owned(),
                    received_at,
                    status: ProjectInflowStatus::Pending,
                    promoted_task_id: None,
                    acknowledged_at: Some(received_at),
                    completion_requested_at: None,
                    completion_reaction_at: None,
                    completion_reply_at: None,
                    completion_delivery_error_code: None,
                    completion_delivery_attempt_count: 0,
                    version: 1,
                }
            };
        let actionable_request = make_item(
            "혹시 이 기능은 개발에 얼마나 걸릴까요?",
            OffsetDateTime::UNIX_EPOCH + TimeDuration::seconds(1),
            thread_name.clone(),
        );
        let actionable_request_id = actionable_request.id;
        let mut owner_follow_up = make_item(
            "담당자를 정해서 진행해 주세요.",
            OffsetDateTime::UNIX_EPOCH + TimeDuration::seconds(2),
            thread_name,
        );
        owner_follow_up.sent_by_owner = true;
        let items = vec![
            make_item(
                "ㅠ",
                OffsetDateTime::UNIX_EPOCH,
                owner_follow_up.provider_thread_name.clone(),
            ),
            actionable_request,
            owner_follow_up,
            make_item(
                "ㅇㅇ",
                OffsetDateTime::UNIX_EPOCH + TimeDuration::seconds(3),
                None,
            ),
        ];

        let candidates = group_project_inflow_candidates(items);

        assert_eq!(candidates.len(), 1);
        let response = project_inflow_item_response(
            candidates
                .into_iter()
                .next()
                .expect("one candidate should remain"),
        )
        .expect("candidate should serialize");
        assert_eq!(response.id, actionable_request_id);
        assert_eq!(response.message_count, 3);
        assert_eq!(response.messages.len(), 3);
        assert_eq!(
            response.content_text,
            "혹시 이 기능은 개발에 얼마나 걸릴까요?"
        );
        assert_eq!(response.suggested_task_title, "개발 범위와 예상 일정 확인");
        assert!(response.suggested_task_notes.contains("업무 목적"));
        assert!(
            !response
                .suggested_task_notes
                .contains("보낸 사람 정보 없음")
        );

        let mut owner_comment = make_item(
            "권한이슈 확인일자: 금일",
            OffsetDateTime::UNIX_EPOCH + TimeDuration::seconds(3),
            None,
        );
        owner_comment.sent_by_owner = true;
        assert!(group_project_inflow_candidates(vec![owner_comment]).is_empty());
        assert_eq!(inflow_actionability_score("ip전달 먼저"), 0);
        assert_eq!(inflow_actionability_score("리체크용 해야함"), 0);
    }

    #[test]
    fn inflow_work_description_summarizes_qr_request_without_sender_or_credentials() {
        let project_id = Uuid::now_v7();
        let source_id = Uuid::now_v7();
        let message = |content: &str| ProjectInflowItem {
            id: Uuid::now_v7(),
            project_id,
            project_name: "비스킷링크".to_owned(),
            source_id,
            source_name: "PAYMENTS CS".to_owned(),
            provider_thread_name: Some("spaces/company/threads/qr".to_owned()),
            sender_provider_name: None,
            sender_name: None,
            sent_by_owner: false,
            content_text: content.to_owned(),
            received_at: OffsetDateTime::UNIX_EPOCH,
            status: ProjectInflowStatus::Pending,
            promoted_task_id: None,
            acknowledged_at: None,
            completion_requested_at: None,
            completion_reaction_at: None,
            completion_reply_at: None,
            completion_delivery_error_code: None,
            completion_delivery_attempt_count: 0,
            version: 1,
        };
        let messages = vec![
            message(
                "상위 QR 결제 거래·배송 정보 통지 연동을 요청드립니다.\n연동 가이드와 테스트 계정 정보: dalqtest / 1234",
            ),
            message("수신 URL을 개발한 후 당사로 회신해 주세요."),
            message("거래내역을 쌓는 것이 주 목적인지 판단이 필요합니다."),
            message("테스트 페이지에서 확인 가능한 기능도 점검해 주세요."),
        ];

        let notes = suggested_inflow_task_notes("페이시스 QR 결제 통보 연동 개발", &messages);

        assert!(notes.contains("QR 결제 거래·배송 정보 통지 연동 범위"));
        assert!(notes.contains("거래 통지 수신 URL"));
        assert!(notes.contains("거래내역 조회·저장 목적"));
        assert!(notes.contains("테스트 환경"));
        assert!(!notes.contains("보낸 사람 정보 없음"));
        assert!(!notes.contains("dalqtest"));
        assert!(!notes.contains("1234"));
    }

    #[test]
    fn google_chat_completion_reply_contains_task_owner_and_korea_deadline() {
        let due_at =
            OffsetDateTime::parse("2026-07-24T02:30:00Z", &Rfc3339).expect("deadline should parse");
        let reply = google_chat_completion_reply(&GoogleChatCompletionDelivery {
            inflow_id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            source_id: Uuid::now_v7(),
            provider_message_name: "spaces/company/messages/message-1.message-1".to_owned(),
            provider_thread_name: Some("spaces/company/threads/thread-1".to_owned()),
            task_id: Uuid::now_v7(),
            task_title: "정산 오류 원인 확인".to_owned(),
            assignee_name: Some("김경주".to_owned()),
            due_at: Some(due_at),
            reaction_completed: false,
            reply_completed: false,
            attempt_count: 0,
        });

        assert_eq!(
            reply,
            "✅ Jimin OS에서 할 일로 정리했어요.\n할 일: 정산 오류 원인 확인\n담당자: 김경주\n마감일: 2026년 7월 24일 11:30"
        );
    }

    #[tokio::test]
    async fn google_chat_inflow_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let account_id = "019f68cb-9400-7000-8000-000000000011";
        let project_id = "019f68cb-9400-7000-8000-000000000012";
        let source_id = "019f68cb-9400-7000-8000-000000000013";
        let item_id = "019f68cb-9400-7000-8000-000000000014";
        for request in [
            Request::builder()
                .uri("/v1/google-chat/connections")
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri("/v1/google-chat/connections/authorizations")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"clientKind":"android"}"#))
                .expect("request should be valid"),
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/v1/google-chat/connections/{account_id}?expectedVersion=1"
                ))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .uri(format!("/v1/google-chat/connections/{account_id}/spaces"))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .uri(format!("/v1/projects/{project_id}/google-chat-sources"))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/projects/{project_id}/google-chat-sources/{source_id}/sync"
                ))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .uri(format!("/v1/projects/{project_id}/inflow?status=pending"))
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/projects/{project_id}/inflow/{item_id}/decision"
                ))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"decision":"dismiss","expectedVersion":1}"#))
                .expect("request should be valid"),
        ] {
            let response = router(state.clone())
                .oneshot(request)
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn agent_authentication_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        for request in [
            Request::builder()
                .uri("/v1/agent/authentication")
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("POST")
                .uri("/v1/agent/authentication")
                .body(Body::empty())
                .expect("request should be valid"),
        ] {
            let response = router(state.clone())
                .oneshot(request)
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn agent_model_endpoints_require_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        for request in [
            Request::builder()
                .uri("/v1/agent/models")
                .body(Body::empty())
                .expect("request should be valid"),
            Request::builder()
                .method("PUT")
                .uri("/v1/agent/models")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"modelId":null}"#))
                .expect("request should be valid"),
        ] {
            let response = router(state.clone())
                .oneshot(request)
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn voice_command_endpoint_requires_a_live_signed_session() {
        let (state, _, _) = signed_auth_state(true);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/assistant/voice-commands")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"text":"내일 일정 알려줘","referenceAt":"2026-07-12T09:00:00+09:00","timeZone":"Asia/Seoul"}"#,
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn missing_agent_authentication_maps_to_a_login_request_without_code() {
        let response = agent_authentication_response(None);
        assert_eq!(response.state, "needs_login");
        assert_eq!(response.verification_url, None);
        assert_eq!(response.user_code, None);
    }

    #[tokio::test]
    async fn tauri_mobile_origin_can_preflight_authenticated_requests() {
        let state = ApiState::new("test-sha", false, None);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/v1/access/session")
                    .header("origin", "http://tauri.localhost")
                    .header("access-control-request-method", "POST")
                    .header(
                        "access-control-request-headers",
                        "authorization, content-type",
                    )
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("access-control-allow-origin"),
            Some(&HeaderValue::from_static("http://tauri.localhost"))
        );
    }

    #[tokio::test]
    async fn trusted_network_desktop_dev_origin_can_preflight_session_bootstrap() {
        let state = ApiState::new("test-sha", false, None).with_trusted_network(true);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/v1/access/session")
                    .header("origin", "http://localhost:1420")
                    .header("access-control-request-method", "POST")
                    .header("access-control-request-headers", "content-type")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("access-control-allow-origin"),
            Some(&HeaderValue::from_static("http://localhost:1420"))
        );
    }

    #[tokio::test]
    async fn trusted_network_session_is_not_available_without_private_network_mode() {
        let state = ApiState::new("test-sha", false, None);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/access/session")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"installationId":"019f68cb-9400-7000-8000-000000000000","platform":"android","name":"Jimin OS","appVersion":"0.1.0-dev","osVersion":"Android"}"#,
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn retired_pairing_routes_are_not_exposed() {
        let state = ApiState::new("test-sha", false, None).with_trusted_network(true);
        for path in ["/v1/auth/pairings/exchange", "/v1/device-pairings"] {
            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .body(Body::empty())
                        .expect("request should be valid"),
                )
                .await
                .expect("handler should respond");
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn trusted_network_session_requires_an_available_session_runtime() {
        let state = ApiState::new("test-sha", false, None).with_trusted_network(true);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/access/session")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"installationId":"019f68cb-9400-7000-8000-000000000000","platform":"android","name":"개발용 Android","appVersion":"0.1.0-dev","osVersion":"Android"}"#,
                    ))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn server_honors_graceful_shutdown() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let state = ApiState::new("test-sha", false, None);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(serve_with_shutdown(listener, router(state), async move {
            let _ = shutdown_rx.await;
        }));

        shutdown_tx.send(()).expect("shutdown should be delivered");
        let result = timeout(Duration::from_secs(1), server)
            .await
            .expect("server should stop before timeout")
            .expect("server task should not panic");

        assert!(result.is_ok());
    }
}

pub mod auth;
pub mod calendar_oauth;
pub mod config;
pub mod probe;
mod voice_command;

use std::{collections::BTreeMap, convert::Infallible, future::Future, sync::Arc, time::Duration};

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
        AgentAuthentication, AgentAuthenticationState, AgentJob, AgentJobState, Conversation,
        ConversationMessage, ConversationMessageRole, ConversationMessageStatus,
        ConversationStatus, NewAgentTurn, NewConversation, PendingAgentAction,
        PendingAgentActionDecision, QueuedAgentTurn,
    },
    auth::{Device, DeviceStatus, Profile},
    calendar::{CalendarAccount, CalendarAccountStatus, CreateCalendarOAuthAuthorization},
    planning::{
        NewScheduleEntry, NewTask, ScheduleEntry, ScheduleSource, ScheduleStatus, Task, TaskStatus,
    },
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use utoipa::{OpenApi, ToSchema};

use crate::{
    calendar_oauth::{CalendarOAuthError, CalendarOAuthRuntime, storage_failure_code},
    voice_command::{VoiceCommand, VoiceCommandError},
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
    fn calendar_oauth(&self) -> Option<&Arc<CalendarOAuthRuntime>> {
        self.calendar_oauth.as_ref()
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
    title: String,
    notes: Option<String>,
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
}

/// Safe Google Calendar connection state. Provider credentials and identifiers
/// never leave the server.
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendarConnectionResponse {
    status: String,
    email: Option<String>,
    granted_scopes: Vec<String>,
    last_successful_sync_at: Option<String>,
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
    status: String,
    created_at: String,
    completed_at: Option<String>,
    version: i64,
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
#[serde(rename_all = "camelCase")]
struct VoiceCommandResponse {
    kind: VoiceCommandKind,
    message: String,
    destination: VoiceCommandDestination,
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

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateScheduleRequest {
    title: String,
    notes: Option<String>,
    starts_at: String,
    ends_at: String,
    time_zone: String,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateTaskRequest {
    title: String,
    notes: Option<String>,
    priority: i16,
    due_at: Option<String>,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct VoiceCommandRequest {
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
        list_schedule_entries,
        get_google_calendar_connection,
        start_google_calendar_authorization,
        complete_google_calendar_authorization,
        sync_google_calendar,
        get_home_snapshot,
        create_schedule_entry,
        list_open_tasks,
        create_task,
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
        live,
        ready,
        me,
        devices
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
        DeviceRegistrationRequest,
        ScheduleEntryResponse,
        ScheduleListResponse,
        GoogleCalendarConnectionResponse,
        StartGoogleCalendarAuthorizationRequest,
        StartGoogleCalendarAuthorizationResponse,
        TaskResponse,
        TaskListResponse,
        VoiceCommandKind,
        VoiceCommandDestination,
        VoiceCommandResponse,
        HomeSnapshotResponse,
        ConversationResponse,
        ConversationListResponse,
        QueuedAgentTurnResponse,
        ConversationMessageResponse,
        ConversationMessageListResponse,
        AgentJobResponse,
        PendingAgentActionResponse,
        AgentAuthenticationResponse,
        CreateConversationRequest,
        CreateAgentTurnRequest,
        ResolveAgentActionRequest,
        AgentTurnInput,
        VoiceCommandRequest
    )),
    tags((name = "health", description = "Process and dependency health"))
)]
struct ApiDoc;

#[must_use]
pub fn openapi_document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router(state: ApiState) -> Router {
    let router = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route(
            "/oauth/google/calendar/callback",
            get(complete_google_calendar_authorization),
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
            "/v1/calendar/connections/google",
            get(get_google_calendar_connection),
        )
        .route(
            "/v1/calendar/connections/google/authorizations",
            post(start_google_calendar_authorization),
        )
        .route(
            "/v1/calendar/connections/google/sync",
            post(sync_google_calendar),
        )
        .route("/v1/home", get(get_home_snapshot))
        .route("/v1/tasks", get(list_open_tasks).post(create_task))
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
        .route("/v1/agent/jobs/{job_id}", get(get_agent_job))
        .route(
            "/v1/agent/jobs/{job_id}/approval",
            axum::routing::post(resolve_agent_action),
        )
        .route("/v1/me", get(me))
        .route("/v1/devices", get(devices));

    router
        .fallback(not_found)
        .with_state(state)
        .layer(
            CorsLayer::new()
                // The desktop and mobile WebViews use fixed Tauri origins.
                // Do not widen this to arbitrary web origins: this API accepts
                // bearer tokens from the installed personal client.
                .allow_origin([
                    HeaderValue::from_static("tauri://localhost"),
                    HeaderValue::from_static("http://tauri.localhost"),
                    HeaderValue::from_static("https://tauri.localhost"),
                ])
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                ]),
        )
        .layer(middleware::from_fn(request_context))
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
    let user_id = principal.identity().user_id();
    let (schedule, tasks) = match tokio::try_join!(
        planning.schedule_entries_in_range(user_id, from, to),
        planning.open_tasks_for_user(user_id),
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

    Json(HomeSnapshotResponse { schedule, tasks }).into_response()
}

#[utoipa::path(
    post,
    path = "/v1/schedule-entries",
    tag = "planning",
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
        .create_schedule_entry(&NewScheduleEntry {
            id: uuid::Uuid::now_v7(),
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
    get,
    path = "/v1/tasks",
    tag = "planning",
    responses((status = 200, body = TaskListResponse), (status = 401), (status = 503))
)]
async fn list_open_tasks(
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
        .open_tasks_for_user(principal.identity().user_id())
        .await
    {
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
    match planning
        .create_task(&NewTask {
            id: uuid::Uuid::now_v7(),
            user_id: principal.identity().user_id(),
            title: body.title,
            notes: body.notes,
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

    handle_voice_command(planning, user_id, command, body.time_zone, request_id).await
}

async fn handle_voice_command(
    planning: &Database,
    user_id: uuid::Uuid,
    command: VoiceCommand,
    time_zone: String,
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
                request_id,
            )
            .await
        }
        VoiceCommand::ListTasks => list_voice_tasks(planning, user_id, request_id).await,
        VoiceCommand::CreateTask { title } => {
            create_voice_task(planning, user_id, title, request_id).await
        }
        VoiceCommand::NeedsScheduleDetails => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::NeedsDetails,
            message: "일정 이름과 시간을 함께 말해 주세요. 예: 내일 오후 3시에 치과 일정 등록해 줘"
                .to_owned(),
            destination: VoiceCommandDestination::Conversation,
        })
        .into_response(),
        VoiceCommand::NeedsTaskDetails => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::NeedsDetails,
            message: "추가할 할 일을 함께 말해 주세요. 예: 할 일에 장보기 추가해 줘".to_owned(),
            destination: VoiceCommandDestination::Conversation,
        })
        .into_response(),
        VoiceCommand::ContinueConversation => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::ContinueConversation,
            message: "일정이나 할 일 외의 요청은 대화에서 이어서 도와드릴게요.".to_owned(),
            destination: VoiceCommandDestination::Conversation,
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
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn create_voice_schedule(
    planning: &Database,
    user_id: uuid::Uuid,
    input: VoiceScheduleInput,
    request_id: RequestId,
) -> Response {
    let VoiceScheduleInput {
        label,
        title,
        starts_at,
        ends_at,
        time_zone,
    } = input;
    match planning
        .create_schedule_entry(&NewScheduleEntry {
            id: uuid::Uuid::now_v7(),
            user_id,
            title: title.clone(),
            notes: None,
            starts_at,
            ends_at,
            time_zone,
        })
        .await
    {
        Ok(entry) => (
            StatusCode::CREATED,
            Json(VoiceCommandResponse {
                kind: VoiceCommandKind::ScheduleCreated,
                message: format!(
                    "{label} {:02}:{:02}에 {title} 일정을 등록했어요.",
                    entry.starts_at.hour(),
                    entry.starts_at.minute(),
                ),
                destination: VoiceCommandDestination::Calendar,
            }),
        )
            .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn list_voice_tasks(
    planning: &Database,
    user_id: uuid::Uuid,
    request_id: RequestId,
) -> Response {
    match planning.open_tasks_for_user(user_id).await {
        Ok(tasks) => Json(VoiceCommandResponse {
            kind: VoiceCommandKind::TasksListed,
            message: if tasks.is_empty() {
                "열린 할 일이 없어요.".to_owned()
            } else {
                format!("열린 할 일이 {}개 있어요.", tasks.len())
            },
            destination: VoiceCommandDestination::Home,
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn create_voice_task(
    planning: &Database,
    user_id: uuid::Uuid,
    title: String,
    request_id: RequestId,
) -> Response {
    match planning
        .create_task(&NewTask {
            id: uuid::Uuid::now_v7(),
            user_id,
            title: title.clone(),
            notes: None,
            priority: 1,
            due_at: None,
        })
        .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            Json(VoiceCommandResponse {
                kind: VoiceCommandKind::TaskCreated,
                message: format!("{title} 할 일을 추가했어요."),
                destination: VoiceCommandDestination::Home,
            }),
        )
            .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

fn schedule_list_message(label: &str, entries: &[ScheduleEntry]) -> String {
    match entries {
        [] => format!("{label} 일정은 없어요."),
        [entry] => format!("{label} 일정은 1개예요. {}", entry.title),
        [first, ..] => format!(
            "{label} 일정은 {}개예요. 첫 일정은 {}예요.",
            entries.len(),
            first.title
        ),
    }
}

#[utoipa::path(
    post,
    path = "/v1/tasks/{task_id}/complete",
    tag = "planning",
    params(("task_id" = String, Path)),
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
    match planning
        .complete_task(
            principal.identity().user_id(),
            task_id,
            body.expected_version,
        )
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

/// Executes narrow, deterministic personal planning instructions immediately.
/// General conversation still queues a managed Codex turn; only a fully parsed
/// local task or dated schedule bypasses the approval state.
async fn enqueue_conversation_turn(
    agent: &Database,
    turn: &NewAgentTurn,
) -> Result<QueuedAgentTurn, StorageError> {
    let Some(action) = pending_action_from_conversation_text(&turn.content) else {
        return agent.enqueue_agent_turn(turn).await;
    };
    let queued = agent.enqueue_agent_action_turn(turn, action).await?;
    if queued.state != AgentJobState::WaitingApproval
        || !agent
            .resolve_agent_action(
                turn.user_id,
                queued.job_id,
                PendingAgentActionDecision::Approve,
            )
            .await?
    {
        return Err(StorageError::PersistenceUnavailable);
    }
    let job = agent
        .agent_job_for_user(turn.user_id, queued.job_id)
        .await?
        .ok_or(StorageError::PersistenceUnavailable)?;
    Ok(QueuedAgentTurn {
        job_id: queued.job_id,
        message_id: queued.message_id,
        conversation_id: queued.conversation_id,
        state: job.state,
        version: job.version,
    })
}

fn pending_action_from_conversation_text(text: &str) -> Option<PendingAgentAction> {
    let korea_offset = time::UtcOffset::from_hms(9, 0, 0).ok()?;
    let reference_at = OffsetDateTime::now_utc().to_offset(korea_offset);
    match voice_command::interpret(text, reference_at, "Asia/Seoul").ok()? {
        VoiceCommand::CreateTask { title } => Some(PendingAgentAction::CreateTask { title }),
        VoiceCommand::CreateSchedule {
            title,
            starts_at,
            ends_at,
            ..
        } => Some(PendingAgentAction::CreateSchedule {
            title,
            starts_at,
            ends_at,
            time_zone: "Asia/Seoul".to_owned(),
        }),
        VoiceCommand::ListSchedule { .. }
        | VoiceCommand::ListTasks
        | VoiceCommand::NeedsScheduleDetails
        | VoiceCommand::NeedsTaskDetails
        | VoiceCommand::ContinueConversation => None,
    }
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
        Ok(account) => Json(calendar_connection_response(account)).into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
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
            CalendarAccountStatus::ReauthRequired
                | CalendarAccountStatus::Revoked
                | CalendarAccountStatus::Error
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
    let Some(calendar_oauth) = state.calendar_oauth() else {
        return calendar_callback_page(
            StatusCode::SERVICE_UNAVAILABLE,
            "연결을 완료하지 못했어요",
            "서버의 Google Calendar 연결 설정을 확인한 뒤 다시 시도해 주세요.",
        );
    };
    let Some(planning) = state.planning() else {
        return calendar_callback_page(
            StatusCode::SERVICE_UNAVAILABLE,
            "연결을 완료하지 못했어요",
            "잠시 후 앱에서 다시 시도해 주세요.",
        );
    };
    let claimed = match planning
        .claim_calendar_oauth_authorization(&calendar_oauth.state_verifier(&query.state))
        .await
    {
        Ok(Some(authorization)) => authorization,
        Ok(None) => {
            return calendar_callback_page(
                StatusCode::BAD_REQUEST,
                "연결을 완료하지 못했어요",
                "연결 시간이 지났거나 이미 처리된 요청이에요. 앱에서 다시 연결해 주세요.",
            );
        }
        Err(_) => {
            return calendar_callback_page(
                StatusCode::SERVICE_UNAVAILABLE,
                "연결을 완료하지 못했어요",
                "잠시 후 앱에서 다시 시도해 주세요.",
            );
        }
    };
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
            let _ = planning
                .fail_calendar_oauth_authorization(authorization_id, error.failure_code())
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
            calendar_callback_error_page(error)
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
        Ok(Some(account)) if account.status == CalendarAccountStatus::Active => account,
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
            Ok(connection) => Json(calendar_connection_response(connection)).into_response(),
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
    let events = calendar_oauth
        .initial_calendar_event_sync(&connection, &targets)
        .await?;
    for (calendar_id, events) in events {
        planning
            .apply_calendar_event_full_sync(account_id, user_id, calendar_id, &events)
            .await
            .map_err(|_| CalendarOAuthError::ProviderUnavailable)?;
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
) -> GoogleCalendarConnectionResponse {
    let Some(account) = account else {
        return GoogleCalendarConnectionResponse {
            status: "not_connected".to_owned(),
            email: None,
            granted_scopes: Vec::new(),
            last_successful_sync_at: None,
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
        status: status.to_owned(),
        email: Some(account.email),
        granted_scopes: account.granted_scopes,
        last_successful_sync_at: account
            .last_successful_sync_at
            .map(|value| value.format(&Rfc3339).unwrap_or_default()),
        reauth_required: account.status == CalendarAccountStatus::ReauthRequired,
        version: Some(account.version),
    }
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
        version: entry.version,
    })
}

fn task_response(task: Task) -> Result<TaskResponse, ()> {
    Ok(TaskResponse {
        id: task.id,
        title: task.title,
        notes: task.notes,
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
        PendingAgentAction::CreateTask { title } => Ok(PendingAgentActionResponse {
            kind: "create_task".to_owned(),
            title: title.clone(),
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
                "/v1/assistant/voice-commands",
                "/v1/auth/refresh",
                "/v1/calendar/connections/google",
                "/v1/calendar/connections/google/authorizations",
                "/v1/calendar/connections/google/sync",
                "/v1/conversations",
                "/v1/conversations/{conversation_id}/jobs/latest",
                "/v1/conversations/{conversation_id}/messages",
                "/v1/conversations/{conversation_id}/stream",
                "/v1/conversations/{conversation_id}/turns",
                "/v1/devices",
                "/v1/home",
                "/v1/me",
                "/v1/schedule-entries",
                "/v1/tasks",
                "/v1/tasks/{task_id}/complete"
            ]
        );
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

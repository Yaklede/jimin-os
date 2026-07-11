pub mod auth;
pub mod config;
pub mod probe;

use std::{collections::BTreeMap, future::Future, sync::Arc};

use async_trait::async_trait;
use axum::{
    Extension, Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use jimin_application::{ApplicationError, DeviceSession, SessionService};
use jimin_domain::{ClientPlatform, DeviceRegistration};
use jimin_observability::{RequestId, request_context};
use jimin_storage::{
    Database, EXPECTED_SCHEMA_VERSION, Readiness, StorageError,
    agent::{
        AgentJob, AgentJobState, Conversation, ConversationMessage, ConversationMessageRole,
        ConversationMessageStatus, ConversationStatus, NewAgentTurn, NewConversation,
        QueuedAgentTurn,
    },
    auth::{Device, DeviceStatus, Profile},
    planning::{NewScheduleEntry, NewTask, ScheduleEntry, ScheduleStatus, Task, TaskStatus},
};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::net::TcpListener;
use utoipa::{OpenApi, ToSchema};

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
    authentication: Option<Arc<auth::Authentication>>,
    pairing: Option<Arc<PairingRuntime>>,
    planning: Option<Database>,
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
            authentication: None,
            pairing: None,
            planning: None,
            agent: None,
        }
    }

    #[must_use]
    pub fn with_authentication(mut self, authentication: auth::Authentication) -> Self {
        self.authentication = Some(Arc::new(authentication));
        self
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

    /// Issues one QR pairing token for a trusted server administrator or an
    /// already authenticated Jimin OS client.
    ///
    /// # Errors
    ///
    /// Returns a sanitized application error without logging token material.
    pub async fn issue_device_pairing(
        &self,
    ) -> Result<jimin_application::IssuedDevicePairing, ApplicationError> {
        self.sessions.issue_device_pairing().await
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
struct PairingExchangeRequest {
    #[schema(value_type = String)]
    pairing_token: SecretString,
    device: DeviceRegistrationRequest,
}

#[derive(serde::Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RefreshSessionRequest {
    #[schema(value_type = String)]
    refresh_token: SecretString,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DevicePairingResponse {
    pairing_token: String,
    expires_at: String,
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
struct CreateConversationRequest {
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
        pairing_exchange,
        refresh_session,
        create_device_pairing,
        list_schedule_entries,
        create_schedule_entry,
        list_open_tasks,
        create_task,
        complete_task,
        list_conversations,
        create_conversation,
        list_conversation_messages,
        get_latest_conversation_job,
        create_agent_turn,
        get_agent_job,
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
        DevicePairingResponse,
        ScheduleEntryResponse,
        ScheduleListResponse,
        TaskResponse,
        TaskListResponse,
        ConversationResponse,
        ConversationListResponse,
        QueuedAgentTurnResponse,
        ConversationMessageResponse,
        ConversationMessageListResponse,
        AgentJobResponse,
        CreateConversationRequest,
        CreateAgentTurnRequest,
        AgentTurnInput
    )),
    tags((name = "health", description = "Process and dependency health"))
)]
struct ApiDoc;

#[must_use]
pub fn openapi_document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route(
            "/v1/auth/pairings/exchange",
            axum::routing::post(pairing_exchange),
        )
        .route("/v1/auth/refresh", axum::routing::post(refresh_session))
        .route(
            "/v1/device-pairings",
            axum::routing::post(create_device_pairing),
        )
        .route(
            "/v1/schedule-entries",
            get(list_schedule_entries).post(create_schedule_entry),
        )
        .route("/v1/tasks", get(list_open_tasks).post(create_task))
        .route(
            "/v1/tasks/{task_id}/complete",
            axum::routing::post(complete_task),
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
            "/v1/conversations/{conversation_id}/jobs/latest",
            get(get_latest_conversation_job),
        )
        .route("/v1/agent/jobs/{job_id}", get(get_agent_job))
        .route("/v1/me", get(me))
        .route("/v1/devices", get(devices))
        .fallback(not_found)
        .with_state(state)
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
            id: uuid::Uuid::now_v7(),
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

    match agent
        .enqueue_agent_turn(&NewAgentTurn {
            job_id: uuid::Uuid::now_v7(),
            message_id: uuid::Uuid::now_v7(),
            client_message_id: body.client_message_id,
            user_id: principal.identity().user_id(),
            conversation_id,
            content: input.text,
        })
        .await
    {
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
    path = "/v1/auth/pairings/exchange",
    tag = "identity",
    responses(
        (status = 200, description = "One-time device pairing exchanged for a Jimin OS device session", body = DeviceSessionResponse),
        (status = 400, description = "Pairing request or device metadata is invalid"),
        (status = 401, description = "Pairing token is invalid, expired, or already consumed"),
        (status = 503, description = "Authentication service is temporarily unavailable")
    )
)]
async fn pairing_exchange(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    Json(request): Json<PairingExchangeRequest>,
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
    let Ok(device) = DeviceRegistration::new(
        request.device.installation_id,
        request.device.platform,
        request.device.name,
        request.device.app_version,
        request.device.os_version,
    ) else {
        return invalid_request_response(request_id);
    };
    let session = match pairing
        .sessions
        .consume_device_pairing(request.pairing_token, device, uuid::Uuid::now_v7())
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

#[utoipa::path(
    post,
    path = "/v1/device-pairings",
    tag = "identity",
    responses(
        (status = 200, description = "A new one-time QR pairing token", body = DevicePairingResponse),
        (status = 401, description = "Session is absent, invalid, or expired"),
        (status = 503, description = "Authentication service is temporarily unavailable")
    )
)]
async fn create_device_pairing(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    request: Request,
) -> Response {
    if let Err(failure) = auth::authenticate(&state, request.headers()).await {
        return failure.into_response(request_id);
    }
    let Some(pairing) = state.pairing() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service.temporarily_unavailable",
            "잠시 후 다시 시도해 주세요.",
            request_id,
            true,
        );
    };
    let issued = match pairing.issue_device_pairing().await {
        Ok(issued) => issued,
        Err(error) => return application_error_response(&error, request_id),
    };
    let Ok(expires_at) = issued.expires_at().format(&Rfc3339) else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service.temporarily_unavailable",
            "잠시 후 다시 시도해 주세요.",
            request_id,
            true,
        );
    };
    no_store_json(DevicePairingResponse {
        pairing_token: issued.token().serialized().expose_secret().to_owned(),
        expires_at,
    })
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
            "기기 연결 코드를 다시 확인해 주세요.",
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
    })
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

async fn not_found(
    Extension(request_id): Extension<RequestId>,
) -> (StatusCode, Json<ErrorEnvelope>) {
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
                "/v1/agent/jobs/{job_id}",
                "/v1/auth/pairings/exchange",
                "/v1/auth/refresh",
                "/v1/conversations",
                "/v1/conversations/{conversation_id}/jobs/latest",
                "/v1/conversations/{conversation_id}/messages",
                "/v1/conversations/{conversation_id}/turns",
                "/v1/device-pairings",
                "/v1/devices",
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
                    .body(Body::from(r#"{"title":null}"#))
                    .expect("request should be valid"),
            )
            .await
            .expect("handler should respond");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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

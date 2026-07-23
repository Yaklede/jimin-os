use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use jimin_observability::RequestId;
use jimin_storage::{
    meetings::{
        Meeting, MeetingActionItem, MeetingActionKind, MeetingActionStatus, MeetingDecision,
        MeetingDetail, MeetingStatus, NewMeeting,
    },
    planning::{NewScheduleEntry, NewTask},
};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiState, auth, invalid_request_response, storage_error_response, unavailable_response,
};

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateMeetingRequest {
    title: String,
    transcript: String,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    started_at: Option<String>,
    duration_seconds: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DecideMeetingActionRequest {
    decision: MeetingActionDecision,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MeetingActionDecision {
    Approve,
    Reject,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingResponse {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_title: Option<String>,
    title: String,
    transcript: String,
    started_at: Option<String>,
    duration_seconds: Option<i32>,
    status: MeetingStatusResponse,
    summary: Option<String>,
    topics: Vec<String>,
    risks: Vec<String>,
    follow_up: Option<String>,
    analyzed_at: Option<String>,
    created_at: String,
    updated_at: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingListResponse {
    items: Vec<MeetingListItemResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingListItemResponse {
    id: Uuid,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    project_title: Option<String>,
    title: String,
    started_at: Option<String>,
    duration_seconds: Option<i32>,
    status: MeetingStatusResponse,
    summary: Option<String>,
    topics: Vec<String>,
    risks: Vec<String>,
    follow_up: Option<String>,
    analyzed_at: Option<String>,
    created_at: String,
    updated_at: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingDetailResponse {
    #[serde(flatten)]
    meeting: MeetingResponse,
    decisions: Vec<MeetingDecisionResponse>,
    action_items: Vec<MeetingActionItemResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingDecisionResponse {
    id: Uuid,
    content: String,
    rationale: Option<String>,
    source_excerpt: String,
    source_timestamp_seconds: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingActionItemResponse {
    id: Uuid,
    meeting_id: Uuid,
    kind: MeetingActionKindResponse,
    project_id: Option<Uuid>,
    title: String,
    notes: Option<String>,
    priority: i16,
    due_at: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    time_zone: Option<String>,
    source_excerpt: String,
    confidence: i16,
    status: MeetingActionStatusResponse,
    target_entity_id: Uuid,
    version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MeetingStatusResponse {
    Queued,
    Analyzing,
    ReviewReady,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MeetingActionKindResponse {
    Task,
    Schedule,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MeetingActionStatusResponse {
    Suggested,
    Applied,
    Rejected,
}

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/meetings", get(list_meetings).post(create_meeting))
        .route("/v1/meetings/{meeting_id}", get(get_meeting))
        .route(
            "/v1/meetings/{meeting_id}/reanalyze",
            post(reanalyze_meeting),
        )
        .route(
            "/v1/meetings/{meeting_id}/action-items/{item_id}/decisions",
            post(decide_meeting_action),
        )
}

#[utoipa::path(
    get,
    path = "/v1/meetings",
    tag = "meetings",
    responses((status = 200, body = MeetingListResponse), (status = 401), (status = 503))
)]
pub(crate) async fn list_meetings(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .meetings_for_user(principal.identity().user_id())
        .await
    {
        Ok(meetings) => match meetings
            .into_iter()
            .map(meeting_list_item_response)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(items) => Json(MeetingListResponse { items }).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/meetings/{meeting_id}/reanalyze",
    tag = "meetings",
    params(("meeting_id" = Uuid, Path)),
    responses((status = 200, body = MeetingResponse), (status = 401), (status = 404), (status = 409), (status = 503))
)]
pub(crate) async fn reanalyze_meeting(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(meeting_id): Path<Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .retry_meeting_analysis(principal.identity().user_id(), meeting_id)
        .await
    {
        Ok(meeting) => meeting_response(meeting).map(Json).map_or_else(
            |()| unavailable_response(request_id),
            IntoResponse::into_response,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/meetings",
    tag = "meetings",
    request_body = CreateMeetingRequest,
    responses((status = 201, body = MeetingResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
pub(crate) async fn create_meeting(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<CreateMeetingRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let started_at = match body.started_at.as_deref() {
        Some(value) => match OffsetDateTime::parse(value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .create_meeting(&NewMeeting {
            id: Uuid::now_v7(),
            user_id: principal.identity().user_id(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            title: body.title,
            transcript: body.transcript,
            started_at,
            duration_seconds: body.duration_seconds,
        })
        .await
    {
        Ok(meeting) => match meeting_response(meeting) {
            Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/meetings/{meeting_id}",
    tag = "meetings",
    params(("meeting_id" = Uuid, Path)),
    responses((status = 200, body = MeetingDetailResponse), (status = 401), (status = 404), (status = 503))
)]
pub(crate) async fn get_meeting(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(meeting_id): Path<Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .meeting_detail_for_user(principal.identity().user_id(), meeting_id)
        .await
    {
        Ok(Some(detail)) => match meeting_detail_response(detail) {
            Ok(response) => Json(response).into_response(),
            Err(()) => unavailable_response(request_id),
        },
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/meetings/{meeting_id}/action-items/{item_id}/decisions",
    tag = "meetings",
    params(("meeting_id" = Uuid, Path), ("item_id" = Uuid, Path)),
    request_body = DecideMeetingActionRequest,
    responses((status = 200, body = MeetingActionItemResponse), (status = 400), (status = 401), (status = 404), (status = 409), (status = 503))
)]
pub(crate) async fn decide_meeting_action(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((meeting_id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DecideMeetingActionRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user_id = principal.identity().user_id();
    let item = match database
        .meeting_action_item_for_user(user_id, meeting_id, item_id)
        .await
    {
        Ok(Some(item)) => item,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return storage_error_response(&error, request_id),
    };
    if matches!(body.decision, MeetingActionDecision::Approve)
        && item.status == MeetingActionStatus::Suggested
        && let Err(response) = apply_action(database, user_id, &item, request_id).await
    {
        return response;
    }
    let status = match body.decision {
        MeetingActionDecision::Approve => MeetingActionStatus::Applied,
        MeetingActionDecision::Reject => MeetingActionStatus::Rejected,
    };
    match database
        .decide_meeting_action_item(user_id, meeting_id, item_id, status)
        .await
    {
        Ok(item) => meeting_action_item_response(item).map(Json).map_or_else(
            |()| unavailable_response(request_id),
            IntoResponse::into_response,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

async fn apply_action(
    database: &jimin_storage::Database,
    user_id: Uuid,
    item: &MeetingActionItem,
    request_id: RequestId,
) -> Result<(), Response> {
    let result = match item.kind {
        MeetingActionKind::Task => database
            .create_task_idempotently(&NewTask {
                id: item.target_entity_id,
                user_id,
                project_id: item.project_id,
                title: item.title.clone(),
                notes: item.notes.clone(),
                assignee_name: None,
                priority: item.priority,
                due_at: item.due_at,
            })
            .await
            .map(|_| ()),
        MeetingActionKind::Schedule => {
            let (Some(starts_at), Some(ends_at), Some(time_zone)) =
                (item.starts_at, item.ends_at, item.time_zone.clone())
            else {
                return Err(invalid_request_response(request_id));
            };
            match database
                .schedule_entries_in_range(user_id, starts_at, ends_at)
                .await
            {
                Ok(conflicts) if !conflicts.is_empty() => {
                    return Err(StatusCode::CONFLICT.into_response());
                }
                Ok(_) => {}
                Err(error) => return Err(storage_error_response(&error, request_id)),
            }
            let entry = NewScheduleEntry {
                id: item.target_entity_id,
                user_id,
                title: item.title.clone(),
                notes: item.notes.clone(),
                starts_at,
                ends_at,
                time_zone,
            };
            match database.primary_calendar_mutation_target(user_id).await {
                Ok(Some(target)) => database
                    .create_schedule_entry_with_calendar_outbox(&entry, &target)
                    .await
                    .map(|_| ()),
                Ok(None) => database.create_schedule_entry(&entry).await.map(|_| ()),
                Err(error) => Err(error),
            }
        }
    };
    result.map_err(|error| storage_error_response(&error, request_id))
}

fn meeting_detail_response(detail: MeetingDetail) -> Result<MeetingDetailResponse, ()> {
    Ok(MeetingDetailResponse {
        meeting: meeting_response(detail.meeting)?,
        decisions: detail
            .decisions
            .into_iter()
            .map(meeting_decision_response)
            .collect(),
        action_items: detail
            .action_items
            .into_iter()
            .map(meeting_action_item_response)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn meeting_response(meeting: Meeting) -> Result<MeetingResponse, ()> {
    Ok(MeetingResponse {
        id: meeting.id,
        workspace_id: meeting.workspace_id,
        project_id: meeting.project_id,
        project_title: meeting.project_title,
        title: meeting.title,
        transcript: meeting.transcript,
        started_at: format_optional(meeting.started_at)?,
        duration_seconds: meeting.duration_seconds,
        status: meeting_status_response(meeting.status),
        summary: meeting.summary,
        topics: meeting.topics,
        risks: meeting.risks,
        follow_up: meeting.follow_up,
        analyzed_at: format_optional(meeting.analyzed_at)?,
        created_at: format_datetime(meeting.created_at)?,
        updated_at: format_datetime(meeting.updated_at)?,
        version: meeting.version,
    })
}

fn meeting_list_item_response(meeting: Meeting) -> Result<MeetingListItemResponse, ()> {
    Ok(MeetingListItemResponse {
        id: meeting.id,
        workspace_id: meeting.workspace_id,
        project_id: meeting.project_id,
        project_title: meeting.project_title,
        title: meeting.title,
        started_at: format_optional(meeting.started_at)?,
        duration_seconds: meeting.duration_seconds,
        status: meeting_status_response(meeting.status),
        summary: meeting.summary,
        topics: meeting.topics,
        risks: meeting.risks,
        follow_up: meeting.follow_up,
        analyzed_at: format_optional(meeting.analyzed_at)?,
        created_at: format_datetime(meeting.created_at)?,
        updated_at: format_datetime(meeting.updated_at)?,
        version: meeting.version,
    })
}

fn meeting_decision_response(decision: MeetingDecision) -> MeetingDecisionResponse {
    MeetingDecisionResponse {
        id: decision.id,
        content: decision.content,
        rationale: decision.rationale,
        source_excerpt: decision.source_excerpt,
        source_timestamp_seconds: decision.source_timestamp_seconds,
    }
}

fn meeting_action_item_response(item: MeetingActionItem) -> Result<MeetingActionItemResponse, ()> {
    Ok(MeetingActionItemResponse {
        id: item.id,
        meeting_id: item.meeting_id,
        kind: meeting_action_kind_response(item.kind),
        project_id: item.project_id,
        title: item.title,
        notes: item.notes,
        priority: item.priority,
        due_at: format_optional(item.due_at)?,
        starts_at: format_optional(item.starts_at)?,
        ends_at: format_optional(item.ends_at)?,
        time_zone: item.time_zone,
        source_excerpt: item.source_excerpt,
        confidence: item.confidence,
        status: meeting_action_status_response(item.status),
        target_entity_id: item.target_entity_id,
        version: item.version,
    })
}

const fn meeting_status_response(status: MeetingStatus) -> MeetingStatusResponse {
    match status {
        MeetingStatus::Queued => MeetingStatusResponse::Queued,
        MeetingStatus::Analyzing => MeetingStatusResponse::Analyzing,
        MeetingStatus::ReviewReady => MeetingStatusResponse::ReviewReady,
        MeetingStatus::Applied => MeetingStatusResponse::Applied,
        MeetingStatus::Failed => MeetingStatusResponse::Failed,
    }
}

const fn meeting_action_kind_response(kind: MeetingActionKind) -> MeetingActionKindResponse {
    match kind {
        MeetingActionKind::Task => MeetingActionKindResponse::Task,
        MeetingActionKind::Schedule => MeetingActionKindResponse::Schedule,
    }
}

const fn meeting_action_status_response(
    status: MeetingActionStatus,
) -> MeetingActionStatusResponse {
    match status {
        MeetingActionStatus::Suggested => MeetingActionStatusResponse::Suggested,
        MeetingActionStatus::Applied => MeetingActionStatusResponse::Applied,
        MeetingActionStatus::Rejected => MeetingActionStatusResponse::Rejected,
    }
}

fn format_optional(value: Option<OffsetDateTime>) -> Result<Option<String>, ()> {
    value.map(format_datetime).transpose()
}

fn format_datetime(value: OffsetDateTime) -> Result<String, ()> {
    value.format(&Rfc3339).map_err(|_| ())
}

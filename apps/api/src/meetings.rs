use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use jimin_observability::RequestId;
use jimin_storage::{
    meetings::{
        Meeting, MeetingActionItem, MeetingActionItemUpdate, MeetingActionKind,
        MeetingActionStatus, MeetingDecision, MeetingDetail, MeetingRecording,
        MeetingRecordingState, MeetingSpeaker, MeetingStatus, MeetingTranscriptSegment, NewMeeting,
        NewRecordedMeeting, RecordingChunk, RecordingFinalize, RecordingNoteUpdate,
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
    purpose: Option<String>,
    #[serde(default)]
    participants: Vec<String>,
    transcript: String,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    started_at: Option<String>,
    duration_seconds: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartMeetingRecordingRequest {
    title: String,
    purpose: Option<String>,
    #[serde(default)]
    participants: Vec<String>,
    workspace_id: Option<Uuid>,
    project_id: Option<Uuid>,
    started_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateMeetingRecordingNotesRequest {
    notes: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadMeetingRecordingChunkRequest {
    mime_type: String,
    audio_base64: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FinalizeMeetingRecordingRequest {
    mime_type: String,
    duration_milliseconds: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateMeetingActionRequest {
    expected_version: i64,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    priority: i16,
    due_at: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    time_zone: Option<String>,
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
    purpose: Option<String>,
    participants: Vec<String>,
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
    purpose: Option<String>,
    participants: Vec<String>,
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
    recording: Option<MeetingRecordingResponse>,
    speakers: Vec<MeetingSpeakerResponse>,
    transcript_segments: Vec<MeetingTranscriptSegmentResponse>,
    decisions: Vec<MeetingDecisionResponse>,
    action_items: Vec<MeetingActionItemResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartMeetingRecordingResponse {
    meeting: MeetingResponse,
    recording: MeetingRecordingResponse,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingRecordingResponse {
    id: Uuid,
    meeting_id: Uuid,
    state: MeetingRecordingStateResponse,
    mime_type: Option<String>,
    notes: String,
    duration_milliseconds: Option<i64>,
    chunk_count: i32,
    byte_length: i64,
    error_code: Option<String>,
    started_at: String,
    finalized_at: Option<String>,
    finished_at: Option<String>,
    updated_at: String,
    version: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingSpeakerResponse {
    id: Uuid,
    meeting_id: Uuid,
    speaker_key: String,
    display_name: Option<String>,
    ordinal: i16,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeetingTranscriptSegmentResponse {
    id: Uuid,
    meeting_id: Uuid,
    speaker_id: Uuid,
    speaker_key: String,
    speaker_name: Option<String>,
    ordinal: i32,
    starts_at_milliseconds: i64,
    ends_at_milliseconds: i64,
    text: String,
    confidence: Option<i16>,
    is_final: bool,
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
    assignee_name: Option<String>,
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
    Recording,
    Transcribing,
    Queued,
    Analyzing,
    ReviewReady,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MeetingRecordingStateResponse {
    Recording,
    Queued,
    Claimed,
    Running,
    Completed,
    Failed,
    Cancelled,
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
        .route("/v1/meeting-recordings", post(start_meeting_recording))
        .route(
            "/v1/meeting-recordings/{recording_id}/chunks/{sequence}",
            put(upload_meeting_recording_chunk).layer(DefaultBodyLimit::max(12 * 1024 * 1024)),
        )
        .route(
            "/v1/meeting-recordings/{recording_id}/notes",
            put(update_meeting_recording_notes),
        )
        .route(
            "/v1/meeting-recordings/{recording_id}/finalize",
            post(finalize_meeting_recording),
        )
        .route(
            "/v1/meeting-recordings/{recording_id}/cancel",
            post(cancel_meeting_recording),
        )
        .route(
            "/v1/meetings/{meeting_id}/reanalyze",
            post(reanalyze_meeting),
        )
        .route(
            "/v1/meetings/{meeting_id}/action-items/{item_id}",
            put(update_meeting_action),
        )
        .route(
            "/v1/meetings/{meeting_id}/action-items/{item_id}/decisions",
            post(decide_meeting_action),
        )
}

#[utoipa::path(
    post,
    path = "/v1/meeting-recordings",
    tag = "meetings",
    request_body = StartMeetingRecordingRequest,
    responses((status = 201, body = StartMeetingRecordingResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
pub(crate) async fn start_meeting_recording(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<StartMeetingRecordingRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Ok(started_at) = OffsetDateTime::parse(&body.started_at, &Rfc3339) else {
        return invalid_request_response(request_id);
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .create_recorded_meeting(&NewRecordedMeeting {
            meeting_id: Uuid::now_v7(),
            recording_id: Uuid::now_v7(),
            user_id: principal.identity().user_id(),
            workspace_id: body.workspace_id,
            project_id: body.project_id,
            title: body.title,
            purpose: body.purpose,
            participants: body.participants,
            started_at,
        })
        .await
    {
        Ok((meeting, recording)) => {
            match (
                meeting_response(meeting),
                meeting_recording_response(recording),
            ) {
                (Ok(meeting), Ok(recording)) => (
                    StatusCode::CREATED,
                    Json(StartMeetingRecordingResponse { meeting, recording }),
                )
                    .into_response(),
                _ => unavailable_response(request_id),
            }
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/meeting-recordings/{recording_id}/chunks/{sequence}",
    tag = "meetings",
    params(("recording_id" = Uuid, Path), ("sequence" = i32, Path)),
    request_body = UploadMeetingRecordingChunkRequest,
    responses((status = 200, body = MeetingRecordingResponse), (status = 400), (status = 401), (status = 409), (status = 413), (status = 503))
)]
pub(crate) async fn upload_meeting_recording_chunk(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((recording_id, sequence)): Path<(Uuid, i32)>,
    Json(body): Json<UploadMeetingRecordingChunkRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Ok(audio_data) = STANDARD.decode(body.audio_base64) else {
        return invalid_request_response(request_id);
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .append_meeting_recording_chunk(&RecordingChunk {
            recording_id,
            user_id: principal.identity().user_id(),
            sequence,
            mime_type: body.mime_type,
            audio_data,
        })
        .await
    {
        Ok(recording) => meeting_recording_response(recording).map(Json).map_or_else(
            |()| unavailable_response(request_id),
            IntoResponse::into_response,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/meeting-recordings/{recording_id}/notes",
    tag = "meetings",
    params(("recording_id" = Uuid, Path)),
    request_body = UpdateMeetingRecordingNotesRequest,
    responses((status = 200, body = MeetingRecordingResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
pub(crate) async fn update_meeting_recording_notes(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(recording_id): Path<Uuid>,
    Json(body): Json<UpdateMeetingRecordingNotesRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .update_meeting_recording_notes(&RecordingNoteUpdate {
            recording_id,
            user_id: principal.identity().user_id(),
            notes: body.notes,
        })
        .await
    {
        Ok(recording) => meeting_recording_response(recording).map(Json).map_or_else(
            |()| unavailable_response(request_id),
            IntoResponse::into_response,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/meeting-recordings/{recording_id}/finalize",
    tag = "meetings",
    params(("recording_id" = Uuid, Path)),
    request_body = FinalizeMeetingRecordingRequest,
    responses((status = 200, body = MeetingRecordingResponse), (status = 400), (status = 401), (status = 409), (status = 503))
)]
pub(crate) async fn finalize_meeting_recording(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(recording_id): Path<Uuid>,
    Json(body): Json<FinalizeMeetingRecordingRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .finalize_meeting_recording(&RecordingFinalize {
            recording_id,
            user_id: principal.identity().user_id(),
            mime_type: body.mime_type,
            duration_milliseconds: body.duration_milliseconds,
        })
        .await
    {
        Ok(recording) => meeting_recording_response(recording).map(Json).map_or_else(
            |()| unavailable_response(request_id),
            IntoResponse::into_response,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    post,
    path = "/v1/meeting-recordings/{recording_id}/cancel",
    tag = "meetings",
    params(("recording_id" = Uuid, Path)),
    responses((status = 204), (status = 401), (status = 409), (status = 503))
)]
pub(crate) async fn cancel_meeting_recording(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(recording_id): Path<Uuid>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    match database
        .cancel_meeting_recording(principal.identity().user_id(), recording_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
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
            purpose: body.purpose,
            participants: body.participants,
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
    put,
    path = "/v1/meetings/{meeting_id}/action-items/{item_id}",
    tag = "meetings",
    params(("meeting_id" = Uuid, Path), ("item_id" = Uuid, Path)),
    request_body = UpdateMeetingActionRequest,
    responses((status = 200, body = MeetingActionItemResponse), (status = 400), (status = 401), (status = 404), (status = 409), (status = 503))
)]
pub(crate) async fn update_meeting_action(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((meeting_id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMeetingActionRequest>,
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
    let Ok(due_at) = parse_optional_datetime(body.due_at.as_deref()) else {
        return invalid_request_response(request_id);
    };
    let Ok(starts_at) = parse_optional_datetime(body.starts_at.as_deref()) else {
        return invalid_request_response(request_id);
    };
    let Ok(ends_at) = parse_optional_datetime(body.ends_at.as_deref()) else {
        return invalid_request_response(request_id);
    };
    match database
        .update_meeting_action_item(&MeetingActionItemUpdate {
            id: item_id,
            meeting_id,
            user_id,
            expected_version: body.expected_version,
            kind: item.kind,
            title: body.title,
            notes: body.notes,
            assignee_name: body.assignee_name,
            priority: body.priority,
            due_at,
            starts_at,
            ends_at,
            time_zone: body.time_zone,
        })
        .await
    {
        Ok(Some(item)) => meeting_action_item_response(item).map(Json).map_or_else(
            |()| unavailable_response(request_id),
            IntoResponse::into_response,
        ),
        Ok(None) => StatusCode::CONFLICT.into_response(),
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
                parent_task_id: None,
                title: item.title.clone(),
                notes: item.notes.clone(),
                assignee_name: item.assignee_name.clone(),
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
        recording: detail
            .recording
            .map(meeting_recording_response)
            .transpose()?,
        speakers: detail
            .speakers
            .into_iter()
            .map(meeting_speaker_response)
            .collect(),
        transcript_segments: detail
            .transcript_segments
            .into_iter()
            .map(meeting_transcript_segment_response)
            .collect(),
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

fn meeting_recording_response(recording: MeetingRecording) -> Result<MeetingRecordingResponse, ()> {
    Ok(MeetingRecordingResponse {
        id: recording.id,
        meeting_id: recording.meeting_id,
        state: meeting_recording_state_response(recording.state),
        mime_type: recording.mime_type,
        notes: recording.notes,
        duration_milliseconds: recording.duration_milliseconds,
        chunk_count: recording.chunk_count,
        byte_length: recording.byte_length,
        error_code: recording.error_code,
        started_at: format_datetime(recording.started_at)?,
        finalized_at: format_optional(recording.finalized_at)?,
        finished_at: format_optional(recording.finished_at)?,
        updated_at: format_datetime(recording.updated_at)?,
        version: recording.version,
    })
}

fn meeting_speaker_response(speaker: MeetingSpeaker) -> MeetingSpeakerResponse {
    MeetingSpeakerResponse {
        id: speaker.id,
        meeting_id: speaker.meeting_id,
        speaker_key: speaker.speaker_key,
        display_name: speaker.display_name,
        ordinal: speaker.ordinal,
    }
}

fn meeting_transcript_segment_response(
    segment: MeetingTranscriptSegment,
) -> MeetingTranscriptSegmentResponse {
    MeetingTranscriptSegmentResponse {
        id: segment.id,
        meeting_id: segment.meeting_id,
        speaker_id: segment.speaker_id,
        speaker_key: segment.speaker_key,
        speaker_name: segment.speaker_name,
        ordinal: segment.ordinal,
        starts_at_milliseconds: segment.starts_at_milliseconds,
        ends_at_milliseconds: segment.ends_at_milliseconds,
        text: segment.text,
        confidence: segment.confidence,
        is_final: segment.is_final,
    }
}

fn meeting_response(meeting: Meeting) -> Result<MeetingResponse, ()> {
    Ok(MeetingResponse {
        id: meeting.id,
        workspace_id: meeting.workspace_id,
        project_id: meeting.project_id,
        project_title: meeting.project_title,
        title: meeting.title,
        purpose: meeting.purpose,
        participants: meeting.participants,
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
        purpose: meeting.purpose,
        participants: meeting.participants,
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
        assignee_name: item.assignee_name,
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
        MeetingStatus::Recording => MeetingStatusResponse::Recording,
        MeetingStatus::Transcribing => MeetingStatusResponse::Transcribing,
        MeetingStatus::Queued => MeetingStatusResponse::Queued,
        MeetingStatus::Analyzing => MeetingStatusResponse::Analyzing,
        MeetingStatus::ReviewReady => MeetingStatusResponse::ReviewReady,
        MeetingStatus::Applied => MeetingStatusResponse::Applied,
        MeetingStatus::Failed => MeetingStatusResponse::Failed,
    }
}

const fn meeting_recording_state_response(
    state: MeetingRecordingState,
) -> MeetingRecordingStateResponse {
    match state {
        MeetingRecordingState::Recording => MeetingRecordingStateResponse::Recording,
        MeetingRecordingState::Queued => MeetingRecordingStateResponse::Queued,
        MeetingRecordingState::Claimed => MeetingRecordingStateResponse::Claimed,
        MeetingRecordingState::Running => MeetingRecordingStateResponse::Running,
        MeetingRecordingState::Completed => MeetingRecordingStateResponse::Completed,
        MeetingRecordingState::Failed => MeetingRecordingStateResponse::Failed,
        MeetingRecordingState::Cancelled => MeetingRecordingStateResponse::Cancelled,
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

fn parse_optional_datetime(value: Option<&str>) -> Result<Option<OffsetDateTime>, ()> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ()))
        .transpose()
}

fn format_datetime(value: OffsetDateTime) -> Result<String, ()> {
    value.format(&Rfc3339).map_err(|_| ())
}

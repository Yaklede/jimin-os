//! Explicit collection/reporting roles and owner-selected project delivery destinations.
use crate::{ApiState, auth, error_response, storage_error_response, unavailable_response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use jimin_observability::RequestId;
use jimin_storage::task_chat::{ChatSourcePurpose, TaskChatRoute};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/projects/{project_id}/chat-task-routing",
            get(read).put(save).delete(remove),
        )
        .route(
            "/v1/projects/{project_id}/google-chat-sources/{source_id}/purpose",
            put(update_purpose),
        )
}
#[derive(OpenApi)]
#[openapi(paths(read,save,remove,update_purpose),components(schemas(TaskChatRoute,RouteResponse,RouteInput,SourcePurposeInput,ChatSourcePurpose)),tags((name="chat routing",description="업무 수집과 배정·완료 보고 대화방 분리")))]
pub(crate) struct ChatRoutingApiDoc;

#[derive(Serialize, ToSchema)]
struct RouteResponse {
    item: Option<TaskChatRoute>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RouteInput {
    source_id: Uuid,
    directory_webhook_id: Uuid,
    expected_version: i64,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SourcePurposeInput {
    purpose: ChatSourcePurpose,
    expected_version: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionQuery {
    expected_version: i64,
}

fn conflict(id: RequestId) -> Response {
    error_response(
        StatusCode::CONFLICT,
        "google_chat.routing_changed",
        "대화방 연결 설정이 바뀌었어요. 다시 불러온 뒤 저장해 주세요.",
        id,
        false,
    )
}

#[utoipa::path(get,path="/v1/projects/{project_id}/chat-task-routing",tag="chat routing",params(("project_id"=Uuid,Path)),responses((status=200,body=RouteResponse),(status=400),(status=401),(status=503)))]
async fn read(
    State(state): State<ApiState>,
    Extension(id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project): Path<Uuid>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(e) => return e.into_response(id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(id);
    };
    match db.task_chat_route(user, project).await {
        Ok(item) => crate::no_store_json(RouteResponse { item }),
        Err(e) => storage_error_response(&e, id),
    }
}
#[utoipa::path(put,path="/v1/projects/{project_id}/chat-task-routing",tag="chat routing",params(("project_id"=Uuid,Path)),request_body=RouteInput,responses((status=200,body=RouteResponse),(status=400),(status=401),(status=409),(status=422),(status=503)))]
async fn save(
    State(state): State<ApiState>,
    Extension(id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project): Path<Uuid>,
    Json(body): Json<RouteInput>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(e) => return e.into_response(id),
    };
    if state.google_chat_oauth().is_none() {
        return unavailable_response(id);
    }
    let Some(db) = state.planning() else {
        return unavailable_response(id);
    };
    match db
        .save_task_chat_route(
            user,
            project,
            body.source_id,
            body.directory_webhook_id,
            body.expected_version,
        )
        .await
    {
        Ok(Some(item)) => crate::no_store_json(RouteResponse { item: Some(item) }),
        Ok(None) => conflict(id),
        Err(e) => storage_error_response(&e, id),
    }
}
#[utoipa::path(delete,path="/v1/projects/{project_id}/chat-task-routing",tag="chat routing",params(("project_id"=Uuid,Path),("expected_version"=i64,Query)),responses((status=204),(status=400),(status=401),(status=409),(status=503)))]
async fn remove(
    State(state): State<ApiState>,
    Extension(id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project): Path<Uuid>,
    Query(query): Query<VersionQuery>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(e) => return e.into_response(id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(id);
    };
    match db
        .delete_task_chat_route(user, project, query.expected_version)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => conflict(id),
        Err(e) => storage_error_response(&e, id),
    }
}
#[utoipa::path(put,path="/v1/projects/{project_id}/google-chat-sources/{source_id}/purpose",tag="chat routing",params(("project_id"=Uuid,Path),("source_id"=Uuid,Path)),request_body=SourcePurposeInput,responses((status=200,body=crate::ProjectGoogleChatSourceResponse),(status=400),(status=401),(status=409),(status=422),(status=503)))]
async fn update_purpose(
    State(state): State<ApiState>,
    Extension(id): Extension<RequestId>,
    headers: HeaderMap,
    Path((project, source)): Path<(Uuid, Uuid)>,
    Json(body): Json<SourcePurposeInput>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(e) => return e.into_response(id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(id);
    };
    match db
        .update_chat_source_purpose(user, project, source, body.expected_version, body.purpose)
        .await
    {
        Ok(true) => match db.project_google_chat_sources(user, project).await {
            Ok(items) => items
                .into_iter()
                .find(|s| s.id == source)
                .and_then(crate::project_google_chat_source_response)
                .map_or_else(|| unavailable_response(id), crate::no_store_json),
            Err(e) => storage_error_response(&e, id),
        },
        Ok(false) => conflict(id),
        Err(e) => storage_error_response(&e, id),
    }
}

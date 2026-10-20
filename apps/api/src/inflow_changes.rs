use crate::{
    ApiState, auth, invalid_request_response, storage_error_response, unavailable_response,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use jimin_observability::RequestId;
use jimin_storage::inflow_changes::ApproveInflowChange;
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ApproveInflowChangeRequest {
    pub expected_item_version: i64,
    pub task_id: Uuid,
    pub expected_task_version: i64,
    pub source_revision: i32,
    pub notes: String,
    pub due_at: Option<String>,
}
#[utoipa::path(post,path="/v1/inflow/{item_id}/change-approval",tag="planning",params(("item_id"=Uuid,Path)),request_body=ApproveInflowChangeRequest,responses((status=204),(status=400),(status=401),(status=409),(status=503)))]
pub(crate) async fn approve_change(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(item_id): Path<Uuid>,
    Json(body): Json<ApproveInflowChangeRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(value) => value,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(storage) = state.planning() else {
        return unavailable_response(request_id);
    };
    let due_at = match body.due_at {
        Some(value) => match OffsetDateTime::parse(&value, &Rfc3339) {
            Ok(value) => Some(value),
            Err(_) => return invalid_request_response(request_id),
        },
        None => None,
    };
    match storage
        .approve_inflow_change(&ApproveInflowChange {
            user_id: principal.identity().user_id(),
            item_id,
            expected_item_version: body.expected_item_version,
            task_id: body.task_id,
            expected_task_version: body.expected_task_version,
            source_revision: body.source_revision,
            notes: body.notes,
            due_at,
        })
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::CONFLICT.into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

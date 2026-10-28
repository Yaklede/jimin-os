//! Explicit owner confirmation; assignee reports never authorize external writes.
use crate::{ApiState, auth, error_response, storage_error_response, unavailable_response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use jimin_observability::RequestId;
use jimin_storage::completion_reviews::CompletionReview;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use uuid::Uuid;

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/task-completion-reviews", get(list))
        .route("/v1/task-completion-reviews/{id}/decision", post(decide))
}
#[derive(OpenApi)]
#[openapi(paths(list,decide),components(schemas(CompletionReview,ReviewPage,Decision,DecisionKind,DecisionResult)),tags((name="completion reviews",description="담당자의 완료 보고와 소유자의 최종 확인")))]
pub(crate) struct CompletionReviewsApiDoc;
#[derive(Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
struct Page {
    before: Option<Uuid>,
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReviewPage {
    items: Vec<CompletionReview>,
    next_cursor: Option<Uuid>,
}
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum DecisionKind {
    Approve,
    Reject,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Decision {
    kind: DecisionKind,
    expected_version: i64,
    expected_task_version: i64,
    note: Option<String>,
}
#[derive(Serialize, ToSchema)]
struct DecisionResult {
    status: &'static str,
}

#[utoipa::path(get,path="/v1/task-completion-reviews",tag="completion reviews",params(Page),responses((status=200,body=ReviewPage),(status=400),(status=401),(status=503)))]
async fn list(
    State(state): State<ApiState>,
    Extension(id): Extension<RequestId>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(e) => return e.into_response(id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(id);
    };
    match db.completion_reviews(user, page.before).await {
        Ok(mut items) => {
            let more = items.len() > 50;
            items.truncate(50);
            let next_cursor = if more {
                items.last().map(|r| r.id)
            } else {
                None
            };
            Json(ReviewPage { items, next_cursor }).into_response()
        }
        Err(e) => storage_error_response(&e, id),
    }
}
#[utoipa::path(post,path="/v1/task-completion-reviews/{id}/decision",tag="completion reviews",params(("id"=Uuid,Path)),request_body=Decision,responses((status=200,body=DecisionResult),(status=400),(status=401),(status=409),(status=503)))]
async fn decide(
    State(state): State<ApiState>,
    Extension(request): Extension<RequestId>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<Decision>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(e) => return e.into_response(request),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request);
    };
    let approve = matches!(body.kind, DecisionKind::Approve);
    match db
        .decide_completion_review(
            user,
            id,
            body.expected_version,
            body.expected_task_version,
            approve,
            body.note.as_deref(),
        )
        .await
    {
        Ok(true) => Json(DecisionResult {
            status: if approve { "approved" } else { "rejected" },
        })
        .into_response(),
        Ok(false) => error_response(
            StatusCode::CONFLICT,
            "completion_review.changed",
            "The completion report or task changed. Reload and review it again.",
            request,
            false,
        ),
        Err(e) => storage_error_response(&e, request),
    }
}

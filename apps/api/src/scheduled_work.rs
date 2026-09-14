//! Owner-scoped recurring work. Creating a draft does not send any message.
use crate::{ApiState, auth, error_response, storage_error_response, unavailable_response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use jimin_observability::RequestId;
use jimin_storage::{
    StorageError,
    scheduled_work::{
        ScheduledWork, ScheduledWorkDefinition, ScheduledWorkPreview, ScheduledWorkRun,
    },
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, OpenApi, ToSchema};
use uuid::Uuid;

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/v1/scheduled-work", get(list))
        .route("/v1/scheduled-work/preview", post(preview))
        .route("/v1/scheduled-work/runs", get(runs))
        .route("/v1/scheduled-work/{id}", put(save))
        .route("/v1/scheduled-work/{id}/actions", post(action))
}

#[derive(OpenApi)]
#[openapi(paths(list,preview,runs,save,action),components(schemas(SaveRequest,ActionRequest,ActionKind,ActionResult,WorkList,RunList,ScheduledWorkDefinition,ScheduledWork,ScheduledWorkPreview,ScheduledWorkRun)),tags((name="scheduled work",description="예약 업무와 실행 이력")))]
pub(crate) struct ScheduledWorkApiDoc;

#[derive(Serialize, ToSchema)]
struct WorkList {
    items: Vec<ScheduledWork>,
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct RunList {
    items: Vec<ScheduledWorkRun>,
    next_cursor: Option<Uuid>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SaveRequest {
    definition: ScheduledWorkDefinition,
    enabled: bool,
    expected_version: Option<i64>,
}
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum ActionKind {
    RunNow,
    SkipOnce,
    Pause,
    Delete,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ActionRequest {
    kind: ActionKind,
    expected_version: i64,
    request_id: Option<Uuid>,
}
#[derive(Serialize, ToSchema)]
struct ActionResult {
    run: Option<ScheduledWorkRun>,
}
#[derive(Deserialize, IntoParams)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RunQuery {
    scheduled_work_id: Option<Uuid>,
    before: Option<Uuid>,
}

#[utoipa::path(get,path="/v1/scheduled-work",tag="scheduled work",responses((status=200,body=WorkList),(status=401),(status=503)))]
async fn list(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(f) => return f.into_response(request_id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request_id);
    };
    match db.scheduled_work_for_user(user).await {
        Ok(items) => Json(WorkList { items }).into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(post,path="/v1/scheduled-work/preview",tag="scheduled work",request_body=ScheduledWorkDefinition,responses((status=200,body=ScheduledWorkPreview),(status=400),(status=401),(status=503)))]
async fn preview(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(definition): Json<ScheduledWorkDefinition>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(f) => return f.into_response(request_id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request_id);
    };
    match db
        .preview_scheduled_work(user, &definition, OffsetDateTime::now_utc())
        .await
    {
        Ok(preview) => Json(preview).into_response(),
        Err(error) => failure(error, request_id),
    }
}

#[utoipa::path(put,path="/v1/scheduled-work/{id}",tag="scheduled work",params(("id"=Uuid,Path)),request_body=SaveRequest,responses((status=200,body=ScheduledWork),(status=400),(status=401),(status=409),(status=503)))]
async fn save(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveRequest>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(f) => return f.into_response(request_id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request_id);
    };
    match db
        .save_scheduled_work(
            user,
            id,
            &body.definition,
            body.enabled,
            body.expected_version,
            OffsetDateTime::now_utc(),
        )
        .await
    {
        Ok(Some(work)) => Json(work).into_response(),
        Ok(None) => conflict(request_id),
        Err(error) => failure(error, request_id),
    }
}

#[utoipa::path(get,path="/v1/scheduled-work/runs",tag="scheduled work",params(RunQuery),responses((status=200,body=RunList),(status=401),(status=503)))]
async fn runs(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Query(query): Query<RunQuery>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(f) => return f.into_response(request_id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request_id);
    };
    match db
        .scheduled_work_runs(user, query.scheduled_work_id, query.before)
        .await
    {
        Ok(items) => {
            let next_cursor = if items.len() == 50 {
                items.last().map(|r| r.id)
            } else {
                None
            };
            Json(RunList { items, next_cursor }).into_response()
        }
        Err(error) => failure(error, request_id),
    }
}

#[utoipa::path(post,path="/v1/scheduled-work/{id}/actions",tag="scheduled work",params(("id"=Uuid,Path)),request_body=ActionRequest,responses((status=200,body=ActionResult),(status=400),(status=401),(status=409),(status=503)))]
async fn action(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionRequest>,
) -> Response {
    let user = match auth::authenticate(&state, &headers).await {
        Ok(p) => p.identity().user_id(),
        Err(f) => return f.into_response(request_id),
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request_id);
    };
    if id.get_version_num() != 7 || body.expected_version <= 0 {
        return failure(StorageError::InvalidConfiguration, request_id);
    }
    if matches!(body.kind, ActionKind::RunNow) {
        let Some(key) = body.request_id else {
            return failure(StorageError::InvalidConfiguration, request_id);
        };
        return match db
            .run_scheduled_work_now(
                user,
                id,
                key,
                body.expected_version,
                OffsetDateTime::now_utc(),
            )
            .await
        {
            Ok(Some(run)) => Json(ActionResult { run: Some(run) }).into_response(),
            Ok(None) => conflict(request_id),
            Err(error) => failure(error, request_id),
        };
    }
    let result = match body.kind {
        ActionKind::SkipOnce => {
            db.skip_scheduled_work_once(user, id, body.expected_version, OffsetDateTime::now_utc())
                .await
        }
        ActionKind::Pause => {
            db.pause_scheduled_work(user, id, body.expected_version)
                .await
        }
        ActionKind::Delete => {
            db.delete_scheduled_work(user, id, body.expected_version)
                .await
        }
        ActionKind::RunNow => unreachable!(),
    };
    match result {
        Ok(true) => Json(ActionResult { run: None }).into_response(),
        Ok(false) => conflict(request_id),
        Err(error) => failure(error, request_id),
    }
}

#[allow(clippy::needless_pass_by_value)] // Callers hand off classified errors at the response boundary.
fn failure(error: StorageError, id: RequestId) -> Response {
    if matches!(error, StorageError::InvalidConfiguration) {
        error_response(
            StatusCode::BAD_REQUEST,
            "scheduled_work.invalid",
            "예약 조건과 프로젝트 연결, 멘션할 사람을 확인해 주세요.",
            id,
            false,
        )
    } else {
        storage_error_response(&error, id)
    }
}
fn conflict(id: RequestId) -> Response {
    error_response(
        StatusCode::CONFLICT,
        "scheduled_work.conflict",
        "예약이 변경됐거나 삭제됐어요. 새로고침한 뒤 다시 시도해 주세요.",
        id,
        false,
    )
}

/// Runs independently of clients; only sanitized operational errors are logged.
#[must_use]
pub fn spawn_scheduled_work_worker(state: &ApiState) -> Option<tokio::task::JoinHandle<()>> {
    let db = state.planning()?.clone();
    Some(tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if db.reconcile_scheduled_work_runs().await.is_err() {
                tracing::warn!(event = "scheduled_work.reconcile_failed");
                continue;
            }
            for _ in 0..100 {
                match db
                    .process_due_scheduled_work(OffsetDateTime::now_utc())
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(_) => {
                        tracing::warn!(event = "scheduled_work.tick_failed");
                        break;
                    }
                }
            }
        }
    }))
}

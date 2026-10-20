use crate::{
    ApiState, TaskResponse, auth, invalid_request_response, storage_error_response, task_response,
    unavailable_response,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use jimin_observability::RequestId;
use jimin_storage::report_evidence::ReportEvidenceQuery;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;
#[derive(Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EvidenceQuery {
    metric: String,
    period_start: String,
    period_end: String,
    after: Option<Uuid>,
}
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvidencePage {
    items: Vec<TaskResponse>,
    next_cursor: Option<Uuid>,
}
#[utoipa::path(get,path="/v1/projects/{project_id}/weekly-evidence",tag="reports",params(("project_id"=Uuid,Path),EvidenceQuery),responses((status=200,body=EvidencePage),(status=400),(status=401),(status=503)))]
pub(crate) async fn get_evidence(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Query(query): Query<EvidenceQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(value) => value,
        Err(error) => return error.into_response(request_id),
    };
    let (Ok(period_start), Ok(period_end)) = (
        OffsetDateTime::parse(&query.period_start, &Rfc3339),
        OffsetDateTime::parse(&query.period_end, &Rfc3339),
    ) else {
        return invalid_request_response(request_id);
    };
    let Some(storage) = state.planning() else {
        return unavailable_response(request_id);
    };
    match storage
        .weekly_report_evidence(&ReportEvidenceQuery {
            user_id: principal.identity().user_id(),
            project_id,
            metric: query.metric,
            period_start,
            period_end,
            after: query.after,
        })
        .await
    {
        Ok(mut tasks) => {
            let more = tasks.len() > 100;
            tasks.truncate(100);
            let next_cursor = more.then(|| tasks.last().map(|task| task.id)).flatten();
            let Ok(items) = tasks
                .into_iter()
                .map(task_response)
                .collect::<Result<Vec<_>, _>>()
            else {
                return unavailable_response(request_id);
            };
            Json(EvidencePage { items, next_cursor }).into_response()
        }
        Err(error) => storage_error_response(&error, request_id),
    }
}

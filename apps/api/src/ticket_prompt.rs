use std::{path::PathBuf, time::Duration};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, State, rejection::JsonRejection},
    http::HeaderMap,
    response::Response,
    routing::post,
};
use jimin_storage::ticket_prompt::{DraftRequest, DraftResponse, REQUEST_BYTES};

use super::{
    ApiState, RequestId, auth, invalid_request_response, no_store_json, unavailable_response,
};

const DRAFT_ROUTE: &str = "/v1/ticket-prompt/draft";

pub(super) fn router() -> Router<ApiState> {
    Router::new().route(
        DRAFT_ROUTE,
        post(generate).layer(DefaultBodyLimit::max(REQUEST_BYTES)),
    )
}

pub(super) fn client(path: PathBuf) -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .unix_socket(path)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(25))
        .build()
        .ok()
}

#[utoipa::path(
    post,
    path = "/v1/ticket-prompt/draft",
    tag = "agent",
    request_body = DraftRequest,
    responses((status = 200, body = DraftResponse), (status = 400), (status = 401), (status = 503))
)]
pub(super) async fn generate(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    payload: Result<Json<DraftRequest>, JsonRejection>,
) -> Response {
    if let Err(failure) = auth::authenticate(&state, &headers).await {
        return failure.into_response(request_id);
    }
    let Ok(Json(request)) = payload else {
        return invalid_request_response(request_id);
    };
    if !request.is_valid() {
        return invalid_request_response(request_id);
    }
    let Some(client) = &state.ticket_prompt else {
        return unavailable_response(request_id);
    };
    let response = async {
        client
            .post("http://agent/draft")
            .json(&request)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<DraftResponse>()
            .await
            .ok()?
            .grounded(&request)
    }
    .await;
    match response {
        Some(response) => no_store_json(response),
        None => unavailable_response(request_id),
    }
}

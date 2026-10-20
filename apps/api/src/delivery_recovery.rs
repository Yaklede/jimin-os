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
use jimin_storage::webhook::RetryWebhookDeliveryOutcome;
use uuid::Uuid;
#[utoipa::path(post, path="/v1/delivery-recovery/{kind}/{id}/retry", tag="notifications", params(("kind"=String,Path),("id"=Uuid,Path)), responses((status=202),(status=400),(status=401),(status=409),(status=503)))]
pub(crate) async fn retry_delivery(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path((kind, id)): Path<(String, Uuid)>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(value) => value,
        Err(failure) => return failure.into_response(request_id),
    };
    if !matches!(kind.as_str(), "webhook" | "push" | "itsm") {
        return invalid_request_response(request_id);
    }
    let Some(storage) = state.planning() else {
        return unavailable_response(request_id);
    };
    let user = principal.identity().user_id();
    let outcome = if kind == "itsm" {
        storage.retry_itsm_sync(user, id).await
    } else if kind == "push" {
        storage.retry_failed_push_delivery(user, id).await
    } else {
        match storage.delivery_recovery_for_user(user).await {
            Ok(items) => {
                if let Some(item) = items
                    .iter()
                    .find(|item| item.kind == kind && item.id == id && item.can_retry)
                {
                    match item.project_id {
                        Some(project) => storage
                            .retry_webhook_delivery(user, project, id)
                            .await
                            .map(|result| {
                                matches!(
                                    result,
                                    RetryWebhookDeliveryOutcome::Queued
                                        | RetryWebhookDeliveryOutcome::AlreadyQueued
                                )
                            }),
                        None => Ok(false),
                    }
                } else {
                    Ok(false)
                }
            }
            Err(error) => Err(error),
        }
    };
    match outcome {
        Ok(true) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"queued":true})),
        )
            .into_response(),
        Ok(false) => StatusCode::CONFLICT.into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

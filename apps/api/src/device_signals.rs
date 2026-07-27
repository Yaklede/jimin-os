//! Authenticated Android device-signal API.

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use jimin_observability::RequestId;
use jimin_storage::device_signals::{
    CallLogPermission, DeviceSignalState, MissedCallSignal, MissedCallSyncRequest,
    NewMissedCallSignal,
};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{ApiState, auth, error_response, storage_error_response, unavailable_response};

const MAX_CALLS_PER_SYNC: usize = 200;
const DEFAULT_CALL_LOOKBACK_DAYS: i64 = 7;

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/device-signals/missed-calls",
            get(list_missed_calls).put(sync_missed_calls),
        )
        .route("/v1/device-signals/status", get(device_signal_status))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallLogPermissionRequest {
    NotDetermined,
    Granted,
    Denied,
    Unavailable,
}

impl From<CallLogPermissionRequest> for CallLogPermission {
    fn from(value: CallLogPermissionRequest) -> Self {
        match value {
            CallLogPermissionRequest::NotDetermined => Self::NotDetermined,
            CallLogPermissionRequest::Granted => Self::Granted,
            CallLogPermissionRequest::Denied => Self::Denied,
            CallLogPermissionRequest::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SyncMissedCallsRequest {
    permission: CallLogPermissionRequest,
    platform_version: Option<String>,
    app_version: Option<String>,
    calls: Vec<MissedCallRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MissedCallRequest {
    source_id: String,
    occurred_at: String,
    caller_name: Option<String>,
    phone_number: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSignalSyncResponse {
    inserted_count: usize,
    state: DeviceSignalStateResponse,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSignalStateListResponse {
    items: Vec<DeviceSignalStateResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSignalStateResponse {
    device_id: Uuid,
    device_name: String,
    call_log_permission: String,
    platform_version: Option<String>,
    app_version: Option<String>,
    last_synced_at: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MissedCallListQuery {
    since: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissedCallListResponse {
    items: Vec<MissedCallResponse>,
    device_states: Vec<DeviceSignalStateResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissedCallResponse {
    id: Uuid,
    device_id: Uuid,
    device_name: String,
    occurred_at: String,
    caller_name: Option<String>,
    phone_number: Option<String>,
}

#[utoipa::path(
    put,
    path = "/v1/device-signals/missed-calls",
    tag = "device signals",
    request_body = SyncMissedCallsRequest,
    responses(
        (status = 200, body = DeviceSignalSyncResponse),
        (status = 400),
        (status = 401),
        (status = 503)
    )
)]
async fn sync_missed_calls(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<SyncMissedCallsRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    let now = OffsetDateTime::now_utc();
    let permission = CallLogPermission::from(body.permission);
    let Ok(calls) = validated_calls(body.calls, permission, now) else {
        return invalid_request(request_id);
    };
    match database
        .sync_missed_calls(MissedCallSyncRequest {
            user_id: principal.identity().user_id(),
            device_id: principal.identity().device_id(),
            permission,
            platform_version: body.platform_version.as_deref(),
            app_version: body.app_version.as_deref(),
            synced_at: now,
            calls: &calls,
        })
        .await
    {
        Ok(result) => Json(DeviceSignalSyncResponse {
            inserted_count: result.inserted_count,
            state: state_response(result.state),
        })
        .into_response(),
        Err(jimin_storage::StorageError::InvalidConfiguration) => invalid_request(request_id),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/device-signals/status",
    tag = "device signals",
    responses(
        (status = 200, body = DeviceSignalStateListResponse),
        (status = 401),
        (status = 503)
    )
)]
async fn device_signal_status(
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
        .device_signal_states_for_user(principal.identity().user_id())
        .await
    {
        Ok(items) => Json(DeviceSignalStateListResponse {
            items: items.into_iter().map(state_response).collect(),
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    get,
    path = "/v1/device-signals/missed-calls",
    tag = "device signals",
    params(MissedCallListQuery),
    responses(
        (status = 200, body = MissedCallListResponse),
        (status = 400),
        (status = 401),
        (status = 503)
    )
)]
async fn list_missed_calls(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Query(query): Query<MissedCallListQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(database) = state.planning() else {
        return unavailable_response(request_id);
    };
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return invalid_request(request_id);
    }
    let since = match query.since.as_deref() {
        Some(value) => match OffsetDateTime::parse(value, &Rfc3339) {
            Ok(value) => value,
            Err(_) => return invalid_request(request_id),
        },
        None => OffsetDateTime::now_utc() - Duration::days(DEFAULT_CALL_LOOKBACK_DAYS),
    };
    let user_id = principal.identity().user_id();
    let result = tokio::try_join!(
        database.recent_missed_calls_for_user(user_id, since, limit),
        database.device_signal_states_for_user(user_id),
    );
    match result {
        Ok((calls, states)) => Json(MissedCallListResponse {
            items: calls.into_iter().map(call_response).collect(),
            device_states: states.into_iter().map(state_response).collect(),
        })
        .into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

fn validated_calls(
    calls: Vec<MissedCallRequest>,
    permission: CallLogPermission,
    now: OffsetDateTime,
) -> Result<Vec<NewMissedCallSignal>, ()> {
    if calls.len() > MAX_CALLS_PER_SYNC
        || permission != CallLogPermission::Granted && !calls.is_empty()
    {
        return Err(());
    }
    calls
        .into_iter()
        .map(|call| {
            let occurred_at =
                OffsetDateTime::parse(call.occurred_at.trim(), &Rfc3339).map_err(|_| ())?;
            if call.source_id.trim().is_empty()
                || call.source_id.len() > 120
                || call
                    .caller_name
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty() || value.len() > 120)
                || call
                    .phone_number
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty() || value.len() > 64)
                || occurred_at > now + Duration::minutes(5)
                || occurred_at < now - Duration::days(90)
            {
                return Err(());
            }
            Ok(NewMissedCallSignal {
                id: Uuid::now_v7(),
                source_event_id: call.source_id,
                occurred_at,
                caller_name: call.caller_name,
                phone_number: call.phone_number,
            })
        })
        .collect()
}

fn state_response(state: DeviceSignalState) -> DeviceSignalStateResponse {
    DeviceSignalStateResponse {
        device_id: state.device_id,
        device_name: state.device_name,
        call_log_permission: state.permission.as_str().to_owned(),
        platform_version: state.platform_version,
        app_version: state.app_version,
        last_synced_at: state.last_synced_at.map(format_timestamp),
    }
}

fn call_response(call: MissedCallSignal) -> MissedCallResponse {
    MissedCallResponse {
        id: call.id,
        device_id: call.device_id,
        device_name: call.device_name,
        occurred_at: format_timestamp(call.occurred_at),
        caller_name: call.caller_name,
        phone_number: call.phone_number,
    }
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("valid OffsetDateTime must format as RFC 3339")
}

fn invalid_request(request_id: RequestId) -> Response {
    error_response(
        StatusCode::BAD_REQUEST,
        "request.invalid",
        "휴대폰 정보 연결을 다시 시도해 주세요.",
        request_id,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_permission_cannot_upload_call_rows() {
        let result = validated_calls(
            vec![MissedCallRequest {
                source_id: "1".to_owned(),
                occurred_at: "2026-07-27T09:00:00+09:00".to_owned(),
                caller_name: None,
                phone_number: None,
            }],
            CallLogPermission::Denied,
            OffsetDateTime::parse("2026-07-27T10:00:00+09:00", &Rfc3339).expect("fixture"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn granted_permission_accepts_a_recent_missed_call() {
        let result = validated_calls(
            vec![MissedCallRequest {
                source_id: "call-42".to_owned(),
                occurred_at: "2026-07-27T09:00:00+09:00".to_owned(),
                caller_name: Some("홍길동".to_owned()),
                phone_number: Some("010-0000-0000".to_owned()),
            }],
            CallLogPermission::Granted,
            OffsetDateTime::parse("2026-07-27T10:00:00+09:00", &Rfc3339).expect("fixture"),
        );

        assert_eq!(result.expect("valid request").len(), 1);
    }
}

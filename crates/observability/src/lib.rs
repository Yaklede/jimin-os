use std::{
    fmt::{self, Debug, Display},
    time::Instant,
};

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use thiserror::Error;
use tracing::{Instrument, info, info_span};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(Uuid);

impl RequestId {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> Uuid {
        self.0
    }
}

impl Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Debug for Redacted<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl<T> Display for Redacted<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Debug, Error)]
#[error("tracing subscriber could not be initialized")]
pub struct TracingInitError;

/// Installs the process-wide JSON tracing subscriber.
///
/// # Errors
///
/// Returns an error when another subscriber is already installed or the
/// subscriber cannot be registered.
pub fn init_tracing() -> Result<(), TracingInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(false),
        )
        .try_init()
        .map_err(|_| TracingInitError)
}

pub async fn request_context(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .map_or_else(|| RequestId::new(Uuid::now_v7()), RequestId::new);

    request.extensions_mut().insert(request_id);

    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    let started_at = Instant::now();
    let span = info_span!(
        "http.request",
        request_id = %request_id,
        method = %method,
        path = %path
    );

    let mut response = next.run(request).instrument(span).await;
    let status = response.status().as_u16();
    let latency_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);

    info!(
        event = "http.request.completed",
        request_id = %request_id,
        method = %method,
        path = %path,
        status,
        latency_ms
    );

    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    response
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::Request as HttpRequest, middleware, routing::get};
    use tower::ServiceExt;

    use super::*;

    #[test]
    fn redacted_values_do_not_leak_through_formatting() {
        let value = Redacted::new("sensitive-value");

        assert_eq!(format!("{value}"), "[REDACTED]");
        assert_eq!(format!("{value:?}"), "[REDACTED]");
    }

    #[tokio::test]
    async fn preserves_a_valid_incoming_request_id() {
        let expected = Uuid::now_v7();
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_context));
        let request = HttpRequest::builder()
            .uri("/")
            .header(&REQUEST_ID_HEADER, expected.to_string())
            .body(Body::empty())
            .expect("request should be valid");

        let response = app.oneshot(request).await.expect("request should succeed");

        assert_eq!(
            response.headers().get(&REQUEST_ID_HEADER),
            Some(&HeaderValue::from_str(&expected.to_string()).expect("UUID is a valid header"))
        );
    }

    #[tokio::test]
    async fn replaces_an_invalid_request_id_with_a_uuid() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_context));
        let request = HttpRequest::builder()
            .uri("/")
            .header(&REQUEST_ID_HEADER, "not-a-uuid")
            .body(Body::empty())
            .expect("request should be valid");

        let response = app.oneshot(request).await.expect("request should succeed");
        let request_id = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .expect("request ID should be present")
            .to_str()
            .expect("request ID should be text");

        assert!(Uuid::parse_str(request_id).is_ok());
    }
}

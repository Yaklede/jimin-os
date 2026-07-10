//! HTTP authentication boundary for protected Jimin OS resources.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    http::{HeaderMap, header::AUTHORIZATION},
    response::Response,
};
use jimin_auth::{AccessTokenVerifier, SessionIdentity};
use jimin_storage::{
    Database, StorageError,
    auth::{Device, Profile},
};

use crate::ApiState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedPrincipal {
    identity: SessionIdentity,
}

impl AuthenticatedPrincipal {
    #[must_use]
    pub const fn identity(&self) -> SessionIdentity {
        self.identity
    }
}

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn session_is_active(&self, identity: SessionIdentity) -> Result<bool, StorageError>;
    async fn profile_for_user(&self, user_id: uuid::Uuid) -> Result<Option<Profile>, StorageError>;
    async fn devices_for_user(&self, user_id: uuid::Uuid) -> Result<Vec<Device>, StorageError>;
}

#[async_trait]
impl AuthRepository for Database {
    async fn session_is_active(&self, identity: SessionIdentity) -> Result<bool, StorageError> {
        self.is_session_active(identity).await
    }

    async fn profile_for_user(&self, user_id: uuid::Uuid) -> Result<Option<Profile>, StorageError> {
        self.profile_for_user(user_id).await
    }

    async fn devices_for_user(&self, user_id: uuid::Uuid) -> Result<Vec<Device>, StorageError> {
        self.devices_for_user(user_id).await
    }
}

pub struct Authentication {
    verifier: AccessTokenVerifier,
    repository: Arc<dyn AuthRepository>,
}

impl Authentication {
    #[must_use]
    pub fn new(verifier: AccessTokenVerifier, repository: Arc<dyn AuthRepository>) -> Self {
        Self {
            verifier,
            repository,
        }
    }

    #[must_use]
    pub fn repository(&self) -> &Arc<dyn AuthRepository> {
        &self.repository
    }
}

/// Validates one strict bearer header, then confirms the signed session is
/// still active in persistent storage.
///
/// # Errors
///
/// Returns [`AuthenticationFailure::Unauthorized`] for malformed, invalid, or
/// revoked sessions and [`AuthenticationFailure::Unavailable`] when the
/// persistent session guard cannot be checked.
pub async fn authenticate(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<AuthenticatedPrincipal, AuthenticationFailure> {
    let authentication = state
        .authentication()
        .ok_or(AuthenticationFailure::Unavailable)?;
    let token = bearer_token(headers).ok_or(AuthenticationFailure::Unauthorized)?;
    let identity = authentication
        .verifier
        .verify(token)
        .map_err(|_| AuthenticationFailure::Unauthorized)?;
    let active = authentication
        .repository
        .session_is_active(identity)
        .await
        .map_err(|_| AuthenticationFailure::Unavailable)?;
    if !active {
        return Err(AuthenticationFailure::Unauthorized);
    }
    Ok(AuthenticatedPrincipal { identity })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationFailure {
    Unauthorized,
    Unavailable,
}

impl AuthenticationFailure {
    #[must_use]
    pub fn into_response(self, request_id: jimin_observability::RequestId) -> Response {
        match self {
            Self::Unauthorized => crate::error_response(
                axum::http::StatusCode::UNAUTHORIZED,
                "auth.session_expired",
                "다시 로그인해 주세요.",
                request_id,
                false,
            ),
            Self::Unavailable => crate::error_response(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "service.temporarily_unavailable",
                "잠시 후 다시 시도해 주세요.",
                request_id,
                true,
            ),
        }
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let value = value.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    if token.is_empty() || token.contains(char::is_whitespace) {
        return None;
    }
    Some(token)
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderValue, header::AUTHORIZATION};

    use super::*;

    #[test]
    fn bearer_parser_requires_exactly_one_well_formed_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer valid-token"),
        );
        assert_eq!(bearer_token(&headers), Some("valid-token"));

        headers.append(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer another-token"),
        );
        assert_eq!(bearer_token(&headers), None);

        let mut malformed = HeaderMap::new();
        malformed.insert(
            AUTHORIZATION,
            HeaderValue::from_static("bearer valid-token"),
        );
        assert_eq!(bearer_token(&malformed), None);
    }
}

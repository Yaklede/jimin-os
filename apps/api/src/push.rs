//! Authenticated Android FCM registration and server-owned reminder delivery.

use std::time::Duration as StdDuration;

use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use jimin_observability::RequestId;
use jimin_storage::{
    StorageError,
    push::{ClaimedPushDelivery, EncryptedPushToken, PushRegistrationState},
};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand::RngExt;
use reqwest::{Client, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{ApiState, auth, error_response, storage_error_response, unavailable_response};

const NONCE_BYTES: usize = 24;
const MAX_FCM_TOKEN_BYTES: usize = 4 * 1024;
const MAX_PROVIDER_BODY_BYTES: usize = 64 * 1024;
const GOOGLE_OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_OAUTH_AUDIENCE: &str = "https://oauth2.googleapis.com/token";
const PEM_BEGIN: &str = concat!("-----BEGIN ", "PRIVATE KEY-----");
#[cfg(test)]
const PEM_END: &str = concat!("-----END ", "PRIVATE KEY-----");
const PEM_END_LINE: &str = concat!("-----END ", "PRIVATE KEY-----\n");

pub struct PushRuntime {
    key: [u8; 32],
    service_account: FirebaseServiceAccount,
    client: Client,
    access_token: Mutex<Option<CachedAccessToken>>,
}

struct FirebaseServiceAccount {
    project_id: String,
    client_email: String,
    private_key: SecretString,
    token_uri: String,
}

struct CachedAccessToken {
    value: SecretString,
    expires_at: OffsetDateTime,
}

#[derive(Deserialize)]
struct FirebaseServiceAccountDocument {
    #[serde(rename = "type")]
    account_type: String,
    project_id: String,
    private_key: String,
    client_email: String,
    token_uri: String,
}

#[derive(Serialize)]
struct ServiceAccountClaims<'a> {
    iss: &'a str,
    scope: &'static str,
    aud: &'static str,
    iat: i64,
    exp: i64,
}

#[derive(Deserialize)]
struct GoogleAccessTokenResponse {
    access_token: String,
    expires_in: i64,
    token_type: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PushRegistrationResponse {
    enabled: bool,
    provider: &'static str,
    last_seen_at: Option<String>,
    last_delivered_at: Option<String>,
    last_error_code: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegisterPushTokenRequest {
    provider: String,
    token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushRuntimeError {
    Invalid,
    Authentication,
    Unavailable,
    Rejected(i32),
    Unregistered(i32),
}

impl PushRuntimeError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "push.configuration_invalid",
            Self::Authentication => "push.provider_authentication_failed",
            Self::Unavailable => "push.provider_unavailable",
            Self::Rejected(_) => "push.provider_rejected",
            Self::Unregistered(_) => "push.token_unregistered",
        }
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::Unavailable | Self::Rejected(429))
    }

    #[must_use]
    pub const fn invalidates_token(self) -> bool {
        matches!(self, Self::Unregistered(_))
    }

    #[must_use]
    pub const fn response_code(self) -> Option<i32> {
        match self {
            Self::Rejected(code) | Self::Unregistered(code) => Some(code),
            Self::Invalid | Self::Authentication | Self::Unavailable => None,
        }
    }
}

impl PushRuntime {
    /// Builds a Firebase HTTP v1 runtime from a deployment-owned service
    /// account and derives a push-only token encryption key.
    ///
    /// # Errors
    ///
    /// Returns [`PushRuntimeError::Invalid`] for malformed credentials, seed,
    /// private key, or HTTP client configuration.
    pub fn new(
        seed: &SecretString,
        service_account_json: &SecretString,
    ) -> Result<Self, PushRuntimeError> {
        if seed.expose_secret().len() < 32 {
            return Err(PushRuntimeError::Invalid);
        }
        let document: FirebaseServiceAccountDocument =
            serde_json::from_str(service_account_json.expose_secret())
                .map_err(|_| PushRuntimeError::Invalid)?;
        if !valid_service_account(&document) {
            return Err(PushRuntimeError::Invalid);
        }
        let encoding_key = EncodingKey::from_rsa_pem(document.private_key.as_bytes())
            .map_err(|_| PushRuntimeError::Invalid)?;
        drop(encoding_key);

        let mut digest = Sha256::new();
        digest.update(b"jimin-os/fcm-registration/v1\0");
        digest.update(seed.expose_secret().as_bytes());
        let key: [u8; 32] = digest.finalize().into();
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(StdDuration::from_secs(5))
            .timeout(StdDuration::from_secs(15))
            .build()
            .map_err(|_| PushRuntimeError::Invalid)?;
        Ok(Self {
            key,
            service_account: FirebaseServiceAccount {
                project_id: document.project_id,
                client_email: document.client_email,
                private_key: SecretString::from(document.private_key),
                token_uri: document.token_uri,
            },
            client,
            access_token: Mutex::new(None),
        })
    }

    /// Validates and encrypts one FCM registration token using the device ID as
    /// authenticated associated data.
    ///
    /// # Errors
    ///
    /// Returns an invalid or encryption error for malformed input.
    pub fn encrypt_token(
        &self,
        device_id: Uuid,
        token: &SecretString,
    ) -> Result<EncryptedPushToken, PushRuntimeError> {
        let plaintext = token.expose_secret().trim();
        if device_id.get_version_num() != 7 || !valid_fcm_token(plaintext) {
            return Err(PushRuntimeError::Invalid);
        }
        let mut nonce = [0_u8; NONCE_BYTES];
        rand::rng().fill(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: device_id.as_bytes(),
                },
            )
            .map_err(|_| PushRuntimeError::Authentication)?;
        let fingerprint = Sha256::digest(plaintext.as_bytes()).to_vec();
        Ok(EncryptedPushToken {
            ciphertext,
            nonce: nonce.to_vec(),
            fingerprint,
        })
    }

    /// Sends one data-only FCM reminder through the HTTP v1 API.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider error. Tokens and reminder content are not
    /// attached to the error or logs.
    pub async fn deliver(&self, delivery: &ClaimedPushDelivery) -> Result<i32, PushRuntimeError> {
        let token = self.decrypt_token(delivery)?;
        let access_token = self.google_access_token().await?;
        let endpoint = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.service_account.project_id
        );
        let target_at_epoch_millis =
            (delivery.target_at.unix_timestamp_nanos() / 1_000_000).to_string();
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(access_token.expose_secret())
            .json(&serde_json::json!({
                "message": {
                    "token": token.expose_secret(),
                    "data": {
                        "itemType": delivery.item_type,
                        "itemId": delivery.item_id.to_string(),
                        "destination": delivery.destination,
                        "projectId": delivery.project_id.map_or_else(String::new, |id| id.to_string()),
                        "title": delivery.title,
                        "body": delivery.body,
                        "targetAtEpochMillis": target_at_epoch_millis
                    },
                    "android": { "priority": "HIGH" }
                }
            }))
            .send()
            .await
            .map_err(|_| PushRuntimeError::Unavailable)?;
        let status = i32::from(response.status().as_u16());
        if response.status().is_success() {
            return Ok(status);
        }
        let body = response
            .bytes()
            .await
            .map_err(|_| PushRuntimeError::Unavailable)?;
        if body.len() > MAX_PROVIDER_BODY_BYTES {
            return Err(PushRuntimeError::Rejected(status));
        }
        let body = std::str::from_utf8(&body).unwrap_or_default();
        if body.contains("UNREGISTERED") || body.contains("SENDER_ID_MISMATCH") {
            return Err(PushRuntimeError::Unregistered(status));
        }
        if status == 429 || (500..=599).contains(&status) {
            Err(PushRuntimeError::Unavailable)
        } else if matches!(status, 401 | 403) {
            Err(PushRuntimeError::Authentication)
        } else {
            Err(PushRuntimeError::Rejected(status))
        }
    }

    fn decrypt_token(
        &self,
        delivery: &ClaimedPushDelivery,
    ) -> Result<SecretString, PushRuntimeError> {
        let nonce: [u8; NONCE_BYTES] = delivery
            .token_nonce
            .as_slice()
            .try_into()
            .map_err(|_| PushRuntimeError::Authentication)?;
        if delivery.token_ciphertext.is_empty()
            || delivery.token_ciphertext.len() > MAX_FCM_TOKEN_BYTES + 32
        {
            return Err(PushRuntimeError::Authentication);
        }
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &delivery.token_ciphertext,
                    aad: delivery.device_id.as_bytes(),
                },
            )
            .map_err(|_| PushRuntimeError::Authentication)?;
        let value = String::from_utf8(plaintext).map_err(|_| PushRuntimeError::Authentication)?;
        if !valid_fcm_token(&value) {
            return Err(PushRuntimeError::Authentication);
        }
        Ok(SecretString::from(value))
    }

    async fn google_access_token(&self) -> Result<SecretString, PushRuntimeError> {
        let mut cached = self.access_token.lock().await;
        let now = OffsetDateTime::now_utc();
        if let Some(token) = cached.as_ref()
            && token.expires_at > now + time::Duration::minutes(1)
        {
            return Ok(token.value.clone());
        }
        let assertion = self.service_account_assertion(now)?;
        let response = self
            .client
            .post(&self.service_account.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await
            .map_err(|_| PushRuntimeError::Unavailable)?;
        if !response.status().is_success() {
            return Err(PushRuntimeError::Authentication);
        }
        let response: GoogleAccessTokenResponse = response
            .json()
            .await
            .map_err(|_| PushRuntimeError::Authentication)?;
        if response.token_type != "Bearer"
            || response.access_token.is_empty()
            || !(60..=3_600).contains(&response.expires_in)
        {
            return Err(PushRuntimeError::Authentication);
        }
        let value = SecretString::from(response.access_token);
        *cached = Some(CachedAccessToken {
            value: value.clone(),
            expires_at: now + time::Duration::seconds(response.expires_in),
        });
        Ok(value)
    }

    fn service_account_assertion(&self, now: OffsetDateTime) -> Result<String, PushRuntimeError> {
        let claims = ServiceAccountClaims {
            iss: &self.service_account.client_email,
            scope: GOOGLE_OAUTH_SCOPE,
            aud: GOOGLE_OAUTH_AUDIENCE,
            iat: now.unix_timestamp(),
            exp: (now + time::Duration::hours(1)).unix_timestamp(),
        };
        let key =
            EncodingKey::from_rsa_pem(self.service_account.private_key.expose_secret().as_bytes())
                .map_err(|_| PushRuntimeError::Invalid)?;
        encode(&Header::new(Algorithm::RS256), &claims, &key)
            .map_err(|_| PushRuntimeError::Authentication)
    }
}

#[utoipa::path(
    get,
    path = "/v1/push/registration",
    tag = "notifications",
    responses((status = 200, body = PushRegistrationResponse), (status = 401), (status = 503))
)]
pub(crate) async fn get_push_registration(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .push_registration_state(
            principal.identity().user_id(),
            principal.identity().device_id(),
        )
        .await
    {
        Ok(registration) => registration_response(registration).into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    put,
    path = "/v1/push/registration",
    tag = "notifications",
    request_body = RegisterPushTokenRequest,
    responses((status = 200, body = PushRegistrationResponse), (status = 400), (status = 401), (status = 503))
)]
pub(crate) async fn register_push_token(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(body): Json<RegisterPushTokenRequest>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    if body.provider != "fcm" {
        return error_response(
            StatusCode::BAD_REQUEST,
            "request.invalid",
            "알림 연결 정보를 확인해 주세요.",
            request_id,
            false,
        );
    }
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    let Some(runtime) = state.push() else {
        return unavailable_response(request_id);
    };
    let Ok(encrypted) = runtime.encrypt_token(
        principal.identity().device_id(),
        &SecretString::from(body.token),
    ) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "request.invalid",
            "알림 연결 정보를 확인해 주세요.",
            request_id,
            false,
        );
    };
    match planning
        .register_push_token(
            Uuid::now_v7(),
            principal.identity().user_id(),
            principal.identity().device_id(),
            &encrypted,
        )
        .await
    {
        Ok(registration) => registration_response(registration).into_response(),
        Err(StorageError::InvalidConfiguration) => error_response(
            StatusCode::BAD_REQUEST,
            "push.android_device_required",
            "휴대폰에서 알림 연결을 다시 시도해 주세요.",
            request_id,
            false,
        ),
        Err(error) => storage_error_response(&error, request_id),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/push/registration",
    tag = "notifications",
    responses((status = 204), (status = 401), (status = 503))
)]
pub(crate) async fn delete_push_registration(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(failure) => return failure.into_response(request_id),
    };
    let Some(planning) = state.planning() else {
        return unavailable_response(request_id);
    };
    match planning
        .disable_push_registration(
            principal.identity().user_id(),
            principal.identity().device_id(),
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => storage_error_response(&error, request_id),
    }
}

fn registration_response(registration: PushRegistrationState) -> Json<PushRegistrationResponse> {
    Json(PushRegistrationResponse {
        enabled: registration.enabled,
        provider: "fcm",
        last_seen_at: registration.last_seen_at.map(format_timestamp),
        last_delivered_at: registration.last_delivered_at.map(format_timestamp),
        last_error_code: registration.last_error_code,
    })
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .expect("valid OffsetDateTime must format as RFC 3339")
}

fn valid_service_account(document: &FirebaseServiceAccountDocument) -> bool {
    document.account_type == "service_account"
        && valid_project_id(&document.project_id)
        && document.client_email.ends_with(".gserviceaccount.com")
        && document.client_email.len() <= 320
        && !document.client_email.chars().any(char::is_whitespace)
        && document.private_key.starts_with(PEM_BEGIN)
        && document.private_key.ends_with(PEM_END_LINE)
        && document.private_key.len() <= 16 * 1024
        && document.token_uri == GOOGLE_TOKEN_URI
}

fn valid_project_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (6..=30).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn valid_fcm_token(value: &str) -> bool {
    (20..=MAX_FCM_TOKEN_BYTES).contains(&value.len())
        && value.trim() == value
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> PushRuntime {
        let client = Client::builder().build().unwrap();
        PushRuntime {
            key: [7; 32],
            service_account: FirebaseServiceAccount {
                project_id: "jimin-os-test".to_owned(),
                client_email: "push@jimin-os-test.iam.gserviceaccount.com".to_owned(),
                private_key: SecretString::from("not-used-in-this-test"),
                token_uri: GOOGLE_TOKEN_URI.to_owned(),
            },
            client,
            access_token: Mutex::new(None),
        }
    }

    #[test]
    fn fcm_token_is_encrypted_and_bound_to_the_device() {
        let runtime = runtime();
        let device_id = Uuid::now_v7();
        let token = SecretString::from("fcm-registration-token-that-is-private".to_owned());
        let encrypted = runtime.encrypt_token(device_id, &token).unwrap();
        assert!(
            !encrypted
                .ciphertext
                .windows(token.expose_secret().len())
                .any(|window| window == token.expose_secret().as_bytes())
        );
        let delivery = ClaimedPushDelivery {
            id: Uuid::now_v7(),
            device_id,
            item_type: "task".to_owned(),
            item_id: Uuid::now_v7(),
            destination: "calendar".to_owned(),
            project_id: None,
            title: "곧 마감해요".to_owned(),
            body: "확인해 주세요.".to_owned(),
            target_at: OffsetDateTime::now_utc(),
            attempt_count: 1,
            token_ciphertext: encrypted.ciphertext,
            token_nonce: encrypted.nonce,
        };
        assert_eq!(
            runtime.decrypt_token(&delivery).unwrap().expose_secret(),
            token.expose_secret()
        );
    }

    #[test]
    fn service_account_requires_the_google_token_endpoint() {
        let document = FirebaseServiceAccountDocument {
            account_type: "service_account".to_owned(),
            project_id: "jimin-os-test".to_owned(),
            private_key: format!("{PEM_BEGIN}\nvalue\n{PEM_END}\n"),
            client_email: "push@jimin-os-test.iam.gserviceaccount.com".to_owned(),
            token_uri: "https://example.com/token".to_owned(),
        };
        assert!(!valid_service_account(&document));
    }

    #[test]
    fn provider_failures_do_not_expose_tokens() {
        assert_eq!(
            PushRuntimeError::Unregistered(404).code(),
            "push.token_unregistered"
        );
        assert!(PushRuntimeError::Unregistered(404).invalidates_token());
        assert!(PushRuntimeError::Unavailable.retryable());
    }
}

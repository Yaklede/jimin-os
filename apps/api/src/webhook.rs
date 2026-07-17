//! Encrypted project webhook secrets and bounded outbound delivery.

use std::time::Duration;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use jimin_storage::webhook::{ClaimedWebhookDelivery, EncryptedWebhookSecret, WebhookProvider};
use rand::RngExt;
use reqwest::{Client, header::AUTHORIZATION, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const NONCE_BYTES: usize = 24;
const MAX_AUTH_HEADER_BYTES: usize = 8 * 1024;
const MAX_DESTINATION_BYTES: usize = 4 * 1024;
const MAX_CHAT_MESSAGE_CHARS: usize = 1_800;

pub struct WebhookRuntime {
    key: [u8; 32],
    client: Client,
}

pub struct WebhookDeliveryResult {
    pub response_code: i32,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WebhookRuntimeError {
    #[error("webhook configuration is invalid")]
    Invalid,
    #[error("webhook authentication could not be decrypted")]
    Authentication,
    #[error("webhook destination is unavailable")]
    Unavailable,
    #[error("webhook destination rejected the event")]
    Rejected(i32),
}

impl WebhookRuntimeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "webhook.configuration_invalid",
            Self::Authentication => "webhook.authentication_invalid",
            Self::Unavailable => "webhook.destination_unavailable",
            Self::Rejected(_) => "webhook.destination_rejected",
        }
    }
}

impl WebhookRuntime {
    /// Builds an outbound client and derives a webhook-only encryption key.
    ///
    /// # Errors
    ///
    /// Returns [`WebhookRuntimeError::Invalid`] for an unusable seed or client.
    pub fn new(seed: &SecretString) -> Result<Self, WebhookRuntimeError> {
        if seed.expose_secret().len() < 32 {
            return Err(WebhookRuntimeError::Invalid);
        }
        let mut digest = Sha256::new();
        digest.update(b"jimin-os/project-webhook/v1\0");
        digest.update(seed.expose_secret().as_bytes());
        let key: [u8; 32] = digest.finalize().into();
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| WebhookRuntimeError::Invalid)?;
        Ok(Self { key, client })
    }

    /// Encrypts a bounded authorization header with webhook-specific AAD.
    ///
    /// # Errors
    ///
    /// Returns an invalid or authentication error for unsafe input.
    pub fn encrypt_authentication(
        &self,
        webhook_id: Uuid,
        value: &SecretString,
    ) -> Result<EncryptedWebhookSecret, WebhookRuntimeError> {
        let plaintext = value.expose_secret();
        if webhook_id.get_version_num() != 7
            || plaintext.is_empty()
            || plaintext.len() > MAX_AUTH_HEADER_BYTES
            || plaintext.contains(['\r', '\n'])
        {
            return Err(WebhookRuntimeError::Invalid);
        }
        let mut nonce = [0_u8; NONCE_BYTES];
        rand::rng().fill(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let nonce_value = XNonce::from(nonce);
        let ciphertext = cipher
            .encrypt(
                &nonce_value,
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: webhook_id.as_bytes(),
                },
            )
            .map_err(|_| WebhookRuntimeError::Authentication)?;
        Ok(EncryptedWebhookSecret {
            ciphertext,
            nonce: nonce.to_vec(),
        })
    }

    /// Validates and encrypts a Google Chat or Discord incoming-webhook URL.
    /// The plaintext is never returned from storage-facing API responses.
    ///
    /// # Errors
    ///
    /// Returns an invalid or authentication error for an unsafe destination.
    pub fn encrypt_destination(
        &self,
        webhook_id: Uuid,
        provider: WebhookProvider,
        value: &SecretString,
    ) -> Result<EncryptedWebhookSecret, WebhookRuntimeError> {
        let plaintext = value.expose_secret().trim();
        if webhook_id.get_version_num() != 7
            || plaintext.is_empty()
            || plaintext.len() > MAX_DESTINATION_BYTES
            || !valid_provider_url(provider, plaintext)
        {
            return Err(WebhookRuntimeError::Invalid);
        }
        self.encrypt_secret(webhook_id, b"destination", plaintext.as_bytes())
    }

    /// Delivers one claimed event without following redirects.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration, transport, or rejection error.
    pub async fn deliver(
        &self,
        delivery: &ClaimedWebhookDelivery,
    ) -> Result<WebhookDeliveryResult, WebhookRuntimeError> {
        let provider =
            WebhookProvider::parse(&delivery.provider).ok_or(WebhookRuntimeError::Invalid)?;
        let destination = self.decrypt_destination(delivery, provider)?;
        let url = reqwest::Url::parse(destination.expose_secret())
            .map_err(|_| WebhookRuntimeError::Invalid)?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(WebhookRuntimeError::Invalid);
        }
        let outbound_payload = provider_payload(provider, &delivery.event_type, &delivery.payload)?;
        let mut request = self
            .client
            .post(url)
            .header("X-Jimin-Delivery", delivery.id.to_string())
            .header("X-Jimin-Event", &delivery.event_type)
            .header("Idempotency-Key", delivery.id.to_string())
            .json(&outbound_payload);
        if let Some(authentication) = self.decrypt_authentication(delivery)? {
            request = request.header(AUTHORIZATION, authentication.expose_secret());
        }
        let response = request
            .send()
            .await
            .map_err(|_| WebhookRuntimeError::Unavailable)?;
        let status = i32::from(response.status().as_u16());
        if response.status().is_success() {
            Ok(WebhookDeliveryResult {
                response_code: status,
            })
        } else {
            Err(WebhookRuntimeError::Rejected(status))
        }
    }

    fn decrypt_authentication(
        &self,
        delivery: &ClaimedWebhookDelivery,
    ) -> Result<Option<SecretString>, WebhookRuntimeError> {
        let (Some(ciphertext), Some(nonce)) = (
            delivery.auth_header_ciphertext.as_ref(),
            delivery.auth_header_nonce.as_ref(),
        ) else {
            if delivery.auth_header_ciphertext.is_none() && delivery.auth_header_nonce.is_none() {
                return Ok(None);
            }
            return Err(WebhookRuntimeError::Authentication);
        };
        if nonce.len() != NONCE_BYTES
            || ciphertext.is_empty()
            || ciphertext.len() > MAX_AUTH_HEADER_BYTES + 32
        {
            return Err(WebhookRuntimeError::Authentication);
        }
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let nonce_bytes: [u8; NONCE_BYTES] = nonce
            .as_slice()
            .try_into()
            .map_err(|_| WebhookRuntimeError::Authentication)?;
        let nonce_value = XNonce::from(nonce_bytes);
        let plaintext = cipher
            .decrypt(
                &nonce_value,
                Payload {
                    msg: ciphertext,
                    aad: delivery.webhook_id.as_bytes(),
                },
            )
            .map_err(|_| WebhookRuntimeError::Authentication)?;
        let value =
            String::from_utf8(plaintext).map_err(|_| WebhookRuntimeError::Authentication)?;
        if value.is_empty() || value.len() > MAX_AUTH_HEADER_BYTES || value.contains(['\r', '\n']) {
            return Err(WebhookRuntimeError::Authentication);
        }
        Ok(Some(SecretString::from(value)))
    }

    fn encrypt_secret(
        &self,
        webhook_id: Uuid,
        purpose: &[u8],
        plaintext: &[u8],
    ) -> Result<EncryptedWebhookSecret, WebhookRuntimeError> {
        let mut nonce = [0_u8; NONCE_BYTES];
        rand::rng().fill(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let nonce_value = XNonce::from(nonce);
        let aad = secret_aad(webhook_id, purpose);
        let ciphertext = cipher
            .encrypt(
                &nonce_value,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| WebhookRuntimeError::Authentication)?;
        Ok(EncryptedWebhookSecret {
            ciphertext,
            nonce: nonce.to_vec(),
        })
    }

    fn decrypt_destination(
        &self,
        delivery: &ClaimedWebhookDelivery,
        provider: WebhookProvider,
    ) -> Result<SecretString, WebhookRuntimeError> {
        if provider == WebhookProvider::Legacy {
            return Ok(SecretString::from(delivery.legacy_url.clone()));
        }
        let (Some(ciphertext), Some(nonce)) = (
            delivery.destination_ciphertext.as_ref(),
            delivery.destination_nonce.as_ref(),
        ) else {
            return Err(WebhookRuntimeError::Authentication);
        };
        if nonce.len() != NONCE_BYTES
            || ciphertext.is_empty()
            || ciphertext.len() > MAX_DESTINATION_BYTES + 32
        {
            return Err(WebhookRuntimeError::Authentication);
        }
        let nonce_bytes: [u8; NONCE_BYTES] = nonce
            .as_slice()
            .try_into()
            .map_err(|_| WebhookRuntimeError::Authentication)?;
        let cipher = XChaCha20Poly1305::new((&self.key).into());
        let aad = secret_aad(delivery.webhook_id, b"destination");
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce_bytes),
                Payload {
                    msg: ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| WebhookRuntimeError::Authentication)?;
        let value =
            String::from_utf8(plaintext).map_err(|_| WebhookRuntimeError::Authentication)?;
        if !valid_provider_url(provider, &value) {
            return Err(WebhookRuntimeError::Authentication);
        }
        Ok(SecretString::from(value))
    }
}

fn secret_aad(webhook_id: Uuid, purpose: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(16 + purpose.len() + 1);
    aad.extend_from_slice(webhook_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(purpose);
    aad
}

fn valid_provider_url(provider: WebhookProvider, value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value.trim()) else {
        return false;
    };
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    match provider {
        WebhookProvider::GoogleChat => {
            url.host_str() == Some("chat.googleapis.com")
                && url.path().starts_with("/v1/spaces/")
                && url.path().ends_with("/messages")
                && url
                    .query_pairs()
                    .any(|(key, value)| key == "key" && !value.is_empty())
                && url
                    .query_pairs()
                    .any(|(key, value)| key == "token" && !value.is_empty())
        }
        WebhookProvider::Discord => {
            matches!(url.host_str(), Some("discord.com" | "discordapp.com"))
                && url.path().starts_with("/api/webhooks/")
        }
        WebhookProvider::Legacy => matches!(url.scheme(), "http" | "https"),
    }
}

fn provider_payload(
    provider: WebhookProvider,
    event_type: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, WebhookRuntimeError> {
    if provider == WebhookProvider::Legacy {
        return Ok(payload.clone());
    }
    let message = payload
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| default_event_message(event_type), str::to_owned);
    if message.is_empty() || message.chars().count() > MAX_CHAT_MESSAGE_CHARS {
        return Err(WebhookRuntimeError::Invalid);
    }
    Ok(match provider {
        WebhookProvider::GoogleChat => serde_json::json!({ "text": message }),
        WebhookProvider::Discord => serde_json::json!({
            "content": message,
            "allowed_mentions": { "parse": [] }
        }),
        WebhookProvider::Legacy => unreachable!(),
    })
}

fn default_event_message(event_type: &str) -> String {
    let event = match event_type {
        "project.updated" => "프로젝트가 변경됐어요.",
        "project.deleted" => "프로젝트가 삭제됐어요.",
        "task.created" => "새 일이 추가됐어요.",
        "task.updated" => "일이 변경됐어요.",
        "task.completed" => "일을 완료했어요.",
        "task.restored" => "완료한 일을 다시 열었어요.",
        "task.deleted" => "일이 삭제됐어요.",
        "webhook.test" => "Jimin OS에서 시험 메시지를 보냈어요.",
        "chat.message" => "Jimin OS에서 메시지를 보냈어요.",
        _ => "프로젝트에 변화가 생겼어요.",
    };
    event.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_ciphertext_is_bound_to_the_webhook_id() {
        let runtime = WebhookRuntime::new(&SecretString::from("x".repeat(64))).unwrap();
        let first = Uuid::now_v7();
        let encrypted = runtime
            .encrypt_authentication(first, &SecretString::from("Bearer private"))
            .unwrap();
        let delivery = ClaimedWebhookDelivery {
            id: Uuid::now_v7(),
            webhook_id: first,
            project_id: Uuid::now_v7(),
            event_type: "webhook.test".to_owned(),
            payload: serde_json::json!({}),
            attempt_count: 1,
            provider: "legacy".to_owned(),
            legacy_url: "https://example.com/hook".to_owned(),
            destination_ciphertext: None,
            destination_nonce: None,
            auth_header_ciphertext: Some(encrypted.ciphertext),
            auth_header_nonce: Some(encrypted.nonce),
        };
        assert_eq!(
            runtime
                .decrypt_authentication(&delivery)
                .unwrap()
                .unwrap()
                .expose_secret(),
            "Bearer private"
        );
    }

    #[test]
    fn typed_destination_is_encrypted_and_bound_to_the_webhook() {
        let runtime = WebhookRuntime::new(&SecretString::from("x".repeat(64))).unwrap();
        let webhook_id = Uuid::now_v7();
        let destination =
            SecretString::from("https://discord.com/api/webhooks/123/private-token".to_owned());
        let encrypted = runtime
            .encrypt_destination(webhook_id, WebhookProvider::Discord, &destination)
            .unwrap();
        assert!(
            !encrypted
                .ciphertext
                .windows("private-token".len())
                .any(|window| window == b"private-token")
        );

        let delivery = ClaimedWebhookDelivery {
            id: Uuid::now_v7(),
            webhook_id,
            project_id: Uuid::now_v7(),
            event_type: "chat.message".to_owned(),
            payload: serde_json::json!({ "message": "배포가 완료됐어요." }),
            attempt_count: 1,
            provider: "discord".to_owned(),
            legacy_url: "encrypted://discord".to_owned(),
            destination_ciphertext: Some(encrypted.ciphertext),
            destination_nonce: Some(encrypted.nonce),
            auth_header_ciphertext: None,
            auth_header_nonce: None,
        };
        assert_eq!(
            runtime
                .decrypt_destination(&delivery, WebhookProvider::Discord)
                .unwrap()
                .expose_secret(),
            destination.expose_secret()
        );
        assert_eq!(
            provider_payload(WebhookProvider::Discord, "chat.message", &delivery.payload,).unwrap(),
            serde_json::json!({
                "content": "배포가 완료됐어요.",
                "allowed_mentions": { "parse": [] }
            })
        );
    }

    #[test]
    fn typed_destination_rejects_a_url_for_the_wrong_provider() {
        let runtime = WebhookRuntime::new(&SecretString::from("x".repeat(64))).unwrap();
        assert!(
            runtime
                .encrypt_destination(
                    Uuid::now_v7(),
                    WebhookProvider::GoogleChat,
                    &SecretString::from("https://discord.com/api/webhooks/123/token"),
                )
                .is_err()
        );
    }

    #[test]
    fn google_chat_destination_and_text_payload_follow_the_provider_contract() {
        let runtime = WebhookRuntime::new(&SecretString::from("x".repeat(64))).unwrap();
        assert!(
            runtime
                .encrypt_destination(
                    Uuid::now_v7(),
                    WebhookProvider::GoogleChat,
                    &SecretString::from(
                        "https://chat.googleapis.com/v1/spaces/AAAA/messages?key=shared&token=private",
                    ),
                )
                .is_ok()
        );
        assert_eq!(
            provider_payload(
                WebhookProvider::GoogleChat,
                "chat.message",
                &serde_json::json!({ "message": "내일 회의가 확정됐어요." }),
            )
            .unwrap(),
            serde_json::json!({ "text": "내일 회의가 확정됐어요." })
        );
    }
}

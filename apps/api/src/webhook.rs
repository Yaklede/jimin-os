//! Encrypted project webhook secrets and bounded outbound delivery.

use std::time::Duration;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use jimin_storage::webhook::{ClaimedWebhookDelivery, EncryptedWebhookSecret};
use rand::RngExt;
use reqwest::{Client, header::AUTHORIZATION, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const NONCE_BYTES: usize = 24;
const MAX_AUTH_HEADER_BYTES: usize = 8 * 1024;

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

    /// Delivers one claimed event without following redirects.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration, transport, or rejection error.
    pub async fn deliver(
        &self,
        delivery: &ClaimedWebhookDelivery,
    ) -> Result<WebhookDeliveryResult, WebhookRuntimeError> {
        let url = reqwest::Url::parse(&delivery.url).map_err(|_| WebhookRuntimeError::Invalid)?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(WebhookRuntimeError::Invalid);
        }
        let mut request = self
            .client
            .post(url)
            .header("X-Jimin-Delivery", delivery.id.to_string())
            .header("X-Jimin-Event", &delivery.event_type)
            .header("Idempotency-Key", delivery.id.to_string())
            .json(&delivery.payload);
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
            url: "https://example.com/hook".to_owned(),
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
}

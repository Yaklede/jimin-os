//! Encrypted project webhook secrets and bounded outbound delivery.

use std::time::Duration;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use jimin_storage::webhook::{
    ClaimedWebhookDelivery, EncryptedWebhookSecret, GoogleChatMentionDirectory, WebhookProvider,
};
use rand::RngExt;
use reqwest::{Client, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const NONCE_BYTES: usize = 24;
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
        let outbound_payload = provider_payload(
            provider,
            &delivery.event_type,
            &delivery.payload,
            &delivery.mention_directory,
        )?;
        let request = self
            .client
            .post(url)
            .header("X-Jimin-Delivery", delivery.id.to_string())
            .header("X-Jimin-Event", &delivery.event_type)
            .header("Idempotency-Key", delivery.id.to_string())
            .json(&outbound_payload);
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
        let ciphertext = &delivery.destination_ciphertext;
        let nonce = &delivery.destination_nonce;
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
    }
}

fn provider_payload(
    provider: WebhookProvider,
    event_type: &str,
    payload: &serde_json::Value,
    mention_directory: &GoogleChatMentionDirectory,
) -> Result<serde_json::Value, WebhookRuntimeError> {
    if !mention_directory.is_valid_for(provider) {
        return Err(WebhookRuntimeError::Invalid);
    }
    let message = payload
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| default_event_message(event_type), str::to_owned);
    let message = match provider {
        WebhookProvider::GoogleChat => expand_google_chat_mentions(&message, mention_directory),
        WebhookProvider::Discord => message,
    };
    if message.is_empty() || message.chars().count() > MAX_CHAT_MESSAGE_CHARS {
        return Err(WebhookRuntimeError::Invalid);
    }
    Ok(match provider {
        WebhookProvider::GoogleChat => serde_json::json!({ "text": message }),
        WebhookProvider::Discord => serde_json::json!({
            "content": message,
            "allowed_mentions": { "parse": [] }
        }),
    })
}

fn expand_google_chat_mentions(message: &str, directory: &GoogleChatMentionDirectory) -> String {
    let mut expanded = message.to_owned();
    let mut users = directory.users.iter().collect::<Vec<_>>();
    users.sort_by(|(left, _), (right, _)| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    for (name, user_id) in users {
        let replacement = format!("<{user_id}>");
        expanded = replace_mention_token(&expanded, &format!("@{{{name}}}"), &replacement, false);
        expanded = replace_mention_token(&expanded, &format!("@{name}"), &replacement, true);
    }
    expanded
}

fn replace_mention_token(
    message: &str,
    token: &str,
    replacement: &str,
    require_boundary: bool,
) -> String {
    let mut output = String::with_capacity(message.len());
    let mut cursor = 0;
    while let Some(relative_index) = message[cursor..].find(token) {
        let index = cursor + relative_index;
        let end = index + token.len();
        let previous = message[..index].chars().next_back();
        let next = message[end..].chars().next();
        let boundary_matches = !require_boundary
            || (previous.is_none_or(|character| !is_mention_identifier(character))
                && next.is_none_or(|character| !is_mention_identifier(character)));
        output.push_str(&message[cursor..index]);
        if boundary_matches {
            output.push_str(replacement);
        } else {
            output.push_str(token);
        }
        cursor = end;
    }
    output.push_str(&message[cursor..]);
    output
}

fn is_mention_identifier(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '.' | '+' | '-')
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
            destination_ciphertext: encrypted.ciphertext,
            destination_nonce: encrypted.nonce,
            mention_directory: GoogleChatMentionDirectory::default(),
        };
        assert_eq!(
            runtime
                .decrypt_destination(&delivery, WebhookProvider::Discord)
                .unwrap()
                .expose_secret(),
            destination.expose_secret()
        );
        assert_eq!(
            provider_payload(
                WebhookProvider::Discord,
                "chat.message",
                &delivery.payload,
                &delivery.mention_directory,
            )
            .unwrap(),
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
                &GoogleChatMentionDirectory::default(),
            )
            .unwrap(),
            serde_json::json!({ "text": "내일 회의가 확정됐어요." })
        );
    }

    #[test]
    fn google_chat_mentions_only_explicit_registered_names() {
        let directory = GoogleChatMentionDirectory {
            users: [
                (
                    "홍길동".to_owned(),
                    "users/123456789012345678901".to_owned(),
                ),
                (
                    "김개발".to_owned(),
                    "users/987654321098765432109".to_owned(),
                ),
            ]
            .into_iter()
            .collect(),
        };
        let payload = provider_payload(
            WebhookProvider::GoogleChat,
            "chat.message",
            &serde_json::json!({
                "message": "@홍길동 확인해 주세요. 김개발 님에게는 공유만 하고, @{김개발}도 확인해 주세요."
            }),
            &directory,
        )
        .unwrap();
        assert_eq!(
            payload,
            serde_json::json!({
                "text": "<users/123456789012345678901> 확인해 주세요. 김개발 님에게는 공유만 하고, <users/987654321098765432109>도 확인해 주세요."
            })
        );
    }

    #[test]
    fn mention_expansion_ignores_email_like_and_partial_tokens() {
        let directory = GoogleChatMentionDirectory {
            users: [(
                "홍길동".to_owned(),
                "users/123456789012345678901".to_owned(),
            )]
            .into_iter()
            .collect(),
        };
        let payload = provider_payload(
            WebhookProvider::GoogleChat,
            "chat.message",
            &serde_json::json!({ "message": "mail@홍길동.example과 @홍길동추가, @없는사람은 그대로예요." }),
            &directory,
        )
        .unwrap();
        assert_eq!(
            payload,
            serde_json::json!({ "text": "mail@홍길동.example과 @홍길동추가, @없는사람은 그대로예요." })
        );
    }
}

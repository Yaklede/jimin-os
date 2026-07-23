//! OAuth and provider runtime for project-owned Google Chat inflow.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit as AeadKeyInit, Payload},
};
use hmac::{Hmac, Mac, digest::KeyInit as HmacKeyInit};
use jimin_domain::{ClientPlatform, PkceVerifier};
use jimin_google::{
    GoogleAuthError, GoogleAuthorizationCode, GoogleChatAdapter, GoogleChatMessageEntry,
    GoogleChatSpaceEntry, GoogleIdentityAdapter, GoogleOAuthProfile,
};
use jimin_storage::{
    calendar::EncryptedCalendarSecret,
    google_chat::{
        ClaimedGoogleChatOAuthAuthorization, CompleteGoogleChatOAuthAuthorization,
        GoogleChatAccountConnection, GoogleChatCompletionDelivery, GoogleChatSourceSyncConnection,
        ProviderGoogleChatMessage,
    },
};
use rand::Rng;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::config::CalendarOAuthSettings;

const AUTHORIZATION_LIFETIME: Duration = Duration::minutes(10);
const INITIAL_SYNC_LOOKBACK: Duration = Duration::days(7);
const RANDOM_STATE_BYTES: usize = 32;
const RANDOM_PKCE_BYTES: usize = 64;
const XCHACHA_NONCE_BYTES: usize = 24;
const CHAT_SPACES_SCOPE: &str = "https://www.googleapis.com/auth/chat.spaces.readonly";
const CHAT_MESSAGES_SCOPE: &str = "https://www.googleapis.com/auth/chat.messages.readonly";
const CHAT_REACTIONS_SCOPE: &str = "https://www.googleapis.com/auth/chat.messages.reactions.create";
const CHAT_MESSAGES_CREATE_SCOPE: &str = "https://www.googleapis.com/auth/chat.messages.create";

type HmacSha256 = Hmac<Sha256>;

pub struct GoogleChatOAuthRuntime {
    google: GoogleIdentityAdapter,
    chat: GoogleChatAdapter,
    crypto: GoogleChatCrypto,
    redirect_uri: String,
    encryption_key_version: i32,
}

pub struct NewGoogleChatOAuthAuthorization {
    pub state_verifier: Vec<u8>,
    pub pkce_verifier: EncryptedCalendarSecret,
    pub authorization_url: String,
    pub expires_at: OffsetDateTime,
}

pub struct GoogleChatCompletionOutcome {
    pub reaction_completed: bool,
    pub reply_completed: bool,
    pub failure_code: Option<&'static str>,
}

impl GoogleChatOAuthRuntime {
    /// Builds the server-only company Chat OAuth and encryption runtime.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when provider or encryption settings are invalid.
    pub fn new(settings: &CalendarOAuthSettings) -> Result<Self, GoogleChatOAuthError> {
        let profiles = [
            ClientPlatform::Macos,
            ClientPlatform::Ios,
            ClientPlatform::Android,
        ]
        .into_iter()
        .map(|platform| {
            GoogleOAuthProfile::new_with_client_secret(
                platform,
                settings.client_id(),
                settings.client_secret().clone(),
                [settings.redirect_uri().to_owned()],
                true,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GoogleChatOAuthError::Configuration)?;
        Ok(Self {
            google: GoogleIdentityAdapter::new(profiles)
                .map_err(|_| GoogleChatOAuthError::Configuration)?,
            chat: GoogleChatAdapter::new(settings.client_id(), settings.client_secret().clone())
                .map_err(|_| GoogleChatOAuthError::Configuration)?,
            crypto: GoogleChatCrypto::new(settings.encryption_key())?,
            redirect_uri: settings.redirect_uri().to_owned(),
            encryption_key_version: settings.encryption_key_version(),
        })
    }

    /// Creates a short-lived, PKCE-protected company account consent request.
    ///
    /// # Errors
    ///
    /// Returns a configuration or encryption error when the request cannot be created.
    pub fn begin_authorization(
        &self,
        authorization_id: Uuid,
        client_kind: ClientPlatform,
    ) -> Result<NewGoogleChatOAuthAuthorization, GoogleChatOAuthError> {
        if authorization_id.get_version_num() != 7 {
            return Err(GoogleChatOAuthError::Configuration);
        }
        let state = random_url_safe(RANDOM_STATE_BYTES);
        let pkce_verifier = random_url_safe(RANDOM_PKCE_BYTES);
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce_verifier.as_bytes()));
        let authorization_url = self
            .google
            .chat_authorization_url(client_kind, &state, &code_challenge, true)
            .map_err(GoogleChatOAuthError::from_google)?;
        let encrypted_pkce = self.crypto.encrypt(
            pkce_verifier.as_bytes(),
            &pkce_aad(authorization_id),
            self.encryption_key_version,
        )?;
        Ok(NewGoogleChatOAuthAuthorization {
            state_verifier: self.crypto.state_verifier(&state),
            pkce_verifier: encrypted_pkce,
            authorization_url,
            expires_at: OffsetDateTime::now_utc() + AUTHORIZATION_LIFETIME,
        })
    }

    #[must_use]
    pub fn state_verifier(&self, state: &str) -> Vec<u8> {
        self.crypto.state_verifier(state)
    }

    /// Exchanges a claimed authorization and encrypts any returned refresh token.
    ///
    /// # Errors
    ///
    /// Returns a sanitized callback, permission, provider, or encryption error.
    pub async fn complete_authorization(
        &self,
        authorization: ClaimedGoogleChatOAuthAuthorization,
        code: SecretString,
    ) -> Result<CompleteGoogleChatOAuthAuthorization, GoogleChatOAuthError> {
        let pkce = self
            .crypto
            .decrypt(&authorization.pkce_verifier, &pkce_aad(authorization.id))?;
        let verifier = PkceVerifier::parse(pkce.expose_secret().to_owned())
            .map_err(|_| GoogleChatOAuthError::InvalidCallback)?;
        let grant = self
            .google
            .exchange_chat(GoogleAuthorizationCode {
                platform: authorization.client_kind,
                authorization_code: code,
                code_verifier: Some(verifier),
                redirect_uri: self.redirect_uri.clone(),
            })
            .await
            .map_err(GoogleChatOAuthError::from_google)?;
        let scopes = required_chat_scopes(grant.granted_scopes())?;
        let provider_subject = grant.identity().subject().clone();
        let refresh_token = grant
            .refresh_token()
            .map(|token| {
                self.crypto.encrypt(
                    token.expose_secret().as_bytes(),
                    &refresh_token_aad(authorization.user_id, provider_subject.as_str()),
                    self.encryption_key_version,
                )
            })
            .transpose()?;
        Ok(CompleteGoogleChatOAuthAuthorization {
            authorization_id: authorization.id,
            account_id: Uuid::now_v7(),
            user_id: authorization.user_id,
            provider_subject,
            email: grant.identity().email().clone(),
            granted_scopes: scopes,
            refresh_token,
        })
    }

    /// Lists Chat spaces visible to one linked company identity.
    ///
    /// # Errors
    ///
    /// Returns a sanitized credential or provider error.
    pub async fn list_spaces(
        &self,
        connection: &GoogleChatAccountConnection,
    ) -> Result<Vec<GoogleChatSpaceEntry>, GoogleChatOAuthError> {
        let refresh_token = self.decrypt_account_refresh_token(connection)?;
        let access_token = self
            .chat
            .refresh_access_token(&refresh_token)
            .await
            .map_err(GoogleChatOAuthError::from_google)?;
        self.chat
            .list_spaces(&access_token)
            .await
            .map_err(GoogleChatOAuthError::from_google)
    }

    /// Best-effort revokes one decrypted company refresh credential at Google.
    ///
    /// # Errors
    ///
    /// Returns a sanitized credential or provider error.
    pub async fn revoke_account(
        &self,
        connection: &GoogleChatAccountConnection,
    ) -> Result<(), GoogleChatOAuthError> {
        let refresh_token = self.decrypt_account_refresh_token(connection)?;
        self.chat
            .revoke_refresh_token(&refresh_token)
            .await
            .map_err(GoogleChatOAuthError::from_google)
    }

    /// Loads a bounded window of new messages for one persisted project source.
    ///
    /// # Errors
    ///
    /// Returns a sanitized credential or provider error.
    pub async fn list_source_messages(
        &self,
        connection: &GoogleChatSourceSyncConnection,
        reconcile_recent_senders: bool,
    ) -> Result<Vec<ProviderGoogleChatMessage>, GoogleChatOAuthError> {
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id, &connection.provider_subject),
        )?;
        let access_token = self
            .chat
            .refresh_access_token(&refresh_token)
            .await
            .map_err(GoogleChatOAuthError::from_google)?;
        let created_after = Some(source_messages_created_after(
            connection,
            reconcile_recent_senders,
            OffsetDateTime::now_utc(),
        ));
        let messages = self
            .chat
            .list_messages(&access_token, &connection.space_name, created_after)
            .await
            .map_err(GoogleChatOAuthError::from_google)?;
        Ok(messages.into_iter().map(provider_message).collect())
    }

    /// Adds the configured reaction to provider messages after durable ingestion.
    ///
    /// # Errors
    ///
    /// Returns a sanitized credential or provider error before per-message attempts begin.
    pub async fn acknowledge_messages(
        &self,
        connection: &GoogleChatSourceSyncConnection,
        message_names: &[String],
    ) -> Result<Vec<bool>, GoogleChatOAuthError> {
        if message_names.is_empty() {
            return Ok(Vec::new());
        }
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id, &connection.provider_subject),
        )?;
        let access_token = self
            .chat
            .refresh_access_token(&refresh_token)
            .await
            .map_err(GoogleChatOAuthError::from_google)?;
        let mut outcomes = Vec::with_capacity(message_names.len());
        for message_name in message_names {
            outcomes.push(
                self.chat
                    .acknowledge_message(&access_token, message_name)
                    .await
                    .is_ok(),
            );
        }
        Ok(outcomes)
    }

    /// Adds a completion reaction and posts one idempotent reply to the source
    /// thread. A partial success is returned so the durable retry only repeats
    /// the missing provider action.
    pub async fn deliver_completion(
        &self,
        connection: &GoogleChatSourceSyncConnection,
        delivery: &GoogleChatCompletionDelivery,
        reply_text: &str,
    ) -> GoogleChatCompletionOutcome {
        let refresh_token = match self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id, &connection.provider_subject),
        ) {
            Ok(token) => token,
            Err(error) => {
                return GoogleChatCompletionOutcome {
                    reaction_completed: false,
                    reply_completed: false,
                    failure_code: Some(error.failure_code()),
                };
            }
        };
        let access_token = match self.chat.refresh_access_token(&refresh_token).await {
            Ok(token) => token,
            Err(error) => {
                let error = GoogleChatOAuthError::from_google(error);
                return GoogleChatCompletionOutcome {
                    reaction_completed: false,
                    reply_completed: false,
                    failure_code: Some(error.failure_code()),
                };
            }
        };
        let reaction_completed = delivery.reaction_completed
            || self
                .chat
                .complete_message(&access_token, &delivery.provider_message_name)
                .await
                .is_ok();
        let (reply_completed, reply_failure) = if delivery.reply_completed {
            (true, None)
        } else if !Self::completion_scope_granted(&connection.granted_scopes) {
            (false, Some("google_chat.write_scope_missing"))
        } else if let Some(thread_name) = delivery.provider_thread_name.as_deref() {
            match self
                .chat
                .reply_to_thread(
                    &access_token,
                    &connection.space_name,
                    thread_name,
                    reply_text,
                    &delivery.task_id.to_string(),
                )
                .await
            {
                Ok(()) => (true, None),
                Err(_) => (false, Some("google_chat.completion_reply_failed")),
            }
        } else {
            (false, Some("google_chat.thread_unavailable"))
        };
        let failure_code = if reaction_completed && reply_completed {
            None
        } else {
            reply_failure.or(Some("google_chat.completion_reaction_failed"))
        };
        GoogleChatCompletionOutcome {
            reaction_completed,
            reply_completed,
            failure_code,
        }
    }

    #[must_use]
    pub fn completion_scope_granted(scopes: &[String]) -> bool {
        scopes
            .iter()
            .any(|scope| scope == CHAT_MESSAGES_CREATE_SCOPE)
    }

    fn decrypt_account_refresh_token(
        &self,
        connection: &GoogleChatAccountConnection,
    ) -> Result<SecretString, GoogleChatOAuthError> {
        self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id, &connection.provider_subject),
        )
    }
}

fn source_messages_created_after(
    connection: &GoogleChatSourceSyncConnection,
    reconcile_recent_senders: bool,
    now: OffsetDateTime,
) -> OffsetDateTime {
    connection.last_provider_message_at.map_or_else(
        || now - INITIAL_SYNC_LOOKBACK,
        |last| {
            if reconcile_recent_senders && connection.last_successful_sync_at.is_some() {
                last - INITIAL_SYNC_LOOKBACK
            } else {
                last
            }
        },
    )
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GoogleChatOAuthError {
    #[error("Google Chat configuration is invalid")]
    Configuration,
    #[error("Google Chat callback is invalid")]
    InvalidCallback,
    #[error("Google Chat authorization was rejected")]
    ProviderRejected,
    #[error("Google Chat is temporarily unavailable")]
    ProviderUnavailable,
    #[error("Google Chat did not grant required permissions")]
    RequiredScopeMissing,
    #[error("Google Chat credential encryption failed")]
    Encryption,
}

impl GoogleChatOAuthError {
    #[must_use]
    pub const fn failure_code(self) -> &'static str {
        match self {
            Self::Configuration => "google_chat.configuration_invalid",
            Self::InvalidCallback => "google_chat.invalid_callback",
            Self::ProviderRejected => "google_chat.authorization_rejected",
            Self::ProviderUnavailable => "google_chat.provider_unavailable",
            Self::RequiredScopeMissing => "google_chat.required_scope_missing",
            Self::Encryption => "google_chat.credential_encryption_failed",
        }
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::ProviderUnavailable)
    }

    #[must_use]
    pub const fn reauth_required(self) -> bool {
        matches!(self, Self::ProviderRejected | Self::RequiredScopeMissing)
    }

    fn from_google(error: GoogleAuthError) -> Self {
        match error {
            GoogleAuthError::ProviderUnavailable => Self::ProviderUnavailable,
            GoogleAuthError::IdentityRejected => Self::InvalidCallback,
            GoogleAuthError::InvalidRequest
            | GoogleAuthError::ProviderRejected
            | GoogleAuthError::CalendarSyncTokenExpired
            | GoogleAuthError::CalendarEventConflict
            | GoogleAuthError::CalendarEventNotFound
            | GoogleAuthError::CalendarEventRejected => Self::ProviderRejected,
        }
    }
}

struct GoogleChatCrypto {
    encryption_key: [u8; 32],
    state_key: [u8; 32],
}

impl GoogleChatCrypto {
    fn new(secret: &SecretString) -> Result<Self, GoogleChatOAuthError> {
        if secret.expose_secret().len() < 32 {
            return Err(GoogleChatOAuthError::Configuration);
        }
        Ok(Self {
            encryption_key: derive_key(secret, b"jimin-os/google-chat/aead/v1"),
            state_key: derive_key(secret, b"jimin-os/google-chat/state/v1"),
        })
    }

    fn state_verifier(&self, state: &str) -> Vec<u8> {
        let mut mac = <HmacSha256 as HmacKeyInit>::new_from_slice(&self.state_key)
            .expect("SHA-256 HMAC accepts a fixed derived key");
        mac.update(state.as_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    fn encrypt(
        &self,
        plaintext: &[u8],
        aad: &[u8],
        key_version: i32,
    ) -> Result<EncryptedCalendarSecret, GoogleChatOAuthError> {
        if plaintext.is_empty() || key_version <= 0 {
            return Err(GoogleChatOAuthError::Encryption);
        }
        let mut nonce = [0_u8; XCHACHA_NONCE_BYTES];
        rand::rng().fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&self.encryption_key).into());
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| GoogleChatOAuthError::Encryption)?;
        Ok(EncryptedCalendarSecret {
            ciphertext,
            nonce: nonce.to_vec(),
            key_version,
        })
    }

    fn decrypt(
        &self,
        secret: &EncryptedCalendarSecret,
        aad: &[u8],
    ) -> Result<SecretString, GoogleChatOAuthError> {
        let nonce: [u8; XCHACHA_NONCE_BYTES] = secret
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| GoogleChatOAuthError::InvalidCallback)?;
        let cipher = XChaCha20Poly1305::new((&self.encryption_key).into());
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &secret.ciphertext,
                    aad,
                },
            )
            .map_err(|_| GoogleChatOAuthError::InvalidCallback)?;
        String::from_utf8(plaintext)
            .map(SecretString::from)
            .map_err(|_| GoogleChatOAuthError::InvalidCallback)
    }
}

fn required_chat_scopes(scopes: &[String]) -> Result<Vec<String>, GoogleChatOAuthError> {
    let required = [
        CHAT_SPACES_SCOPE,
        CHAT_MESSAGES_SCOPE,
        CHAT_REACTIONS_SCOPE,
        CHAT_MESSAGES_CREATE_SCOPE,
    ];
    if required
        .iter()
        .any(|required| !scopes.iter().any(|scope| scope == required))
    {
        return Err(GoogleChatOAuthError::RequiredScopeMissing);
    }
    Ok(required.into_iter().map(str::to_owned).collect())
}

fn provider_message(entry: GoogleChatMessageEntry) -> ProviderGoogleChatMessage {
    ProviderGoogleChatMessage {
        provider_message_name: entry.name,
        provider_thread_name: entry.thread_name,
        sender_provider_name: entry.sender_provider_name,
        sender_name: entry.sender_name,
        content_text: entry.text,
        received_at: entry.create_time,
    }
}

fn random_url_safe(byte_length: usize) -> String {
    let mut bytes = vec![0_u8; byte_length];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn derive_key(secret: &SecretString, label: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(label);
    digest.update([0_u8]);
    digest.update(secret.expose_secret().as_bytes());
    digest.finalize().into()
}

fn pkce_aad(authorization_id: Uuid) -> Vec<u8> {
    format!("jimin-os/google-chat/pkce/{authorization_id}").into_bytes()
}

fn refresh_token_aad(user_id: Uuid, provider_subject: &str) -> Vec<u8> {
    format!("jimin-os/google-chat/refresh/{user_id}/{provider_subject}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_consent_requires_read_acknowledgement_and_reply_scopes() {
        assert!(required_chat_scopes(&[CHAT_MESSAGES_SCOPE.to_owned()]).is_err());
        assert_eq!(
            required_chat_scopes(&[
                CHAT_SPACES_SCOPE.to_owned(),
                CHAT_MESSAGES_SCOPE.to_owned(),
                CHAT_REACTIONS_SCOPE.to_owned(),
                CHAT_MESSAGES_CREATE_SCOPE.to_owned(),
            ])
            .expect("required scopes"),
            vec![
                CHAT_SPACES_SCOPE.to_owned(),
                CHAT_MESSAGES_SCOPE.to_owned(),
                CHAT_REACTIONS_SCOPE.to_owned(),
                CHAT_MESSAGES_CREATE_SCOPE.to_owned(),
            ]
        );
        assert!(!GoogleChatOAuthRuntime::completion_scope_granted(&[
            CHAT_MESSAGES_SCOPE.to_owned(),
        ]));
        assert!(GoogleChatOAuthRuntime::completion_scope_granted(&[
            CHAT_MESSAGES_CREATE_SCOPE.to_owned(),
        ]));
    }

    #[test]
    fn first_manual_sync_respects_fresh_only_connection_cursor() {
        let now = OffsetDateTime::UNIX_EPOCH + Duration::days(20);
        let connected_at = now - Duration::minutes(2);
        let connection = GoogleChatSourceSyncConnection {
            source_id: Uuid::now_v7(),
            account_id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            project_id: Uuid::now_v7(),
            provider_subject: "provider-user".to_owned(),
            granted_scopes: vec![],
            space_name: "spaces/company".to_owned(),
            acknowledge_with_reaction: true,
            last_provider_message_at: Some(connected_at),
            last_successful_sync_at: None,
            source_had_error: false,
            account_needs_recovery: false,
            refresh_token: EncryptedCalendarSecret {
                ciphertext: vec![1],
                nonce: vec![2; XCHACHA_NONCE_BYTES],
                key_version: 1,
            },
        };

        assert_eq!(
            source_messages_created_after(&connection, true, now),
            connected_at
        );
        let mut existing = connection;
        existing.last_successful_sync_at = Some(now - Duration::minutes(1));
        assert_eq!(
            source_messages_created_after(&existing, true, now),
            connected_at - INITIAL_SYNC_LOOKBACK
        );
    }
}

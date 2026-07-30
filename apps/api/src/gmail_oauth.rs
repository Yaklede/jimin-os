//! Server-owned multi-account Gmail OAuth and synchronization runtime.
//!
//! Raw OAuth state, PKCE plaintext, access tokens, refresh tokens, and Gmail
//! provider responses are confined to this module. Storage receives only HMAC
//! state verifiers, AEAD ciphertext, verified identity metadata, and bounded
//! message headers/snippets.

use std::collections::BTreeSet;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit as AeadKeyInit, Payload},
};
use futures_util::{StreamExt, TryStreamExt, stream};
use hmac::{Hmac, Mac, digest::KeyInit as HmacKeyInit};
use jimin_domain::{ClientPlatform, PkceVerifier};
use jimin_google::{
    GoogleAuthError, GoogleAuthorizationCode, GoogleCalendarAdapter, GoogleGmailHistoryChange,
    GoogleGmailInboxBatch, GoogleGmailMessageEntry, GoogleIdentityAdapter, GoogleOAuthProfile,
};
use jimin_storage::gmail::{
    ClaimedGmailOAuthAuthorization, CompleteGmailOAuthAuthorization, EncryptedGmailSecret,
    GmailHistorySyncMode, GmailSyncConnection, ProviderGmailMessage,
};
use rand::Rng;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::config::CalendarOAuthSettings;

const AUTHORIZATION_LIFETIME: Duration = Duration::minutes(10);
const RANDOM_STATE_BYTES: usize = 32;
const RANDOM_PKCE_BYTES: usize = 64;
const XCHACHA_NONCE_BYTES: usize = 24;
const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";
const OPENID_SCOPE: &str = "openid";
const EMAIL_SCOPE: &str = "email";
const GOOGLE_EMAIL_SCOPE: &str = "https://www.googleapis.com/auth/userinfo.email";
const MAX_GMAIL_HISTORY_PAGES: usize = 100;
const MAX_GMAIL_HISTORY_CHANGED_MESSAGES: usize = 5_000;
const GMAIL_MESSAGE_FETCH_CONCURRENCY: usize = 8;

type HmacSha256 = Hmac<Sha256>;

/// Gmail OAuth runtime assembled only from deployment-owned settings.
pub struct GmailOAuthRuntime {
    google: GoogleIdentityAdapter,
    gmail: GoogleCalendarAdapter,
    crypto: GmailCrypto,
    redirect_uri: String,
    encryption_key_version: i32,
}

pub struct GmailInboxSyncBatch {
    pub mode: GmailHistorySyncMode,
    pub expected_provider_history_id: Option<String>,
    pub next_provider_history_id: String,
    pub messages: Vec<ProviderGmailMessage>,
    pub skipped_message_count: usize,
}

impl GmailOAuthRuntime {
    /// Uses the existing Google web client while requesting Gmail-only scopes.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration error for unusable deployment values.
    pub fn new(settings: &CalendarOAuthSettings) -> Result<Self, GmailOAuthError> {
        let mut profiles = Vec::new();
        for platform in [
            ClientPlatform::Macos,
            ClientPlatform::Ios,
            ClientPlatform::Android,
        ] {
            profiles.push(
                GoogleOAuthProfile::new_with_client_secret(
                    platform,
                    settings.client_id(),
                    settings.client_secret().clone(),
                    [settings.redirect_uri().to_owned()],
                    true,
                )
                .map_err(|_| GmailOAuthError::Configuration)?,
            );
        }
        Ok(Self {
            google: GoogleIdentityAdapter::new(profiles)
                .map_err(|_| GmailOAuthError::Configuration)?,
            gmail: GoogleCalendarAdapter::new(
                settings.client_id(),
                settings.client_secret().clone(),
            )
            .map_err(|_| GmailOAuthError::Configuration)?,
            crypto: GmailCrypto::new(settings.encryption_key())?,
            redirect_uri: settings.redirect_uri().to_owned(),
            encryption_key_version: settings.encryption_key_version(),
        })
    }

    /// Generates a short-lived server-owned consent request. Account selection
    /// and fresh consent are deliberate for personal/company identity choice.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration, validation, or encryption error.
    pub fn begin_authorization(
        &self,
        authorization_id: Uuid,
        client_kind: ClientPlatform,
        login_hint: Option<&str>,
    ) -> Result<NewGmailOAuthAuthorization, GmailOAuthError> {
        if authorization_id.get_version_num() != 7 {
            return Err(GmailOAuthError::Configuration);
        }
        let state = random_url_safe(RANDOM_STATE_BYTES);
        let pkce_verifier = random_url_safe(RANDOM_PKCE_BYTES);
        let code_challenge = pkce_challenge(&pkce_verifier);
        let authorization_url = self
            .google
            .gmail_authorization_url(client_kind, &state, &code_challenge, true, login_hint)
            .map_err(GmailOAuthError::from_google)?;
        let encrypted_pkce = self.crypto.encrypt(
            pkce_verifier.as_bytes(),
            &pkce_aad(authorization_id),
            self.encryption_key_version,
        )?;
        Ok(NewGmailOAuthAuthorization {
            state_verifier: self.crypto.state_verifier(&state),
            pkce_verifier: encrypted_pkce,
            authorization_url,
            expires_at: OffsetDateTime::now_utc() + AUTHORIZATION_LIFETIME,
        })
    }

    /// HMACs a callback state without retaining its plaintext.
    #[must_use]
    pub fn state_verifier(&self, state: &str) -> Vec<u8> {
        self.crypto.state_verifier(state)
    }

    /// Exchanges one claimed callback, verifies Gmail scope, and encrypts a
    /// newly issued refresh token with stable owner+provider AAD. Reconnects
    /// may legitimately omit a refresh token; storage preserves the old one.
    ///
    /// # Errors
    ///
    /// Returns a sanitized callback, provider, scope, or encryption error.
    pub async fn complete_authorization(
        &self,
        authorization: ClaimedGmailOAuthAuthorization,
        code: SecretString,
    ) -> Result<CompleteGmailOAuthAuthorization, GmailOAuthError> {
        let pkce = self
            .crypto
            .decrypt(&authorization.pkce_verifier, &pkce_aad(authorization.id))?;
        let pkce_verifier = PkceVerifier::parse(pkce.expose_secret().to_owned())
            .map_err(|_| GmailOAuthError::InvalidCallback)?;
        let grant = self
            .google
            .exchange_gmail(GoogleAuthorizationCode {
                platform: authorization.client_kind,
                authorization_code: code,
                code_verifier: Some(pkce_verifier),
                redirect_uri: self.redirect_uri.clone(),
            })
            .await
            .map_err(GmailOAuthError::from_google)?;
        if authorization
            .expected_provider_subject
            .as_ref()
            .is_some_and(|expected| expected != grant.identity().subject())
        {
            return Err(GmailOAuthError::IdentityMismatch);
        }
        let granted_scopes = gmail_only_scopes(grant.granted_scopes())?;
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
        Ok(CompleteGmailOAuthAuthorization {
            authorization_id: authorization.id,
            account_id: Uuid::now_v7(),
            user_id: authorization.user_id,
            workspace_id: authorization.workspace_id,
            provider_subject,
            email: grant.identity().email().clone(),
            granted_scopes,
            refresh_token,
        })
    }

    /// Synchronizes a bounded metadata-only inbox snapshot or catches up from
    /// the account's persisted Gmail History cursor.
    ///
    /// # Errors
    ///
    /// Returns a sanitized credential or provider failure.
    pub async fn inbox_sync(
        &self,
        connection: &GmailSyncConnection,
    ) -> Result<GmailInboxSyncBatch, GmailOAuthError> {
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id, &connection.provider_subject),
        )?;
        let access_token = self
            .gmail
            .refresh_access_token(&refresh_token)
            .await
            .map_err(GmailOAuthError::from_google)?;
        let Some(history_id) = connection.provider_history_id.as_deref() else {
            return self
                .baseline_sync(&access_token, GmailHistorySyncMode::Baseline, None)
                .await
                .map_err(GmailOAuthError::from_google);
        };
        match self.incremental_sync(&access_token, history_id).await {
            Ok(batch) => Ok(batch),
            Err(GoogleAuthError::GmailHistoryIdExpired) => self
                .baseline_sync(
                    &access_token,
                    GmailHistorySyncMode::Rebuild,
                    Some(history_id),
                )
                .await
                .map_err(GmailOAuthError::from_google),
            Err(error) => Err(GmailOAuthError::from_google(error)),
        }
    }

    async fn baseline_sync(
        &self,
        access_token: &SecretString,
        mode: GmailHistorySyncMode,
        expected_provider_history_id: Option<&str>,
    ) -> Result<GmailInboxSyncBatch, GoogleAuthError> {
        // Read the baseline before the bounded snapshot. Messages arriving
        // during the import are then replayed by the next History catch-up.
        let next_provider_history_id = self.gmail.gmail_profile_history_id(access_token).await?;
        let GoogleGmailInboxBatch {
            messages,
            skipped_message_count,
        } = self
            .gmail
            .list_gmail_inbox_messages(access_token, None, None)
            .await?;
        Ok(GmailInboxSyncBatch {
            mode,
            expected_provider_history_id: expected_provider_history_id.map(str::to_owned),
            next_provider_history_id,
            messages: messages.into_iter().map(provider_message).collect(),
            skipped_message_count,
        })
    }

    async fn incremental_sync(
        &self,
        access_token: &SecretString,
        start_history_id: &str,
    ) -> Result<GmailInboxSyncBatch, GoogleAuthError> {
        let mut page_token = None;
        let mut changed_message_ids = BTreeSet::new();
        for _ in 0..MAX_GMAIL_HISTORY_PAGES {
            let page = self
                .gmail
                .list_gmail_history_page(access_token, start_history_id, page_token.as_deref())
                .await?;
            for change in &page.changes {
                collect_inbox_additions(change, &mut changed_message_ids);
                if changed_message_ids.len() > MAX_GMAIL_HISTORY_CHANGED_MESSAGES {
                    return Err(GoogleAuthError::ProviderRejected);
                }
            }
            let next_provider_history_id = page.history_id;
            let Some(next_page_token) = page.next_page_token else {
                let fetched = stream::iter(changed_message_ids.into_iter().map(|message_id| {
                    let gmail = &self.gmail;
                    async move { gmail.get_gmail_message(access_token, &message_id).await }
                }))
                .buffer_unordered(GMAIL_MESSAGE_FETCH_CONCURRENCY)
                .try_collect::<Vec<_>>()
                .await?;
                let mut messages = Vec::with_capacity(fetched.len());
                let mut skipped_message_count = 0;
                for entry in fetched {
                    match entry {
                        Some(entry) if entry.is_inbox => messages.push(provider_message(entry)),
                        Some(_) => {}
                        None => skipped_message_count += 1,
                    }
                }
                return Ok(GmailInboxSyncBatch {
                    mode: GmailHistorySyncMode::Incremental,
                    expected_provider_history_id: Some(start_history_id.to_owned()),
                    next_provider_history_id,
                    messages,
                    skipped_message_count,
                });
            };
            page_token = Some(next_page_token);
        }
        Err(GoogleAuthError::ProviderUnavailable)
    }
}

fn collect_inbox_additions(change: &GoogleGmailHistoryChange, message_ids: &mut BTreeSet<String>) {
    message_ids.extend(
        change
            .messages_added
            .iter()
            .filter(|message| message.label_ids.iter().any(|label| label == "INBOX"))
            .map(|message| message.provider_message_id.clone()),
    );
    message_ids.extend(
        change
            .labels_added
            .iter()
            .filter(|event| event.label_ids.iter().any(|label| label == "INBOX"))
            .map(|event| event.message.provider_message_id.clone()),
    );
}

/// Persistable half of a new OAuth request. Raw browser state is present only
/// in `authorization_url`.
pub struct NewGmailOAuthAuthorization {
    pub state_verifier: Vec<u8>,
    pub pkce_verifier: EncryptedGmailSecret,
    pub authorization_url: String,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GmailOAuthError {
    #[error("Gmail OAuth configuration is invalid")]
    Configuration,
    #[error("Gmail OAuth callback is invalid")]
    InvalidCallback,
    #[error("Google rejected Gmail authorization")]
    ProviderRejected,
    #[error("Gmail is temporarily unavailable")]
    ProviderUnavailable,
    #[error("Google did not grant the Gmail read permission")]
    RequiredScopeMissing,
    #[error("Gmail API is not enabled for the configured Google project")]
    ApiNotEnabled,
    #[error("The linked Google account cannot grant Gmail read permission")]
    PermissionDenied,
    #[error("Google returned permissions outside the Gmail identity boundary")]
    ScopeBoundaryViolation,
    #[error("Google account does not match the Gmail account being reconnected")]
    IdentityMismatch,
    #[error("Gmail credential encryption failed")]
    Encryption,
}

impl GmailOAuthError {
    #[must_use]
    pub const fn failure_code(self) -> &'static str {
        match self {
            Self::Configuration => "gmail.configuration_invalid",
            Self::InvalidCallback => "gmail.invalid_callback",
            Self::ProviderRejected => "gmail.authorization_rejected",
            Self::ProviderUnavailable => "gmail.provider_unavailable",
            Self::RequiredScopeMissing => "gmail.required_scope_missing",
            Self::ApiNotEnabled => "gmail.api_not_enabled",
            Self::PermissionDenied => "gmail.permission_denied",
            Self::ScopeBoundaryViolation => "gmail.scope_boundary_violation",
            Self::IdentityMismatch => "gmail.account_mismatch",
            Self::Encryption => "gmail.credential_encryption_failed",
        }
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::ProviderUnavailable)
    }

    #[must_use]
    pub const fn reauth_required(self) -> bool {
        matches!(
            self,
            Self::InvalidCallback
                | Self::ProviderRejected
                | Self::RequiredScopeMissing
                | Self::PermissionDenied
                | Self::ScopeBoundaryViolation
                | Self::IdentityMismatch
                | Self::Encryption
        )
    }

    fn from_google(error: GoogleAuthError) -> Self {
        match error {
            GoogleAuthError::ProviderUnavailable
            | GoogleAuthError::ProviderDataInvalid
            | GoogleAuthError::GmailHistoryIdExpired => Self::ProviderUnavailable,
            GoogleAuthError::GmailApiNotEnabled => Self::ApiNotEnabled,
            GoogleAuthError::GmailPermissionDenied => Self::PermissionDenied,
            GoogleAuthError::InvalidRequest | GoogleAuthError::ProviderRejected => {
                Self::ProviderRejected
            }
            GoogleAuthError::IdentityRejected => Self::InvalidCallback,
            GoogleAuthError::CalendarSyncTokenExpired
            | GoogleAuthError::CalendarEventConflict
            | GoogleAuthError::CalendarEventNotFound
            | GoogleAuthError::CalendarEventRejected => Self::ProviderRejected,
        }
    }
}

struct GmailCrypto {
    encryption_key: [u8; 32],
    state_key: [u8; 32],
}

impl GmailCrypto {
    fn new(secret: &SecretString) -> Result<Self, GmailOAuthError> {
        if secret.expose_secret().len() < 32 {
            return Err(GmailOAuthError::Configuration);
        }
        Ok(Self {
            encryption_key: derive_key(secret, b"jimin-os/gmail/aead/v1"),
            state_key: derive_key(secret, b"jimin-os/gmail/state/v1"),
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
    ) -> Result<EncryptedGmailSecret, GmailOAuthError> {
        if plaintext.is_empty() || key_version <= 0 {
            return Err(GmailOAuthError::Encryption);
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
            .map_err(|_| GmailOAuthError::Encryption)?;
        Ok(EncryptedGmailSecret {
            ciphertext,
            nonce: nonce.to_vec(),
            key_version,
        })
    }

    fn decrypt(
        &self,
        secret: &EncryptedGmailSecret,
        aad: &[u8],
    ) -> Result<SecretString, GmailOAuthError> {
        if secret.nonce.len() != XCHACHA_NONCE_BYTES || secret.ciphertext.is_empty() {
            return Err(GmailOAuthError::InvalidCallback);
        }
        let nonce: [u8; XCHACHA_NONCE_BYTES] = secret
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| GmailOAuthError::InvalidCallback)?;
        let cipher = XChaCha20Poly1305::new((&self.encryption_key).into());
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &secret.ciphertext,
                    aad,
                },
            )
            .map_err(|_| GmailOAuthError::InvalidCallback)?;
        let value = String::from_utf8(plaintext).map_err(|_| GmailOAuthError::InvalidCallback)?;
        Ok(SecretString::from(value))
    }
}

fn provider_message(entry: GoogleGmailMessageEntry) -> ProviderGmailMessage {
    ProviderGmailMessage {
        provider_message_id: entry.provider_message_id,
        provider_thread_id: entry.provider_thread_id,
        received_at: entry.received_at,
        sender: entry.sender,
        subject: entry.subject,
        snippet: entry.snippet,
        body_text: entry.body_text,
        reference_links: entry.reference_links,
        list_id: entry.list_id,
        list_unsubscribe: entry.list_unsubscribe,
        precedence: entry.precedence,
        auto_submitted: entry.auto_submitted,
        is_unread: entry.is_unread,
    }
}

fn gmail_only_scopes(scopes: &[String]) -> Result<Vec<String>, GmailOAuthError> {
    let scopes = scopes.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if !scopes.contains(GMAIL_READONLY_SCOPE) {
        return Err(GmailOAuthError::RequiredScopeMissing);
    }
    if scopes.iter().any(|scope| {
        !matches!(
            *scope,
            GMAIL_READONLY_SCOPE | OPENID_SCOPE | EMAIL_SCOPE | GOOGLE_EMAIL_SCOPE
        )
    }) {
        return Err(GmailOAuthError::ScopeBoundaryViolation);
    }
    Ok(scopes.into_iter().map(ToOwned::to_owned).collect())
}

fn random_url_safe(byte_length: usize) -> String {
    let mut bytes = vec![0_u8; byte_length];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn derive_key(secret: &SecretString, label: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(label);
    digest.update([0_u8]);
    digest.update(secret.expose_secret().as_bytes());
    digest.finalize().into()
}

fn pkce_aad(authorization_id: Uuid) -> Vec<u8> {
    format!("jimin-os/gmail/pkce/{authorization_id}").into_bytes()
}

fn refresh_token_aad(user_id: Uuid, provider_subject: &str) -> Vec<u8> {
    format!("jimin-os/gmail/refresh/{user_id}/{provider_subject}").into_bytes()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use jimin_google::{
        GoogleAuthError, GoogleGmailHistoryChange, GoogleGmailHistoryLabelChange,
        GoogleGmailHistoryMessage,
    };
    use secrecy::SecretString;
    use uuid::Uuid;

    use super::{
        GMAIL_READONLY_SCOPE, GmailCrypto, GmailOAuthError, collect_inbox_additions,
        gmail_only_scopes, pkce_aad, refresh_token_aad,
    };

    #[test]
    fn encrypted_pkce_is_bound_to_its_authorization() {
        let crypto = GmailCrypto::new(&SecretString::from(
            "0123456789abcdef0123456789abcdef".to_owned(),
        ))
        .expect("test key should be accepted");
        let authorization_id = Uuid::now_v7();
        let encrypted = crypto
            .encrypt(b"verifier", &pkce_aad(authorization_id), 1)
            .expect("PKCE should encrypt");

        assert!(
            crypto
                .decrypt(&encrypted, &pkce_aad(authorization_id))
                .is_ok()
        );
        assert!(
            crypto
                .decrypt(&encrypted, &pkce_aad(Uuid::now_v7()))
                .is_err()
        );
    }

    #[test]
    fn refresh_token_aad_separates_google_identities() {
        let user_id = Uuid::now_v7();
        assert_ne!(
            refresh_token_aad(user_id, "personal-google-sub"),
            refresh_token_aad(user_id, "company-google-sub")
        );
    }

    #[test]
    fn gmail_scope_contract_rejects_missing_or_cross_product_permissions() {
        assert_eq!(
            gmail_only_scopes(&["openid".to_owned()]),
            Err(GmailOAuthError::RequiredScopeMissing)
        );
        assert_eq!(
            gmail_only_scopes(&[
                GMAIL_READONLY_SCOPE.to_owned(),
                "https://www.googleapis.com/auth/calendar.events".to_owned(),
            ]),
            Err(GmailOAuthError::ScopeBoundaryViolation)
        );
    }

    #[test]
    fn gmail_scope_contract_keeps_the_actual_bounded_provider_set() {
        assert_eq!(
            gmail_only_scopes(&[
                GMAIL_READONLY_SCOPE.to_owned(),
                "openid".to_owned(),
                "https://www.googleapis.com/auth/userinfo.email".to_owned(),
                GMAIL_READONLY_SCOPE.to_owned(),
            ]),
            Ok(vec![
                GMAIL_READONLY_SCOPE.to_owned(),
                "https://www.googleapis.com/auth/userinfo.email".to_owned(),
                "openid".to_owned(),
            ])
        );
    }

    #[test]
    fn gmail_provider_failures_keep_server_setup_separate_from_account_permission() {
        let api_not_enabled = GmailOAuthError::from_google(GoogleAuthError::GmailApiNotEnabled);
        let permission_denied =
            GmailOAuthError::from_google(GoogleAuthError::GmailPermissionDenied);
        let quota_exhausted = GmailOAuthError::from_google(GoogleAuthError::ProviderUnavailable);

        assert_eq!(api_not_enabled.failure_code(), "gmail.api_not_enabled");
        assert!(!api_not_enabled.reauth_required());
        assert_eq!(permission_denied.failure_code(), "gmail.permission_denied");
        assert!(permission_denied.reauth_required());
        assert_eq!(quota_exhausted.failure_code(), "gmail.provider_unavailable");
        assert!(quota_exhausted.retryable());
        assert!(!quota_exhausted.reauth_required());
    }

    #[test]
    fn history_candidate_collection_keeps_only_inbox_additions_and_deduplicates() {
        let inbox_message = GoogleGmailHistoryMessage {
            provider_message_id: "message-1".to_owned(),
            provider_thread_id: "thread-1".to_owned(),
            label_ids: vec!["INBOX".to_owned()],
        };
        let archived_message = GoogleGmailHistoryMessage {
            provider_message_id: "message-2".to_owned(),
            provider_thread_id: "thread-2".to_owned(),
            label_ids: vec!["IMPORTANT".to_owned()],
        };
        let change = GoogleGmailHistoryChange {
            history_id: "101".to_owned(),
            messages_added: vec![inbox_message.clone(), archived_message],
            messages_deleted: Vec::new(),
            labels_added: vec![GoogleGmailHistoryLabelChange {
                message: inbox_message,
                label_ids: vec!["INBOX".to_owned()],
            }],
            labels_removed: Vec::new(),
        };
        let mut ids = BTreeSet::new();

        collect_inbox_additions(&change, &mut ids);

        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec!["message-1"]);
    }
}

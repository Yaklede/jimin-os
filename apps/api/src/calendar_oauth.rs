//! Server-owned Google Calendar consent primitives.
//!
//! This module owns raw OAuth state, PKCE plaintext, and Google refresh-token
//! handling for their shortest possible lifetime. Persistent storage receives
//! only an HMAC state verifier and AEAD ciphertext through `jimin-storage`.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit as AeadKeyInit, Payload},
};
use hmac::{Hmac, Mac, digest::KeyInit as HmacKeyInit};
use jimin_domain::{ClientPlatform, PkceVerifier};
use jimin_google::{
    GoogleAuthError, GoogleAuthorizationCode, GoogleCalendarAdapter, GoogleCalendarEventEntry,
    GoogleCalendarEventMutation, GoogleCalendarEventStatus, GoogleCalendarEventTime,
    GoogleCalendarGrant, GoogleCalendarListEntry, GoogleCalendarVisibility,
    GoogleGmailMessageEntry, GoogleIdentityAdapter, GoogleOAuthProfile,
};
use jimin_storage::{
    StorageError,
    calendar::{
        CalendarSyncConnection, CalendarSyncTarget, ClaimedCalendarOAuthAuthorization,
        CompleteCalendarOAuthAuthorization, EncryptedCalendarSecret, ProviderCalendar,
        ProviderCalendarEvent, ProviderCalendarEventStatus, ProviderCalendarEventTime,
        ProviderCalendarVisibility,
    },
    calendar_mutation::{ClaimedScheduleCalendarMutation, ScheduleCalendarMutationOperation},
    gmail::ProviderGmailMessage,
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
const CALENDAR_EVENTS_SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";
const CALENDAR_LIST_SCOPE: &str = "https://www.googleapis.com/auth/calendar.calendarlist.readonly";
const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

type HmacSha256 = Hmac<Sha256>;

/// Calendar OAuth runtime assembled entirely from deployment-owned settings.
pub struct CalendarOAuthRuntime {
    google: GoogleIdentityAdapter,
    calendar: GoogleCalendarAdapter,
    crypto: CalendarCrypto,
    redirect_uri: String,
    encryption_key_version: i32,
}

impl CalendarOAuthRuntime {
    /// Builds fixed OAuth profiles for every Jimin OS client surface. All use
    /// the same server callback, not client-supplied redirect URLs.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarOAuthError::Configuration`] for unusable deployment
    /// settings without exposing the setting content.
    pub fn new(settings: &CalendarOAuthSettings) -> Result<Self, CalendarOAuthError> {
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
                .map_err(|_| CalendarOAuthError::Configuration)?,
            );
        }
        Ok(Self {
            google: GoogleIdentityAdapter::new(profiles)
                .map_err(|_| CalendarOAuthError::Configuration)?,
            calendar: GoogleCalendarAdapter::new(
                settings.client_id(),
                settings.client_secret().clone(),
            )
            .map_err(|_| CalendarOAuthError::Configuration)?,
            crypto: CalendarCrypto::new(settings.encryption_key())?,
            redirect_uri: settings.redirect_uri().to_owned(),
            encryption_key_version: settings.encryption_key_version(),
        })
    }

    /// Generates all short-lived authorization material for one persisted
    /// transaction. The raw state is used only in the returned URL.
    ///
    /// # Errors
    ///
    /// Returns [`CalendarOAuthError`] when server-owned configuration cannot
    /// produce a valid Google authorization URL or encrypt the PKCE verifier.
    pub fn begin_authorization(
        &self,
        authorization_id: Uuid,
        client_kind: ClientPlatform,
        force_consent: bool,
    ) -> Result<NewCalendarOAuthAuthorization, CalendarOAuthError> {
        if authorization_id.get_version_num() != 7 {
            return Err(CalendarOAuthError::Configuration);
        }
        let state = random_url_safe(RANDOM_STATE_BYTES);
        let pkce_verifier = random_url_safe(RANDOM_PKCE_BYTES);
        let code_challenge = pkce_challenge(&pkce_verifier);
        let authorization_url = self
            .google
            .calendar_authorization_url(client_kind, &state, &code_challenge, force_consent)
            .map_err(CalendarOAuthError::from_google)?;
        let encrypted_pkce = self.crypto.encrypt(
            pkce_verifier.as_bytes(),
            &pkce_aad(authorization_id),
            self.encryption_key_version,
        )?;
        Ok(NewCalendarOAuthAuthorization {
            state_verifier: self.crypto.state_verifier(&state),
            pkce_verifier: encrypted_pkce,
            authorization_url,
            expires_at: OffsetDateTime::now_utc() + AUTHORIZATION_LIFETIME,
        })
    }

    /// HMACs a raw state query value without retaining its plaintext.
    #[must_use]
    pub fn state_verifier(&self, state: &str) -> Vec<u8> {
        self.crypto.state_verifier(state)
    }

    /// Exchanges a claimed callback, validates the linked Google identity,
    /// and prepares the encrypted refresh token for an atomic DB completion.
    ///
    /// # Errors
    ///
    /// Returns a sanitized [`CalendarOAuthError`] when the callback cannot be
    /// decrypted, Google rejects it, or the consent is not for the signed-in
    /// Jimin OS account.
    pub async fn complete_authorization(
        &self,
        authorization: ClaimedCalendarOAuthAuthorization,
        code: SecretString,
    ) -> Result<CompleteCalendarOAuthAuthorization, CalendarOAuthError> {
        let pkce = self
            .crypto
            .decrypt(&authorization.pkce_verifier, &pkce_aad(authorization.id))?;
        let pkce_verifier = PkceVerifier::parse(pkce.expose_secret().to_owned())
            .map_err(|_| CalendarOAuthError::InvalidCallback)?;
        let grant = self
            .google
            .exchange_calendar(GoogleAuthorizationCode {
                platform: authorization.client_kind,
                authorization_code: code,
                code_verifier: Some(pkce_verifier),
                redirect_uri: self.redirect_uri.clone(),
            })
            .await
            .map_err(CalendarOAuthError::from_google)?;
        self.completion_from_grant(&authorization, &grant)
    }

    fn completion_from_grant(
        &self,
        authorization: &ClaimedCalendarOAuthAuthorization,
        grant: &GoogleCalendarGrant,
    ) -> Result<CompleteCalendarOAuthAuthorization, CalendarOAuthError> {
        if grant.identity().subject() != &authorization.expected_google_subject {
            return Err(CalendarOAuthError::IdentityMismatch);
        }
        let granted_scopes = calendar_scopes(grant.granted_scopes())?;
        let refresh_token = grant
            .refresh_token()
            .map(|token| {
                self.crypto.encrypt(
                    token.expose_secret().as_bytes(),
                    &refresh_token_aad(authorization.user_id),
                    self.encryption_key_version,
                )
            })
            .transpose()?;
        Ok(CompleteCalendarOAuthAuthorization {
            authorization_id: authorization.id,
            account_id: Uuid::now_v7(),
            user_id: authorization.user_id,
            provider_subject: grant.identity().subject().clone(),
            email: grant.identity().email().clone(),
            granted_scopes,
            refresh_token,
        })
    }

    /// Uses the newly persisted refresh credential to load the complete
    /// Google Calendar list before the account becomes active.
    ///
    /// # Errors
    ///
    /// Returns a sanitized [`CalendarOAuthError`] when credential decryption,
    /// token refresh, or provider list retrieval fails.
    pub async fn initial_calendar_list_sync(
        &self,
        connection: &CalendarSyncConnection,
    ) -> Result<Vec<ProviderCalendar>, CalendarOAuthError> {
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id),
        )?;
        let access_token = self
            .calendar
            .refresh_access_token(&refresh_token)
            .await
            .map_err(CalendarOAuthError::from_google)?;
        let entries = self
            .calendar
            .list_calendars(&access_token)
            .await
            .map_err(CalendarOAuthError::from_google)?;
        Ok(entries.into_iter().map(provider_calendar).collect())
    }

    /// Fetches every selected calendar's events only after its Calendar list
    /// has been persisted. The short-lived access token stays within this
    /// method and raw provider payloads never cross the API boundary.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider or credential failure without leaking a
    /// calendar ID, token, or event content.
    pub async fn calendar_event_sync(
        &self,
        connection: &CalendarSyncConnection,
        targets: &[CalendarSyncTarget],
    ) -> Result<Vec<CalendarEventSyncBatch>, CalendarOAuthError> {
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id),
        )?;
        let access_token = self
            .calendar
            .refresh_access_token(&refresh_token)
            .await
            .map_err(CalendarOAuthError::from_google)?;
        let mut synced = Vec::with_capacity(targets.len());
        for target in targets {
            let previous_sync_token = target
                .sync_token
                .as_ref()
                .map(|token| {
                    self.crypto
                        .decrypt(token, &sync_token_aad(target.calendar_id))
                })
                .transpose()?;
            let (response, is_full_sync) = if let Some(sync_token) = previous_sync_token.as_ref() {
                match self
                    .calendar
                    .list_event_changes(
                        &access_token,
                        &target.provider_calendar_id,
                        &target.time_zone,
                        Some(sync_token),
                    )
                    .await
                {
                    Ok(response) => (response, false),
                    Err(GoogleAuthError::CalendarSyncTokenExpired) => (
                        self.calendar
                            .list_events(
                                &access_token,
                                &target.provider_calendar_id,
                                &target.time_zone,
                            )
                            .await
                            .map_err(CalendarOAuthError::from_google)?,
                        true,
                    ),
                    Err(error) => return Err(CalendarOAuthError::from_google(error)),
                }
            } else {
                (
                    self.calendar
                        .list_events(
                            &access_token,
                            &target.provider_calendar_id,
                            &target.time_zone,
                        )
                        .await
                        .map_err(CalendarOAuthError::from_google)?,
                    true,
                )
            };
            let next_sync_token = self.crypto.encrypt(
                response.next_sync_token.expose_secret().as_bytes(),
                &sync_token_aad(target.calendar_id),
                self.encryption_key_version,
            )?;
            synced.push(CalendarEventSyncBatch {
                calendar_id: target.calendar_id,
                events: response
                    .events
                    .into_iter()
                    .map(provider_calendar_event)
                    .collect(),
                next_sync_token,
                is_full_sync,
            });
        }
        Ok(synced)
    }

    /// Updates one provider event with its stored `ETag` after decrypting the
    /// account credential only inside the server process.
    ///
    /// # Errors
    ///
    /// Returns a sanitized connection, credential, or provider error.
    pub async fn update_calendar_event(
        &self,
        connection: &CalendarSyncConnection,
        target: &jimin_storage::calendar::CalendarEventMutationTarget,
        mutation: GoogleCalendarEventMutation,
    ) -> Result<ProviderCalendarEvent, CalendarOAuthError> {
        let access_token = self.calendar_access_token(connection).await?;
        self.calendar
            .update_event(
                &access_token,
                &target.provider_calendar_id,
                &target.provider_event_id,
                target.provider_etag.as_deref(),
                &mutation,
            )
            .await
            .map(provider_calendar_event)
            .map_err(CalendarOAuthError::from_google)
    }

    /// Creates one event in the selected primary provider calendar.
    ///
    /// # Errors
    ///
    /// Returns a sanitized connection, credential, or provider error.
    pub async fn create_calendar_event(
        &self,
        connection: &CalendarSyncConnection,
        target: &jimin_storage::calendar::PrimaryCalendarMutationTarget,
        mutation: GoogleCalendarEventMutation,
    ) -> Result<ProviderCalendarEvent, CalendarOAuthError> {
        let access_token = self.calendar_access_token(connection).await?;
        self.calendar
            .create_event(&access_token, &target.provider_calendar_id, &mutation)
            .await
            .map(provider_calendar_event)
            .map_err(CalendarOAuthError::from_google)
    }

    /// Deletes one provider event with its stored `ETag`.
    ///
    /// # Errors
    ///
    /// Returns a sanitized connection, credential, or provider error.
    pub async fn delete_calendar_event(
        &self,
        connection: &CalendarSyncConnection,
        target: &jimin_storage::calendar::CalendarEventMutationTarget,
    ) -> Result<(), CalendarOAuthError> {
        let access_token = self.calendar_access_token(connection).await?;
        self.calendar
            .delete_event(
                &access_token,
                &target.provider_calendar_id,
                &target.provider_event_id,
                target.provider_etag.as_deref(),
            )
            .await
            .map_err(CalendarOAuthError::from_google)
    }

    /// Dispatches one leased schedule mutation using only the matching account
    /// credential. Deterministic create IDs and provider `ETags` make replay
    /// converge without logging event content or OAuth material.
    ///
    /// # Errors
    ///
    /// Returns a sanitized identity, credential, validation, conflict, or
    /// provider failure.
    pub async fn dispatch_schedule_calendar_mutation(
        &self,
        connection: &CalendarSyncConnection,
        mutation: &ClaimedScheduleCalendarMutation,
    ) -> Result<Option<String>, CalendarOAuthError> {
        if connection.account_id != mutation.account_id || connection.user_id != mutation.user_id {
            return Err(CalendarOAuthError::IdentityMismatch);
        }
        let access_token = self.calendar_access_token(connection).await?;
        match mutation.operation {
            ScheduleCalendarMutationOperation::Create => {
                let provider = self
                    .calendar
                    .create_event_with_id(
                        &access_token,
                        &mutation.provider_calendar_id,
                        &mutation.provider_event_id,
                        &schedule_google_mutation(mutation),
                    )
                    .await
                    .map_err(CalendarOAuthError::from_google)?;
                Ok(provider.provider_etag)
            }
            ScheduleCalendarMutationOperation::Update => {
                let provider = self
                    .calendar
                    .update_event(
                        &access_token,
                        &mutation.provider_calendar_id,
                        &mutation.provider_event_id,
                        mutation.provider_etag.as_deref(),
                        &schedule_google_mutation(mutation),
                    )
                    .await
                    .map_err(CalendarOAuthError::from_google)?;
                Ok(provider.provider_etag)
            }
            ScheduleCalendarMutationOperation::Delete => {
                self.calendar
                    .delete_event(
                        &access_token,
                        &mutation.provider_calendar_id,
                        &mutation.provider_event_id,
                        mutation.provider_etag.as_deref(),
                    )
                    .await
                    .map_err(CalendarOAuthError::from_google)?;
                Ok(None)
            }
        }
    }

    /// Revokes the detached encrypted refresh credential after local Calendar
    /// data has already been purged. Provider failure is intentionally safe to
    /// ignore at the HTTP boundary because disconnect must remain local-first.
    ///
    /// # Errors
    ///
    /// Returns a sanitized credential or provider failure.
    pub async fn revoke_calendar_connection(
        &self,
        connection: &CalendarSyncConnection,
    ) -> Result<(), CalendarOAuthError> {
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id),
        )?;
        self.calendar
            .revoke_refresh_token(&refresh_token)
            .await
            .map_err(CalendarOAuthError::from_google)
    }

    async fn calendar_access_token(
        &self,
        connection: &CalendarSyncConnection,
    ) -> Result<SecretString, CalendarOAuthError> {
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id),
        )?;
        self.calendar
            .refresh_access_token(&refresh_token)
            .await
            .map_err(CalendarOAuthError::from_google)
    }

    /// Loads a bounded inbox metadata view only when the account has granted
    /// Gmail read access. Existing Calendar-only accounts remain usable until
    /// the owner reconnects with the expanded Google consent scope.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider or credential error and never requests
    /// Gmail bodies or attachments.
    pub async fn initial_gmail_inbox_sync(
        &self,
        connection: &CalendarSyncConnection,
    ) -> Result<Option<Vec<ProviderGmailMessage>>, CalendarOAuthError> {
        if !connection
            .granted_scopes
            .iter()
            .any(|scope| scope == GMAIL_READONLY_SCOPE)
        {
            return Ok(None);
        }
        let refresh_token = self.crypto.decrypt(
            &connection.refresh_token,
            &refresh_token_aad(connection.user_id),
        )?;
        let access_token = self
            .calendar
            .refresh_access_token(&refresh_token)
            .await
            .map_err(CalendarOAuthError::from_google)?;
        let messages = self
            .calendar
            .list_gmail_inbox_messages(&access_token)
            .await
            .map_err(CalendarOAuthError::from_google)?;
        Ok(Some(
            messages.into_iter().map(provider_gmail_message).collect(),
        ))
    }
}

fn schedule_google_mutation(
    mutation: &ClaimedScheduleCalendarMutation,
) -> GoogleCalendarEventMutation {
    GoogleCalendarEventMutation {
        title: mutation.payload.title.clone(),
        description: mutation.payload.notes.clone(),
        start: mutation.payload.starts_at,
        end: mutation.payload.ends_at,
        time_zone: mutation.payload.time_zone.clone(),
    }
}

/// Newly generated material that is safe to persist only through the matching
/// storage command. The raw state is embedded in `authorization_url` only.
pub struct NewCalendarOAuthAuthorization {
    pub state_verifier: Vec<u8>,
    pub pkce_verifier: EncryptedCalendarSecret,
    pub authorization_url: String,
    pub expires_at: OffsetDateTime,
}

/// One atomic provider event batch ready for storage. Full batches replace a
/// calendar read model; incremental batches apply only the returned changes.
pub struct CalendarEventSyncBatch {
    pub calendar_id: Uuid,
    pub events: Vec<ProviderCalendarEvent>,
    pub next_sync_token: EncryptedCalendarSecret,
    pub is_full_sync: bool,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CalendarOAuthError {
    #[error("Calendar OAuth configuration is invalid")]
    Configuration,
    #[error("Calendar OAuth callback is invalid")]
    InvalidCallback,
    #[error("Google Calendar authorization was rejected")]
    ProviderRejected,
    #[error("Google Calendar is temporarily unavailable")]
    ProviderUnavailable,
    #[error("Google Calendar event changed before the requested mutation")]
    Conflict,
    #[error("Google Calendar event no longer exists")]
    EventNotFound,
    #[error("Google Calendar rejected the event payload")]
    EventRejected,
    #[error("Google account does not match the signed-in Jimin OS account")]
    IdentityMismatch,
    #[error("Google Calendar did not grant the required permissions")]
    RequiredScopeMissing,
    #[error("Calendar credential encryption failed")]
    Encryption,
}

impl CalendarOAuthError {
    #[must_use]
    pub const fn failure_code(self) -> &'static str {
        match self {
            Self::Configuration => "calendar.configuration_invalid",
            Self::InvalidCallback
            | Self::ProviderRejected
            | Self::RequiredScopeMissing
            | Self::Encryption => "calendar.authorization_failed",
            Self::ProviderUnavailable => "calendar.provider_unavailable",
            Self::Conflict => "calendar.event_conflict",
            Self::EventNotFound => "calendar.event_not_found",
            Self::EventRejected => "calendar.event_rejected",
            Self::IdentityMismatch => "calendar.account_mismatch",
        }
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::ProviderUnavailable)
    }

    fn from_google(error: GoogleAuthError) -> Self {
        match error {
            GoogleAuthError::ProviderUnavailable => Self::ProviderUnavailable,
            GoogleAuthError::CalendarSyncTokenExpired => Self::ProviderRejected,
            GoogleAuthError::CalendarEventConflict => Self::Conflict,
            GoogleAuthError::CalendarEventNotFound => Self::EventNotFound,
            GoogleAuthError::CalendarEventRejected => Self::EventRejected,
            GoogleAuthError::InvalidRequest | GoogleAuthError::ProviderRejected => {
                Self::ProviderRejected
            }
            GoogleAuthError::IdentityRejected => Self::InvalidCallback,
        }
    }
}

struct CalendarCrypto {
    encryption_key: [u8; 32],
    state_key: [u8; 32],
}

impl CalendarCrypto {
    fn new(secret: &SecretString) -> Result<Self, CalendarOAuthError> {
        if secret.expose_secret().len() < 32 {
            return Err(CalendarOAuthError::Configuration);
        }
        Ok(Self {
            encryption_key: derive_key(secret, b"jimin-os/calendar/aead/v1"),
            state_key: derive_key(secret, b"jimin-os/calendar/state/v1"),
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
    ) -> Result<EncryptedCalendarSecret, CalendarOAuthError> {
        if plaintext.is_empty() || key_version <= 0 {
            return Err(CalendarOAuthError::Encryption);
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
            .map_err(|_| CalendarOAuthError::Encryption)?;
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
    ) -> Result<SecretString, CalendarOAuthError> {
        if secret.nonce.len() != XCHACHA_NONCE_BYTES || secret.ciphertext.is_empty() {
            return Err(CalendarOAuthError::InvalidCallback);
        }
        let nonce: [u8; XCHACHA_NONCE_BYTES] = secret
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| CalendarOAuthError::InvalidCallback)?;
        let cipher = XChaCha20Poly1305::new((&self.encryption_key).into());
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &secret.ciphertext,
                    aad,
                },
            )
            .map_err(|_| CalendarOAuthError::InvalidCallback)?;
        let value =
            String::from_utf8(plaintext).map_err(|_| CalendarOAuthError::InvalidCallback)?;
        Ok(SecretString::from(value))
    }
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
    format!("jimin-os/calendar/pkce/{authorization_id}").into_bytes()
}

fn refresh_token_aad(user_id: Uuid) -> Vec<u8> {
    format!("jimin-os/calendar/refresh/{user_id}").into_bytes()
}

fn sync_token_aad(calendar_id: Uuid) -> Vec<u8> {
    format!("jimin-os/calendar/sync/{calendar_id}").into_bytes()
}

fn calendar_scopes(scopes: &[String]) -> Result<Vec<String>, CalendarOAuthError> {
    if !scopes.iter().any(|scope| scope == CALENDAR_EVENTS_SCOPE)
        || !scopes.iter().any(|scope| scope == CALENDAR_LIST_SCOPE)
        || !scopes.iter().any(|scope| scope == GMAIL_READONLY_SCOPE)
    {
        return Err(CalendarOAuthError::RequiredScopeMissing);
    }
    Ok(vec![
        CALENDAR_EVENTS_SCOPE.to_owned(),
        CALENDAR_LIST_SCOPE.to_owned(),
        GMAIL_READONLY_SCOPE.to_owned(),
    ])
}

fn provider_calendar(entry: GoogleCalendarListEntry) -> ProviderCalendar {
    ProviderCalendar {
        provider_calendar_id: entry.provider_calendar_id,
        name: entry.name,
        description: entry.description,
        time_zone: entry.time_zone,
        color_id: entry.color_id,
        access_role: entry.access_role,
        is_primary: entry.is_primary,
        provider_selected: entry.provider_selected,
        visibility: match entry.visibility {
            GoogleCalendarVisibility::Visible => ProviderCalendarVisibility::Visible,
            GoogleCalendarVisibility::Hidden => ProviderCalendarVisibility::Hidden,
            GoogleCalendarVisibility::Deleted => ProviderCalendarVisibility::Deleted,
        },
        provider_etag: entry.provider_etag,
    }
}

fn provider_calendar_event(entry: GoogleCalendarEventEntry) -> ProviderCalendarEvent {
    ProviderCalendarEvent {
        provider_event_id: entry.provider_event_id,
        provider_etag: entry.provider_etag,
        provider_updated_at: entry.provider_updated_at,
        ical_uid: entry.ical_uid,
        status: match entry.status {
            GoogleCalendarEventStatus::Confirmed => ProviderCalendarEventStatus::Confirmed,
            GoogleCalendarEventStatus::Tentative => ProviderCalendarEventStatus::Tentative,
            GoogleCalendarEventStatus::Cancelled => ProviderCalendarEventStatus::Cancelled,
        },
        event_type: entry.event_type,
        title: entry.title,
        description: entry.description,
        location: entry.location,
        time: entry.time.map(|time| match time {
            GoogleCalendarEventTime::Date { start, end } => {
                ProviderCalendarEventTime::Date { start, end }
            }
            GoogleCalendarEventTime::DateTime {
                start,
                end,
                time_zone,
            } => ProviderCalendarEventTime::DateTime {
                start,
                end,
                time_zone,
            },
        }),
        recurrence: entry.recurrence,
        recurring_provider_event_id: entry.recurring_provider_event_id,
        visibility: entry.visibility,
        transparency: entry.transparency,
        html_link: entry.html_link,
        is_editable: entry.is_editable,
    }
}

fn provider_gmail_message(entry: GoogleGmailMessageEntry) -> ProviderGmailMessage {
    ProviderGmailMessage {
        provider_message_id: entry.provider_message_id,
        provider_thread_id: entry.provider_thread_id,
        received_at: entry.received_at,
        sender: entry.sender,
        subject: entry.subject,
        snippet: entry.snippet,
        is_unread: entry.is_unread,
    }
}

/// Maps callback failures to the narrow storage error surface used by routes.
pub fn storage_failure_code(error: &StorageError) -> &'static str {
    match error {
        StorageError::IdentityConflict => "calendar.account_mismatch",
        StorageError::InvalidConfiguration => "calendar.authorization_failed",
        StorageError::MigrationUnavailable | StorageError::PersistenceUnavailable => {
            "calendar.provider_unavailable"
        }
    }
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn event_level_provider_failures_do_not_invalidate_the_account() {
        let missing = CalendarOAuthError::from_google(GoogleAuthError::CalendarEventNotFound);
        let rejected = CalendarOAuthError::from_google(GoogleAuthError::CalendarEventRejected);

        assert_eq!(missing, CalendarOAuthError::EventNotFound);
        assert_eq!(missing.failure_code(), "calendar.event_not_found");
        assert_eq!(rejected, CalendarOAuthError::EventRejected);
        assert_eq!(rejected.failure_code(), "calendar.event_rejected");
        assert!(!missing.retryable());
        assert!(!rejected.retryable());
    }

    #[test]
    fn state_verifier_is_keyed_and_does_not_echo_state() {
        let crypto = CalendarCrypto::new(&SecretString::from("x".repeat(32)))
            .expect("test crypto should build");
        let verifier = crypto.state_verifier("state-value");
        assert_eq!(verifier.len(), 32);
        assert_ne!(verifier, b"state-value");
        assert_ne!(verifier, crypto.state_verifier("other-state"));
    }

    #[test]
    fn encrypted_pkce_is_bound_to_its_authorization() {
        let crypto = CalendarCrypto::new(&SecretString::from("x".repeat(32)))
            .expect("test crypto should build");
        let authorization_id = Uuid::now_v7();
        let encrypted = crypto
            .encrypt(b"verifier", &pkce_aad(authorization_id), 1)
            .expect("PKCE should encrypt");
        let plaintext = crypto
            .decrypt(&encrypted, &pkce_aad(authorization_id))
            .expect("matching AAD should decrypt");
        assert_eq!(plaintext.expose_secret(), "verifier");
        assert!(
            crypto
                .decrypt(&encrypted, &pkce_aad(Uuid::now_v7()))
                .is_err()
        );
    }

    #[test]
    fn calendar_scope_filter_requires_all_requested_google_scopes() {
        let scopes = vec![
            CALENDAR_EVENTS_SCOPE.to_owned(),
            CALENDAR_LIST_SCOPE.to_owned(),
            GMAIL_READONLY_SCOPE.to_owned(),
        ];
        assert_eq!(
            calendar_scopes(&scopes).expect("scopes should be accepted"),
            scopes
        );
        assert!(
            calendar_scopes(&[
                CALENDAR_EVENTS_SCOPE.to_owned(),
                CALENDAR_LIST_SCOPE.to_owned(),
            ])
            .is_err()
        );
    }
}

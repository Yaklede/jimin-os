//! Fixed-endpoint Google authorization-code and `OpenID` Connect verification.
//!
//! The adapter receives an authorization code only from a platform OAuth flow,
//! exchanges it with Google's fixed endpoint, and returns a validated identity.
//! It never persists or logs the code, provider access token, refresh token, or
//! ID token.

use std::{collections::BTreeMap, time::Duration};

use jimin_application::VerifiedGoogleIdentity;
use jimin_domain::{ClientPlatform, EmailAddress, GoogleSubject, PkceVerifier};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use reqwest::{
    Client, Response,
    header::{CACHE_CONTROL, CONTENT_TYPE, IF_MATCH},
    redirect::Policy,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, Duration as TimeDuration, Month, OffsetDateTime};
use tokio::sync::Mutex;

const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_TOKEN_REVOCATION_ENDPOINT: &str = "https://oauth2.googleapis.com/revoke";
const GOOGLE_JWKS_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_CALENDAR_LIST_ENDPOINT: &str =
    "https://www.googleapis.com/calendar/v3/users/me/calendarList";
const GOOGLE_CALENDAR_EVENTS_ENDPOINT: &str = "https://www.googleapis.com/calendar/v3/calendars";
const GOOGLE_GMAIL_MESSAGES_ENDPOINT: &str =
    "https://gmail.googleapis.com/gmail/v1/users/me/messages";
const GOOGLE_CHAT_SPACES_ENDPOINT: &str = "https://chat.googleapis.com/v1/spaces";
const GOOGLE_ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];
const MAX_AUTHORIZATION_CODE_BYTES: usize = 4 * 1024;
const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_JWKS_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_CALENDAR_LIST_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CALENDAR_LIST_PAGES: usize = 100;
const MAX_CALENDAR_LIST_ITEMS: usize = 10_000;
const MAX_CALENDAR_EVENT_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CALENDAR_EVENT_PAGES: usize = 100;
const MAX_CALENDAR_EVENT_ITEMS: usize = 100_000;
const MAX_RECURRENCE_RULES: usize = 128;
const MAX_GMAIL_INBOX_MESSAGES: usize = 50;
const MAX_GMAIL_LIST_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_GMAIL_MESSAGE_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_CHAT_LIST_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHAT_LIST_PAGES: usize = 50;
const MAX_CHAT_ITEMS: usize = 5_000;
const GOOGLE_CHAT_MESSAGE_ORDER: &str = "createTime ASC";
const DEFAULT_JWKS_TTL: Duration = Duration::from_mins(5);
const MAX_JWKS_TTL: Duration = Duration::from_hours(24);

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GoogleAuthError {
    #[error("Google OAuth request is invalid")]
    InvalidRequest,
    #[error("Google OAuth provider rejected the login")]
    ProviderRejected,
    #[error("Google identity verification failed")]
    IdentityRejected,
    #[error("Google identity provider is temporarily unavailable")]
    ProviderUnavailable,
    #[error("Google Calendar incremental synchronization token expired")]
    CalendarSyncTokenExpired,
    #[error("Google Calendar event changed before the requested mutation")]
    CalendarEventConflict,
    #[error("Google Calendar event no longer exists")]
    CalendarEventNotFound,
    #[error("Google Calendar rejected the event payload")]
    CalendarEventRejected,
}

/// One server-owned client profile. Callers cannot supply a client ID, token
/// endpoint, or arbitrary redirect URI per request.
#[derive(Debug, Clone)]
pub struct GoogleOAuthProfile {
    platform: ClientPlatform,
    client_id: String,
    client_secret: Option<SecretString>,
    redirect_uris: Vec<String>,
    pkce_required: bool,
}

impl GoogleOAuthProfile {
    /// Creates a strict profile from deployment-owned configuration.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::InvalidRequest`] for blank values or
    /// unsupported redirect URI values. Duplicate redirects are normalized.
    pub fn new(
        platform: ClientPlatform,
        client_id: impl Into<String>,
        redirect_uris: impl IntoIterator<Item = String>,
        pkce_required: bool,
    ) -> Result<Self, GoogleAuthError> {
        Self::with_optional_secret(
            platform,
            client_id.into(),
            None,
            redirect_uris,
            pkce_required,
        )
    }

    /// Creates a strict server-side client profile which has a confidential
    /// OAuth client secret. The secret remains deployment-owned and is used
    /// only at the fixed Google token endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::InvalidRequest`] when the client profile or
    /// secret is malformed.
    pub fn new_with_client_secret(
        platform: ClientPlatform,
        client_id: impl Into<String>,
        client_secret: SecretString,
        redirect_uris: impl IntoIterator<Item = String>,
        pkce_required: bool,
    ) -> Result<Self, GoogleAuthError> {
        if client_secret.expose_secret().is_empty()
            || client_secret.expose_secret().len() > 4_096
            || client_secret.expose_secret().chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        Self::with_optional_secret(
            platform,
            client_id.into(),
            Some(client_secret),
            redirect_uris,
            pkce_required,
        )
    }

    fn with_optional_secret(
        platform: ClientPlatform,
        client_id: String,
        client_secret: Option<SecretString>,
        redirect_uris: impl IntoIterator<Item = String>,
        pkce_required: bool,
    ) -> Result<Self, GoogleAuthError> {
        let client_id = validate_text(client_id, 255)?;
        let mut redirect_uris: Vec<_> = redirect_uris
            .into_iter()
            .map(validate_redirect_uri)
            .collect::<Result<_, _>>()?;
        redirect_uris.sort();
        redirect_uris.dedup();
        if redirect_uris.is_empty() {
            return Err(GoogleAuthError::InvalidRequest);
        }
        Ok(Self {
            platform,
            client_id,
            client_secret,
            redirect_uris,
            pkce_required,
        })
    }

    #[must_use]
    pub const fn platform(&self) -> ClientPlatform {
        self.platform
    }

    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    fn client_secret(&self) -> Option<&SecretString> {
        self.client_secret.as_ref()
    }

    fn accepts_redirect(&self, redirect_uri: &str) -> bool {
        self.redirect_uris
            .iter()
            .any(|allowed| allowed == redirect_uri)
    }
}

/// Input received from a platform's completed Google authorization-code flow.
pub struct GoogleAuthorizationCode {
    pub platform: ClientPlatform,
    pub authorization_code: SecretString,
    pub code_verifier: Option<PkceVerifier>,
    pub redirect_uri: String,
}

/// The server-only result of a Google Calendar consent exchange. Provider
/// access and refresh credentials are deliberately never serializable.
pub struct GoogleCalendarGrant {
    identity: VerifiedGoogleIdentity,
    refresh_token: Option<SecretString>,
    granted_scopes: Vec<String>,
}

/// The server-only result of company Google Chat consent.
pub struct GoogleChatGrant {
    identity: VerifiedGoogleIdentity,
    refresh_token: Option<SecretString>,
    granted_scopes: Vec<String>,
}

impl GoogleChatGrant {
    #[must_use]
    pub const fn identity(&self) -> &VerifiedGoogleIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn refresh_token(&self) -> Option<&SecretString> {
        self.refresh_token.as_ref()
    }

    #[must_use]
    pub fn granted_scopes(&self) -> &[String] {
        &self.granted_scopes
    }
}

impl GoogleCalendarGrant {
    #[must_use]
    pub const fn identity(&self) -> &VerifiedGoogleIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn refresh_token(&self) -> Option<&SecretString> {
        self.refresh_token.as_ref()
    }

    #[must_use]
    pub fn granted_scopes(&self) -> &[String] {
        &self.granted_scopes
    }
}

struct CachedJwks {
    keys: JwkSet,
    expires_at: OffsetDateTime,
}

/// Google adapter with a bounded, server-local JWKS cache.
pub struct GoogleIdentityAdapter {
    client: Client,
    profiles: BTreeMap<String, GoogleOAuthProfile>,
    jwks: Mutex<Option<CachedJwks>>,
}

/// Fixed Google Calendar provider adapter. It owns no persisted credentials:
/// callers pass a short-lived access token or a refresh token decrypted only
/// in the server process.
pub struct GoogleCalendarAdapter {
    client: Client,
    client_id: String,
    client_secret: SecretString,
}

/// Fixed Google Chat provider adapter for company workspace ingestion.
pub struct GoogleChatAdapter {
    client: Client,
    client_id: String,
    client_secret: SecretString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleChatSpaceEntry {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleChatMessageEntry {
    pub name: String,
    pub thread_name: Option<String>,
    pub sender_name: Option<String>,
    pub text: String,
    pub create_time: OffsetDateTime,
}

/// One validated Calendar list entry. Provider IDs remain server-only and are
/// intentionally not serializable or printable.
pub struct GoogleCalendarListEntry {
    pub provider_calendar_id: String,
    pub name: String,
    pub description: Option<String>,
    pub time_zone: String,
    pub color_id: Option<String>,
    pub access_role: String,
    pub is_primary: bool,
    pub provider_selected: bool,
    pub visibility: GoogleCalendarVisibility,
    pub provider_etag: Option<String>,
}

/// Provider visibility normalized from `deleted` and `hidden` flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleCalendarVisibility {
    Visible,
    Hidden,
    Deleted,
}

/// Fully normalized provider event used only by the server-side sync path.
/// It intentionally contains no attendee, conference, or attachment data.
pub struct GoogleCalendarEventEntry {
    pub provider_event_id: String,
    pub provider_etag: Option<String>,
    pub provider_updated_at: Option<OffsetDateTime>,
    pub ical_uid: Option<String>,
    pub status: GoogleCalendarEventStatus,
    pub event_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub time: Option<GoogleCalendarEventTime>,
    pub recurrence: Option<Vec<String>>,
    pub recurring_provider_event_id: Option<String>,
    pub visibility: Option<String>,
    pub transparency: Option<String>,
    pub html_link: Option<String>,
    pub is_editable: bool,
}

/// One validated Calendar events response assembled across every provider
/// page. The next synchronization token stays secret and must be persisted
/// encrypted by the caller.
pub struct GoogleCalendarEventSync {
    pub events: Vec<GoogleCalendarEventEntry>,
    pub next_sync_token: SecretString,
}

/// Validated timed-event replacement sent to the fixed Google Calendar API.
/// Calendar and event identifiers are supplied separately and never accepted
/// as arbitrary endpoint URLs.
pub struct GoogleCalendarEventMutation {
    pub title: String,
    pub description: Option<String>,
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
    pub time_zone: String,
}

/// The two mutually exclusive time representations accepted by Google
/// Calendar. All-day events retain their date semantics until persistence.
pub enum GoogleCalendarEventTime {
    Date {
        start: Date,
        end: Date,
    },
    DateTime {
        start: OffsetDateTime,
        end: OffsetDateTime,
        time_zone: String,
    },
}

/// Google provider event status after strict normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleCalendarEventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

/// Bounded Gmail inbox metadata. Message bodies, attachments, and raw header
/// collections are discarded by the adapter before this value is returned.
pub struct GoogleGmailMessageEntry {
    pub provider_message_id: String,
    pub provider_thread_id: String,
    pub received_at: Option<OffsetDateTime>,
    pub sender: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub is_unread: bool,
}

impl GoogleCalendarAdapter {
    /// Creates a Google Calendar adapter with fixed provider endpoints.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::InvalidRequest`] for malformed deployment
    /// client settings or [`GoogleAuthError::ProviderUnavailable`] when the
    /// HTTP client cannot be configured.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: SecretString,
    ) -> Result<Self, GoogleAuthError> {
        let client_id = validate_text(client_id.into(), 255)?;
        if client_secret.expose_secret().is_empty()
            || client_secret.expose_secret().len() > 4_096
            || client_secret.expose_secret().chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        Ok(Self {
            client,
            client_id,
            client_secret,
        })
    }

    /// Exchanges a stored refresh token for a short-lived access token.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider error without exposing token data.
    pub async fn refresh_access_token(
        &self,
        refresh_token: &SecretString,
    ) -> Result<SecretString, GoogleAuthError> {
        let value = refresh_token.expose_secret();
        if value.is_empty()
            || value.len() > MAX_TOKEN_RESPONSE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let response = self
            .client
            .post(GOOGLE_TOKEN_ENDPOINT)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", value),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.expose_secret()),
            ])
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(classify_provider_status(response.status().as_u16()));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_TOKEN_RESPONSE_BYTES).await?;
        let response: GoogleRefreshTokenResponse =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
        if response.access_token.is_empty()
            || response.access_token.len() > MAX_TOKEN_RESPONSE_BYTES
            || response.access_token.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::ProviderRejected);
        }
        Ok(SecretString::from(response.access_token))
    }

    /// Best-effort revokes a refresh credential at Google's fixed revocation
    /// endpoint. Callers must still delete the local encrypted credential when
    /// this request fails because local disconnect must not depend on provider
    /// availability.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation or provider error without exposing the
    /// credential or response body.
    pub async fn revoke_refresh_token(
        &self,
        refresh_token: &SecretString,
    ) -> Result<(), GoogleAuthError> {
        let token = refresh_token.expose_secret();
        if token.is_empty()
            || token.len() > MAX_TOKEN_RESPONSE_BYTES
            || token.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let response = self
            .client
            .post(GOOGLE_TOKEN_REVOCATION_ENDPOINT)
            .form(&[("token", token)])
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(classify_provider_status(response.status().as_u16()))
        }
    }

    /// Loads every page of the Google Calendar list with the stable full-sync
    /// query parameters required by the provider contract.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider error for an unavailable, rejected, or
    /// malformed response. It never retains provider response bodies.
    pub async fn list_calendars(
        &self,
        access_token: &SecretString,
    ) -> Result<Vec<GoogleCalendarListEntry>, GoogleAuthError> {
        let token = access_token.expose_secret();
        if token.is_empty()
            || token.len() > MAX_TOKEN_RESPONSE_BYTES
            || token.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let mut next_page_token: Option<String> = None;
        let mut calendars = Vec::new();
        for _ in 0..MAX_CALENDAR_LIST_PAGES {
            let mut url = reqwest::Url::parse(GOOGLE_CALENDAR_LIST_ENDPOINT)
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("showDeleted", "true");
                query.append_pair("showHidden", "true");
                query.append_pair("maxResults", "250");
                if let Some(page_token) = &next_page_token {
                    query.append_pair("pageToken", page_token);
                }
            }
            let response = self
                .client
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            if !response.status().is_success() {
                return Err(classify_provider_status(response.status().as_u16()));
            }
            if !is_json_response(&response) {
                return Err(GoogleAuthError::ProviderUnavailable);
            }
            let payload = bounded_body(response, MAX_CALENDAR_LIST_RESPONSE_BYTES).await?;
            let page: GoogleCalendarListPage =
                serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
            for item in page.items {
                calendars.push(normalize_calendar_list_item(item)?);
                if calendars.len() > MAX_CALENDAR_LIST_ITEMS {
                    return Err(GoogleAuthError::ProviderRejected);
                }
            }
            next_page_token = page.next_page_token;
            if next_page_token.is_none() {
                return Ok(calendars);
            }
        }
        Err(GoogleAuthError::ProviderRejected)
    }

    /// Loads every page for one Calendar using the stable full-sync query
    /// contract. Recurring masters and exceptions are preserved, while
    /// cancelled entries are retained so storage can tombstone old events.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider error and never retains the response body
    /// after validation. The caller supplies a validated calendar time zone
    /// used only when Google omits event-level time-zone metadata.
    pub async fn list_events(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        calendar_time_zone: &str,
    ) -> Result<GoogleCalendarEventSync, GoogleAuthError> {
        self.list_event_changes(access_token, provider_calendar_id, calendar_time_zone, None)
            .await
    }

    /// Loads only changes since a previously persisted Google sync token.
    /// A provider HTTP 410 response is classified separately so callers can
    /// discard the expired token and safely restart with a full sync.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::CalendarSyncTokenExpired`] for an expired
    /// token and a sanitized provider error for all other failures.
    pub async fn list_event_changes(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        calendar_time_zone: &str,
        sync_token: Option<&SecretString>,
    ) -> Result<GoogleCalendarEventSync, GoogleAuthError> {
        let token = access_token.expose_secret();
        let provider_calendar_id = validate_text(provider_calendar_id.to_owned(), 1_024)?;
        let calendar_time_zone = validate_text(calendar_time_zone.to_owned(), 80)?;
        if token.is_empty()
            || token.len() > MAX_TOKEN_RESPONSE_BYTES
            || token.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        if sync_token.is_some_and(|value| {
            value.expose_secret().is_empty()
                || value.expose_secret().len() > MAX_TOKEN_RESPONSE_BYTES
                || value.expose_secret().chars().any(char::is_control)
        }) {
            return Err(GoogleAuthError::InvalidRequest);
        }

        let mut next_page_token: Option<String> = None;
        let mut events = Vec::new();
        for _ in 0..MAX_CALENDAR_EVENT_PAGES {
            let mut url = reqwest::Url::parse(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            url.path_segments_mut()
                .map_err(|()| GoogleAuthError::ProviderUnavailable)?
                .push(&provider_calendar_id)
                .push("events");
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("singleEvents", "false");
                query.append_pair("showDeleted", "true");
                query.append_pair("maxResults", "2500");
                if let Some(sync_token) = sync_token {
                    query.append_pair("syncToken", sync_token.expose_secret());
                }
                if let Some(page_token) = &next_page_token {
                    query.append_pair("pageToken", page_token);
                }
            }
            let response = self
                .client
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            if let Some(error) =
                classify_calendar_event_status(response.status().as_u16(), sync_token.is_some())
            {
                return Err(error);
            }
            if !is_json_response(&response) {
                return Err(GoogleAuthError::ProviderUnavailable);
            }
            let payload = bounded_body(response, MAX_CALENDAR_EVENT_RESPONSE_BYTES).await?;
            let page: GoogleCalendarEventPage =
                serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
            for item in page.items {
                events.push(normalize_calendar_event_item(item, &calendar_time_zone)?);
                if events.len() > MAX_CALENDAR_EVENT_ITEMS {
                    return Err(GoogleAuthError::ProviderRejected);
                }
            }
            next_page_token = page.next_page_token;
            if next_page_token.is_none() {
                let next_sync_token = page
                    .next_sync_token
                    .filter(|value| {
                        !value.is_empty()
                            && value.len() <= MAX_TOKEN_RESPONSE_BYTES
                            && !value.chars().any(char::is_control)
                    })
                    .ok_or(GoogleAuthError::ProviderRejected)?;
                return Ok(GoogleCalendarEventSync {
                    events,
                    next_sync_token: SecretString::from(next_sync_token),
                });
            }
        }
        Err(GoogleAuthError::ProviderRejected)
    }

    /// Replaces one timed event using its last provider `ETag` as an optimistic
    /// concurrency guard.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::CalendarEventConflict`] when Google reports
    /// that the event changed, or a sanitized provider failure otherwise.
    pub async fn update_event(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        provider_event_id: &str,
        provider_etag: Option<&str>,
        mutation: &GoogleCalendarEventMutation,
    ) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
        let (token, calendar_id, event_id, etag, body) = validated_event_mutation_request(
            access_token,
            provider_calendar_id,
            provider_event_id,
            provider_etag,
            mutation,
        )?;
        let mut url = reqwest::Url::parse(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        url.path_segments_mut()
            .map_err(|()| GoogleAuthError::ProviderUnavailable)?
            .push(&calendar_id)
            .push("events")
            .push(&event_id);
        let mut request = self.client.put(url).bearer_auth(token).json(&body);
        if let Some(etag) = etag {
            request = request.header(IF_MATCH, etag);
        }
        let Ok(response) = request.send().await else {
            return self
                .existing_event_if_matching(access_token, &calendar_id, &event_id, mutation)
                .await;
        };
        if !response.status().is_success() {
            if matches!(response.status().as_u16(), 409 | 412) {
                return self
                    .existing_event_if_matching(access_token, &calendar_id, &event_id, mutation)
                    .await;
            }
            return Err(classify_calendar_mutation_status(
                response.status().as_u16(),
            ));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_CALENDAR_EVENT_RESPONSE_BYTES).await?;
        let item: GoogleCalendarEventItem =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
        normalize_calendar_event_item(item, &mutation.time_zone)
    }

    /// Creates one timed event in a fixed Google Calendar collection.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation, provider, or transport error.
    pub async fn create_event(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        mutation: &GoogleCalendarEventMutation,
    ) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
        self.create_event_inner(access_token, provider_calendar_id, None, mutation)
            .await
    }

    /// Creates one event with a caller-owned deterministic Google event ID.
    /// Replaying the same mutation after an unknown provider response loads and
    /// validates that event instead of creating a duplicate.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation, conflict, transport, or provider error.
    pub async fn create_event_with_id(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        provider_event_id: &str,
        mutation: &GoogleCalendarEventMutation,
    ) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
        let provider_event_id = validate_google_event_id(provider_event_id)?;
        self.create_event_inner(
            access_token,
            provider_calendar_id,
            Some(provider_event_id),
            mutation,
        )
        .await
    }

    async fn create_event_inner(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        provider_event_id: Option<String>,
        mutation: &GoogleCalendarEventMutation,
    ) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
        let token = validate_access_token(access_token)?;
        let calendar_id = validate_text(provider_calendar_id.to_owned(), 1_024)?;
        let mut body = validated_event_mutation_body(mutation)?;
        body.id.clone_from(&provider_event_id);
        let mut url = reqwest::Url::parse(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        url.path_segments_mut()
            .map_err(|()| GoogleAuthError::ProviderUnavailable)?
            .push(&calendar_id)
            .push("events");
        let Ok(response) = self
            .client
            .post(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
        else {
            let Some(provider_event_id) = provider_event_id.as_deref() else {
                return Err(GoogleAuthError::ProviderUnavailable);
            };
            return match self
                .existing_event_if_matching(access_token, &calendar_id, provider_event_id, mutation)
                .await
            {
                Ok(event) => Ok(event),
                Err(GoogleAuthError::CalendarEventConflict) => {
                    Err(GoogleAuthError::CalendarEventConflict)
                }
                Err(_) => Err(GoogleAuthError::ProviderUnavailable),
            };
        };
        if !response.status().is_success() {
            if response.status().as_u16() == 409
                && let Some(provider_event_id) = provider_event_id.as_deref()
            {
                return self
                    .existing_event_if_matching(
                        access_token,
                        &calendar_id,
                        provider_event_id,
                        mutation,
                    )
                    .await;
            }
            return Err(classify_calendar_mutation_status(
                response.status().as_u16(),
            ));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_CALENDAR_EVENT_RESPONSE_BYTES).await?;
        let item: GoogleCalendarEventItem =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
        normalize_calendar_event_item(item, &mutation.time_zone)
    }

    async fn existing_event_if_matching(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        provider_event_id: &str,
        mutation: &GoogleCalendarEventMutation,
    ) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
        let event = self
            .get_event(
                access_token,
                provider_calendar_id,
                provider_event_id,
                &mutation.time_zone,
            )
            .await?;
        if event_matches_mutation(&event, mutation) {
            Ok(event)
        } else {
            Err(GoogleAuthError::CalendarEventConflict)
        }
    }

    /// Loads one bounded event from a fixed Calendar endpoint for idempotency
    /// reconciliation. Provider response bodies are discarded after parsing.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation, transport, or provider error.
    pub async fn get_event(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        provider_event_id: &str,
        calendar_time_zone: &str,
    ) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
        let token = validate_access_token(access_token)?;
        let calendar_id = validate_text(provider_calendar_id.to_owned(), 1_024)?;
        let event_id = validate_text(provider_event_id.to_owned(), 1_024)?;
        let calendar_time_zone = validate_text(calendar_time_zone.to_owned(), 80)?;
        let mut url = reqwest::Url::parse(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        url.path_segments_mut()
            .map_err(|()| GoogleAuthError::ProviderUnavailable)?
            .push(&calendar_id)
            .push("events")
            .push(&event_id);
        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(classify_calendar_mutation_status(
                response.status().as_u16(),
            ));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_CALENDAR_EVENT_RESPONSE_BYTES).await?;
        let item: GoogleCalendarEventItem =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
        normalize_calendar_event_item(item, &calendar_time_zone)
    }

    /// Deletes one event using its provider `ETag` as an optimistic concurrency
    /// guard.
    ///
    /// # Errors
    ///
    /// Returns a conflict for a stale `ETag` and a sanitized provider error for
    /// all other failures.
    pub async fn delete_event(
        &self,
        access_token: &SecretString,
        provider_calendar_id: &str,
        provider_event_id: &str,
        provider_etag: Option<&str>,
    ) -> Result<(), GoogleAuthError> {
        let token = validate_access_token(access_token)?;
        let calendar_id = validate_text(provider_calendar_id.to_owned(), 1_024)?;
        let event_id = validate_text(provider_event_id.to_owned(), 1_024)?;
        let etag = provider_etag
            .map(|value| validate_text(value.to_owned(), 2_048))
            .transpose()?;
        let mut url = reqwest::Url::parse(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        url.path_segments_mut()
            .map_err(|()| GoogleAuthError::ProviderUnavailable)?
            .push(&calendar_id)
            .push("events")
            .push(&event_id);
        let mut request = self.client.delete(url).bearer_auth(token);
        if let Some(etag) = etag {
            request = request.header(IF_MATCH, etag);
        }
        let response = request
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if response.status().is_success() || response.status().as_u16() == 404 {
            Ok(())
        } else {
            Err(classify_calendar_mutation_status(
                response.status().as_u16(),
            ))
        }
    }

    /// Lists a bounded, read-only view of the Gmail inbox. The adapter first
    /// receives message IDs, then requests only metadata headers for each
    /// entry; it never requests body parts or attachments.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider error and retains no Gmail response body
    /// after the metadata has been normalized.
    pub async fn list_gmail_inbox_messages(
        &self,
        access_token: &SecretString,
    ) -> Result<Vec<GoogleGmailMessageEntry>, GoogleAuthError> {
        let token = access_token.expose_secret();
        if token.is_empty()
            || token.len() > MAX_TOKEN_RESPONSE_BYTES
            || token.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let mut list_url = reqwest::Url::parse(GOOGLE_GMAIL_MESSAGES_ENDPOINT)
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        {
            let mut query = list_url.query_pairs_mut();
            query.append_pair("labelIds", "INBOX");
            query.append_pair("maxResults", "50");
        }
        let response = self
            .client
            .get(list_url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(classify_provider_status(response.status().as_u16()));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_GMAIL_LIST_RESPONSE_BYTES).await?;
        let list: GoogleGmailMessageListResponse =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
        if list.messages.len() > MAX_GMAIL_INBOX_MESSAGES {
            return Err(GoogleAuthError::ProviderRejected);
        }

        let mut messages = Vec::with_capacity(list.messages.len());
        for reference in list.messages {
            let provider_message_id = validate_text(reference.id, 255)?;
            let mut message_url = reqwest::Url::parse(GOOGLE_GMAIL_MESSAGES_ENDPOINT)
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            message_url
                .path_segments_mut()
                .map_err(|()| GoogleAuthError::ProviderUnavailable)?
                .push(&provider_message_id);
            {
                let mut query = message_url.query_pairs_mut();
                query.append_pair("format", "metadata");
                query.append_pair("metadataHeaders", "From");
                query.append_pair("metadataHeaders", "Subject");
            }
            let response = self
                .client
                .get(message_url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            if !response.status().is_success() {
                return Err(classify_provider_status(response.status().as_u16()));
            }
            if !is_json_response(&response) {
                return Err(GoogleAuthError::ProviderUnavailable);
            }
            let payload = bounded_body(response, MAX_GMAIL_MESSAGE_RESPONSE_BYTES).await?;
            let message: GoogleGmailMessageResponse =
                serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
            messages.push(normalize_gmail_message(message, &provider_message_id)?);
        }
        Ok(messages)
    }
}

impl GoogleChatAdapter {
    /// Creates the fixed Chat API client used only by the server process.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration error for invalid OAuth settings.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: SecretString,
    ) -> Result<Self, GoogleAuthError> {
        let client_id = validate_text(client_id.into(), 255)?;
        if client_secret.expose_secret().is_empty()
            || client_secret.expose_secret().len() > 4_096
            || client_secret.expose_secret().chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        Ok(Self {
            client,
            client_id,
            client_secret,
        })
    }

    /// Exchanges one encrypted-at-rest refresh token for a short-lived access
    /// token. Provider token values never leave this adapter boundary.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation or provider error.
    pub async fn refresh_access_token(
        &self,
        refresh_token: &SecretString,
    ) -> Result<SecretString, GoogleAuthError> {
        let value = refresh_token.expose_secret();
        if value.is_empty()
            || value.len() > MAX_TOKEN_RESPONSE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let response = self
            .client
            .post(GOOGLE_TOKEN_ENDPOINT)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", value),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.expose_secret()),
            ])
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(classify_provider_status(response.status().as_u16()));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_TOKEN_RESPONSE_BYTES).await?;
        let response: GoogleRefreshTokenResponse =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
        Ok(SecretString::from(validate_text(
            response.access_token,
            MAX_TOKEN_RESPONSE_BYTES,
        )?))
    }

    /// Best-effort revokes a linked Chat refresh credential. Local deletion
    /// remains the source of truth when Google is temporarily unavailable.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation or provider error.
    pub async fn revoke_refresh_token(
        &self,
        refresh_token: &SecretString,
    ) -> Result<(), GoogleAuthError> {
        let token = refresh_token.expose_secret();
        if token.is_empty()
            || token.len() > MAX_TOKEN_RESPONSE_BYTES
            || token.chars().any(char::is_control)
        {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let response = self
            .client
            .post(GOOGLE_TOKEN_REVOCATION_ENDPOINT)
            .form(&[("token", token)])
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(classify_provider_status(response.status().as_u16()))
        }
    }

    /// Lists bounded Chat spaces visible to the linked work identity.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation or provider error.
    pub async fn list_spaces(
        &self,
        access_token: &SecretString,
    ) -> Result<Vec<GoogleChatSpaceEntry>, GoogleAuthError> {
        let token = validate_access_token(access_token)?;
        let mut spaces = Vec::new();
        let mut page_token: Option<String> = None;
        for _ in 0..MAX_CHAT_LIST_PAGES {
            let mut url = reqwest::Url::parse(GOOGLE_CHAT_SPACES_ENDPOINT)
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("pageSize", "100");
                if let Some(page_token) = page_token.as_deref() {
                    query.append_pair("pageToken", page_token);
                }
            }
            let response = self
                .client
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            if !response.status().is_success() {
                return Err(classify_provider_status(response.status().as_u16()));
            }
            if !is_json_response(&response) {
                return Err(GoogleAuthError::ProviderUnavailable);
            }
            let payload = bounded_body(response, MAX_CHAT_LIST_RESPONSE_BYTES).await?;
            let page: GoogleChatSpacePage =
                serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
            for space in page.spaces {
                spaces.push(normalize_chat_space(space)?);
                if spaces.len() > MAX_CHAT_ITEMS {
                    return Err(GoogleAuthError::ProviderRejected);
                }
            }
            page_token = page.next_page_token;
            if page_token.is_none() {
                return Ok(spaces);
            }
        }
        Err(GoogleAuthError::ProviderRejected)
    }

    /// Lists newly created public messages for one validated Chat space.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation or provider error.
    pub async fn list_messages(
        &self,
        access_token: &SecretString,
        space_name: &str,
        created_after: Option<OffsetDateTime>,
    ) -> Result<Vec<GoogleChatMessageEntry>, GoogleAuthError> {
        let token = validate_access_token(access_token)?;
        let space_id = validated_chat_space_id(space_name)?;
        let mut messages = Vec::new();
        let mut page_token: Option<String> = None;
        for _ in 0..MAX_CHAT_LIST_PAGES {
            let mut url = reqwest::Url::parse("https://chat.googleapis.com/v1")
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            url.path_segments_mut()
                .map_err(|()| GoogleAuthError::ProviderUnavailable)?
                .push("spaces")
                .push(space_id)
                .push("messages");
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("pageSize", "100");
                query.append_pair("orderBy", GOOGLE_CHAT_MESSAGE_ORDER);
                if let Some(created_after) = created_after {
                    let timestamp = created_after
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(|_| GoogleAuthError::InvalidRequest)?;
                    query.append_pair("filter", &format!("createTime > \"{timestamp}\""));
                }
                if let Some(page_token) = page_token.as_deref() {
                    query.append_pair("pageToken", page_token);
                }
            }
            let response = self
                .client
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
            if !response.status().is_success() {
                return Err(classify_provider_status(response.status().as_u16()));
            }
            if !is_json_response(&response) {
                return Err(GoogleAuthError::ProviderUnavailable);
            }
            let payload = bounded_body(response, MAX_CHAT_LIST_RESPONSE_BYTES).await?;
            let page: GoogleChatMessagePage =
                serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)?;
            for message in page.messages {
                if let Some(message) = normalize_chat_message(message)? {
                    messages.push(message);
                }
                if messages.len() > MAX_CHAT_ITEMS {
                    return Err(GoogleAuthError::ProviderRejected);
                }
            }
            page_token = page.next_page_token;
            if page_token.is_none() {
                return Ok(messages);
            }
        }
        Err(GoogleAuthError::ProviderRejected)
    }

    /// Adds the ingestion acknowledgement reaction after a message has been
    /// stored durably. Repeating the same reaction is treated as success.
    ///
    /// # Errors
    ///
    /// Returns a sanitized validation or provider error.
    pub async fn acknowledge_message(
        &self,
        access_token: &SecretString,
        message_name: &str,
    ) -> Result<(), GoogleAuthError> {
        let token = validate_access_token(access_token)?;
        let segments = validated_chat_message_segments(message_name)?;
        let mut url = reqwest::Url::parse("https://chat.googleapis.com/v1")
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        let mut path = url
            .path_segments_mut()
            .map_err(|()| GoogleAuthError::ProviderUnavailable)?;
        for segment in segments {
            path.push(segment);
        }
        path.push("reactions");
        drop(path);
        let response = self
            .client
            .post(url)
            .bearer_auth(token)
            .json(&GoogleChatReactionBody {
                emoji: GoogleChatEmoji { unicode: "👀" },
            })
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if response.status().is_success() || response.status().as_u16() == 409 {
            Ok(())
        } else {
            Err(classify_provider_status(response.status().as_u16()))
        }
    }
}

impl GoogleIdentityAdapter {
    /// Builds the adapter with fixed HTTPS endpoints and a no-redirect client.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::InvalidRequest`] for duplicate platform
    /// profiles or an unavailable HTTP client configuration.
    pub fn new(
        profiles: impl IntoIterator<Item = GoogleOAuthProfile>,
    ) -> Result<Self, GoogleAuthError> {
        let mut indexed = BTreeMap::new();
        for profile in profiles {
            if indexed
                .insert(profile.platform().as_str().to_owned(), profile)
                .is_some()
            {
                return Err(GoogleAuthError::InvalidRequest);
            }
        }
        if indexed.is_empty() {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        Ok(Self {
            client,
            profiles: indexed,
            jwks: Mutex::new(None),
        })
    }

    /// Exchanges a one-time authorization code and verifies Google's signed
    /// identity token against the configured platform profile.
    ///
    /// # Errors
    ///
    /// Returns a deliberately sanitized error; no provider response body,
    /// authorization code, or token is retained in the error.
    pub async fn exchange(
        &self,
        request: GoogleAuthorizationCode,
    ) -> Result<VerifiedGoogleIdentity, GoogleAuthError> {
        let profile = self.profile_for(request.platform)?;
        validate_exchange_request(&request, profile)?;
        let token_response = self.exchange_token(&request, profile).await?;
        self.verify_identity_token(&token_response.id_token, profile)
            .await
    }

    /// Builds an exact, server-owned consent URL for Calendar access. The
    /// caller supplies only fresh opaque state and PKCE material generated by
    /// the server; neither client ID nor callback URI can be overridden.
    ///
    /// # Errors
    ///
    /// Returns [`GoogleAuthError::InvalidRequest`] when the profile, state, or
    /// PKCE challenge is malformed.
    pub fn calendar_authorization_url(
        &self,
        platform: ClientPlatform,
        state: &str,
        code_challenge: &str,
        force_consent: bool,
    ) -> Result<String, GoogleAuthError> {
        self.authorization_url(
            platform,
            state,
            code_challenge,
            force_consent,
            "openid email https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/calendar.calendarlist.readonly",
            false,
        )
    }

    /// Builds consent for a distinct company Chat identity. `select_account`
    /// is always requested so the owner's personal Calendar login is never
    /// silently reused as a project workspace account.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the profile, state, or PKCE challenge is invalid.
    pub fn chat_authorization_url(
        &self,
        platform: ClientPlatform,
        state: &str,
        code_challenge: &str,
        force_consent: bool,
    ) -> Result<String, GoogleAuthError> {
        self.authorization_url(
            platform,
            state,
            code_challenge,
            force_consent,
            "openid email https://www.googleapis.com/auth/chat.spaces.readonly https://www.googleapis.com/auth/chat.messages.readonly https://www.googleapis.com/auth/chat.messages.reactions.create",
            true,
        )
    }

    /// Exchanges Calendar consent and returns the verified identity, granted
    /// scope set, and any newly issued refresh token to the server boundary.
    ///
    /// # Errors
    ///
    /// Returns a sanitized provider error without retaining authorization
    /// codes or tokens in logs or error text.
    pub async fn exchange_calendar(
        &self,
        request: GoogleAuthorizationCode,
    ) -> Result<GoogleCalendarGrant, GoogleAuthError> {
        let profile = self.profile_for(request.platform)?;
        validate_exchange_request(&request, profile)?;
        let token_response = self.exchange_token(&request, profile).await?;
        let identity = self
            .verify_identity_token(&token_response.id_token, profile)
            .await?;
        Ok(GoogleCalendarGrant {
            identity,
            refresh_token: token_response.refresh_token.map(SecretString::from),
            granted_scopes: parse_scopes(token_response.scope),
        })
    }

    /// Exchanges a company Chat authorization and verifies its Google identity.
    ///
    /// # Errors
    ///
    /// Returns a sanitized request, provider, or identity verification error.
    pub async fn exchange_chat(
        &self,
        request: GoogleAuthorizationCode,
    ) -> Result<GoogleChatGrant, GoogleAuthError> {
        let profile = self.profile_for(request.platform)?;
        validate_exchange_request(&request, profile)?;
        let token_response = self.exchange_token(&request, profile).await?;
        let identity = self
            .verify_identity_token(&token_response.id_token, profile)
            .await?;
        Ok(GoogleChatGrant {
            identity,
            refresh_token: token_response.refresh_token.map(SecretString::from),
            granted_scopes: parse_scopes(token_response.scope),
        })
    }

    fn authorization_url(
        &self,
        platform: ClientPlatform,
        state: &str,
        code_challenge: &str,
        force_consent: bool,
        scopes: &str,
        select_account: bool,
    ) -> Result<String, GoogleAuthError> {
        if !valid_url_safe_value(state, 128) || !valid_url_safe_value(code_challenge, 128) {
            return Err(GoogleAuthError::InvalidRequest);
        }
        let profile = self.profile_for(platform)?;
        let redirect_uri = profile
            .redirect_uris
            .first()
            .ok_or(GoogleAuthError::InvalidRequest)?;
        let mut url = reqwest::Url::parse(GOOGLE_AUTHORIZATION_ENDPOINT)
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("client_id", profile.client_id());
            query.append_pair("redirect_uri", redirect_uri);
            query.append_pair("response_type", "code");
            query.append_pair("scope", scopes);
            query.append_pair("state", state);
            query.append_pair("code_challenge", code_challenge);
            query.append_pair("code_challenge_method", "S256");
            query.append_pair("access_type", "offline");
            query.append_pair("include_granted_scopes", "true");
            let prompt = match (force_consent, select_account) {
                (true, true) => Some("consent select_account"),
                (true, false) => Some("consent"),
                (false, true) => Some("select_account"),
                (false, false) => None,
            };
            if let Some(prompt) = prompt {
                query.append_pair("prompt", prompt);
            }
        }
        Ok(url.into())
    }

    async fn exchange_token(
        &self,
        request: &GoogleAuthorizationCode,
        profile: &GoogleOAuthProfile,
    ) -> Result<GoogleTokenResponse, GoogleAuthError> {
        let response = self
            .client
            .post(GOOGLE_TOKEN_ENDPOINT)
            .form(&token_exchange_form(request, profile))
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(classify_provider_status(response.status().as_u16()));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let payload = bounded_body(response, MAX_TOKEN_RESPONSE_BYTES).await?;
        serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderRejected)
    }

    fn profile_for(
        &self,
        platform: ClientPlatform,
    ) -> Result<&GoogleOAuthProfile, GoogleAuthError> {
        self.profiles
            .get(platform.as_str())
            .ok_or(GoogleAuthError::InvalidRequest)
    }

    async fn verify_identity_token(
        &self,
        token: &str,
        profile: &GoogleOAuthProfile,
    ) -> Result<VerifiedGoogleIdentity, GoogleAuthError> {
        if token.is_empty() || token.len() > MAX_TOKEN_RESPONSE_BYTES {
            return Err(GoogleAuthError::IdentityRejected);
        }
        let header = decode_header(token).map_err(|_| GoogleAuthError::IdentityRejected)?;
        if header.alg != Algorithm::RS256 {
            return Err(GoogleAuthError::IdentityRejected);
        }
        let key_id = header.kid.ok_or(GoogleAuthError::IdentityRejected)?;
        let key = self.decoding_key(&key_id).await?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&GOOGLE_ISSUERS);
        validation.set_audience(&[profile.client_id()]);
        validation.required_spec_claims = ["exp", "iat", "iss", "aud", "sub"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        validation.leeway = 0;
        let claims = decode::<GoogleIdClaims>(token, &key, &validation)
            .map_err(|_| GoogleAuthError::IdentityRejected)?
            .claims;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if claims.iat > now + 60
            || claims.exp <= now
            || !GOOGLE_ISSUERS.contains(&claims.iss.as_str())
            || claims.aud != profile.client_id()
            || !claims.email_verified
            || claims
                .azp
                .as_deref()
                .is_some_and(|azp| azp != profile.client_id())
        {
            return Err(GoogleAuthError::IdentityRejected);
        }
        let subject =
            GoogleSubject::parse(claims.sub).map_err(|_| GoogleAuthError::IdentityRejected)?;
        let email =
            EmailAddress::parse(claims.email).map_err(|_| GoogleAuthError::IdentityRejected)?;
        VerifiedGoogleIdentity::new(subject, email, claims.name)
            .map_err(|_| GoogleAuthError::IdentityRejected)
    }

    async fn decoding_key(&self, key_id: &str) -> Result<DecodingKey, GoogleAuthError> {
        let mut cached = self.jwks.lock().await;
        let now = OffsetDateTime::now_utc();
        let stale_or_missing = cached
            .as_ref()
            .is_none_or(|entry| entry.expires_at <= now || entry.keys.find(key_id).is_none());
        if stale_or_missing {
            *cached = Some(self.fetch_jwks().await?);
        }
        let jwk = cached
            .as_ref()
            .and_then(|entry| entry.keys.find(key_id))
            .ok_or(GoogleAuthError::IdentityRejected)?;
        DecodingKey::from_jwk(jwk).map_err(|_| GoogleAuthError::IdentityRejected)
    }

    async fn fetch_jwks(&self) -> Result<CachedJwks, GoogleAuthError> {
        let response = self
            .client
            .get(GOOGLE_JWKS_ENDPOINT)
            .send()
            .await
            .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(classify_provider_status(response.status().as_u16()));
        }
        if !is_json_response(&response) {
            return Err(GoogleAuthError::ProviderUnavailable);
        }
        let cache_ttl = cache_ttl(&response);
        let payload = bounded_body(response, MAX_JWKS_RESPONSE_BYTES).await?;
        let keys =
            serde_json::from_slice(&payload).map_err(|_| GoogleAuthError::ProviderUnavailable)?;
        Ok(CachedJwks {
            keys,
            expires_at: OffsetDateTime::now_utc()
                + TimeDuration::try_from(cache_ttl)
                    .map_err(|_| GoogleAuthError::ProviderUnavailable)?,
        })
    }
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    id_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct GoogleRefreshTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarListPage {
    #[serde(default)]
    items: Vec<GoogleCalendarListItem>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The Google Calendar REST payload represents primary, selected, deleted, and hidden as independent provider flags."
)]
struct GoogleCalendarListItem {
    id: String,
    summary: Option<String>,
    description: Option<String>,
    time_zone: Option<String>,
    color_id: Option<String>,
    access_role: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    selected: bool,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    hidden: bool,
    etag: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarEventPage {
    #[serde(default)]
    items: Vec<GoogleCalendarEventItem>,
    next_page_token: Option<String>,
    next_sync_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarEventItem {
    id: String,
    etag: Option<String>,
    updated: Option<String>,
    i_cal_uid: Option<String>,
    status: String,
    event_type: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    start: Option<GoogleCalendarEventDateTime>,
    end: Option<GoogleCalendarEventDateTime>,
    recurrence: Option<Vec<String>>,
    recurring_event_id: Option<String>,
    visibility: Option<String>,
    transparency: Option<String>,
    html_link: Option<String>,
    #[serde(default)]
    guests_can_modify: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarEventDateTime {
    date: Option<String>,
    date_time: Option<String>,
    time_zone: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarMutationBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    start: GoogleCalendarMutationDateTime,
    end: GoogleCalendarMutationDateTime,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarMutationDateTime {
    date_time: String,
    time_zone: String,
}

#[derive(Deserialize)]
struct GoogleGmailMessageListResponse {
    #[serde(default)]
    messages: Vec<GoogleGmailMessageReference>,
}

#[derive(Deserialize)]
struct GoogleGmailMessageReference {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleGmailMessageResponse {
    id: String,
    thread_id: String,
    internal_date: Option<String>,
    label_ids: Option<Vec<String>>,
    snippet: Option<String>,
    payload: Option<GoogleGmailMessagePayload>,
}

#[derive(Deserialize)]
struct GoogleGmailMessagePayload {
    #[serde(default)]
    headers: Vec<GoogleGmailHeader>,
}

#[derive(Deserialize)]
struct GoogleGmailHeader {
    name: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleChatSpacePage {
    #[serde(default)]
    spaces: Vec<GoogleChatSpaceResource>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleChatSpaceResource {
    name: String,
    display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleChatMessagePage {
    #[serde(default)]
    messages: Vec<GoogleChatMessageResource>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleChatMessageResource {
    name: String,
    text: Option<String>,
    create_time: String,
    sender: Option<GoogleChatSender>,
    thread: Option<GoogleChatThread>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleChatSender {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct GoogleChatThread {
    name: Option<String>,
}

#[derive(Serialize)]
struct GoogleChatReactionBody<'a> {
    emoji: GoogleChatEmoji<'a>,
}

#[derive(Serialize)]
struct GoogleChatEmoji<'a> {
    unicode: &'a str,
}

#[derive(Deserialize)]
struct GoogleIdClaims {
    iss: String,
    aud: String,
    sub: String,
    azp: Option<String>,
    exp: i64,
    iat: i64,
    email: String,
    email_verified: bool,
    name: Option<String>,
}

fn validate_exchange_request(
    request: &GoogleAuthorizationCode,
    profile: &GoogleOAuthProfile,
) -> Result<(), GoogleAuthError> {
    let code = request.authorization_code.expose_secret();
    if code.is_empty()
        || code.len() > MAX_AUTHORIZATION_CODE_BYTES
        || code.chars().any(char::is_control)
        || !profile.accepts_redirect(&request.redirect_uri)
        || (profile.pkce_required && request.code_verifier.is_none())
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(())
}

fn token_exchange_form<'a>(
    request: &'a GoogleAuthorizationCode,
    profile: &'a GoogleOAuthProfile,
) -> Vec<(&'a str, &'a str)> {
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", request.authorization_code.expose_secret()),
        ("client_id", profile.client_id()),
        ("redirect_uri", &request.redirect_uri),
    ];
    if let Some(verifier) = &request.code_verifier {
        form.push(("code_verifier", verifier.expose_for_provider_exchange()));
    }
    if let Some(client_secret) = profile.client_secret() {
        form.push(("client_secret", client_secret.expose_secret()));
    }
    form
}

fn parse_scopes(scope: Option<String>) -> Vec<String> {
    let mut scopes = scope
        .unwrap_or_default()
        .split_ascii_whitespace()
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    scopes.sort();
    scopes.dedup();
    scopes
}

fn normalize_chat_space(
    resource: GoogleChatSpaceResource,
) -> Result<GoogleChatSpaceEntry, GoogleAuthError> {
    let name = validate_text(resource.name, 256)?;
    let _ = validated_chat_space_id(&name)?;
    let display_name = resource
        .display_name
        .map(|value| validate_text(value, 500))
        .transpose()?
        .unwrap_or_else(|| name.clone());
    Ok(GoogleChatSpaceEntry { name, display_name })
}

fn normalize_chat_message(
    resource: GoogleChatMessageResource,
) -> Result<Option<GoogleChatMessageEntry>, GoogleAuthError> {
    let text = match resource.text {
        Some(value) if !value.trim().is_empty() => validate_provider_free_text(value, 32_768)?,
        _ => return Ok(None),
    };
    let name = validate_text(resource.name, 1_024)?;
    let _ = validated_chat_message_segments(&name)?;
    let thread_name = resource
        .thread
        .and_then(|thread| thread.name)
        .map(|value| validate_text(value, 1_024))
        .transpose()?;
    let sender_name = resource
        .sender
        .and_then(|sender| sender.display_name)
        .map(|value| validate_text(value, 500))
        .transpose()?;
    let create_time = OffsetDateTime::parse(
        &resource.create_time,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| GoogleAuthError::ProviderRejected)?;
    Ok(Some(GoogleChatMessageEntry {
        name,
        thread_name,
        sender_name,
        text,
        create_time,
    }))
}

fn normalize_calendar_list_item(
    item: GoogleCalendarListItem,
) -> Result<GoogleCalendarListEntry, GoogleAuthError> {
    let provider_calendar_id = validate_text(item.id, 1_024)?;
    let name = item
        .summary
        .map(|value| validate_text(value, 500))
        .transpose()?
        .ok_or(GoogleAuthError::ProviderRejected)?;
    let description = item
        .description
        .map(|value| validate_provider_free_text(value, 8_192))
        .transpose()?;
    let time_zone = item
        .time_zone
        .map(|value| validate_text(value, 80))
        .transpose()?
        .ok_or(GoogleAuthError::ProviderRejected)?;
    let color_id = item
        .color_id
        .map(|value| validate_text(value, 120))
        .transpose()?;
    let access_role = match item.access_role.as_str() {
        "freeBusyReader" => "free_busy_reader",
        "reader" | "writer" | "owner" => item.access_role.as_str(),
        _ => return Err(GoogleAuthError::ProviderRejected),
    }
    .to_owned();
    let provider_etag = item
        .etag
        .map(|value| validate_text(value, 2_048))
        .transpose()?;
    Ok(GoogleCalendarListEntry {
        provider_calendar_id,
        name,
        description,
        time_zone,
        color_id,
        access_role,
        is_primary: item.primary,
        provider_selected: item.selected,
        visibility: if item.deleted {
            GoogleCalendarVisibility::Deleted
        } else if item.hidden {
            GoogleCalendarVisibility::Hidden
        } else {
            GoogleCalendarVisibility::Visible
        },
        provider_etag,
    })
}

fn normalize_calendar_event_item(
    item: GoogleCalendarEventItem,
    calendar_time_zone: &str,
) -> Result<GoogleCalendarEventEntry, GoogleAuthError> {
    let provider_event_id = validate_text(item.id, 1_024)?;
    let status = match item.status.as_str() {
        "confirmed" => GoogleCalendarEventStatus::Confirmed,
        "tentative" => GoogleCalendarEventStatus::Tentative,
        "cancelled" => GoogleCalendarEventStatus::Cancelled,
        _ => return Err(GoogleAuthError::ProviderRejected),
    };
    let event_type = match item.event_type.as_deref().unwrap_or("default") {
        "default" => "default",
        "birthday" => "birthday",
        "focusTime" => "focus_time",
        "fromGmail" => "from_gmail",
        "outOfOffice" => "out_of_office",
        "workingLocation" => "working_location",
        _ => return Err(GoogleAuthError::ProviderRejected),
    }
    .to_owned();
    let provider_etag = item
        .etag
        .map(|value| validate_text(value, 2_048))
        .transpose()?;
    let provider_updated_at = item
        .updated
        .map(|value| OffsetDateTime::parse(&value, &time::format_description::well_known::Rfc3339))
        .transpose()
        .map_err(|_| GoogleAuthError::ProviderRejected)?;
    let ical_uid = item
        .i_cal_uid
        .map(|value| validate_text(value, 2_048))
        .transpose()?;
    let title = item
        .summary
        .map(|value| validate_text(value, 300))
        .transpose()?;
    if status != GoogleCalendarEventStatus::Cancelled && title.is_none() {
        return Err(GoogleAuthError::ProviderRejected);
    }
    let description = item
        .description
        .map(|value| validate_provider_free_text(value, 8_192))
        .transpose()?;
    let location = item
        .location
        .map(|value| validate_provider_free_text(value, 1_024))
        .transpose()?;
    let recurrence = item
        .recurrence
        .map(|rules| {
            if rules.len() > MAX_RECURRENCE_RULES {
                return Err(GoogleAuthError::ProviderRejected);
            }
            rules
                .into_iter()
                .map(|rule| validate_text(rule, 1_024))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let recurring_provider_event_id = item
        .recurring_event_id
        .map(|value| validate_text(value, 1_024))
        .transpose()?;
    let visibility = item
        .visibility
        .map(|value| match value.as_str() {
            "default" | "public" | "private" | "confidential" => Ok(value),
            _ => Err(GoogleAuthError::ProviderRejected),
        })
        .transpose()?;
    let transparency = item
        .transparency
        .map(|value| match value.as_str() {
            "opaque" | "transparent" => Ok(value),
            _ => Err(GoogleAuthError::ProviderRejected),
        })
        .transpose()?;
    let html_link = item
        .html_link
        .map(|value| validate_https_url(value, 4_096))
        .transpose()?;
    let time = normalize_event_time(item.start, item.end, calendar_time_zone, status)?;
    Ok(GoogleCalendarEventEntry {
        provider_event_id,
        provider_etag,
        provider_updated_at,
        ical_uid,
        status,
        event_type,
        title,
        description,
        location,
        time,
        recurrence,
        recurring_provider_event_id,
        visibility,
        transparency,
        html_link,
        is_editable: item.guests_can_modify,
    })
}

fn validate_access_token(token: &SecretString) -> Result<&str, GoogleAuthError> {
    let value = token.expose_secret();
    if value.is_empty()
        || value.len() > MAX_TOKEN_RESPONSE_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(value)
}

fn validate_google_event_id(value: &str) -> Result<String, GoogleAuthError> {
    if !(5..=1_024).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'v'))
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(value.to_owned())
}

fn event_matches_mutation(
    event: &GoogleCalendarEventEntry,
    mutation: &GoogleCalendarEventMutation,
) -> bool {
    let Some(GoogleCalendarEventTime::DateTime {
        start,
        end,
        time_zone,
    }) = event.time.as_ref()
    else {
        return false;
    };
    event.status != GoogleCalendarEventStatus::Cancelled
        && event.title.as_deref() == Some(mutation.title.as_str())
        && event.description.as_deref() == mutation.description.as_deref()
        && *start == mutation.start
        && *end == mutation.end
        && time_zone == &mutation.time_zone
}

fn validated_event_mutation_request<'a>(
    access_token: &'a SecretString,
    provider_calendar_id: &str,
    provider_event_id: &str,
    provider_etag: Option<&str>,
    mutation: &GoogleCalendarEventMutation,
) -> Result<
    (
        &'a str,
        String,
        String,
        Option<String>,
        GoogleCalendarMutationBody,
    ),
    GoogleAuthError,
> {
    let token = validate_access_token(access_token)?;
    let calendar_id = validate_text(provider_calendar_id.to_owned(), 1_024)?;
    let event_id = validate_text(provider_event_id.to_owned(), 1_024)?;
    let etag = provider_etag
        .map(|value| validate_text(value.to_owned(), 2_048))
        .transpose()?;
    let body = validated_event_mutation_body(mutation)?;
    Ok((token, calendar_id, event_id, etag, body))
}

fn validated_event_mutation_body(
    mutation: &GoogleCalendarEventMutation,
) -> Result<GoogleCalendarMutationBody, GoogleAuthError> {
    let title = validate_text(mutation.title.clone(), 300)?;
    let description = mutation
        .description
        .clone()
        .map(|value| validate_provider_free_text(value, 8_192))
        .transpose()?;
    let time_zone = validate_text(mutation.time_zone.clone(), 80)?;
    if mutation.end <= mutation.start {
        return Err(GoogleAuthError::InvalidRequest);
    }
    let start = mutation
        .start
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| GoogleAuthError::InvalidRequest)?;
    let end = mutation
        .end
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| GoogleAuthError::InvalidRequest)?;
    Ok(GoogleCalendarMutationBody {
        id: None,
        summary: title,
        description,
        start: GoogleCalendarMutationDateTime {
            date_time: start,
            time_zone: time_zone.clone(),
        },
        end: GoogleCalendarMutationDateTime {
            date_time: end,
            time_zone,
        },
    })
}

fn normalize_gmail_message(
    message: GoogleGmailMessageResponse,
    expected_message_id: &str,
) -> Result<GoogleGmailMessageEntry, GoogleAuthError> {
    let provider_message_id = validate_text(message.id, 255)?;
    if provider_message_id != expected_message_id {
        return Err(GoogleAuthError::ProviderRejected);
    }
    let provider_thread_id = validate_text(message.thread_id, 255)?;
    let headers = message
        .payload
        .map_or_else(Vec::new, |payload| payload.headers);
    let sender = gmail_header_value(&headers, "from");
    let subject = gmail_header_value(&headers, "subject");
    let snippet = message
        .snippet
        .as_deref()
        .and_then(|value| normalize_gmail_text(value, 512));
    let received_at = message
        .internal_date
        .and_then(|value| parse_gmail_internal_date(&value));
    let is_unread = message
        .label_ids
        .as_deref()
        .is_some_and(|labels| labels.iter().any(|label| label == "UNREAD"));
    Ok(GoogleGmailMessageEntry {
        provider_message_id,
        provider_thread_id,
        received_at,
        sender,
        subject,
        snippet,
        is_unread,
    })
}

fn gmail_header_value(headers: &[GoogleGmailHeader], expected_name: &str) -> Option<String> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(expected_name))
        .and_then(|header| normalize_gmail_text(&header.value, 1_024))
}

fn normalize_gmail_text(value: &str, maximum_bytes: usize) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty() && normalized.len() <= maximum_bytes).then_some(normalized)
}

fn parse_gmail_internal_date(value: &str) -> Option<OffsetDateTime> {
    let milliseconds = value.parse::<i64>().ok()?;
    OffsetDateTime::from_unix_timestamp(milliseconds / 1_000).ok()
}

fn normalize_event_time(
    start: Option<GoogleCalendarEventDateTime>,
    end: Option<GoogleCalendarEventDateTime>,
    calendar_time_zone: &str,
    status: GoogleCalendarEventStatus,
) -> Result<Option<GoogleCalendarEventTime>, GoogleAuthError> {
    let (Some(start), Some(end)) = (start, end) else {
        return if status == GoogleCalendarEventStatus::Cancelled {
            Ok(None)
        } else {
            Err(GoogleAuthError::ProviderRejected)
        };
    };
    let source_time_zone = start.time_zone.clone().or(end.time_zone.clone());
    match (start.date, end.date, start.date_time, end.date_time) {
        (Some(start), Some(end), None, None) => {
            let start = parse_google_date(&start)?;
            let end = parse_google_date(&end)?;
            if end <= start {
                return Err(GoogleAuthError::ProviderRejected);
            }
            Ok(Some(GoogleCalendarEventTime::Date { start, end }))
        }
        (None, None, Some(start), Some(end)) => {
            let start =
                OffsetDateTime::parse(&start, &time::format_description::well_known::Rfc3339)
                    .map_err(|_| GoogleAuthError::ProviderRejected)?;
            let end = OffsetDateTime::parse(&end, &time::format_description::well_known::Rfc3339)
                .map_err(|_| GoogleAuthError::ProviderRejected)?;
            if end <= start {
                return Err(GoogleAuthError::ProviderRejected);
            }
            let time_zone = source_time_zone.unwrap_or_else(|| calendar_time_zone.to_owned());
            Ok(Some(GoogleCalendarEventTime::DateTime {
                start,
                end,
                time_zone: validate_text(time_zone, 80)?,
            }))
        }
        _ => Err(GoogleAuthError::ProviderRejected),
    }
}

fn parse_google_date(value: &str) -> Result<Date, GoogleAuthError> {
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(GoogleAuthError::ProviderRejected);
    };
    let year = year
        .parse::<i32>()
        .map_err(|_| GoogleAuthError::ProviderRejected)?;
    let month = month
        .parse::<u8>()
        .map_err(|_| GoogleAuthError::ProviderRejected)
        .and_then(|number| {
            Month::try_from(number).map_err(|_| GoogleAuthError::ProviderRejected)
        })?;
    let day = day
        .parse::<u8>()
        .map_err(|_| GoogleAuthError::ProviderRejected)?;
    Date::from_calendar_date(year, month, day).map_err(|_| GoogleAuthError::ProviderRejected)
}

fn validate_https_url(value: String, maximum_bytes: usize) -> Result<String, GoogleAuthError> {
    let value = validate_text(value, maximum_bytes)?;
    let url = reqwest::Url::parse(&value).map_err(|_| GoogleAuthError::ProviderRejected)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
    {
        return Err(GoogleAuthError::ProviderRejected);
    }
    Ok(value)
}

fn valid_url_safe_value(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
}

async fn bounded_body(
    response: Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, GoogleAuthError> {
    if response
        .content_length()
        .is_some_and(|size| size > maximum_bytes as u64)
    {
        return Err(GoogleAuthError::ProviderUnavailable);
    }
    let body = response
        .bytes()
        .await
        .map_err(|_| GoogleAuthError::ProviderUnavailable)?;
    if body.len() > maximum_bytes {
        return Err(GoogleAuthError::ProviderUnavailable);
    }
    Ok(body.to_vec())
}

fn cache_ttl(response: &Response) -> Duration {
    let Some(header) = response
        .headers()
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
    else {
        return DEFAULT_JWKS_TTL;
    };
    let seconds = header.split(',').find_map(|directive| {
        directive
            .trim()
            .strip_prefix("max-age=")
            .and_then(|value| value.parse::<u64>().ok())
    });
    seconds
        .map_or(DEFAULT_JWKS_TTL, Duration::from_secs)
        .min(MAX_JWKS_TTL)
}

fn is_json_response(response: &Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim() == "application/json")
        })
}

fn validate_redirect_uri(value: String) -> Result<String, GoogleAuthError> {
    let value = validate_text(value, 2_048)?;
    if !(value.starts_with("https://") || value.contains("://") || value.contains(':')) {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(value)
}

fn validate_text(value: String, maximum_bytes: usize) -> Result<String, GoogleAuthError> {
    if value.trim().is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control)
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(value)
}

fn validate_provider_free_text(
    value: String,
    maximum_bytes: usize,
) -> Result<String, GoogleAuthError> {
    if value.trim().is_empty()
        || value.len() > maximum_bytes
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(value)
}

fn validated_chat_space_id(space_name: &str) -> Result<&str, GoogleAuthError> {
    let Some(space_id) = space_name.strip_prefix("spaces/") else {
        return Err(GoogleAuthError::InvalidRequest);
    };
    if space_id.is_empty()
        || space_id.len() > 240
        || !space_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok(space_id)
}

fn validated_chat_message_segments(message_name: &str) -> Result<[&str; 4], GoogleAuthError> {
    let segments = message_name.split('/').collect::<Vec<_>>();
    let [spaces, space_id, messages, message_id] = segments.as_slice() else {
        return Err(GoogleAuthError::InvalidRequest);
    };
    if *spaces != "spaces"
        || *messages != "messages"
        || validated_chat_space_id(&format!("spaces/{space_id}")).is_err()
        || message_id.is_empty()
        || message_id.len() > 240
        || matches!(*message_id, "." | "..")
        || !message_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(GoogleAuthError::InvalidRequest);
    }
    Ok([spaces, space_id, messages, message_id])
}

fn classify_provider_status(status: u16) -> GoogleAuthError {
    if status == 429 || status >= 500 {
        GoogleAuthError::ProviderUnavailable
    } else {
        GoogleAuthError::ProviderRejected
    }
}

fn classify_calendar_event_status(status: u16, incremental: bool) -> Option<GoogleAuthError> {
    if (200..300).contains(&status) {
        None
    } else if status == 410 && incremental {
        Some(GoogleAuthError::CalendarSyncTokenExpired)
    } else {
        Some(classify_provider_status(status))
    }
}

fn classify_calendar_mutation_status(status: u16) -> GoogleAuthError {
    match status {
        400 => GoogleAuthError::CalendarEventRejected,
        404 => GoogleAuthError::CalendarEventNotFound,
        409 | 412 => GoogleAuthError::CalendarEventConflict,
        _ => classify_provider_status(status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_authorization_requests_calendar_scopes_without_gmail() {
        let profile = GoogleOAuthProfile::new_with_client_secret(
            ClientPlatform::Android,
            "calendar-client-id",
            SecretString::from("calendar-client-secret"),
            ["https://os.jimin.ai.kr/oauth/google/calendar/callback".to_owned()],
            true,
        )
        .expect("test OAuth profile should be valid");
        let adapter = GoogleIdentityAdapter::new([profile])
            .expect("test Google identity adapter should build");

        let authorization_url = adapter
            .calendar_authorization_url(
                ClientPlatform::Android,
                "state-value",
                "challenge-value",
                false,
            )
            .expect("authorization URL should be generated");
        let url = reqwest::Url::parse(&authorization_url).expect("authorization URL should parse");
        let scope = url
            .query_pairs()
            .find_map(|(key, value)| (key == "scope").then(|| value.into_owned()))
            .expect("scope query parameter should exist");

        assert!(scope.contains("https://www.googleapis.com/auth/calendar.events"));
        assert!(scope.contains("https://www.googleapis.com/auth/calendar.calendarlist.readonly"));
        assert!(!scope.contains("https://www.googleapis.com/auth/gmail.readonly"));
    }

    #[test]
    fn chat_authorization_requests_work_scopes_and_account_selection() {
        let profile = GoogleOAuthProfile::new_with_client_secret(
            ClientPlatform::Android,
            "chat-client-id",
            SecretString::from("chat-client-secret"),
            ["https://os.jimin.ai.kr/oauth/google/calendar/callback".to_owned()],
            true,
        )
        .expect("test OAuth profile should be valid");
        let adapter = GoogleIdentityAdapter::new([profile])
            .expect("test Google identity adapter should build");

        let authorization_url = adapter
            .chat_authorization_url(
                ClientPlatform::Android,
                "state-value",
                "challenge-value",
                true,
            )
            .expect("Chat authorization URL should be generated");
        let url = reqwest::Url::parse(&authorization_url).expect("authorization URL should parse");
        let query = url.query_pairs().collect::<BTreeMap<_, _>>();
        let scope = query
            .get("scope")
            .expect("scope query parameter should exist");

        assert!(scope.contains("https://www.googleapis.com/auth/chat.spaces.readonly"));
        assert!(scope.contains("https://www.googleapis.com/auth/chat.messages.readonly"));
        assert!(scope.contains("https://www.googleapis.com/auth/chat.messages.reactions.create"));
        assert_eq!(
            query.get("prompt").map(AsRef::as_ref),
            Some("consent select_account")
        );
    }

    #[test]
    fn chat_message_order_matches_google_api_contract() {
        assert_eq!(GOOGLE_CHAT_MESSAGE_ORDER, "createTime ASC");
    }

    #[test]
    fn chat_message_names_accept_google_system_ids_without_path_traversal() {
        assert!(
            validated_chat_message_segments("spaces/AAAAAAAAAAA/messages/BBBBBBBBBBB.BBBBBBBBBBB")
                .is_ok()
        );
        assert!(validated_chat_message_segments("spaces/AAAAAAAAAAA/messages/.").is_err());
        assert!(validated_chat_message_segments("spaces/AAAAAAAAAAA/messages/..").is_err());
    }

    #[test]
    fn calendar_event_status_requires_full_reset_only_for_incremental_http_410() {
        assert_eq!(
            classify_calendar_event_status(410, true),
            Some(GoogleAuthError::CalendarSyncTokenExpired)
        );
        assert_eq!(
            classify_calendar_event_status(410, false),
            Some(GoogleAuthError::ProviderRejected)
        );
        assert_eq!(classify_calendar_event_status(200, true), None);
        assert_eq!(
            classify_calendar_event_status(503, true),
            Some(GoogleAuthError::ProviderUnavailable)
        );
    }

    #[test]
    fn calendar_event_page_captures_the_terminal_sync_token() {
        let page: GoogleCalendarEventPage =
            serde_json::from_str(r#"{"items":[],"nextSyncToken":"opaque-sync-token"}"#)
                .expect("valid Calendar event page");

        assert!(page.next_page_token.is_none());
        assert_eq!(page.next_sync_token.as_deref(), Some("opaque-sync-token"));
    }

    #[test]
    fn profile_requires_unique_redirects_and_exact_matching() {
        let profile = GoogleOAuthProfile::new(
            ClientPlatform::Macos,
            "client-id",
            ["jimin-os://oauth".to_owned(), "jimin-os://oauth".to_owned()],
            true,
        )
        .expect("profile should be valid");

        assert!(profile.accepts_redirect("jimin-os://oauth"));
        assert!(!profile.accepts_redirect("jimin-os://other"));
    }

    #[test]
    fn cache_control_ttl_is_bounded() {
        assert_eq!(MAX_JWKS_TTL, Duration::from_hours(24));
    }

    #[test]
    fn calendar_list_entry_normalizes_visibility_and_access_role() {
        let item: GoogleCalendarListItem = serde_json::from_str(
            r#"{
                "id": "primary",
                "summary": "Personal",
                "timeZone": "Asia/Seoul",
                "accessRole": "owner",
                "primary": true,
                "selected": true,
                "hidden": true
            }"#,
        )
        .expect("fixture should deserialize");

        let entry = normalize_calendar_list_item(item).expect("entry should normalize");

        assert_eq!(entry.access_role, "owner");
        assert_eq!(entry.visibility, GoogleCalendarVisibility::Hidden);
    }

    #[test]
    fn calendar_event_normalizes_timed_event_and_google_type() {
        let item: GoogleCalendarEventItem = serde_json::from_str(
            r#"{
                "id": "event-1",
                "status": "confirmed",
                "eventType": "focusTime",
                "summary": "집중 시간",
                "start": {"dateTime": "2026-07-12T09:00:00+09:00"},
                "end": {"dateTime": "2026-07-12T10:00:00+09:00"}
            }"#,
        )
        .expect("fixture should deserialize");

        let entry =
            normalize_calendar_event_item(item, "Asia/Seoul").expect("event should normalize");

        assert_eq!(entry.event_type, "focus_time");
        assert_eq!(entry.status, GoogleCalendarEventStatus::Confirmed);
        assert!(matches!(
            entry.time,
            Some(GoogleCalendarEventTime::DateTime { time_zone, .. }) if time_zone == "Asia/Seoul"
        ));
    }

    #[test]
    fn calendar_event_accepts_multiline_provider_description() {
        let item: GoogleCalendarEventItem = serde_json::from_str(
            r#"{
                "id": "event-1",
                "status": "confirmed",
                "summary": "회의",
                "description": "첫 번째 안건\n두 번째 안건\r\n\t확인할 내용",
                "location": "회의실 A\n3층",
                "start": {"dateTime": "2026-07-12T09:00:00+09:00"},
                "end": {"dateTime": "2026-07-12T10:00:00+09:00"}
            }"#,
        )
        .expect("fixture should deserialize");

        let entry =
            normalize_calendar_event_item(item, "Asia/Seoul").expect("event should normalize");

        assert_eq!(
            entry.description.as_deref(),
            Some("첫 번째 안건\n두 번째 안건\r\n\t확인할 내용")
        );
        assert_eq!(entry.location.as_deref(), Some("회의실 A\n3층"));
    }

    #[test]
    fn cancelled_calendar_event_can_omit_stale_fields() {
        let item: GoogleCalendarEventItem =
            serde_json::from_str(r#"{"id": "event-1", "status": "cancelled"}"#)
                .expect("fixture should deserialize");

        let entry =
            normalize_calendar_event_item(item, "Asia/Seoul").expect("tombstone should normalize");

        assert_eq!(entry.status, GoogleCalendarEventStatus::Cancelled);
        assert!(entry.time.is_none());
        assert!(entry.title.is_none());
    }

    #[test]
    fn gmail_metadata_discards_header_controls_and_marks_unread() {
        let message: GoogleGmailMessageResponse = serde_json::from_str(
            r#"{
                "id": "message-1",
                "threadId": "thread-1",
                "internalDate": "1780000000000",
                "labelIds": ["INBOX", "UNREAD"],
                "snippet": "일정을 확인해 주세요.",
                "payload": {
                    "headers": [
                        {"name": "From", "value": "Jimin <jimin@example.com>"},
                        {"name": "Subject", "value": "내일 회의"}
                    ]
                }
            }"#,
        )
        .expect("fixture should deserialize");

        let entry =
            normalize_gmail_message(message, "message-1").expect("metadata should normalize");

        assert_eq!(entry.subject.as_deref(), Some("내일 회의"));
        assert!(entry.is_unread);
        assert!(entry.received_at.is_some());
    }

    #[test]
    fn calendar_mutation_classifies_stale_etags_as_conflicts() {
        assert_eq!(
            classify_calendar_mutation_status(400),
            GoogleAuthError::CalendarEventRejected
        );
        assert_eq!(
            classify_calendar_mutation_status(404),
            GoogleAuthError::CalendarEventNotFound
        );
        assert_eq!(
            classify_calendar_mutation_status(412),
            GoogleAuthError::CalendarEventConflict
        );
        assert_eq!(
            classify_calendar_mutation_status(409),
            GoogleAuthError::CalendarEventConflict
        );
        assert_eq!(
            classify_calendar_mutation_status(503),
            GoogleAuthError::ProviderUnavailable
        );
    }

    #[test]
    fn deterministic_calendar_event_ids_use_google_base32hex_alphabet() {
        assert_eq!(
            validate_google_event_id("jos019b1234abcdef0123456789abcdef0")
                .expect("generated ID should be valid"),
            "jos019b1234abcdef0123456789abcdef0"
        );
        assert_eq!(
            validate_google_event_id("contains-w").expect_err("w is outside Google's alphabet"),
            GoogleAuthError::InvalidRequest
        );
    }

    #[test]
    fn provider_event_matches_only_the_retried_desired_state() {
        let starts_at =
            OffsetDateTime::from_unix_timestamp(1_800_000_000).expect("timestamp should be valid");
        let mutation = GoogleCalendarEventMutation {
            title: "회의".to_owned(),
            description: Some("준비".to_owned()),
            start: starts_at,
            end: starts_at + TimeDuration::hours(1),
            time_zone: "Asia/Seoul".to_owned(),
        };
        let event = GoogleCalendarEventEntry {
            provider_event_id: "jos019b1234abcdef0123456789abcdef0".to_owned(),
            provider_etag: Some("etag".to_owned()),
            provider_updated_at: None,
            ical_uid: None,
            status: GoogleCalendarEventStatus::Confirmed,
            event_type: "default".to_owned(),
            title: Some("회의".to_owned()),
            description: Some("준비".to_owned()),
            location: None,
            time: Some(GoogleCalendarEventTime::DateTime {
                start: mutation.start,
                end: mutation.end,
                time_zone: mutation.time_zone.clone(),
            }),
            recurrence: None,
            recurring_provider_event_id: None,
            visibility: None,
            transparency: None,
            html_link: None,
            is_editable: true,
        };
        assert!(event_matches_mutation(&event, &mutation));
        let changed = GoogleCalendarEventMutation {
            title: "다른 회의".to_owned(),
            ..mutation
        };
        assert!(!event_matches_mutation(&event, &changed));
    }

    #[tokio::test]
    async fn calendar_revocation_rejects_an_empty_credential_before_network_io() {
        let adapter =
            GoogleCalendarAdapter::new("calendar-client", SecretString::from("calendar-secret"))
                .expect("adapter should be valid");

        assert_eq!(
            adapter
                .revoke_refresh_token(&SecretString::from(String::new()))
                .await,
            Err(GoogleAuthError::InvalidRequest)
        );
    }
}

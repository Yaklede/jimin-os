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
    header::{CACHE_CONTROL, CONTENT_TYPE},
    redirect::Policy,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use thiserror::Error;
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::Mutex;

const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_JWKS_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];
const MAX_AUTHORIZATION_CODE_BYTES: usize = 4 * 1024;
const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_JWKS_RESPONSE_BYTES: usize = 256 * 1024;
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
            query.append_pair(
                "scope",
                "openid email https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/calendar.calendarlist.readonly",
            );
            query.append_pair("state", state);
            query.append_pair("code_challenge", code_challenge);
            query.append_pair("code_challenge_method", "S256");
            query.append_pair("access_type", "offline");
            query.append_pair("include_granted_scopes", "true");
            if force_consent {
                query.append_pair("prompt", "consent");
            }
        }
        Ok(url.into())
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

fn classify_provider_status(status: u16) -> GoogleAuthError {
    if status == 429 || status >= 500 {
        GoogleAuthError::ProviderUnavailable
    } else {
        GoogleAuthError::ProviderRejected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

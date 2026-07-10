//! Authentication primitives for Jimin OS server-owned sessions.
//!
//! This crate deliberately keeps Google OAuth and database operations outside
//! of the token implementation. It issues short-lived Ed25519 access tokens
//! and derives a non-reversible verifier for device refresh tokens.

use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, decode_header, encode,
};
use rand::Rng;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use uuid::Uuid;

const ACCESS_TOKEN_AUDIENCE: &str = "jimin-os";
const MAX_ACCESS_TOKEN_BYTES: usize = 16 * 1024;
const REFRESH_TOKEN_PREFIX: &str = "josr_";
const REFRESH_TOKEN_SECRET_BYTES: usize = 32;
const MINIMUM_PEPPER_BYTES: usize = 32;
const MAX_ACCESS_TOKEN_TTL: Duration = Duration::from_mins(15);

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    #[error("access-token configuration is invalid")]
    InvalidAccessTokenConfiguration,
    #[error("access-token key material is invalid")]
    InvalidAccessTokenKey,
    #[error("access-token credentials are invalid")]
    InvalidAccessTokenCredentials,
    #[error("access token is invalid")]
    InvalidAccessToken,
    #[error("refresh-token configuration is invalid")]
    InvalidRefreshTokenConfiguration,
    #[error("refresh token is invalid")]
    InvalidRefreshToken,
    #[error("the system clock cannot produce a token timestamp")]
    InvalidSystemTime,
}

/// Server-owned settings for Jimin OS access tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessTokenSettings {
    issuer: String,
    key_id: String,
    ttl: Duration,
}

impl AccessTokenSettings {
    /// Creates bounded settings that are safe for access-token issuance and
    /// verification.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidAccessTokenConfiguration`] for a blank
    /// issuer/key ID or a TTL outside the short-lived token policy.
    pub fn new(
        issuer: impl Into<String>,
        key_id: impl Into<String>,
        ttl: Duration,
    ) -> Result<Self, AuthError> {
        let issuer = validate_identifier(issuer.into())?;
        let key_id = validate_identifier(key_id.into())?;
        if ttl.is_zero() || ttl > MAX_ACCESS_TOKEN_TTL {
            return Err(AuthError::InvalidAccessTokenConfiguration);
        }
        Ok(Self {
            issuer,
            key_id,
            ttl,
        })
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub const fn ttl(&self) -> Duration {
        self.ttl
    }
}

/// Internal IDs bound into a signed access token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIdentity {
    user: Uuid,
    session: Uuid,
    device: Uuid,
    token: Uuid,
}

impl SessionIdentity {
    /// Creates the identity for a single access-token issuance.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidAccessTokenCredentials`] when the token ID
    /// is not `UUIDv7`. Existing persisted IDs are parsed again by the route
    /// guard, so user/session/device IDs may be any valid UUID.
    pub fn new(
        user_id: Uuid,
        session_id: Uuid,
        device_id: Uuid,
        token_id: Uuid,
    ) -> Result<Self, AuthError> {
        if token_id.get_version_num() != 7 {
            return Err(AuthError::InvalidAccessTokenCredentials);
        }
        Ok(Self {
            user: user_id,
            session: session_id,
            device: device_id,
            token: token_id,
        })
    }

    #[must_use]
    pub const fn user_id(&self) -> Uuid {
        self.user
    }

    #[must_use]
    pub const fn session_id(&self) -> Uuid {
        self.session
    }

    #[must_use]
    pub const fn device_id(&self) -> Uuid {
        self.device
    }

    #[must_use]
    pub const fn token_id(&self) -> Uuid {
        self.token
    }
}

/// A signed access token and its unambiguous expiry time.
pub struct IssuedAccessToken {
    token: SecretString,
    expires_at: SystemTime,
}

impl IssuedAccessToken {
    #[must_use]
    pub const fn token(&self) -> &SecretString {
        &self.token
    }

    #[must_use]
    pub const fn expires_at(&self) -> SystemTime {
        self.expires_at
    }
}

/// Creates Ed25519 signed access tokens from a mounted PKCS#8 private key.
pub struct AccessTokenIssuer {
    settings: AccessTokenSettings,
    key: EncodingKey,
}

impl AccessTokenIssuer {
    /// Loads an Ed25519 private key without retaining it in normal logs.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidAccessTokenKey`] for an unusable mounted
    /// key value.
    pub fn from_ed25519_pem(
        settings: AccessTokenSettings,
        private_key_pem: &SecretString,
    ) -> Result<Self, AuthError> {
        let key = EncodingKey::from_ed_pem(private_key_pem.expose_secret().as_bytes())
            .map_err(|_| AuthError::InvalidAccessTokenKey)?;
        Ok(Self { settings, key })
    }

    /// Signs a short-lived token for an active user/device session.
    ///
    /// # Errors
    ///
    /// Returns an error when system time cannot be represented as JWT numeric
    /// dates or signing unexpectedly fails.
    pub fn issue(
        &self,
        identity: SessionIdentity,
        now: SystemTime,
    ) -> Result<IssuedAccessToken, AuthError> {
        let issued_at = unix_timestamp(now)?;
        let expires_at = now
            .checked_add(self.settings.ttl())
            .ok_or(AuthError::InvalidSystemTime)?;
        let claims = AccessTokenClaims {
            iss: self.settings.issuer().to_owned(),
            aud: ACCESS_TOKEN_AUDIENCE.to_owned(),
            sub: identity.user_id().to_string(),
            sid: identity.session_id().to_string(),
            did: identity.device_id().to_string(),
            jti: identity.token_id().to_string(),
            iat: issued_at,
            nbf: issued_at,
            exp: unix_timestamp(expires_at)?,
        };
        let header = Header {
            alg: Algorithm::EdDSA,
            kid: Some(self.settings.key_id().to_owned()),
            ..Header::default()
        };
        let token =
            encode(&header, &claims, &self.key).map_err(|_| AuthError::InvalidAccessToken)?;
        Ok(IssuedAccessToken {
            token: SecretString::from(token),
            expires_at,
        })
    }
}

/// Verifies access tokens against one or more public keys to support safe key
/// rotation. The issuer and audience remain server-owned configuration.
pub struct AccessTokenVerifier {
    issuer: String,
    keys: BTreeMap<String, DecodingKey>,
}

impl AccessTokenVerifier {
    /// Builds a verifier from public Ed25519 keys selected by an exact `kid`.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidAccessTokenKey`] for invalid PEM material,
    /// duplicate key IDs, or an empty key set.
    pub fn from_ed25519_pems(
        issuer: impl Into<String>,
        keys: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, AuthError> {
        let issuer = validate_identifier(issuer.into())?;
        let mut parsed_keys = BTreeMap::new();
        for (key_id, pem) in keys {
            let key_id = validate_identifier(key_id)?;
            let key = DecodingKey::from_ed_pem(pem.as_bytes())
                .map_err(|_| AuthError::InvalidAccessTokenKey)?;
            if parsed_keys.insert(key_id, key).is_some() {
                return Err(AuthError::InvalidAccessTokenKey);
            }
        }
        if parsed_keys.is_empty() {
            return Err(AuthError::InvalidAccessTokenKey);
        }
        Ok(Self {
            issuer,
            keys: parsed_keys,
        })
    }

    /// Verifies an `EdDSA` token and returns only the internal session identity.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidAccessToken`] for malformed, expired,
    /// wrong-audience, wrong-issuer, missing-key, or invalid-claim tokens.
    pub fn verify(&self, token: &str) -> Result<SessionIdentity, AuthError> {
        if token.is_empty() || token.len() > MAX_ACCESS_TOKEN_BYTES {
            return Err(AuthError::InvalidAccessToken);
        }
        let header = decode_header(token).map_err(|_| AuthError::InvalidAccessToken)?;
        if header.alg != Algorithm::EdDSA {
            return Err(AuthError::InvalidAccessToken);
        }
        let key_id = header.kid.ok_or(AuthError::InvalidAccessToken)?;
        let key = self
            .keys
            .get(&key_id)
            .ok_or(AuthError::InvalidAccessToken)?;

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[ACCESS_TOKEN_AUDIENCE]);
        validation.required_spec_claims = ["exp", "nbf", "iat", "iss", "aud", "sub"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        validation.validate_nbf = true;
        validation.leeway = 0;

        let claims = decode::<AccessTokenClaims>(token, key, &validation)
            .map_err(|_| AuthError::InvalidAccessToken)?
            .claims;
        let now = unix_timestamp(SystemTime::now()).map_err(|_| AuthError::InvalidAccessToken)?;
        if claims.iat > now {
            return Err(AuthError::InvalidAccessToken);
        }
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidAccessToken)?;
        let session_id = Uuid::parse_str(&claims.sid).map_err(|_| AuthError::InvalidAccessToken)?;
        let device_id = Uuid::parse_str(&claims.did).map_err(|_| AuthError::InvalidAccessToken)?;
        let token_id = Uuid::parse_str(&claims.jti).map_err(|_| AuthError::InvalidAccessToken)?;
        SessionIdentity::new(user_id, session_id, device_id, token_id)
            .map_err(|_| AuthError::InvalidAccessToken)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessTokenClaims {
    iss: String,
    aud: String,
    sub: String,
    sid: String,
    did: String,
    jti: String,
    iat: i64,
    nbf: i64,
    exp: i64,
}

/// Server-only HMAC key for deriving database refresh-token verifiers.
pub struct RefreshTokenPepper(SecretString);

impl RefreshTokenPepper {
    /// Builds a pepper from a mounted secret. The value must provide at least
    /// 256 bits of input material and is never serialized or logged.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidRefreshTokenConfiguration`] if the secret
    /// is too short or contains a NUL byte.
    pub fn new(value: SecretString) -> Result<Self, AuthError> {
        let exposed = value.expose_secret();
        if exposed.len() < MINIMUM_PEPPER_BYTES || exposed.contains('\0') {
            return Err(AuthError::InvalidRefreshTokenConfiguration);
        }
        Ok(Self(value))
    }

    fn verifier_for(&self, token: &RefreshToken) -> RefreshTokenVerifier {
        let mut mac = HmacSha256::new_from_slice(self.0.expose_secret().as_bytes())
            .expect("HMAC accepts every key length");
        mac.update(token.serialized.expose_secret().as_bytes());
        RefreshTokenVerifier(mac.finalize().into_bytes().to_vec())
    }
}

/// A device-session refresh token. Its raw serialized form belongs only in the
/// platform secure store and must never be stored in `PostgreSQL` or logs.
pub struct RefreshToken {
    serialized: SecretString,
    session_id: Uuid,
}

impl RefreshToken {
    /// Generates a token with a 256-bit random secret bound to a `UUIDv7` session.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidRefreshToken`] when the session ID is not
    /// `UUIDv7`.
    pub fn generate(session_id: Uuid) -> Result<Self, AuthError> {
        if session_id.get_version_num() != 7 {
            return Err(AuthError::InvalidRefreshToken);
        }
        let mut secret = [0_u8; REFRESH_TOKEN_SECRET_BYTES];
        rand::rng().fill_bytes(&mut secret);
        let encoded_secret = URL_SAFE_NO_PAD.encode(secret);
        let serialized = format!("{REFRESH_TOKEN_PREFIX}{session_id}.{encoded_secret}");
        Ok(Self {
            serialized: SecretString::from(serialized),
            session_id,
        })
    }

    /// Parses a raw token from the secure client channel without logging it.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::InvalidRefreshToken`] for an invalid prefix,
    /// session ID, separator, encoding, or secret length.
    pub fn parse(serialized: SecretString) -> Result<Self, AuthError> {
        let value = serialized.expose_secret();
        let remainder = value
            .strip_prefix(REFRESH_TOKEN_PREFIX)
            .ok_or(AuthError::InvalidRefreshToken)?;
        let (session_id, encoded_secret) = remainder
            .split_once('.')
            .ok_or(AuthError::InvalidRefreshToken)?;
        if encoded_secret.contains('.') {
            return Err(AuthError::InvalidRefreshToken);
        }
        let session_id = Uuid::parse_str(session_id).map_err(|_| AuthError::InvalidRefreshToken)?;
        if session_id.get_version_num() != 7 {
            return Err(AuthError::InvalidRefreshToken);
        }
        let secret = URL_SAFE_NO_PAD
            .decode(encoded_secret)
            .map_err(|_| AuthError::InvalidRefreshToken)?;
        if secret.len() != REFRESH_TOKEN_SECRET_BYTES {
            return Err(AuthError::InvalidRefreshToken);
        }
        Ok(Self {
            serialized,
            session_id,
        })
    }

    #[must_use]
    pub const fn serialized(&self) -> &SecretString {
        &self.serialized
    }

    #[must_use]
    pub const fn session_id(&self) -> Uuid {
        self.session_id
    }

    #[must_use]
    pub fn verifier(&self, pepper: &RefreshTokenPepper) -> RefreshTokenVerifier {
        pepper.verifier_for(self)
    }
}

/// The HMAC verifier persisted in `session_refresh_tokens.token_verifier`.
#[derive(Clone, PartialEq, Eq)]
pub struct RefreshTokenVerifier(Vec<u8>);

impl RefreshTokenVerifier {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for RefreshTokenVerifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RefreshTokenVerifier([REDACTED])")
    }
}

fn validate_identifier(value: String) -> Result<String, AuthError> {
    if value.trim().is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
        return Err(AuthError::InvalidAccessTokenConfiguration);
    }
    Ok(value)
}

fn unix_timestamp(time: SystemTime) -> Result<i64, AuthError> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|_| AuthError::InvalidSystemTime)?
        .as_secs()
        .try_into()
        .map_err(|_| AuthError::InvalidSystemTime)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ed25519_dalek::{
        SigningKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey},
    };
    use pkcs8::LineEnding;
    use secrecy::ExposeSecret;

    use super::*;

    fn version_seven_uuid() -> Uuid {
        Uuid::now_v7()
    }

    fn access_token_keys() -> (SecretString, String) {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let private = signing_key
            .to_pkcs8_pem(LineEnding::LF)
            .expect("test private key should encode");
        let public = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("test public key should encode");
        (SecretString::from(private.to_string()), public.clone())
    }

    #[test]
    fn eddsa_access_tokens_bind_the_expected_session_identity() {
        let (private_key, public_key) = access_token_keys();
        let settings =
            AccessTokenSettings::new("https://jimin-os.test", "m1-test", Duration::from_mins(5))
                .expect("settings should be valid");
        let token_issuer = AccessTokenIssuer::from_ed25519_pem(settings, &private_key)
            .expect("private key should load");
        let verifier = AccessTokenVerifier::from_ed25519_pems(
            "https://jimin-os.test",
            [("m1-test".to_owned(), public_key)],
        )
        .expect("public key should load");
        let identity = SessionIdentity::new(
            version_seven_uuid(),
            version_seven_uuid(),
            version_seven_uuid(),
            version_seven_uuid(),
        )
        .expect("identity should be valid");

        let issued = token_issuer
            .issue(identity, SystemTime::now())
            .expect("token should issue");

        assert_eq!(
            verifier
                .verify(issued.token().expose_secret())
                .expect("token should verify"),
            identity
        );
    }

    #[test]
    fn verifier_rejects_wrong_issuer_and_missing_key() {
        let (private_key, public_key) = access_token_keys();
        let settings =
            AccessTokenSettings::new("https://jimin-os.test", "m1-test", Duration::from_mins(1))
                .expect("settings should be valid");
        let issuer = AccessTokenIssuer::from_ed25519_pem(settings, &private_key)
            .expect("private key should load");
        let token = issuer
            .issue(
                SessionIdentity::new(
                    version_seven_uuid(),
                    version_seven_uuid(),
                    version_seven_uuid(),
                    version_seven_uuid(),
                )
                .expect("identity should be valid"),
                SystemTime::now(),
            )
            .expect("token should issue");
        let wrong_issuer = AccessTokenVerifier::from_ed25519_pems(
            "https://other.test",
            [("m1-test".to_owned(), public_key.clone())],
        )
        .expect("public key should load");
        let missing_key = AccessTokenVerifier::from_ed25519_pems(
            "https://jimin-os.test",
            [("other".to_owned(), public_key)],
        )
        .expect("public key should load");

        assert!(matches!(
            wrong_issuer.verify(token.token().expose_secret()),
            Err(AuthError::InvalidAccessToken)
        ));
        assert!(matches!(
            missing_key.verify(token.token().expose_secret()),
            Err(AuthError::InvalidAccessToken)
        ));
    }

    #[test]
    fn refresh_tokens_are_opaque_and_pepper_bound() {
        let pepper = RefreshTokenPepper::new(SecretString::from(
            "test-only-refresh-token-pepper-material-32",
        ))
        .expect("pepper should be valid");
        let other_pepper =
            RefreshTokenPepper::new(SecretString::from("different-test-refresh-token-pepper-32"))
                .expect("pepper should be valid");
        let token = RefreshToken::generate(version_seven_uuid()).expect("token should generate");
        let parsed = RefreshToken::parse(SecretString::from(token.serialized().expose_secret()))
            .expect("generated token should parse");

        assert_eq!(token.session_id(), parsed.session_id());
        assert_eq!(token.verifier(&pepper), parsed.verifier(&pepper));
        assert_ne!(token.verifier(&pepper), token.verifier(&other_pepper));
        assert_eq!(token.verifier(&pepper).as_bytes().len(), 32);
    }

    #[test]
    fn refresh_tokens_reject_invalid_or_non_v7_session_ids() {
        let malformed = SecretString::from("josr_not-a-uuid.invalid");
        assert!(matches!(
            RefreshToken::parse(malformed),
            Err(AuthError::InvalidRefreshToken)
        ));
        assert!(matches!(
            RefreshToken::generate(Uuid::nil()),
            Err(AuthError::InvalidRefreshToken)
        ));
    }

    #[test]
    fn short_lived_access_token_policy_is_enforced() {
        assert!(matches!(
            AccessTokenSettings::new("issuer", "key", Duration::from_mins(16)),
            Err(AuthError::InvalidAccessTokenConfiguration)
        ));
    }
}

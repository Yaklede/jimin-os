//! Application use cases shared by HTTP and future background adapters.
//!
//! Device enrollment is explicit and server-owned: a trusted server surface
//! issues a short-lived QR token, then the new device consumes it once. Google
//! identity types remain here only for the later Calendar integration edge.

use std::time::{Duration, SystemTime};

use jimin_auth::{
    AccessTokenIssuer, AuthError, IssuedAccessToken, PairingToken, PairingTokenPepper,
    RefreshToken, RefreshTokenPepper, SessionIdentity,
};
use jimin_domain::{DeviceRegistration, EmailAddress, GoogleSubject};
use jimin_storage::{
    Database, StorageError,
    auth::{
        ConsumeDevicePairing, CreateDevicePairing, Device, PairingConsumption, Profile,
        RefreshRotation, RotateRefreshToken,
    },
};
use secrecy::SecretString;
use thiserror::Error;
use time::{Duration as TimeDuration, OffsetDateTime};
use uuid::Uuid;

const MINIMUM_SESSION_TTL: Duration = Duration::from_hours(1);
const MAXIMUM_SESSION_TTL: Duration = Duration::from_hours(2_160);
const MINIMUM_PAIRING_TTL: Duration = Duration::from_mins(1);
const MAXIMUM_PAIRING_TTL: Duration = Duration::from_mins(15);

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("the verified Google identity is invalid")]
    InvalidIdentity,
    #[error("the session lifetime configuration is invalid")]
    InvalidSessionLifetime,
    #[error("the session is no longer valid")]
    SessionExpired,
    #[error("the refresh token was reused")]
    RefreshReused,
    #[error("the device pairing token is invalid or no longer available")]
    PairingRejected,
    #[error("the server session operation is unavailable")]
    Storage(#[source] StorageError),
    #[error("the access token operation is unavailable")]
    AccessToken(#[source] AuthError),
}

/// Google identity claims that have already passed signature, issuer, audience,
/// expiry, and `email_verified` checks at the OAuth adapter boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedGoogleIdentity {
    subject: GoogleSubject,
    email: EmailAddress,
    display_name: Option<String>,
}

impl VerifiedGoogleIdentity {
    /// Creates a sanitized identity suitable for the application layer.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::InvalidIdentity`] for an overlong, blank, or
    /// control-character display name.
    pub fn new(
        subject: GoogleSubject,
        email: EmailAddress,
        display_name: Option<String>,
    ) -> Result<Self, ApplicationError> {
        let display_name = display_name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if display_name
            .as_ref()
            .is_some_and(|value| value.chars().count() > 120 || value.chars().any(char::is_control))
        {
            return Err(ApplicationError::InvalidIdentity);
        }
        Ok(Self {
            subject,
            email,
            display_name,
        })
    }

    #[must_use]
    pub fn subject(&self) -> &GoogleSubject {
        &self.subject
    }

    #[must_use]
    pub fn email(&self) -> &EmailAddress {
        &self.email
    }

    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

/// Bounded server-side session lifetime. Access tokens keep their separate,
/// shorter lifetime in [`AccessTokenIssuer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLifetime(Duration);

impl SessionLifetime {
    /// Validates the device session window against the M1 bounded policy.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::InvalidSessionLifetime`] outside one hour to
    /// ninety days.
    pub fn new(value: Duration) -> Result<Self, ApplicationError> {
        if !(MINIMUM_SESSION_TTL..=MAXIMUM_SESSION_TTL).contains(&value) {
            return Err(ApplicationError::InvalidSessionLifetime);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_std(self) -> Duration {
        self.0
    }
}

/// Bounded lifetime for a QR token displayed by the trusted personal server.
/// The token is deliberately much shorter than a device session and can only
/// be consumed once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairingLifetime(Duration);

impl PairingLifetime {
    /// Creates a QR pairing lifetime between one and fifteen minutes.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::InvalidSessionLifetime`] outside the
    /// bounded device-enrollment policy.
    pub fn new(value: Duration) -> Result<Self, ApplicationError> {
        if !(MINIMUM_PAIRING_TTL..=MAXIMUM_PAIRING_TTL).contains(&value) {
            return Err(ApplicationError::InvalidSessionLifetime);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_std(self) -> Duration {
        self.0
    }
}

/// A one-time pairing token returned only to the trusted server surface that
/// renders a QR code. Its secret must never be persisted or sent to another
/// device except through the pairing scan flow.
pub struct IssuedDevicePairing {
    token: PairingToken,
    expires_at: OffsetDateTime,
}

impl IssuedDevicePairing {
    #[must_use]
    pub const fn token(&self) -> &PairingToken {
        &self.token
    }

    #[must_use]
    pub const fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
}

/// A Jimin OS session response. Raw tokens are represented by secret-aware
/// types and are intended for immediate HTTPS response serialization only.
pub struct DeviceSession {
    access_token: IssuedAccessToken,
    refresh_token: RefreshToken,
    profile: Profile,
    device: Device,
    sync_cursor: Option<i64>,
}

impl DeviceSession {
    #[must_use]
    pub const fn access_token(&self) -> &IssuedAccessToken {
        &self.access_token
    }

    #[must_use]
    pub const fn refresh_token(&self) -> &RefreshToken {
        &self.refresh_token
    }

    #[must_use]
    pub const fn profile(&self) -> &Profile {
        &self.profile
    }

    #[must_use]
    pub const fn device(&self) -> &Device {
        &self.device
    }

    #[must_use]
    pub const fn sync_cursor(&self) -> Option<i64> {
        self.sync_cursor
    }
}

/// Coordinates QR device enrollment, refresh rotation, and access-token
/// issuance without knowing anything about an HTTP framework.
pub struct SessionService {
    database: Database,
    access_token_issuer: AccessTokenIssuer,
    refresh_token_pepper: RefreshTokenPepper,
    pairing_token_pepper: PairingTokenPepper,
    session_lifetime: SessionLifetime,
    pairing_lifetime: PairingLifetime,
}

impl SessionService {
    #[must_use]
    pub fn new(
        database: Database,
        access_token_issuer: AccessTokenIssuer,
        refresh_token_pepper: RefreshTokenPepper,
        pairing_token_pepper: PairingTokenPepper,
        session_lifetime: SessionLifetime,
        pairing_lifetime: PairingLifetime,
    ) -> Self {
        Self {
            database,
            access_token_issuer,
            refresh_token_pepper,
            pairing_token_pepper,
            session_lifetime,
            pairing_lifetime,
        }
    }

    /// Issues a fresh short-lived pairing token from a trusted server surface.
    /// Creating a newer token revokes any earlier pending token for the same
    /// personal server owner.
    ///
    /// # Errors
    ///
    /// Returns a sanitized storage or token error; the raw token remains inside
    /// [`IssuedDevicePairing`] for immediate QR rendering only.
    pub async fn issue_device_pairing(&self) -> Result<IssuedDevicePairing, ApplicationError> {
        let pairing_id = Uuid::now_v7();
        let token = PairingToken::generate(pairing_id).map_err(ApplicationError::AccessToken)?;
        let now = OffsetDateTime::now_utc();
        let expires_at = add_pairing_lifetime(now, self.pairing_lifetime)?;
        self.database
            .create_device_pairing(&CreateDevicePairing {
                owner_user_id: Uuid::now_v7(),
                pairing_id,
                token_verifier: token
                    .verifier(&self.pairing_token_pepper)
                    .as_bytes()
                    .to_vec(),
                expires_at,
            })
            .await
            .map_err(ApplicationError::Storage)?;
        Ok(IssuedDevicePairing { token, expires_at })
    }

    /// Consumes a scanned QR pairing token and creates one device session. The
    /// raw pairing token is parsed and HMAC-derived before storage sees it.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::PairingRejected`] for malformed, expired,
    /// consumed, or unknown pairing tokens.
    pub async fn consume_device_pairing(
        &self,
        serialized_pairing_token: SecretString,
        device: DeviceRegistration,
        request_id: Uuid,
    ) -> Result<DeviceSession, ApplicationError> {
        let pairing = PairingToken::parse(serialized_pairing_token)
            .map_err(|_| ApplicationError::PairingRejected)?;
        let session_id = Uuid::now_v7();
        let refresh_token =
            RefreshToken::generate(session_id).map_err(ApplicationError::AccessToken)?;
        let now = OffsetDateTime::now_utc();
        let session_expires_at = add_session_lifetime(now, self.session_lifetime)?;
        let consumption = self
            .database
            .consume_device_pairing(&ConsumeDevicePairing {
                pairing_id: pairing.pairing_id(),
                token_verifier: pairing
                    .verifier(&self.pairing_token_pepper)
                    .as_bytes()
                    .to_vec(),
                device,
                session_id,
                family_id: Uuid::now_v7(),
                refresh_token_id: Uuid::now_v7(),
                refresh_token_verifier: refresh_token
                    .verifier(&self.refresh_token_pepper)
                    .as_bytes()
                    .to_vec(),
                session_expires_at,
                refresh_token_expires_at: session_expires_at,
                request_id,
            })
            .await
            .map_err(ApplicationError::Storage)?;
        let PairingConsumption::Consumed(provisioned) = consumption else {
            return Err(ApplicationError::PairingRejected);
        };
        let access_token = self.issue_access_token(
            provisioned.profile.id,
            provisioned.session_id,
            provisioned.device.id,
        )?;

        Ok(DeviceSession {
            access_token,
            refresh_token,
            profile: provisioned.profile,
            device: provisioned.device,
            sync_cursor: Some(provisioned.sync_cursor),
        })
    }

    /// Rotates a refresh token and issues a new access token. A replayed token
    /// revokes its family in storage before [`ApplicationError::RefreshReused`]
    /// is returned.
    ///
    /// # Errors
    ///
    /// Returns only sanitized session failures; caller-provided token material
    /// is never included in the error value.
    pub async fn refresh(
        &self,
        serialized_refresh_token: SecretString,
        request_id: Uuid,
    ) -> Result<DeviceSession, ApplicationError> {
        let presented = RefreshToken::parse(serialized_refresh_token)
            .map_err(|_| ApplicationError::SessionExpired)?;
        let replacement = RefreshToken::generate(presented.session_id())
            .map_err(ApplicationError::AccessToken)?;
        let rotation = self
            .database
            .rotate_refresh_token(&RotateRefreshToken {
                session_id: presented.session_id(),
                presented_verifier: presented
                    .verifier(&self.refresh_token_pepper)
                    .as_bytes()
                    .to_vec(),
                new_refresh_token_id: Uuid::now_v7(),
                new_refresh_token_verifier: replacement
                    .verifier(&self.refresh_token_pepper)
                    .as_bytes()
                    .to_vec(),
                new_refresh_token_expires_at: add_session_lifetime(
                    OffsetDateTime::now_utc(),
                    self.session_lifetime,
                )?,
                request_id,
            })
            .await
            .map_err(ApplicationError::Storage)?;
        let RefreshRotation::Rotated(rotated) = rotation else {
            return match rotation {
                RefreshRotation::Reused => Err(ApplicationError::RefreshReused),
                RefreshRotation::Rejected => Err(ApplicationError::SessionExpired),
                RefreshRotation::Rotated(_) => unreachable!("rotated token handled above"),
            };
        };
        let access_token =
            self.issue_access_token(rotated.profile.id, rotated.session_id, rotated.device.id)?;
        let sync_cursor = self
            .database
            .current_sync_cursor_for_user(rotated.profile.id)
            .await
            .map_err(ApplicationError::Storage)?;

        Ok(DeviceSession {
            access_token,
            refresh_token: replacement,
            profile: rotated.profile,
            device: rotated.device,
            sync_cursor: Some(sync_cursor),
        })
    }

    fn issue_access_token(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        device_id: Uuid,
    ) -> Result<IssuedAccessToken, ApplicationError> {
        let identity = SessionIdentity::new(user_id, session_id, device_id, Uuid::now_v7())
            .map_err(ApplicationError::AccessToken)?;
        self.access_token_issuer
            .issue(identity, SystemTime::now())
            .map_err(ApplicationError::AccessToken)
    }
}

fn add_session_lifetime(
    now: OffsetDateTime,
    lifetime: SessionLifetime,
) -> Result<OffsetDateTime, ApplicationError> {
    let seconds: i64 = lifetime
        .as_std()
        .as_secs()
        .try_into()
        .map_err(|_| ApplicationError::InvalidSessionLifetime)?;
    now.checked_add(TimeDuration::seconds(seconds))
        .ok_or(ApplicationError::InvalidSessionLifetime)
}

fn add_pairing_lifetime(
    now: OffsetDateTime,
    lifetime: PairingLifetime,
) -> Result<OffsetDateTime, ApplicationError> {
    let seconds: i64 = lifetime
        .as_std()
        .as_secs()
        .try_into()
        .map_err(|_| ApplicationError::InvalidSessionLifetime)?;
    now.checked_add(TimeDuration::seconds(seconds))
        .ok_or(ApplicationError::InvalidSessionLifetime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifetime_is_strictly_bounded() {
        assert!(SessionLifetime::new(Duration::from_mins(59)).is_err());
        assert!(SessionLifetime::new(Duration::from_hours(2_184)).is_err());
        assert!(SessionLifetime::new(Duration::from_hours(720)).is_ok());
    }

    #[test]
    fn pairing_lifetime_is_short_and_bounded() {
        assert!(PairingLifetime::new(Duration::from_secs(59)).is_err());
        assert!(PairingLifetime::new(Duration::from_mins(16)).is_err());
        assert!(PairingLifetime::new(Duration::from_mins(10)).is_ok());
    }

    #[test]
    fn verified_identity_removes_blank_display_names() {
        let identity = VerifiedGoogleIdentity::new(
            GoogleSubject::parse("subject").expect("subject should be valid"),
            EmailAddress::parse("owner@example.test").expect("email should be valid"),
            Some("   ".to_owned()),
        )
        .expect("identity should be valid");

        assert_eq!(identity.display_name(), None);
    }
}

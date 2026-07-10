//! Application use cases shared by HTTP and future background adapters.
//!
//! Google token exchange belongs at the adapter edge. This crate accepts only
//! a previously verified identity and turns it into a Jimin OS device session.

use std::time::{Duration, SystemTime};

use jimin_auth::{
    AccessTokenIssuer, AuthError, IssuedAccessToken, RefreshToken, RefreshTokenPepper,
    SessionIdentity,
};
use jimin_domain::{DeviceRegistration, EmailAddress, EmailAllowlist, GoogleSubject};
use jimin_storage::{
    Database, StorageError,
    auth::{Device, Profile, ProvisionLogin, RefreshRotation, RotateRefreshToken},
};
use secrecy::SecretString;
use thiserror::Error;
use time::{Duration as TimeDuration, OffsetDateTime};
use uuid::Uuid;

const MINIMUM_SESSION_TTL: Duration = Duration::from_hours(1);
const MAXIMUM_SESSION_TTL: Duration = Duration::from_hours(2_160);

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("the Google account is not allowed")]
    AccountNotAllowed,
    #[error("the verified Google identity is invalid")]
    InvalidIdentity,
    #[error("the session lifetime configuration is invalid")]
    InvalidSessionLifetime,
    #[error("the session is no longer valid")]
    SessionExpired,
    #[error("the refresh token was reused")]
    RefreshReused,
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

/// Coordinates validated identity, device persistence, refresh rotation, and
/// access-token issuance without knowing anything about an HTTP framework.
pub struct SessionService {
    database: Database,
    allowlist: EmailAllowlist,
    access_token_issuer: AccessTokenIssuer,
    refresh_token_pepper: RefreshTokenPepper,
    session_lifetime: SessionLifetime,
}

impl SessionService {
    #[must_use]
    pub fn new(
        database: Database,
        allowlist: EmailAllowlist,
        access_token_issuer: AccessTokenIssuer,
        refresh_token_pepper: RefreshTokenPepper,
        session_lifetime: SessionLifetime,
    ) -> Self {
        Self {
            database,
            allowlist,
            access_token_issuer,
            refresh_token_pepper,
            session_lifetime,
        }
    }

    /// Creates a device-specific session after a Google adapter verified the
    /// authorization-code response. The raw refresh token is returned once and
    /// never reaches the database.
    ///
    /// # Errors
    ///
    /// Returns a sanitized application error for an unallowed account, storage
    /// failure, or token issue failure.
    pub async fn login(
        &self,
        identity: VerifiedGoogleIdentity,
        device: DeviceRegistration,
        request_id: Uuid,
    ) -> Result<DeviceSession, ApplicationError> {
        if !self.allowlist.permits(identity.email()) {
            return Err(ApplicationError::AccountNotAllowed);
        }

        let session_id = Uuid::now_v7();
        let refresh_token =
            RefreshToken::generate(session_id).map_err(ApplicationError::AccessToken)?;
        let now = OffsetDateTime::now_utc();
        let session_expires_at = add_session_lifetime(now, self.session_lifetime)?;
        let provisioned = self
            .database
            .provision_login(&ProvisionLogin {
                user_id: Uuid::now_v7(),
                google_subject: identity.subject,
                email: identity.email,
                display_name: identity.display_name,
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

        Ok(DeviceSession {
            access_token,
            refresh_token: replacement,
            profile: rotated.profile,
            device: rotated.device,
            sync_cursor: None,
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

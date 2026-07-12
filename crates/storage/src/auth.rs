//! `PostgreSQL` persistence for Jimin OS users, paired devices, and sessions.
//!
//! All methods in this module are intentionally scoped by internal `user_id`
//! and never accept a client-provided identity as an authorization key.

use jimin_auth::SessionIdentity;
use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use sqlx::{Postgres, Transaction};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

const ACTIVE_STATUS: &str = "active";
const SYNC_UPSERT: &str = "upsert";

/// Validated, server-side inputs for creating a device session after Google
/// identity verification. Raw provider tokens and raw refresh tokens are not
/// part of this command.
pub struct ProvisionLogin {
    pub user_id: Uuid,
    pub google_subject: GoogleSubject,
    pub email: EmailAddress,
    pub display_name: Option<String>,
    pub device: DeviceRegistration,
    pub session_id: Uuid,
    pub family_id: Uuid,
    pub refresh_token_id: Uuid,
    pub refresh_token_verifier: Vec<u8>,
    pub session_expires_at: OffsetDateTime,
    pub refresh_token_expires_at: OffsetDateTime,
    pub request_id: Uuid,
}

impl ProvisionLogin {
    /// Validates the generated IDs and fixed-size HMAC verifier before any
    /// transaction is opened.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for non-UUIDv7 IDs or a
    /// malformed refresh verifier.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !all_version_seven(&[
            self.user_id,
            self.session_id,
            self.family_id,
            self.refresh_token_id,
        ]) || !valid_refresh_verifier(&self.refresh_token_verifier)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        if self.refresh_token_expires_at < self.session_expires_at {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Validated input for issuing one short-lived QR pairing token from a trusted
/// server-side surface. The raw pairing token never reaches this command.
pub struct CreateDevicePairing {
    pub owner_user_id: Uuid,
    pub pairing_id: Uuid,
    pub token_verifier: Vec<u8>,
    pub expires_at: OffsetDateTime,
}

impl CreateDevicePairing {
    /// Validates generated IDs, verifier length, and expiry before storage.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !all_version_seven(&[self.owner_user_id, self.pairing_id])
            || !valid_refresh_verifier(&self.token_verifier)
            || self.expires_at <= OffsetDateTime::now_utc()
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Safe metadata returned when the trusted server creates a QR pairing token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedDevicePairing {
    pub pairing_id: Uuid,
    pub owner_user_id: Uuid,
    pub expires_at: OffsetDateTime,
}

/// Validated inputs for consuming an enrolled-device pairing token. All token
/// values have already been HMAC-derived with a server-only pairing pepper.
pub struct ConsumeDevicePairing {
    pub pairing_id: Uuid,
    pub token_verifier: Vec<u8>,
    pub device: DeviceRegistration,
    pub session_id: Uuid,
    pub family_id: Uuid,
    pub refresh_token_id: Uuid,
    pub refresh_token_verifier: Vec<u8>,
    pub session_expires_at: OffsetDateTime,
    pub refresh_token_expires_at: OffsetDateTime,
    pub request_id: Uuid,
}

impl ConsumeDevicePairing {
    /// Validates generated IDs and verifier lengths before acquiring the
    /// one-time-token lock.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !all_version_seven(&[
            self.pairing_id,
            self.session_id,
            self.family_id,
            self.refresh_token_id,
        ]) || !valid_refresh_verifier(&self.token_verifier)
            || !valid_refresh_verifier(&self.refresh_token_verifier)
            || self.refresh_token_expires_at < self.session_expires_at
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// A pairing outcome deliberately does not tell an untrusted client whether a
/// token existed, was already consumed, or expired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingConsumption {
    Consumed(Box<ProvisionedLogin>),
    Rejected,
}

/// The safe subset of a user row returned to the API and client cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: Uuid,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub time_zone: String,
    pub version: i64,
}

/// The safe subset of device metadata returned to the owning user only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: Uuid,
    pub platform: ClientPlatform,
    pub name: String,
    pub app_version: String,
    pub os_version: Option<String>,
    pub status: DeviceStatus,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Active,
    Revoked,
}

impl DeviceStatus {
    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "active" => Ok(Self::Active),
            "revoked" => Ok(Self::Revoked),
            _ => Err(StorageError::PersistenceUnavailable),
        }
    }
}

/// Result of an atomic user/device/session provision transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedLogin {
    pub profile: Profile,
    pub device: Device,
    pub session_id: Uuid,
    pub family_id: Uuid,
    pub sync_cursor: i64,
}

/// Inputs for a single-use refresh token rotation. Both verifier values are
/// derived with the server pepper before this command reaches storage.
pub struct RotateRefreshToken {
    pub session_id: Uuid,
    pub presented_verifier: Vec<u8>,
    pub new_refresh_token_id: Uuid,
    pub new_refresh_token_verifier: Vec<u8>,
    pub new_refresh_token_expires_at: OffsetDateTime,
    pub request_id: Uuid,
}

impl RotateRefreshToken {
    /// Validates generated IDs and HMAC verifier lengths before opening a lock.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed input.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !all_version_seven(&[self.session_id, self.new_refresh_token_id])
            || !valid_refresh_verifier(&self.presented_verifier)
            || !valid_refresh_verifier(&self.new_refresh_token_verifier)
            || self.new_refresh_token_expires_at <= OffsetDateTime::now_utc()
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Result of a refresh request. Only a successful rotation returns user/device
/// state; invalid, expired, and replayed requests intentionally reveal no row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshRotation {
    Rotated(Box<RotatedRefreshToken>),
    Reused,
    Rejected,
}

/// Safe session details returned only after a successful refresh rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotatedRefreshToken {
    pub profile: Profile,
    pub device: Device,
    pub session_id: Uuid,
    pub family_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    id: Uuid,
    email: Option<String>,
    display_name: Option<String>,
    time_zone: String,
    status: String,
    version: i64,
}

impl TryFrom<ProfileRow> for Profile {
    type Error = StorageError;

    fn try_from(row: ProfileRow) -> Result<Self, Self::Error> {
        if row.status != ACTIVE_STATUS {
            return Err(StorageError::IdentityConflict);
        }
        Ok(Self {
            id: row.id,
            email: row.email,
            display_name: row.display_name,
            time_zone: row.time_zone,
            version: row.version,
        })
    }
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    platform: String,
    name: String,
    app_version: String,
    os_version: Option<String>,
    status: String,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct SessionLockRow {
    user_id: Uuid,
    device_id: Uuid,
    family_id: Uuid,
    status: String,
    expires_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
struct RefreshTokenRow {
    id: Uuid,
    token_verifier: Vec<u8>,
    status: String,
    expires_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
struct PairingTokenLockRow {
    owner_user_id: Uuid,
    token_verifier: Vec<u8>,
    status: String,
    expires_at: OffsetDateTime,
}

impl TryFrom<DeviceRow> for Device {
    type Error = StorageError;

    fn try_from(row: DeviceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            platform: parse_platform(&row.platform)?,
            name: row.name,
            app_version: row.app_version,
            os_version: row.os_version,
            status: DeviceStatus::parse(&row.status)?,
            version: row.version,
        })
    }
}

impl Database {
    /// Upserts the validated owner and registered device, then creates a new
    /// independent session family and refresh-token verifier in one transaction.
    ///
    /// The transaction also appends user/device change records and one sanitized
    /// login audit record. No raw credential reaches a SQL bind or audit column.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error without exposing SQL, email, or
    /// provider details.
    #[allow(
        clippy::too_many_lines,
        reason = "The complete login transaction is intentionally visible in one method."
    )]
    pub async fn provision_login(
        &self,
        command: &ProvisionLogin,
    ) -> Result<ProvisionedLogin, StorageError> {
        command.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify_database_error(&error))?;

        let profile = sqlx::query_as::<_, ProfileRow>(
            "\
            INSERT INTO users (
                id, google_sub, email, normalized_email, display_name, last_login_at, status
            ) VALUES ($1, $2, $3, $4, $5, NOW(), 'active')
            ON CONFLICT (google_sub) DO UPDATE
            SET email = EXCLUDED.email,
                normalized_email = EXCLUDED.normalized_email,
                display_name = EXCLUDED.display_name,
                last_login_at = NOW()
            RETURNING id, email, display_name, time_zone, status, version",
        )
        .bind(command.user_id)
        .bind(command.google_subject.as_str())
        .bind(command.email.display())
        .bind(command.email.normalized())
        .bind(&command.display_name)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))
        .and_then(Profile::try_from)?;

        let device = sqlx::query_as::<_, DeviceRow>(
            "\
            INSERT INTO devices (
                id, user_id, installation_id, platform, name, app_version, os_version,
                status, last_seen_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', NOW())
            ON CONFLICT (user_id, installation_id) DO UPDATE
            SET platform = EXCLUDED.platform,
                name = EXCLUDED.name,
                app_version = EXCLUDED.app_version,
                os_version = EXCLUDED.os_version,
                status = 'active',
                revoked_at = NULL,
                last_seen_at = NOW()
            RETURNING id, platform, name, app_version, os_version, status, version",
        )
        .bind(Uuid::now_v7())
        .bind(profile.id)
        .bind(command.device.installation_id())
        .bind(command.device.platform().as_str())
        .bind(command.device.name())
        .bind(command.device.app_version())
        .bind(command.device.os_version())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))
        .and_then(Device::try_from)?;

        sqlx::query(
            "\
            INSERT INTO sessions (
                id, user_id, device_id, family_id, status, expires_at, last_used_at
            ) VALUES ($1, $2, $3, $4, 'active', $5, NOW())",
        )
        .bind(command.session_id)
        .bind(profile.id)
        .bind(device.id)
        .bind(command.family_id)
        .bind(command.session_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;

        sqlx::query(
            "\
            INSERT INTO session_refresh_tokens (
                id, session_id, token_verifier, status, expires_at
            ) VALUES ($1, $2, $3, 'active', $4)",
        )
        .bind(command.refresh_token_id)
        .bind(command.session_id)
        .bind(&command.refresh_token_verifier)
        .bind(command.refresh_token_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;

        append_change(
            &mut transaction,
            profile.id,
            "user",
            profile.id,
            profile.version,
        )
        .await?;
        append_change(
            &mut transaction,
            profile.id,
            "device",
            device.id,
            device.version,
        )
        .await?;
        write_login_audit(
            &mut transaction,
            profile.id,
            device.id,
            command.session_id,
            command.request_id,
        )
        .await?;

        let sync_cursor = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sequence), 0) FROM sync_changes WHERE user_id = $1",
        )
        .bind(profile.id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;

        transaction
            .commit()
            .await
            .map_err(|error| classify_database_error(&error))?;

        Ok(ProvisionedLogin {
            profile,
            device,
            session_id: command.session_id,
            family_id: command.family_id,
            sync_cursor,
        })
    }

    /// Creates a fresh one-time pairing record for the single local Jimin OS
    /// owner. Any earlier pending token is revoked before the new token is
    /// stored, so a QR image cannot be used after a replacement is generated.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing token material.
    pub async fn create_device_pairing(
        &self,
        command: &CreateDevicePairing,
    ) -> Result<CreatedDevicePairing, StorageError> {
        command.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify_database_error(&error))?;

        let created_owner = sqlx::query_scalar::<_, Uuid>(
            "\
            INSERT INTO users (
                id, google_sub, email, normalized_email, display_name, last_login_at,
                status, identity_kind
            ) VALUES ($1, NULL, NULL, NULL, 'Jimin OS', NOW(), 'active', 'local_device')
            ON CONFLICT DO NOTHING
            RETURNING id",
        )
        .bind(command.owner_user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;

        let owner_user_id = match created_owner {
            Some(user_id) => user_id,
            None => sqlx::query_scalar::<_, Uuid>(
                "\
                SELECT id
                FROM users
                WHERE identity_kind = 'local_device' AND status = 'active'
                FOR UPDATE",
            )
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| classify_database_error(&error))?
            .ok_or(StorageError::IdentityConflict)?,
        };

        sqlx::query(
            "\
            UPDATE device_pairing_tokens
            SET status = 'revoked'
            WHERE owner_user_id = $1 AND status = 'pending'",
        )
        .bind(owner_user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        sqlx::query(
            "\
            INSERT INTO device_pairing_tokens (
                id, owner_user_id, token_verifier, status, expires_at
            ) VALUES ($1, $2, $3, 'pending', $4)",
        )
        .bind(command.pairing_id)
        .bind(owner_user_id)
        .bind(&command.token_verifier)
        .bind(command.expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        sqlx::query(
            "\
            INSERT INTO audit_logs (
                id, actor_user_id, action, target_type, target_id, outcome, metadata
            ) VALUES ($1, $2, 'auth.pairing.issued', 'device_pairing', $3, 'success', '{}')",
        )
        .bind(Uuid::now_v7())
        .bind(owner_user_id)
        .bind(command.pairing_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        transaction
            .commit()
            .await
            .map_err(|error| classify_database_error(&error))?;

        Ok(CreatedDevicePairing {
            pairing_id: command.pairing_id,
            owner_user_id,
            expires_at: command.expires_at,
        })
    }

    /// Consumes a QR pairing token exactly once and atomically creates the
    /// requesting device's session and refresh-token verifier.
    ///
    /// # Errors
    ///
    /// Returns [`PairingConsumption::Rejected`] for every invalid, expired,
    /// revoked, or previously consumed token without revealing its state.
    #[allow(
        clippy::too_many_lines,
        reason = "Pairing consumption must visibly hold one transaction from token lock to session creation."
    )]
    pub async fn consume_device_pairing(
        &self,
        command: &ConsumeDevicePairing,
    ) -> Result<PairingConsumption, StorageError> {
        command.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify_database_error(&error))?;
        let pairing = sqlx::query_as::<_, PairingTokenLockRow>(
            "\
            SELECT owner_user_id, token_verifier, status, expires_at
            FROM device_pairing_tokens
            WHERE id = $1
            FOR UPDATE",
        )
        .bind(command.pairing_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        let Some(pairing) = pairing else {
            return Ok(PairingConsumption::Rejected);
        };
        let valid_verifier = pairing
            .token_verifier
            .ct_eq(&command.token_verifier)
            .unwrap_u8()
            == 1;
        if !valid_verifier
            || pairing.status != "pending"
            || pairing.expires_at <= OffsetDateTime::now_utc()
        {
            if pairing.status == "pending" && pairing.expires_at <= OffsetDateTime::now_utc() {
                sqlx::query("UPDATE device_pairing_tokens SET status = 'expired' WHERE id = $1")
                    .bind(command.pairing_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|error| classify_database_error(&error))?;
                transaction
                    .commit()
                    .await
                    .map_err(|error| classify_database_error(&error))?;
            }
            return Ok(PairingConsumption::Rejected);
        }

        let profile =
            active_profile_in_transaction(&mut transaction, pairing.owner_user_id).await?;
        let Some(profile) = profile else {
            return Ok(PairingConsumption::Rejected);
        };
        let device = sqlx::query_as::<_, DeviceRow>(
            "\
            INSERT INTO devices (
                id, user_id, installation_id, platform, name, app_version, os_version,
                status, last_seen_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', NOW())
            ON CONFLICT (user_id, installation_id) DO UPDATE
            SET platform = EXCLUDED.platform,
                name = EXCLUDED.name,
                app_version = EXCLUDED.app_version,
                os_version = EXCLUDED.os_version,
                status = 'active',
                revoked_at = NULL,
                last_seen_at = NOW()
            RETURNING id, platform, name, app_version, os_version, status, version",
        )
        .bind(Uuid::now_v7())
        .bind(profile.id)
        .bind(command.device.installation_id())
        .bind(command.device.platform().as_str())
        .bind(command.device.name())
        .bind(command.device.app_version())
        .bind(command.device.os_version())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))
        .and_then(Device::try_from)?;
        sqlx::query(
            "\
            INSERT INTO sessions (
                id, user_id, device_id, family_id, status, expires_at, last_used_at
            ) VALUES ($1, $2, $3, $4, 'active', $5, NOW())",
        )
        .bind(command.session_id)
        .bind(profile.id)
        .bind(device.id)
        .bind(command.family_id)
        .bind(command.session_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        sqlx::query(
            "\
            INSERT INTO session_refresh_tokens (
                id, session_id, token_verifier, status, expires_at
            ) VALUES ($1, $2, $3, 'active', $4)",
        )
        .bind(command.refresh_token_id)
        .bind(command.session_id)
        .bind(&command.refresh_token_verifier)
        .bind(command.refresh_token_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        sqlx::query(
            "\
            UPDATE device_pairing_tokens
            SET status = 'consumed', consumed_at = NOW()
            WHERE id = $1 AND status = 'pending'",
        )
        .bind(command.pairing_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        append_change(
            &mut transaction,
            profile.id,
            "user",
            profile.id,
            profile.version,
        )
        .await?;
        append_change(
            &mut transaction,
            profile.id,
            "device",
            device.id,
            device.version,
        )
        .await?;
        write_pairing_audit(
            &mut transaction,
            profile.id,
            device.id,
            command.pairing_id,
            command.session_id,
            command.request_id,
        )
        .await?;
        let sync_cursor = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sequence), 0) FROM sync_changes WHERE user_id = $1",
        )
        .bind(profile.id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        transaction
            .commit()
            .await
            .map_err(|error| classify_database_error(&error))?;

        Ok(PairingConsumption::Consumed(Box::new(ProvisionedLogin {
            profile,
            device,
            session_id: command.session_id,
            family_id: command.family_id,
            sync_cursor,
        })))
    }

    /// Rotates one device refresh token while holding the session and all
    /// session-token rows locked. Reusing a rotated/revoked verifier marks the
    /// entire family compromised before the method returns.
    ///
    /// # Errors
    ///
    /// Returns an unavailable error only for storage failures. Invalid and
    /// replayed token inputs are represented by [`RefreshRotation`] to avoid
    /// exposing session state to a caller.
    #[allow(
        clippy::too_many_lines,
        reason = "Rotation and compromise must remain visibly transactional."
    )]
    pub async fn rotate_refresh_token(
        &self,
        command: &RotateRefreshToken,
    ) -> Result<RefreshRotation, StorageError> {
        command.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify_database_error(&error))?;
        let session = sqlx::query_as::<_, SessionLockRow>(
            "\
            SELECT user_id, device_id, family_id, status, expires_at
            FROM sessions
            WHERE id = $1
            FOR UPDATE",
        )
        .bind(command.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        let Some(session) = session else {
            return Ok(RefreshRotation::Rejected);
        };

        let refresh_tokens = sqlx::query_as::<_, RefreshTokenRow>(
            "\
            SELECT id, token_verifier, status, expires_at
            FROM session_refresh_tokens
            WHERE session_id = $1
            ORDER BY created_at ASC
            FOR UPDATE",
        )
        .bind(command.session_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        let matching_token =
            find_matching_refresh_token(&refresh_tokens, &command.presented_verifier);
        let Some(matching_token) = matching_token else {
            return Ok(RefreshRotation::Rejected);
        };

        if matching_token.status != ACTIVE_STATUS {
            compromise_refresh_family(
                &mut transaction,
                session.user_id,
                session.device_id,
                session.family_id,
                command.session_id,
                command.request_id,
            )
            .await?;
            transaction
                .commit()
                .await
                .map_err(|error| classify_database_error(&error))?;
            return Ok(RefreshRotation::Reused);
        }

        if matching_token.expires_at <= OffsetDateTime::now_utc() {
            sqlx::query(
                "\
                UPDATE session_refresh_tokens
                SET status = 'revoked'
                WHERE id = $1 AND status = 'active'",
            )
            .bind(matching_token.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify_database_error(&error))?;
            transaction
                .commit()
                .await
                .map_err(|error| classify_database_error(&error))?;
            return Ok(RefreshRotation::Rejected);
        }

        if session.status != ACTIVE_STATUS || session.expires_at <= OffsetDateTime::now_utc() {
            mark_session_expired_if_needed(&mut transaction, command.session_id, &session.status)
                .await?;
            transaction
                .commit()
                .await
                .map_err(|error| classify_database_error(&error))?;
            return Ok(RefreshRotation::Rejected);
        }

        let profile = active_profile_in_transaction(&mut transaction, session.user_id).await?;
        let device = active_device_in_transaction(&mut transaction, session.device_id).await?;
        let (Some(profile), Some(device)) = (profile, device) else {
            transaction
                .commit()
                .await
                .map_err(|error| classify_database_error(&error))?;
            return Ok(RefreshRotation::Rejected);
        };

        sqlx::query(
            "\
            INSERT INTO session_refresh_tokens (
                id, session_id, token_verifier, status, expires_at
            ) VALUES ($1, $2, $3, 'active', $4)",
        )
        .bind(command.new_refresh_token_id)
        .bind(command.session_id)
        .bind(&command.new_refresh_token_verifier)
        .bind(command.new_refresh_token_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        sqlx::query(
            "\
            UPDATE session_refresh_tokens
            SET status = 'rotated', used_at = NOW(), rotated_to_id = $1
            WHERE id = $2 AND status = 'active'",
        )
        .bind(command.new_refresh_token_id)
        .bind(matching_token.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
        sqlx::query("UPDATE sessions SET last_used_at = NOW() WHERE id = $1")
            .bind(command.session_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify_database_error(&error))?;
        write_refresh_audit(
            &mut transaction,
            profile.id,
            device.id,
            command.session_id,
            command.request_id,
            "auth.refresh.rotated",
            "success",
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify_database_error(&error))?;

        Ok(RefreshRotation::Rotated(Box::new(RotatedRefreshToken {
            profile,
            device,
            session_id: command.session_id,
            family_id: session.family_id,
        })))
    }

    /// Checks that the token claim still maps to an active, unexpired session,
    /// active device, and active user. It is safe to use at every route guard.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PersistenceUnavailable`] only when the database
    /// cannot answer the guard query; an inactive or missing row returns `false`.
    pub async fn is_session_active(&self, identity: SessionIdentity) -> Result<bool, StorageError> {
        sqlx::query_scalar::<_, bool>(
            "\
            SELECT EXISTS(
                SELECT 1
                FROM sessions session
                JOIN devices device ON device.id = session.device_id
                JOIN users user_account ON user_account.id = session.user_id
                WHERE session.id = $1
                  AND session.user_id = $2
                  AND session.device_id = $3
                  AND session.status = 'active'
                  AND session.expires_at > NOW()
                  AND device.status = 'active'
                  AND user_account.status = 'active'
            )",
        )
        .bind(identity.session_id())
        .bind(identity.user_id())
        .bind(identity.device_id())
        .fetch_one(self.pool())
        .await
        .map_err(|error| classify_database_error(&error))
    }

    /// Returns the profile only when it belongs to the authenticated user.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PersistenceUnavailable`] for an unavailable
    /// database and `Ok(None)` for a missing/inactive profile.
    pub async fn profile_for_user(&self, user_id: Uuid) -> Result<Option<Profile>, StorageError> {
        let profile = sqlx::query_as::<_, ProfileRow>(
            "\
            SELECT id, email, display_name, time_zone, status, version
            FROM users
            WHERE id = $1 AND status = 'active'",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify_database_error(&error))?;
        profile.map(Profile::try_from).transpose()
    }

    /// Lists only the devices owned by the authenticated user.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PersistenceUnavailable`] when the database is
    /// unavailable or contains an invalid platform/status enum.
    pub async fn devices_for_user(&self, user_id: Uuid) -> Result<Vec<Device>, StorageError> {
        let devices = sqlx::query_as::<_, DeviceRow>(
            "\
            SELECT id, platform, name, app_version, os_version, status, version
            FROM devices
            WHERE user_id = $1
            ORDER BY last_seen_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify_database_error(&error))?;
        devices.into_iter().map(Device::try_from).collect()
    }

    /// Returns the newest persisted sync sequence for a user session response.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PersistenceUnavailable`] when the database
    /// cannot return the current cursor.
    pub async fn current_sync_cursor_for_user(&self, user_id: Uuid) -> Result<i64, StorageError> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sequence), 0) FROM sync_changes WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| classify_database_error(&error))
    }
}

fn all_version_seven(ids: &[Uuid]) -> bool {
    ids.iter().all(|id| id.get_version_num() == 7)
}

fn valid_refresh_verifier(verifier: &[u8]) -> bool {
    verifier.len() == 32
}

fn find_matching_refresh_token<'row>(
    refresh_tokens: &'row [RefreshTokenRow],
    presented_verifier: &[u8],
) -> Option<&'row RefreshTokenRow> {
    let mut matching_token = None;
    for token in refresh_tokens {
        let matches = token.token_verifier.ct_eq(presented_verifier).unwrap_u8() == 1;
        if matches {
            matching_token = Some(token);
        }
    }
    matching_token
}

async fn active_profile_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<Option<Profile>, StorageError> {
    let row = sqlx::query_as::<_, ProfileRow>(
        "\
        SELECT id, email, display_name, time_zone, status, version
        FROM users
        WHERE id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    row.map(Profile::try_from).transpose()
}

async fn active_device_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    device_id: Uuid,
) -> Result<Option<Device>, StorageError> {
    let row = sqlx::query_as::<_, DeviceRow>(
        "\
        SELECT id, platform, name, app_version, os_version, status, version
        FROM devices
        WHERE id = $1 AND status = 'active'",
    )
    .bind(device_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    row.map(Device::try_from).transpose()
}

async fn mark_session_expired_if_needed(
    transaction: &mut Transaction<'_, Postgres>,
    session_id: Uuid,
    status: &str,
) -> Result<(), StorageError> {
    if status == ACTIVE_STATUS {
        sqlx::query(
            "\
            UPDATE sessions
            SET status = 'expired', revoked_at = NOW(), revocation_reason = 'session_expired'
            WHERE id = $1 AND status = 'active'",
        )
        .bind(session_id)
        .execute(&mut **transaction)
        .await
        .map_err(|error| classify_database_error(&error))?;
    }
    Ok(())
}

async fn compromise_refresh_family(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    device_id: Uuid,
    family_id: Uuid,
    session_id: Uuid,
    request_id: Uuid,
) -> Result<(), StorageError> {
    sqlx::query(
        "\
        UPDATE sessions
        SET status = 'compromised', revoked_at = NOW(), revocation_reason = 'refresh_reuse'
        WHERE family_id = $1 AND status = 'active'",
    )
    .bind(family_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    sqlx::query(
        "\
        UPDATE session_refresh_tokens token
        SET status = 'revoked'
        FROM sessions session
        WHERE token.session_id = session.id
          AND session.family_id = $1
          AND token.status = 'active'",
    )
    .bind(family_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    write_refresh_audit(
        transaction,
        user_id,
        device_id,
        session_id,
        request_id,
        "auth.refresh.reused",
        "rejected",
    )
    .await
}

async fn write_refresh_audit(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    device_id: Uuid,
    session_id: Uuid,
    request_id: Uuid,
    action: &str,
    outcome: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        "\
        INSERT INTO audit_logs (
            id, actor_user_id, actor_device_id, action, target_type, target_id,
            outcome, request_id, metadata
        ) VALUES ($1, $2, $3, $4, 'session', $5, $6, $7, '{}')",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(device_id)
    .bind(action)
    .bind(session_id)
    .bind(outcome)
    .bind(request_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    Ok(())
}

pub(crate) async fn append_change(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    entity_version: i64,
) -> Result<(), StorageError> {
    sqlx::query(
        "\
        INSERT INTO sync_changes (user_id, entity_type, entity_id, operation, entity_version)
        VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(SYNC_UPSERT)
    .bind(entity_version)
    .execute(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    Ok(())
}

async fn write_login_audit(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    device_id: Uuid,
    session_id: Uuid,
    request_id: Uuid,
) -> Result<(), StorageError> {
    sqlx::query(
        "\
        INSERT INTO audit_logs (
            id, actor_user_id, actor_device_id, action, target_type, target_id,
            outcome, request_id, metadata
        ) VALUES (
            $1, $2, $3, 'auth.session.issued', 'session', $4, 'success', $5,
            jsonb_build_object('source', 'google_identity')
        )",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(device_id)
    .bind(session_id)
    .bind(request_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    Ok(())
}

async fn write_pairing_audit(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    device_id: Uuid,
    pairing_id: Uuid,
    session_id: Uuid,
    request_id: Uuid,
) -> Result<(), StorageError> {
    sqlx::query(
        "\
        INSERT INTO audit_logs (
            id, actor_user_id, actor_device_id, action, target_type, target_id,
            outcome, request_id, metadata
        ) VALUES (
            $1, $2, $3, 'auth.pairing.consumed', 'session', $4, 'success', $5,
            jsonb_build_object('pairing_id', $6::text)
        )",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(device_id)
    .bind(session_id)
    .bind(request_id)
    .bind(pairing_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| classify_database_error(&error))?;
    Ok(())
}

fn parse_platform(value: &str) -> Result<ClientPlatform, StorageError> {
    match value {
        "macos" => Ok(ClientPlatform::Macos),
        "ios" => Ok(ClientPlatform::Ios),
        "android" => Ok(ClientPlatform::Android),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn classify_database_error(error: &sqlx::Error) -> StorageError {
    let is_unique_violation = error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "23505");
    if is_unique_violation {
        StorageError::IdentityConflict
    } else {
        StorageError::PersistenceUnavailable
    }
}

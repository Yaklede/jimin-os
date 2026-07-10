//! `PostgreSQL` persistence for authenticated M1 users, devices, and sessions.
//!
//! All methods in this module are intentionally scoped by internal `user_id`
//! and never accept a Google email as an authorization key.

use jimin_auth::SessionIdentity;
use jimin_domain::{ClientPlatform, DeviceRegistration, EmailAddress, GoogleSubject};
use sqlx::{Postgres, Transaction};
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
        let ids_are_version_seven = [
            self.user_id,
            self.session_id,
            self.family_id,
            self.refresh_token_id,
        ]
        .into_iter()
        .all(|id| id.get_version_num() == 7);
        if !ids_are_version_seven || self.refresh_token_verifier.len() != 32 {
            return Err(StorageError::InvalidConfiguration);
        }
        if self.refresh_token_expires_at < self.session_expires_at {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// The safe subset of a user row returned to the API and client cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: Uuid,
    pub email: String,
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

#[derive(sqlx::FromRow)]
struct ProfileRow {
    id: Uuid,
    email: String,
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
}

async fn append_change(
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

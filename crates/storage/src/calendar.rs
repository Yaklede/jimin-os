//! Google Calendar connection metadata and durable sync records.
//!
//! Provider credentials are intentionally represented only as encrypted SQL
//! columns in the migration. This module exposes the safe, client-visible
//! connection summary without returning refresh tokens, sync tokens, or
//! provider event payloads.

use jimin_domain::{ClientPlatform, EmailAddress, GoogleSubject};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

const STATE_VERIFIER_BYTES: usize = 32;
const XCHACHA_NONCE_BYTES: usize = 24;
const MAX_CIPHERTEXT_BYTES: usize = 8 * 1024;
const MAX_GRANTED_SCOPES: usize = 16;
const MAX_SCOPE_BYTES: usize = 512;
const MAX_FAILURE_CODE_BYTES: usize = 120;

/// Safe state of the single Google Calendar account linked to a personal user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarAccountStatus {
    Connecting,
    Active,
    ReauthRequired,
    Revoking,
    Revoked,
    Error,
}

impl CalendarAccountStatus {
    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "connecting" => Ok(Self::Connecting),
            "active" => Ok(Self::Active),
            "reauth_required" => Ok(Self::ReauthRequired),
            "revoking" => Ok(Self::Revoking),
            "revoked" => Ok(Self::Revoked),
            "error" => Ok(Self::Error),
            _ => Err(StorageError::PersistenceUnavailable),
        }
    }
}

/// Calendar account metadata that may be shown to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarAccount {
    pub id: Uuid,
    pub email: String,
    pub status: CalendarAccountStatus,
    pub granted_scopes: Vec<String>,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
struct CalendarAccountRow {
    id: Uuid,
    email: String,
    status: String,
    granted_scopes: Vec<String>,
    last_successful_sync_at: Option<OffsetDateTime>,
    version: i64,
}

impl TryFrom<CalendarAccountRow> for CalendarAccount {
    type Error = StorageError;

    fn try_from(row: CalendarAccountRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            email: row.email,
            status: CalendarAccountStatus::parse(&row.status)?,
            granted_scopes: row.granted_scopes,
            last_successful_sync_at: row.last_successful_sync_at,
            version: row.version,
        })
    }
}

impl Database {
    /// Returns the owner's Google Calendar connection without credential data.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when the database cannot be
    /// queried or an unknown status is found in a persisted row.
    pub async fn calendar_account_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<CalendarAccount>, StorageError> {
        let row = sqlx::query_as::<_, CalendarAccountRow>(
            "\
            SELECT id, email, status, granted_scopes, last_successful_sync_at, version
            FROM calendar_accounts
            WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        row.map(CalendarAccount::try_from).transpose()
    }

    /// Creates a short-lived server-owned OAuth transaction. Only the HMAC
    /// verifier of the browser state and an encrypted PKCE verifier reach
    /// `PostgreSQL`; the raw state never does.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed command
    /// material and [`StorageError::PersistenceUnavailable`] when the
    /// transaction cannot be persisted.
    pub async fn create_calendar_oauth_authorization(
        &self,
        command: &CreateCalendarOAuthAuthorization,
    ) -> Result<CreatedCalendarOAuthAuthorization, StorageError> {
        command.validate()?;
        sqlx::query(
            "\
            INSERT INTO calendar_oauth_authorizations (
                id, user_id, session_id, device_id, state_verifier,
                pkce_verifier_ciphertext, pkce_nonce, encryption_key_version,
                client_kind, status, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10)",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.session_id)
        .bind(command.device_id)
        .bind(&command.state_verifier)
        .bind(&command.pkce_verifier.ciphertext)
        .bind(&command.pkce_verifier.nonce)
        .bind(command.pkce_verifier.key_version)
        .bind(command.client_kind.as_str())
        .bind(command.expires_at)
        .execute(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        Ok(CreatedCalendarOAuthAuthorization {
            id: command.id,
            expires_at: command.expires_at,
        })
    }

    /// Atomically claims one pending OAuth state for a callback exchange.
    /// Expired, duplicate, cancelled, and already-consumed states are all
    /// intentionally indistinguishable to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PersistenceUnavailable`] when the database
    /// cannot complete the claim.
    pub async fn claim_calendar_oauth_authorization(
        &self,
        state_verifier: &[u8],
    ) -> Result<Option<ClaimedCalendarOAuthAuthorization>, StorageError> {
        if state_verifier.len() != STATE_VERIFIER_BYTES {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, ClaimedCalendarOAuthAuthorizationRow>(
            "\
            UPDATE calendar_oauth_authorizations AS authorization
            SET status = 'exchanging'
            FROM users AS user_account
            WHERE authorization.state_verifier = $1
              AND authorization.status = 'pending'
              AND authorization.expires_at > NOW()
              AND user_account.id = authorization.user_id
              AND user_account.status = 'active'
            RETURNING authorization.id, authorization.user_id,
                authorization.pkce_verifier_ciphertext,
                authorization.pkce_nonce,
                authorization.encryption_key_version,
                authorization.client_kind,
                user_account.google_sub",
        )
        .bind(state_verifier)
        .fetch_optional(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        row.map(ClaimedCalendarOAuthAuthorization::try_from)
            .transpose()
    }

    /// Marks a claimed authorization as failed and cryptographically deletes
    /// its one-time PKCE material. Failure codes are internal classifications,
    /// not provider response bodies.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for unsafe error codes
    /// and [`StorageError::PersistenceUnavailable`] when persistence fails.
    pub async fn fail_calendar_oauth_authorization(
        &self,
        authorization_id: Uuid,
        failure_code: &str,
    ) -> Result<(), StorageError> {
        if authorization_id.get_version_num() != 7 || !valid_failure_code(failure_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "\
            UPDATE calendar_oauth_authorizations
            SET status = 'failed',
                failure_code = $2,
                pkce_verifier_ciphertext = NULL,
                pkce_nonce = NULL,
                encryption_key_version = NULL
            WHERE id = $1 AND status = 'exchanging'",
        )
        .bind(authorization_id)
        .bind(failure_code)
        .execute(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(())
    }

    /// Completes a claimed authorization and stores only encrypted provider
    /// credentials. A new consent response may omit a refresh token; in that
    /// case an existing token is preserved, while a first connection is
    /// rejected rather than being recorded as usable without credentials.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::IdentityConflict`] if the Google subject does
    /// not match the authenticated Jimin OS user, and a classified storage
    /// error for an invalid or unavailable transaction.
    #[allow(
        clippy::too_many_lines,
        reason = "The complete callback transaction keeps credential preservation and one-time PKCE deletion auditable in one place."
    )]
    pub async fn complete_calendar_oauth_authorization(
        &self,
        command: &CompleteCalendarOAuthAuthorization,
    ) -> Result<CalendarAccount, StorageError> {
        command.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;

        let authorization = sqlx::query_as::<_, CompletionAuthorizationRow>(
            "\
            SELECT authorization.user_id, authorization.status, user_account.google_sub
            FROM calendar_oauth_authorizations AS authorization
            JOIN users AS user_account ON user_account.id = authorization.user_id
            WHERE authorization.id = $1
            FOR UPDATE",
        )
        .bind(command.authorization_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?
        .ok_or(StorageError::InvalidConfiguration)?;
        if authorization.user_id != command.user_id || authorization.status != "exchanging" {
            return Err(StorageError::InvalidConfiguration);
        }
        if authorization.google_sub != command.provider_subject.as_str() {
            return Err(StorageError::IdentityConflict);
        }

        let existing_refresh_token = sqlx::query_scalar::<_, Option<Vec<u8>>>(
            "\
            SELECT refresh_token_ciphertext
            FROM calendar_accounts
            WHERE user_id = $1
            FOR UPDATE",
        )
        .bind(command.user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?
        .flatten();
        if command.refresh_token.is_none() && existing_refresh_token.is_none() {
            return Err(StorageError::InvalidConfiguration);
        }

        let refresh_ciphertext = command
            .refresh_token
            .as_ref()
            .map(|secret| secret.ciphertext.as_slice());
        let refresh_nonce = command
            .refresh_token
            .as_ref()
            .map(|secret| secret.nonce.as_slice());
        let refresh_key_version = command
            .refresh_token
            .as_ref()
            .map(|secret| secret.key_version);
        let row = sqlx::query_as::<_, CalendarAccountRow>(
            "\
            INSERT INTO calendar_accounts (
                id, user_id, provider, provider_subject, email, status,
                granted_scopes, refresh_token_ciphertext, refresh_token_nonce,
                encryption_key_version
            ) VALUES ($1, $2, 'google', $3, $4, 'connecting', $5, $6, $7, $8)
            ON CONFLICT (user_id) DO UPDATE
            SET provider_subject = EXCLUDED.provider_subject,
                email = EXCLUDED.email,
                status = 'connecting',
                granted_scopes = EXCLUDED.granted_scopes,
                refresh_token_ciphertext = COALESCE(
                    EXCLUDED.refresh_token_ciphertext,
                    calendar_accounts.refresh_token_ciphertext
                ),
                refresh_token_nonce = COALESCE(
                    EXCLUDED.refresh_token_nonce,
                    calendar_accounts.refresh_token_nonce
                ),
                encryption_key_version = COALESCE(
                    EXCLUDED.encryption_key_version,
                    calendar_accounts.encryption_key_version
                ),
                last_error_code = NULL
            RETURNING id, email, status, granted_scopes, last_successful_sync_at, version",
        )
        .bind(command.account_id)
        .bind(command.user_id)
        .bind(command.provider_subject.as_str())
        .bind(command.email.display())
        .bind(&command.granted_scopes)
        .bind(refresh_ciphertext)
        .bind(refresh_nonce)
        .bind(refresh_key_version)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        sqlx::query(
            "\
            UPDATE calendar_oauth_authorizations
            SET status = 'completed',
                failure_code = NULL,
                pkce_verifier_ciphertext = NULL,
                pkce_nonce = NULL,
                encryption_key_version = NULL
            WHERE id = $1 AND status = 'exchanging'",
        )
        .bind(command.authorization_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        transaction
            .commit()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        CalendarAccount::try_from(row)
    }
}

/// Encrypted material bound to a Calendar record by the application layer.
/// The storage crate never receives the plaintext token or PKCE verifier.
pub struct EncryptedCalendarSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_version: i32,
}

impl EncryptedCalendarSecret {
    fn valid(&self) -> bool {
        !self.ciphertext.is_empty()
            && self.ciphertext.len() <= MAX_CIPHERTEXT_BYTES
            && self.nonce.len() == XCHACHA_NONCE_BYTES
            && self.key_version > 0
    }
}

/// Inputs for a server-owned, one-time Calendar OAuth transaction.
pub struct CreateCalendarOAuthAuthorization {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub device_id: Uuid,
    pub state_verifier: Vec<u8>,
    pub pkce_verifier: EncryptedCalendarSecret,
    pub client_kind: ClientPlatform,
    pub expires_at: OffsetDateTime,
}

impl CreateCalendarOAuthAuthorization {
    fn validate(&self) -> Result<(), StorageError> {
        if !all_version_seven(&[self.id, self.user_id, self.session_id, self.device_id])
            || self.state_verifier.len() != STATE_VERIFIER_BYTES
            || !self.pkce_verifier.valid()
            || self.expires_at <= OffsetDateTime::now_utc()
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

/// Safe result returned after a transaction is persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedCalendarOAuthAuthorization {
    pub id: Uuid,
    pub expires_at: OffsetDateTime,
}

/// The trusted callback context returned after an atomic state claim. It must
/// remain inside the server process and is intentionally not serializable.
pub struct ClaimedCalendarOAuthAuthorization {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expected_google_subject: GoogleSubject,
    pub client_kind: ClientPlatform,
    pub pkce_verifier: EncryptedCalendarSecret,
}

#[derive(sqlx::FromRow)]
struct ClaimedCalendarOAuthAuthorizationRow {
    id: Uuid,
    user_id: Uuid,
    pkce_verifier_ciphertext: Option<Vec<u8>>,
    pkce_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
    client_kind: String,
    google_sub: String,
}

impl TryFrom<ClaimedCalendarOAuthAuthorizationRow> for ClaimedCalendarOAuthAuthorization {
    type Error = StorageError;

    fn try_from(row: ClaimedCalendarOAuthAuthorizationRow) -> Result<Self, Self::Error> {
        let pkce_verifier = EncryptedCalendarSecret {
            ciphertext: row
                .pkce_verifier_ciphertext
                .ok_or(StorageError::PersistenceUnavailable)?,
            nonce: row.pkce_nonce.ok_or(StorageError::PersistenceUnavailable)?,
            key_version: row
                .encryption_key_version
                .ok_or(StorageError::PersistenceUnavailable)?,
        };
        if !pkce_verifier.valid() {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(Self {
            id: row.id,
            user_id: row.user_id,
            expected_google_subject: GoogleSubject::parse(row.google_sub)
                .map_err(|_| StorageError::PersistenceUnavailable)?,
            client_kind: parse_client_platform(&row.client_kind)?,
            pkce_verifier,
        })
    }
}

/// Inputs already verified at the callback boundary for making a Calendar
/// connection durable.
pub struct CompleteCalendarOAuthAuthorization {
    pub authorization_id: Uuid,
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub provider_subject: GoogleSubject,
    pub email: EmailAddress,
    pub granted_scopes: Vec<String>,
    pub refresh_token: Option<EncryptedCalendarSecret>,
}

impl CompleteCalendarOAuthAuthorization {
    fn validate(&self) -> Result<(), StorageError> {
        if !all_version_seven(&[self.authorization_id, self.account_id, self.user_id])
            || !valid_scopes(&self.granted_scopes)
            || self
                .refresh_token
                .as_ref()
                .is_some_and(|secret| !secret.valid())
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct CompletionAuthorizationRow {
    user_id: Uuid,
    status: String,
    google_sub: String,
}

fn all_version_seven(ids: &[Uuid]) -> bool {
    ids.iter().all(|id| id.get_version_num() == 7)
}

fn parse_client_platform(value: &str) -> Result<ClientPlatform, StorageError> {
    match value {
        "macos" => Ok(ClientPlatform::Macos),
        "ios" => Ok(ClientPlatform::Ios),
        "android" => Ok(ClientPlatform::Android),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn valid_scopes(scopes: &[String]) -> bool {
    !scopes.is_empty()
        && scopes.len() <= MAX_GRANTED_SCOPES
        && scopes.iter().all(|scope| {
            !scope.is_empty()
                && scope.len() <= MAX_SCOPE_BYTES
                && !scope.chars().any(char::is_control)
        })
}

fn valid_failure_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.')
}

#[cfg(test)]
mod tests {
    use super::CalendarAccountStatus;

    #[test]
    fn calendar_account_status_rejects_unknown_values() {
        assert!(CalendarAccountStatus::parse("active").is_ok());
        assert!(CalendarAccountStatus::parse("unexpected").is_err());
    }
}

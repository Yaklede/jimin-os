//! Google Calendar connection metadata and durable sync records.
//!
//! Provider credentials are intentionally represented only as encrypted SQL
//! columns in the migration. This module exposes the safe, client-visible
//! connection summary without returning refresh tokens, sync tokens, or
//! provider event payloads.

use jimin_domain::{ClientPlatform, EmailAddress, GoogleSubject};
use sqlx::{Postgres, Transaction};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::{Database, StorageError};

const STATE_VERIFIER_BYTES: usize = 32;
const XCHACHA_NONCE_BYTES: usize = 24;
const MAX_CIPHERTEXT_BYTES: usize = 8 * 1024;
const MAX_GRANTED_SCOPES: usize = 16;
const MAX_SCOPE_BYTES: usize = 512;
const MAX_FAILURE_CODE_BYTES: usize = 120;
const MAX_EVENT_TITLE_BYTES: usize = 300;
const MAX_EVENT_DESCRIPTION_BYTES: usize = 8_192;
const MAX_EVENT_LOCATION_BYTES: usize = 1_024;
const MAX_EVENT_URL_BYTES: usize = 4_096;
const MAX_RECURRENCE_RULES: usize = 128;

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

    /// Returns the encrypted credential for an account that is eligible for a
    /// Calendar list synchronization. The caller must decrypt it only inside
    /// the server process and must never serialize this value to a client.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PersistenceUnavailable`] when the row is
    /// malformed or the database cannot be queried.
    pub async fn calendar_sync_connection(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<CalendarSyncConnection>, StorageError> {
        let row = sqlx::query_as::<_, CalendarSyncConnectionRow>(
            "\
            SELECT id, user_id, refresh_token_ciphertext, refresh_token_nonce,
                encryption_key_version
            FROM calendar_accounts
            WHERE id = $1
              AND user_id = $2
              AND status IN ('connecting', 'active')",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        row.map(CalendarSyncConnection::try_from).transpose()
    }

    /// Returns the currently selected provider calendars after the Calendar
    /// list has been applied. Credentials never leave the matching account
    /// connection record.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error without exposing provider IDs
    /// to an untrusted caller.
    pub async fn calendar_sync_targets(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<CalendarSyncTarget>, StorageError> {
        let rows = sqlx::query_as::<_, CalendarSyncTargetRow>(
            "\
            SELECT calendar.id, calendar.provider_calendar_id, calendar.time_zone
            FROM calendars AS calendar
            INNER JOIN calendar_accounts AS account ON account.id = calendar.account_id
            WHERE calendar.account_id = $1
              AND account.user_id = $2
              AND account.status IN ('connecting', 'active')
              AND calendar.sync_enabled = TRUE
              AND calendar.provider_deleted_at IS NULL
            ORDER BY calendar.is_primary DESC, calendar.id ASC",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        rows.into_iter().map(CalendarSyncTarget::try_from).collect()
    }

    /// Replaces Calendar metadata only after every provider page has been
    /// validated. The primary and provider-selected visible calendars become
    /// sync-enabled; deleted and hidden calendars remain metadata only.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed provider
    /// entries and [`StorageError::PersistenceUnavailable`] for a failed
    /// transaction.
    #[allow(
        clippy::too_many_lines,
        reason = "The full list transaction keeps tombstoning, calendar upserts, sync-state initialization, and account activation atomic."
    )]
    pub async fn apply_calendar_list_sync(
        &self,
        account_id: Uuid,
        user_id: Uuid,
        entries: &[ProviderCalendar],
    ) -> Result<CalendarListSyncResult, StorageError> {
        if account_id.get_version_num() != 7
            || user_id.get_version_num() != 7
            || !valid_provider_calendars(entries)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        let account_exists = sqlx::query_scalar::<_, Uuid>(
            "\
            SELECT id FROM calendar_accounts
            WHERE id = $1
              AND user_id = $2
              AND status IN ('connecting', 'active')
            FOR UPDATE",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        if account_exists.is_none() {
            return Err(StorageError::InvalidConfiguration);
        }

        let now = OffsetDateTime::now_utc();
        let provider_calendar_ids = entries
            .iter()
            .map(|entry| entry.provider_calendar_id.clone())
            .collect::<Vec<_>>();
        sqlx::query(
            "\
            UPDATE calendars
            SET is_primary = FALSE,
                sync_enabled = FALSE,
                provider_deleted_at = COALESCE(provider_deleted_at, $3)
            WHERE account_id = $1
              AND NOT (provider_calendar_id = ANY($2::TEXT[]))",
        )
        .bind(account_id)
        .bind(&provider_calendar_ids)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        for entry in entries {
            let provider_deleted = entry.visibility == ProviderCalendarVisibility::Deleted;
            let is_primary =
                entry.is_primary && entry.visibility == ProviderCalendarVisibility::Visible;
            let sync_enabled = (is_primary || entry.provider_selected)
                && entry.visibility == ProviderCalendarVisibility::Visible;
            let provider_deleted_at = provider_deleted.then_some(now);
            let calendar_id = sqlx::query_scalar::<_, Uuid>(
                "\
                INSERT INTO calendars (
                    id, account_id, provider_calendar_id, name, description,
                    time_zone, color_id, access_role, is_primary,
                    provider_selected, sync_enabled, provider_etag,
                    provider_deleted_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (account_id, provider_calendar_id) DO UPDATE
                SET name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    time_zone = EXCLUDED.time_zone,
                    color_id = EXCLUDED.color_id,
                    access_role = EXCLUDED.access_role,
                    is_primary = EXCLUDED.is_primary,
                    provider_selected = EXCLUDED.provider_selected,
                    sync_enabled = EXCLUDED.sync_enabled,
                    provider_etag = EXCLUDED.provider_etag,
                    provider_deleted_at = EXCLUDED.provider_deleted_at
                RETURNING id",
            )
            .bind(Uuid::now_v7())
            .bind(account_id)
            .bind(&entry.provider_calendar_id)
            .bind(&entry.name)
            .bind(&entry.description)
            .bind(&entry.time_zone)
            .bind(&entry.color_id)
            .bind(&entry.access_role)
            .bind(is_primary)
            .bind(entry.provider_selected)
            .bind(sync_enabled)
            .bind(&entry.provider_etag)
            .bind(provider_deleted_at)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
            sqlx::query(
                "\
                INSERT INTO calendar_sync_states (
                    id, calendar_id, status, query_fingerprint
                ) VALUES ($1, $2, 'idle', 'google-events-v1')
                ON CONFLICT (calendar_id) DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(calendar_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        }

        sqlx::query(
            "\
            UPDATE calendar_accounts
            SET status = 'active',
                last_successful_sync_at = $3,
                last_error_code = NULL
            WHERE id = $1 AND user_id = $2",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(CalendarListSyncResult {
            calendar_count: entries.len(),
        })
    }

    /// Atomically replaces one selected calendar's provider event read model
    /// only after the server has fetched and validated every provider page.
    /// Missing entries are tombstoned, so a partial provider response can
    /// never erase the currently visible schedule.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed provider
    /// values or ownership mismatches and a classified persistence error for
    /// a transaction failure.
    #[allow(
        clippy::too_many_lines,
        reason = "The full replace remains in one transaction to keep the provider read model and sync state coherent."
    )]
    pub async fn apply_calendar_event_full_sync(
        &self,
        account_id: Uuid,
        user_id: Uuid,
        calendar_id: Uuid,
        events: &[ProviderCalendarEvent],
    ) -> Result<CalendarEventSyncResult, StorageError> {
        if !all_version_seven(&[account_id, user_id, calendar_id]) || !valid_provider_events(events)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        let target_exists = sqlx::query_scalar::<_, Uuid>(
            "\
            SELECT calendar.id
            FROM calendars AS calendar
            INNER JOIN calendar_accounts AS account ON account.id = calendar.account_id
            WHERE calendar.id = $1
              AND calendar.account_id = $2
              AND account.user_id = $3
              AND account.status IN ('connecting', 'active')
              AND calendar.sync_enabled = TRUE
              AND calendar.provider_deleted_at IS NULL
            FOR UPDATE",
        )
        .bind(calendar_id)
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        if target_exists.is_none() {
            return Err(StorageError::InvalidConfiguration);
        }

        let now = OffsetDateTime::now_utc();
        let present_provider_ids = events
            .iter()
            .map(|event| event.provider_event_id.as_str())
            .collect::<Vec<_>>();
        sqlx::query(
            "\
            UPDATE calendar_events
            SET provider_status = 'cancelled',
                provider_deleted_at = COALESCE(provider_deleted_at, $2),
                sync_state = 'synced'
            WHERE calendar_id = $1
              AND NOT (provider_event_id = ANY($3::TEXT[]))",
        )
        .bind(calendar_id)
        .bind(now)
        .bind(&present_provider_ids)
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        let mut active_count = 0_usize;
        for event in events {
            if event.status == ProviderCalendarEventStatus::Cancelled {
                sqlx::query(
                    "\
                    UPDATE calendar_events
                    SET provider_status = 'cancelled',
                        provider_etag = COALESCE($3, provider_etag),
                        provider_updated_at = COALESCE($4, provider_updated_at),
                        provider_deleted_at = COALESCE(provider_deleted_at, $5),
                        sync_state = 'synced'
                    WHERE calendar_id = $1 AND provider_event_id = $2",
                )
                .bind(calendar_id)
                .bind(&event.provider_event_id)
                .bind(&event.provider_etag)
                .bind(event.provider_updated_at)
                .bind(now)
                .execute(&mut *transaction)
                .await
                .map_err(|_| StorageError::PersistenceUnavailable)?;
                continue;
            }
            match event
                .time
                .as_ref()
                .ok_or(StorageError::InvalidConfiguration)?
            {
                ProviderCalendarEventTime::Date { start, end } => {
                    upsert_date_calendar_event(
                        &mut transaction,
                        calendar_id,
                        user_id,
                        event,
                        *start,
                        *end,
                    )
                    .await?;
                }
                ProviderCalendarEventTime::DateTime {
                    start,
                    end,
                    time_zone,
                } => {
                    upsert_timed_calendar_event(
                        &mut transaction,
                        calendar_id,
                        user_id,
                        event,
                        *start,
                        *end,
                        time_zone,
                    )
                    .await?;
                }
            }
            active_count += 1;
        }

        sqlx::query(
            "\
            UPDATE calendar_sync_states
            SET status = 'idle', last_started_at = COALESCE(last_started_at, $2),
                last_successful_sync_at = $2, consecutive_failures = 0,
                next_attempt_at = NULL, last_error_code = NULL
            WHERE calendar_id = $1",
        )
        .bind(calendar_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(CalendarEventSyncResult { active_count })
    }

    /// Preserves the last successfully synchronized Calendar read model while
    /// recording a classified provider error for reconnect or retry UI.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for unsafe IDs or error
    /// codes and [`StorageError::PersistenceUnavailable`] when the update
    /// cannot be stored.
    pub async fn mark_calendar_sync_failure(
        &self,
        account_id: Uuid,
        user_id: Uuid,
        failure_code: &str,
    ) -> Result<(), StorageError> {
        if account_id.get_version_num() != 7
            || user_id.get_version_num() != 7
            || !valid_failure_code(failure_code)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "\
            UPDATE calendar_accounts
            SET status = 'error', last_error_code = $3
            WHERE id = $1
              AND user_id = $2
              AND status IN ('connecting', 'active')",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(failure_code)
        .execute(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(())
    }
}

/// Encrypted material bound to a Calendar record by the application layer.
/// The storage crate never receives the plaintext token or PKCE verifier.
pub struct EncryptedCalendarSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_version: i32,
}

/// Encrypted account credential available only to a server sync worker.
pub struct CalendarSyncConnection {
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub refresh_token: EncryptedCalendarSecret,
}

/// One provider calendar eligible for a server-owned event synchronization.
/// Provider IDs are never serialized by API handlers.
pub struct CalendarSyncTarget {
    pub calendar_id: Uuid,
    pub provider_calendar_id: String,
    pub time_zone: String,
}

#[derive(sqlx::FromRow)]
struct CalendarSyncTargetRow {
    id: Uuid,
    provider_calendar_id: String,
    time_zone: String,
}

impl TryFrom<CalendarSyncTargetRow> for CalendarSyncTarget {
    type Error = StorageError;

    fn try_from(row: CalendarSyncTargetRow) -> Result<Self, Self::Error> {
        if row.id.get_version_num() != 7
            || !valid_required_text(&row.provider_calendar_id, 1_024)
            || !valid_required_text(&row.time_zone, 80)
        {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(Self {
            calendar_id: row.id,
            provider_calendar_id: row.provider_calendar_id,
            time_zone: row.time_zone,
        })
    }
}

#[derive(sqlx::FromRow)]
struct CalendarSyncConnectionRow {
    id: Uuid,
    user_id: Uuid,
    refresh_token_ciphertext: Option<Vec<u8>>,
    refresh_token_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
}

impl TryFrom<CalendarSyncConnectionRow> for CalendarSyncConnection {
    type Error = StorageError;

    fn try_from(row: CalendarSyncConnectionRow) -> Result<Self, Self::Error> {
        let refresh_token = EncryptedCalendarSecret {
            ciphertext: row
                .refresh_token_ciphertext
                .ok_or(StorageError::PersistenceUnavailable)?,
            nonce: row
                .refresh_token_nonce
                .ok_or(StorageError::PersistenceUnavailable)?,
            key_version: row
                .encryption_key_version
                .ok_or(StorageError::PersistenceUnavailable)?,
        };
        if !refresh_token.valid() {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(Self {
            account_id: row.id,
            user_id: row.user_id,
            refresh_token,
        })
    }
}

/// Validated Calendar metadata fetched from Google's Calendar list API.
pub struct ProviderCalendar {
    pub provider_calendar_id: String,
    pub name: String,
    pub description: Option<String>,
    pub time_zone: String,
    pub color_id: Option<String>,
    pub access_role: String,
    pub is_primary: bool,
    pub provider_selected: bool,
    pub visibility: ProviderCalendarVisibility,
    pub provider_etag: Option<String>,
}

/// Provider visibility normalized before persistence. Hidden calendars remain
/// in the metadata list but are not selected for event synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCalendarVisibility {
    Visible,
    Hidden,
    Deleted,
}

/// Validated, server-only event payload fetched from a Calendar provider.
pub struct ProviderCalendarEvent {
    pub provider_event_id: String,
    pub provider_etag: Option<String>,
    pub provider_updated_at: Option<OffsetDateTime>,
    pub ical_uid: Option<String>,
    pub status: ProviderCalendarEventStatus,
    pub event_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub time: Option<ProviderCalendarEventTime>,
    pub recurrence: Option<Vec<String>>,
    pub recurring_provider_event_id: Option<String>,
    pub visibility: Option<String>,
    pub transparency: Option<String>,
    pub html_link: Option<String>,
    pub is_editable: bool,
}

/// Time is stored as either an all-day date range or an absolute timed range.
pub enum ProviderCalendarEventTime {
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

/// Provider status normalized before a read-model transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCalendarEventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

/// Safe count returned after a successful atomic Calendar list application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarListSyncResult {
    pub calendar_count: usize,
}

/// Safe active-event count returned after applying one complete provider list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarEventSyncResult {
    pub active_count: usize,
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

fn valid_provider_calendars(entries: &[ProviderCalendar]) -> bool {
    entries.iter().all(|entry| {
        valid_required_text(&entry.provider_calendar_id, 1_024)
            && valid_required_text(&entry.name, 500)
            && valid_required_text(&entry.time_zone, 80)
            && valid_access_role(&entry.access_role)
            && entry
                .description
                .as_deref()
                .is_none_or(|value| valid_required_text(value, 8_192))
            && entry
                .color_id
                .as_deref()
                .is_none_or(|value| valid_required_text(value, 120))
            && entry
                .provider_etag
                .as_deref()
                .is_none_or(|value| valid_required_text(value, 2_048))
    })
}

fn valid_provider_events(events: &[ProviderCalendarEvent]) -> bool {
    events.iter().all(|event| {
        valid_required_text(&event.provider_event_id, 1_024)
            && valid_event_type(&event.event_type)
            && event
                .provider_etag
                .as_deref()
                .is_none_or(|value| valid_required_text(value, 2_048))
            && event
                .ical_uid
                .as_deref()
                .is_none_or(|value| valid_required_text(value, 2_048))
            && event
                .title
                .as_deref()
                .is_none_or(|value| valid_required_text(value, MAX_EVENT_TITLE_BYTES))
            && (event.status == ProviderCalendarEventStatus::Cancelled || event.title.is_some())
            && event
                .description
                .as_deref()
                .is_none_or(|value| valid_required_text(value, MAX_EVENT_DESCRIPTION_BYTES))
            && event
                .location
                .as_deref()
                .is_none_or(|value| valid_required_text(value, MAX_EVENT_LOCATION_BYTES))
            && event
                .recurrence
                .as_ref()
                .is_none_or(|rules| valid_recurrence(rules))
            && event
                .recurring_provider_event_id
                .as_deref()
                .is_none_or(|value| valid_required_text(value, 1_024))
            && event.visibility.as_deref().is_none_or(|value| {
                matches!(value, "default" | "public" | "private" | "confidential")
            })
            && event
                .transparency
                .as_deref()
                .is_none_or(|value| matches!(value, "opaque" | "transparent"))
            && event
                .html_link
                .as_deref()
                .is_none_or(|value| valid_https_url(value, MAX_EVENT_URL_BYTES))
            && valid_event_time(event)
    })
}

fn valid_event_time(event: &ProviderCalendarEvent) -> bool {
    match (&event.status, &event.time) {
        (ProviderCalendarEventStatus::Cancelled, None) => true,
        (_, Some(ProviderCalendarEventTime::Date { start, end })) => end > start,
        (
            _,
            Some(ProviderCalendarEventTime::DateTime {
                start,
                end,
                time_zone,
            }),
        ) => end > start && valid_required_text(time_zone, 80),
        _ => false,
    }
}

fn valid_event_type(value: &str) -> bool {
    matches!(
        value,
        "default" | "birthday" | "focus_time" | "from_gmail" | "out_of_office" | "working_location"
    )
}

fn valid_recurrence(rules: &[String]) -> bool {
    !rules.is_empty()
        && rules.len() <= MAX_RECURRENCE_RULES
        && rules.iter().all(|rule| valid_required_text(rule, 1_024))
}

fn valid_https_url(value: &str, maximum_bytes: usize) -> bool {
    valid_required_text(value, maximum_bytes)
        && value.starts_with("https://")
        && value[8..]
            .split('/')
            .next()
            .is_some_and(|authority| !authority.is_empty() && !authority.contains('@'))
}

async fn upsert_date_calendar_event(
    transaction: &mut Transaction<'_, Postgres>,
    calendar_id: Uuid,
    user_id: Uuid,
    event: &ProviderCalendarEvent,
    start: Date,
    end: Date,
) -> Result<(), StorageError> {
    upsert_calendar_event(
        transaction,
        calendar_id,
        user_id,
        event,
        "date",
        None,
        None,
        Some(start),
        Some(end),
        None,
    )
    .await
}

async fn upsert_timed_calendar_event(
    transaction: &mut Transaction<'_, Postgres>,
    calendar_id: Uuid,
    user_id: Uuid,
    event: &ProviderCalendarEvent,
    start: OffsetDateTime,
    end: OffsetDateTime,
    time_zone: &str,
) -> Result<(), StorageError> {
    upsert_calendar_event(
        transaction,
        calendar_id,
        user_id,
        event,
        "date_time",
        Some(start),
        Some(end),
        None,
        None,
        Some(time_zone),
    )
    .await
}

#[allow(
    clippy::too_many_arguments,
    reason = "Calendar event storage mirrors the mutually exclusive time columns in the migration."
)]
async fn upsert_calendar_event(
    transaction: &mut Transaction<'_, Postgres>,
    calendar_id: Uuid,
    user_id: Uuid,
    event: &ProviderCalendarEvent,
    time_kind: &str,
    start_at: Option<OffsetDateTime>,
    end_at: Option<OffsetDateTime>,
    start_date: Option<Date>,
    end_date: Option<Date>,
    source_time_zone: Option<&str>,
) -> Result<(), StorageError> {
    let recurrence = event
        .recurrence
        .as_ref()
        .map(|rules| serde_json::json!(rules));
    sqlx::query(
        "\
        INSERT INTO calendar_events (
            id, user_id, calendar_id, provider_event_id, provider_etag,
            provider_updated_at, ical_uid, provider_status, event_type,
            title, description_text, location, time_kind, start_at, end_at,
            start_date, end_date, source_time_zone, recurrence,
            recurring_provider_event_id, visibility, transparency, html_link,
            is_editable, sync_state, provider_deleted_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
            $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, 'synced', NULL
        )
        ON CONFLICT (calendar_id, provider_event_id) DO UPDATE
        SET provider_etag = EXCLUDED.provider_etag,
            provider_updated_at = EXCLUDED.provider_updated_at,
            ical_uid = EXCLUDED.ical_uid,
            provider_status = EXCLUDED.provider_status,
            event_type = EXCLUDED.event_type,
            title = EXCLUDED.title,
            description_text = EXCLUDED.description_text,
            location = EXCLUDED.location,
            time_kind = EXCLUDED.time_kind,
            start_at = EXCLUDED.start_at,
            end_at = EXCLUDED.end_at,
            start_date = EXCLUDED.start_date,
            end_date = EXCLUDED.end_date,
            source_time_zone = EXCLUDED.source_time_zone,
            recurrence = EXCLUDED.recurrence,
            recurring_provider_event_id = EXCLUDED.recurring_provider_event_id,
            visibility = EXCLUDED.visibility,
            transparency = EXCLUDED.transparency,
            html_link = EXCLUDED.html_link,
            is_editable = EXCLUDED.is_editable,
            sync_state = 'synced',
            provider_deleted_at = NULL",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(calendar_id)
    .bind(&event.provider_event_id)
    .bind(&event.provider_etag)
    .bind(event.provider_updated_at)
    .bind(&event.ical_uid)
    .bind(provider_event_status(event.status))
    .bind(&event.event_type)
    .bind(
        event
            .title
            .as_deref()
            .ok_or(StorageError::InvalidConfiguration)?,
    )
    .bind(&event.description)
    .bind(&event.location)
    .bind(time_kind)
    .bind(start_at)
    .bind(end_at)
    .bind(start_date)
    .bind(end_date)
    .bind(source_time_zone)
    .bind(recurrence)
    .bind(&event.recurring_provider_event_id)
    .bind(&event.visibility)
    .bind(&event.transparency)
    .bind(&event.html_link)
    .bind(event.is_editable)
    .execute(&mut **transaction)
    .await
    .map_err(|_| StorageError::PersistenceUnavailable)?;
    Ok(())
}

fn provider_event_status(status: ProviderCalendarEventStatus) -> &'static str {
    match status {
        ProviderCalendarEventStatus::Confirmed => "confirmed",
        ProviderCalendarEventStatus::Tentative => "tentative",
        ProviderCalendarEventStatus::Cancelled => "cancelled",
    }
}

fn valid_required_text(value: &str, maximum_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

fn valid_access_role(value: &str) -> bool {
    matches!(value, "free_busy_reader" | "reader" | "writer" | "owner")
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
    use super::{
        CalendarAccountStatus, ProviderCalendar, ProviderCalendarEvent,
        ProviderCalendarEventStatus, ProviderCalendarEventTime, ProviderCalendarVisibility,
        valid_provider_calendars, valid_provider_events,
    };
    use time::{Date, Month};

    #[test]
    fn calendar_account_status_rejects_unknown_values() {
        assert!(CalendarAccountStatus::parse("active").is_ok());
        assert!(CalendarAccountStatus::parse("unexpected").is_err());
    }

    #[test]
    fn provider_calendar_validation_rejects_unknown_access_roles() {
        let valid = ProviderCalendar {
            provider_calendar_id: "primary".to_owned(),
            name: "Personal".to_owned(),
            description: None,
            time_zone: "Asia/Seoul".to_owned(),
            color_id: None,
            access_role: "owner".to_owned(),
            is_primary: true,
            provider_selected: true,
            visibility: ProviderCalendarVisibility::Visible,
            provider_etag: None,
        };
        assert!(valid_provider_calendars(&[valid]));

        let invalid = ProviderCalendar {
            provider_calendar_id: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
            time_zone: "Asia/Seoul".to_owned(),
            color_id: None,
            access_role: "admin".to_owned(),
            is_primary: false,
            provider_selected: false,
            visibility: ProviderCalendarVisibility::Hidden,
            provider_etag: None,
        };
        assert!(!valid_provider_calendars(&[invalid]));
    }

    #[test]
    fn provider_event_validation_keeps_all_day_time_semantics() {
        let valid = ProviderCalendarEvent {
            provider_event_id: "event-1".to_owned(),
            provider_etag: None,
            provider_updated_at: None,
            ical_uid: None,
            status: ProviderCalendarEventStatus::Confirmed,
            event_type: "default".to_owned(),
            title: Some("연차".to_owned()),
            description: None,
            location: None,
            time: Some(ProviderCalendarEventTime::Date {
                start: Date::from_calendar_date(2026, Month::July, 12)
                    .expect("fixture date should be valid"),
                end: Date::from_calendar_date(2026, Month::July, 13)
                    .expect("fixture date should be valid"),
            }),
            recurrence: None,
            recurring_provider_event_id: None,
            visibility: None,
            transparency: None,
            html_link: None,
            is_editable: false,
        };
        assert!(valid_provider_events(&[valid]));
    }
}

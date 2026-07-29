//! Workspace-scoped Gmail identities and bounded inbox metadata.
//!
//! Gmail is intentionally independent from the user's Calendar credential. A
//! user may link multiple Google identities, but every identity belongs to
//! exactly one personal or company workspace. Message bodies, attachments,
//! raw provider payloads, and plaintext OAuth credentials never enter storage.

use jimin_domain::{ClientPlatform, EmailAddress, GoogleSubject};
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::{append_change, append_delete_change},
};

const MAX_INBOX_MESSAGES: i64 = 100;
const MAX_PROVIDER_ID_BYTES: usize = 255;
const MAX_SENDER_BYTES: usize = 1_024;
const MAX_SUBJECT_BYTES: usize = 998;
const MAX_SNIPPET_BYTES: usize = 512;
const STATE_VERIFIER_BYTES: usize = 32;
const XCHACHA_NONCE_BYTES: usize = 24;
const MAX_CIPHERTEXT_BYTES: usize = 8 * 1_024;
const MAX_GRANTED_SCOPES: usize = 16;
const MAX_SCOPE_BYTES: usize = 512;

/// Lifecycle of one independently authorized Gmail identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailAccountStatus {
    Connecting,
    Active,
    ReauthRequired,
    Revoking,
    Revoked,
    Error,
}

impl GmailAccountStatus {
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

/// Safe account metadata returned to an authenticated owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailAccount {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub workspace_scope: String,
    pub workspace_name: String,
    pub email: String,
    pub status: GmailAccountStatus,
    pub granted_scopes: Vec<String>,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub last_error_code: Option<String>,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
struct GmailAccountRow {
    id: Uuid,
    workspace_id: Uuid,
    workspace_scope: String,
    workspace_name: String,
    email: String,
    status: String,
    granted_scopes: Vec<String>,
    last_successful_sync_at: Option<OffsetDateTime>,
    last_error_code: Option<String>,
    version: i64,
}

impl TryFrom<GmailAccountRow> for GmailAccount {
    type Error = StorageError;

    fn try_from(row: GmailAccountRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            workspace_scope: row.workspace_scope,
            workspace_name: row.workspace_name,
            email: row.email,
            status: GmailAccountStatus::parse(&row.status)?,
            granted_scopes: row.granted_scopes,
            last_successful_sync_at: row.last_successful_sync_at,
            last_error_code: row.last_error_code,
            version: row.version,
        })
    }
}

/// AEAD-protected provider or PKCE material.
pub struct EncryptedGmailSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_version: i32,
}

impl EncryptedGmailSecret {
    fn valid(&self) -> bool {
        !self.ciphertext.is_empty()
            && self.ciphertext.len() <= MAX_CIPHERTEXT_BYTES
            && self.nonce.len() == XCHACHA_NONCE_BYTES
            && self.key_version > 0
    }
}

/// Validated server-created Gmail consent transaction.
pub struct CreateGmailOAuthAuthorization {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub reconnect_account_id: Option<Uuid>,
    pub session_id: Uuid,
    pub device_id: Uuid,
    pub state_verifier: Vec<u8>,
    pub pkce_verifier: EncryptedGmailSecret,
    pub client_kind: ClientPlatform,
    pub expires_at: OffsetDateTime,
}

/// One claimed callback. The workspace is loaded from the server-owned OAuth
/// transaction and is never accepted from the callback query.
pub struct ClaimedGmailOAuthAuthorization {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub expected_provider_subject: Option<GoogleSubject>,
    pub client_kind: ClientPlatform,
    pub pkce_verifier: EncryptedGmailSecret,
}

#[derive(sqlx::FromRow)]
struct ClaimedGmailOAuthAuthorizationRow {
    id: Uuid,
    user_id: Uuid,
    workspace_id: Uuid,
    expected_provider_subject: Option<String>,
    client_kind: String,
    pkce_verifier_ciphertext: Option<Vec<u8>>,
    pkce_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
}

/// Verified OAuth result prepared for atomic persistence.
pub struct CompleteGmailOAuthAuthorization {
    pub authorization_id: Uuid,
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub provider_subject: GoogleSubject,
    pub email: EmailAddress,
    pub granted_scopes: Vec<String>,
    pub refresh_token: Option<EncryptedGmailSecret>,
}

/// Encrypted account material available only inside a server sync worker.
pub struct GmailSyncConnection {
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub provider_subject: String,
    pub refresh_token: EncryptedGmailSecret,
}

#[derive(sqlx::FromRow)]
struct GmailSyncConnectionRow {
    account_id: Uuid,
    user_id: Uuid,
    workspace_id: Uuid,
    provider_subject: String,
    refresh_token_ciphertext: Option<Vec<u8>>,
    refresh_token_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
}

/// Account identity queued for the periodic Gmail reconciliation loop.
pub struct GmailSyncIdentity {
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
}

/// Result of a version-checked account disconnect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteGmailAccountOutcome {
    Deleted,
    AlreadyAbsent,
    VersionConflict,
}

/// Validated metadata for one Gmail message, supplied only by a provider
/// adapter after it has discarded the message body.
pub struct ProviderGmailMessage {
    pub provider_message_id: String,
    pub provider_thread_id: String,
    pub received_at: Option<OffsetDateTime>,
    pub sender: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub is_unread: bool,
}

/// Compact inbox entry available to server-side intelligence. Workspace and
/// account tags make any explicit scoped use auditable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailMessage {
    pub id: Uuid,
    pub account_id: Uuid,
    pub workspace_id: Uuid,
    pub account_email: String,
    pub received_at: Option<OffsetDateTime>,
    pub sender: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub is_unread: bool,
}

#[derive(sqlx::FromRow)]
struct GmailMessageRow {
    id: Uuid,
    account_id: Uuid,
    workspace_id: Uuid,
    account_email: String,
    received_at: Option<OffsetDateTime>,
    sender: Option<String>,
    subject: Option<String>,
    snippet: Option<String>,
    is_unread: bool,
}

impl From<GmailMessageRow> for GmailMessage {
    fn from(row: GmailMessageRow) -> Self {
        Self {
            id: row.id,
            account_id: row.account_id,
            workspace_id: row.workspace_id,
            account_email: row.account_email,
            received_at: row.received_at,
            sender: row.sender,
            subject: row.subject,
            snippet: row.snippet,
            is_unread: row.is_unread,
        }
    }
}

impl Database {
    /// Lists every non-revoked Gmail identity owned by the current user.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when account state is unavailable.
    pub async fn gmail_accounts_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<GmailAccount>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        self.ensure_default_workspaces(user_id).await?;
        let rows = sqlx::query_as::<_, GmailAccountRow>(
            "\
            SELECT account.id, account.workspace_id,
                workspace.scope AS workspace_scope,
                workspace.name AS workspace_name,
                account.email, account.status, account.granted_scopes,
                account.last_successful_sync_at, account.last_error_code,
                account.version
            FROM gmail_accounts AS account
            JOIN workspaces AS workspace
              ON workspace.id = account.workspace_id
             AND workspace.user_id = account.user_id
            WHERE account.user_id = $1 AND account.status <> 'revoked'
            ORDER BY account.workspace_id, account.email, account.id",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(GmailAccount::try_from).collect()
    }

    /// Loads one owned account for a reconnect flow without exposing provider
    /// subject or credential material.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for malformed IDs and a classified
    /// persistence error otherwise.
    pub async fn gmail_account_for_user(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<GmailAccount>, StorageError> {
        if !all_v7(&[user_id, account_id]) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, GmailAccountRow>(
            "\
            SELECT account.id, account.workspace_id,
                workspace.scope AS workspace_scope,
                workspace.name AS workspace_name,
                account.email, account.status, account.granted_scopes,
                account.last_successful_sync_at, account.last_error_code,
                account.version
            FROM gmail_accounts AS account
            JOIN workspaces AS workspace
              ON workspace.id = account.workspace_id
             AND workspace.user_id = account.user_id
            WHERE account.id = $1 AND account.user_id = $2
              AND account.status <> 'revoked'",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(GmailAccount::try_from).transpose()
    }

    /// Persists one device-bound Gmail consent transaction only when the
    /// target workspace belongs to the signed-in owner.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for malformed or foreign workspace input.
    pub async fn create_gmail_oauth_authorization(
        &self,
        command: &CreateGmailOAuthAuthorization,
    ) -> Result<(), StorageError> {
        validate_oauth_command(command)?;
        self.ensure_default_workspaces(command.user_id).await?;
        let inserted = sqlx::query(
            "\
            INSERT INTO gmail_oauth_authorizations (
                id, user_id, workspace_id, reconnect_account_id,
                session_id, device_id, state_verifier,
                pkce_verifier_ciphertext, pkce_nonce, encryption_key_version,
                client_kind, status, expires_at
            )
            SELECT $1, $2, workspace.id, account.id, $5, $6, $7, $8, $9, $10,
                $11, 'pending', $12
            FROM workspaces AS workspace
            LEFT JOIN gmail_accounts AS account
              ON account.id = $4
             AND account.user_id = $2
             AND account.workspace_id = workspace.id
             AND account.status <> 'revoked'
            WHERE workspace.id = $3 AND workspace.user_id = $2
              AND ($4::UUID IS NULL OR account.id IS NOT NULL)",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.workspace_id)
        .bind(command.reconnect_account_id)
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
        .map_err(classify)?;
        if inserted.rows_affected() != 1 {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }

    /// Atomically claims an unexpired Gmail OAuth state exactly once.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error if the claim cannot be recorded.
    pub async fn claim_gmail_oauth_authorization(
        &self,
        state_verifier: &[u8],
    ) -> Result<Option<ClaimedGmailOAuthAuthorization>, StorageError> {
        if state_verifier.len() != STATE_VERIFIER_BYTES {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, ClaimedGmailOAuthAuthorizationRow>(
            "\
            UPDATE gmail_oauth_authorizations AS oauth
            SET status = 'exchanging'
            FROM users, workspaces
            WHERE oauth.state_verifier = $1
              AND oauth.status = 'pending'
              AND oauth.expires_at > NOW()
              AND users.id = oauth.user_id
              AND users.status = 'active'
              AND workspaces.id = oauth.workspace_id
              AND workspaces.user_id = oauth.user_id
            RETURNING oauth.id, oauth.user_id,
                oauth.workspace_id,
                (
                    SELECT provider_subject
                    FROM gmail_accounts
                    WHERE id = oauth.reconnect_account_id
                      AND user_id = oauth.user_id
                      AND workspace_id = oauth.workspace_id
                ) AS expected_provider_subject,
                oauth.client_kind,
                oauth.pkce_verifier_ciphertext,
                oauth.pkce_nonce, oauth.encryption_key_version",
        )
        .bind(state_verifier)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(claimed_authorization).transpose()
    }

    /// Cryptographically deletes claimed PKCE material and records only a safe
    /// failure classification.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for unsafe IDs or error codes.
    pub async fn fail_gmail_oauth_authorization(
        &self,
        authorization_id: Uuid,
        failure_code: &str,
    ) -> Result<(), StorageError> {
        if !is_v7(authorization_id) || !valid_failure_code(failure_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "\
            UPDATE gmail_oauth_authorizations
            SET status = 'failed', failure_code = $2,
                pkce_verifier_ciphertext = NULL, pkce_nonce = NULL,
                encryption_key_version = NULL
            WHERE id = $1 AND status = 'exchanging'",
        )
        .bind(authorization_id)
        .bind(failure_code)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }

    /// Expires abandoned Gmail consent transactions and removes their PKCE
    /// material. This is safe to run repeatedly from every reconciliation
    /// cycle.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when cleanup cannot be recorded.
    pub async fn expire_gmail_oauth_authorizations(&self) -> Result<u64, StorageError> {
        let expired = sqlx::query(
            "\
            UPDATE gmail_oauth_authorizations
            SET status = 'expired',
                failure_code = 'gmail.authorization_expired',
                pkce_verifier_ciphertext = NULL,
                pkce_nonce = NULL,
                encryption_key_version = NULL
            WHERE status IN ('pending', 'exchanging')
              AND expires_at <= NOW()",
        )
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(expired.rows_affected())
    }

    /// Consumes a claimed authorization and creates or reconnects exactly one
    /// Gmail identity inside its original workspace.
    ///
    /// # Errors
    ///
    /// Returns identity conflict if the same Google identity is already bound
    /// to another workspace.
    pub async fn complete_gmail_oauth_authorization(
        &self,
        command: &CompleteGmailOAuthAuthorization,
    ) -> Result<GmailAccount, StorageError> {
        validate_completion(command)?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let account_id = gmail_completion_account_id(&mut transaction, command).await?;
        let account = upsert_completed_gmail_account(&mut transaction, command, account_id).await?;
        sqlx::query(
            "\
            INSERT INTO gmail_sync_states (account_id, workspace_id, status)
            VALUES ($1, $2, 'idle')
            ON CONFLICT (account_id) DO UPDATE
            SET workspace_id = EXCLUDED.workspace_id, status = 'idle',
                last_error_code = NULL",
        )
        .bind(account_id)
        .bind(command.workspace_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        sqlx::query(
            "\
            UPDATE gmail_oauth_authorizations
            SET status = 'completed', failure_code = NULL,
                pkce_verifier_ciphertext = NULL, pkce_nonce = NULL,
                encryption_key_version = NULL
            WHERE id = $1 AND status = 'exchanging'",
        )
        .bind(command.authorization_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        append_change(
            &mut transaction,
            command.user_id,
            "gmail_account",
            account.id,
            account.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(account)
    }

    /// Loads one active account's encrypted sync material.
    ///
    /// # Errors
    ///
    /// Returns a classified error for invalid persisted credential state.
    pub async fn gmail_sync_connection(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<GmailSyncConnection>, StorageError> {
        if !is_v7(account_id) || !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, GmailSyncConnectionRow>(
            "\
            SELECT id AS account_id, user_id, workspace_id, provider_subject,
                refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
            FROM gmail_accounts
            WHERE id = $1 AND user_id = $2
              AND status IN ('active', 'error')",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(sync_connection).transpose()
    }

    /// Returns every active account eligible for periodic synchronization.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when storage is unavailable.
    pub async fn active_gmail_sync_identities(
        &self,
    ) -> Result<Vec<GmailSyncIdentity>, StorageError> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid)>(
            "\
            SELECT id, user_id, workspace_id
            FROM gmail_accounts
            WHERE status IN ('active', 'error')
              AND refresh_token_ciphertext IS NOT NULL
            ORDER BY last_successful_sync_at NULLS FIRST, id",
        )
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(rows
            .into_iter()
            .map(|(account_id, user_id, workspace_id)| GmailSyncIdentity {
                account_id,
                user_id,
                workspace_id,
            })
            .collect())
    }

    /// Applies one bounded account snapshot. Gmail's first page is a window,
    /// so messages omitted by the provider are not tombstoned.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for foreign ownership or unsafe metadata.
    pub async fn apply_gmail_inbox_sync(
        &self,
        account_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
        messages: &[ProviderGmailMessage],
    ) -> Result<GmailAccount, StorageError> {
        if !all_v7(&[account_id, user_id, workspace_id]) || !valid_messages(messages) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let owns_account = sqlx::query_scalar::<_, bool>(
            "\
            SELECT EXISTS(
                SELECT 1 FROM gmail_accounts
                WHERE id = $1 AND user_id = $2 AND workspace_id = $3
                  AND status IN ('active', 'error')
            )",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(workspace_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        if !owns_account {
            return Err(StorageError::InvalidConfiguration);
        }
        upsert_gmail_messages(&mut transaction, account_id, workspace_id, messages).await?;
        sqlx::query(
            "\
            INSERT INTO gmail_sync_states (
                account_id, workspace_id, status, last_successful_sync_at,
                last_error_code
            ) VALUES ($1, $2, 'idle', NOW(), NULL)
            ON CONFLICT (account_id) DO UPDATE
            SET workspace_id = EXCLUDED.workspace_id, status = 'idle',
                last_successful_sync_at = NOW(), last_error_code = NULL",
        )
        .bind(account_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        let row = sqlx::query_as::<_, GmailAccountRow>(
            "\
            UPDATE gmail_accounts
            SET status = 'active', last_successful_sync_at = NOW(),
                last_error_code = NULL
            WHERE id = $1 AND user_id = $2 AND workspace_id = $3
            RETURNING id, workspace_id,
                (SELECT scope FROM workspaces
                 WHERE workspaces.id = gmail_accounts.workspace_id)
                    AS workspace_scope,
                (SELECT name FROM workspaces
                 WHERE workspaces.id = gmail_accounts.workspace_id)
                    AS workspace_name,
                email, status, granted_scopes, last_successful_sync_at,
                last_error_code, version",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(workspace_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let account = GmailAccount::try_from(row)?;
        append_change(
            &mut transaction,
            user_id,
            "gmail_account",
            account.id,
            account.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(account)
    }

    /// Records only a safe sync failure code. Authentication failures can
    /// require reconnecting one account without affecting other mailboxes.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for unsafe input or foreign ownership.
    pub async fn mark_gmail_sync_failure(
        &self,
        account_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
        failure_code: &str,
        reauth_required: bool,
    ) -> Result<Option<GmailAccount>, StorageError> {
        if !all_v7(&[account_id, user_id, workspace_id]) || !valid_failure_code(failure_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, GmailAccountRow>(
            "\
            UPDATE gmail_accounts
            SET status = CASE WHEN $5 THEN 'reauth_required' ELSE 'error' END,
                last_error_code = $4
            WHERE id = $1 AND user_id = $2 AND workspace_id = $3
            RETURNING id, workspace_id,
                (SELECT scope FROM workspaces
                 WHERE workspaces.id = gmail_accounts.workspace_id)
                    AS workspace_scope,
                (SELECT name FROM workspaces
                 WHERE workspaces.id = gmail_accounts.workspace_id)
                    AS workspace_name,
                email, status, granted_scopes, last_successful_sync_at,
                last_error_code, version",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(workspace_id)
        .bind(failure_code)
        .bind(reauth_required)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        sqlx::query(
            "\
            UPDATE gmail_sync_states
            SET status = 'error', last_error_code = $3
            WHERE account_id = $1 AND workspace_id = $2",
        )
        .bind(account_id)
        .bind(workspace_id)
        .bind(failure_code)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        let account = row.map(GmailAccount::try_from).transpose()?;
        if let Some(account) = account.as_ref() {
            append_change(
                &mut transaction,
                user_id,
                "gmail_account",
                account.id,
                account.version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(account)
    }

    /// Disconnects one version-matched Gmail account and only its own cached
    /// messages. Calendar and sibling Gmail identities remain untouched.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration for malformed optimistic concurrency input.
    pub async fn delete_gmail_account(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        expected_version: i64,
    ) -> Result<DeleteGmailAccountOutcome, StorageError> {
        if !all_v7(&[user_id, account_id]) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let current_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM gmail_accounts WHERE id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(current_version) = current_version else {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteGmailAccountOutcome::AlreadyAbsent);
        };
        if current_version != expected_version {
            transaction.commit().await.map_err(classify)?;
            return Ok(DeleteGmailAccountOutcome::VersionConflict);
        }
        sqlx::query("DELETE FROM gmail_accounts WHERE id = $1 AND user_id = $2")
            .bind(account_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        append_delete_change(
            &mut transaction,
            user_id,
            "gmail_account",
            account_id,
            current_version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(DeleteGmailAccountOutcome::Deleted)
    }

    /// Returns recent metadata for an explicit workspace boundary.
    ///
    /// # Errors
    ///
    /// Returns invalid configuration if the workspace is foreign.
    pub async fn recent_gmail_messages_for_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<GmailMessage>, StorageError> {
        if !all_v7(&[user_id, workspace_id]) {
            return Err(StorageError::InvalidConfiguration);
        }
        self.ensure_default_workspaces(user_id).await?;
        let owns_workspace = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2
            )",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(self.pool())
        .await
        .map_err(classify)?;
        if !owns_workspace {
            return Err(StorageError::InvalidConfiguration);
        }
        recent_messages(self, user_id, Some(workspace_id), false).await
    }

    /// Legacy global intelligence calls remain conservative: if more than one
    /// active mailbox exists, no message content is injected. Callers that
    /// know their workspace must use [`Self::recent_gmail_messages_for_workspace`].
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error on unavailable storage.
    pub async fn recent_gmail_messages_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<GmailMessage>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        recent_messages(self, user_id, None, true).await
    }
}

async fn gmail_completion_account_id(
    transaction: &mut Transaction<'_, Postgres>,
    command: &CompleteGmailOAuthAuthorization,
) -> Result<Uuid, StorageError> {
    let authorization = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(
        "\
        SELECT oauth.user_id, oauth.workspace_id,
            oauth.status, account.provider_subject
        FROM gmail_oauth_authorizations AS oauth
        LEFT JOIN gmail_accounts AS account
          ON account.id = oauth.reconnect_account_id
         AND account.user_id = oauth.user_id
         AND account.workspace_id = oauth.workspace_id
        WHERE oauth.id = $1
        FOR UPDATE OF oauth",
    )
    .bind(command.authorization_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)?
    .ok_or(StorageError::InvalidConfiguration)?;
    if authorization.0 != command.user_id
        || authorization.1 != command.workspace_id
        || authorization.2 != "exchanging"
    {
        return Err(StorageError::InvalidConfiguration);
    }
    if authorization
        .3
        .as_deref()
        .is_some_and(|expected| expected != command.provider_subject.as_str())
    {
        return Err(StorageError::IdentityConflict);
    }
    let existing = sqlx::query_as::<_, (Uuid, Uuid, Option<Vec<u8>>)>(
        "\
        SELECT id, workspace_id, refresh_token_ciphertext
        FROM gmail_accounts
        WHERE user_id = $1 AND provider_subject = $2
        FOR UPDATE",
    )
    .bind(command.user_id)
    .bind(command.provider_subject.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)?;
    if existing
        .as_ref()
        .is_some_and(|(_, workspace_id, _)| *workspace_id != command.workspace_id)
        || (authorization.3.is_none() && existing.is_some())
    {
        return Err(StorageError::IdentityConflict);
    }
    if command.refresh_token.is_none()
        && existing
            .as_ref()
            .is_none_or(|(_, _, refresh)| refresh.is_none())
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(existing.as_ref().map_or(command.account_id, |row| row.0))
}

async fn upsert_completed_gmail_account(
    transaction: &mut Transaction<'_, Postgres>,
    command: &CompleteGmailOAuthAuthorization,
    account_id: Uuid,
) -> Result<GmailAccount, StorageError> {
    let row = sqlx::query_as::<_, GmailAccountRow>(
        "\
        INSERT INTO gmail_accounts (
            id, user_id, workspace_id, provider_subject, email, status,
            granted_scopes, refresh_token_ciphertext, refresh_token_nonce,
            encryption_key_version
        ) VALUES ($1, $2, $3, $4, $5, 'active', $6, $7, $8, $9)
        ON CONFLICT (user_id, provider_subject) DO UPDATE
        SET email = EXCLUDED.email, status = 'active',
            granted_scopes = EXCLUDED.granted_scopes,
            refresh_token_ciphertext = COALESCE(
                EXCLUDED.refresh_token_ciphertext,
                gmail_accounts.refresh_token_ciphertext
            ),
            refresh_token_nonce = COALESCE(
                EXCLUDED.refresh_token_nonce,
                gmail_accounts.refresh_token_nonce
            ),
            encryption_key_version = COALESCE(
                EXCLUDED.encryption_key_version,
                gmail_accounts.encryption_key_version
            ),
            last_error_code = NULL
        RETURNING id, workspace_id,
            (SELECT scope FROM workspaces
             WHERE workspaces.id = gmail_accounts.workspace_id)
                AS workspace_scope,
            (SELECT name FROM workspaces
             WHERE workspaces.id = gmail_accounts.workspace_id)
                AS workspace_name,
            email, status, granted_scopes, last_successful_sync_at,
            last_error_code, version",
    )
    .bind(account_id)
    .bind(command.user_id)
    .bind(command.workspace_id)
    .bind(command.provider_subject.as_str())
    .bind(command.email.display())
    .bind(&command.granted_scopes)
    .bind(
        command
            .refresh_token
            .as_ref()
            .map(|value| value.ciphertext.as_slice()),
    )
    .bind(
        command
            .refresh_token
            .as_ref()
            .map(|value| value.nonce.as_slice()),
    )
    .bind(
        command
            .refresh_token
            .as_ref()
            .map(|value| value.key_version),
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)?;
    GmailAccount::try_from(row)
}

async fn upsert_gmail_messages(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    workspace_id: Uuid,
    messages: &[ProviderGmailMessage],
) -> Result<(), StorageError> {
    for message in messages {
        sqlx::query(
            "\
            INSERT INTO gmail_messages (
                id, account_id, workspace_id, provider_message_id,
                provider_thread_id, received_at, sender, subject, snippet,
                is_unread, provider_deleted_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL)
            ON CONFLICT (account_id, provider_message_id) DO UPDATE
            SET provider_thread_id = EXCLUDED.provider_thread_id,
                workspace_id = EXCLUDED.workspace_id,
                received_at = EXCLUDED.received_at,
                sender = EXCLUDED.sender,
                subject = EXCLUDED.subject,
                snippet = EXCLUDED.snippet,
                is_unread = EXCLUDED.is_unread,
                provider_deleted_at = NULL",
        )
        .bind(Uuid::now_v7())
        .bind(account_id)
        .bind(workspace_id)
        .bind(&message.provider_message_id)
        .bind(&message.provider_thread_id)
        .bind(message.received_at)
        .bind(&message.sender)
        .bind(&message.subject)
        .bind(&message.snippet)
        .bind(message.is_unread)
        .execute(&mut **transaction)
        .await
        .map_err(classify)?;
    }
    Ok(())
}

async fn recent_messages(
    database: &Database,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    require_single_active_account: bool,
) -> Result<Vec<GmailMessage>, StorageError> {
    let rows = sqlx::query_as::<_, GmailMessageRow>(
        "\
        SELECT message.id, message.account_id, message.workspace_id,
            account.email AS account_email, message.received_at, message.sender,
            message.subject, message.snippet, message.is_unread
        FROM gmail_messages AS message
        JOIN gmail_accounts AS account
          ON account.id = message.account_id
         AND account.workspace_id = message.workspace_id
        JOIN workspaces AS workspace
          ON workspace.id = message.workspace_id
         AND workspace.user_id = account.user_id
        WHERE account.user_id = $1
          AND account.status = 'active'
          AND message.provider_deleted_at IS NULL
          AND ($2::UUID IS NULL OR message.workspace_id = $2)
          AND (
              NOT $3
              OR (
                  SELECT COUNT(*)
                  FROM gmail_accounts AS eligible
                  WHERE eligible.user_id = $1 AND eligible.status = 'active'
              ) = 1
          )
        ORDER BY message.is_unread DESC,
            message.received_at DESC NULLS LAST, message.id DESC
        LIMIT $4",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(require_single_active_account)
    .bind(MAX_INBOX_MESSAGES)
    .fetch_all(database.pool())
    .await
    .map_err(classify)?;
    Ok(rows.into_iter().map(GmailMessage::from).collect())
}

fn claimed_authorization(
    row: ClaimedGmailOAuthAuthorizationRow,
) -> Result<ClaimedGmailOAuthAuthorization, StorageError> {
    let pkce_verifier = EncryptedGmailSecret {
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
    Ok(ClaimedGmailOAuthAuthorization {
        id: row.id,
        user_id: row.user_id,
        workspace_id: row.workspace_id,
        expected_provider_subject: row
            .expected_provider_subject
            .map(GoogleSubject::parse)
            .transpose()
            .map_err(|_| StorageError::PersistenceUnavailable)?,
        client_kind: parse_client_platform(&row.client_kind)?,
        pkce_verifier,
    })
}

fn sync_connection(row: GmailSyncConnectionRow) -> Result<GmailSyncConnection, StorageError> {
    let refresh_token = EncryptedGmailSecret {
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
    Ok(GmailSyncConnection {
        account_id: row.account_id,
        user_id: row.user_id,
        workspace_id: row.workspace_id,
        provider_subject: row.provider_subject,
        refresh_token,
    })
}

fn validate_oauth_command(command: &CreateGmailOAuthAuthorization) -> Result<(), StorageError> {
    if !all_v7(&[
        command.id,
        command.user_id,
        command.workspace_id,
        command.session_id,
        command.device_id,
    ]) || command.state_verifier.len() != STATE_VERIFIER_BYTES
        || command
            .reconnect_account_id
            .is_some_and(|account_id| !is_v7(account_id))
        || !command.pkce_verifier.valid()
        || command.expires_at <= OffsetDateTime::now_utc()
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn validate_completion(command: &CompleteGmailOAuthAuthorization) -> Result<(), StorageError> {
    if !all_v7(&[
        command.authorization_id,
        command.account_id,
        command.user_id,
        command.workspace_id,
    ]) || !valid_scopes(&command.granted_scopes)
        || command
            .refresh_token
            .as_ref()
            .is_some_and(|secret| !secret.valid())
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
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

fn valid_messages(messages: &[ProviderGmailMessage]) -> bool {
    messages.len() <= usize::try_from(MAX_INBOX_MESSAGES).expect("constant fits usize")
        && messages.iter().all(|message| {
            valid_text(&message.provider_message_id, MAX_PROVIDER_ID_BYTES)
                && valid_text(&message.provider_thread_id, MAX_PROVIDER_ID_BYTES)
                && message
                    .sender
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_SENDER_BYTES))
                && message
                    .subject
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_SUBJECT_BYTES))
                && message
                    .snippet
                    .as_deref()
                    .is_none_or(|value| valid_text(value, MAX_SNIPPET_BYTES))
        })
}

fn valid_text(value: &str, maximum_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

fn valid_failure_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_')
        })
}

fn all_v7(values: &[Uuid]) -> bool {
    values.iter().all(|value| is_v7(*value))
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn parse_client_platform(value: &str) -> Result<ClientPlatform, StorageError> {
    match value {
        "macos" => Ok(ClientPlatform::Macos),
        "ios" => Ok(ClientPlatform::Ios),
        "android" => Ok(ClientPlatform::Android),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use jimin_domain::ClientPlatform;
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    use super::{
        CreateGmailOAuthAuthorization, EncryptedGmailSecret, ProviderGmailMessage,
        valid_failure_code, valid_messages, validate_oauth_command,
    };

    #[test]
    fn inbox_metadata_rejects_controls_and_accepts_a_safe_header() {
        let valid = ProviderGmailMessage {
            provider_message_id: "message-1".to_owned(),
            provider_thread_id: "thread-1".to_owned(),
            received_at: None,
            sender: Some("Jimin <jimin@example.com>".to_owned()),
            subject: Some("계약 검토".to_owned()),
            snippet: Some("오늘 오후에 확인해 주세요.".to_owned()),
            is_unread: true,
        };
        assert!(valid_messages(&[valid]));

        let invalid = ProviderGmailMessage {
            provider_message_id: "message-2".to_owned(),
            provider_thread_id: "thread-2".to_owned(),
            received_at: None,
            sender: Some("unsafe\nheader".to_owned()),
            subject: None,
            snippet: None,
            is_unread: false,
        };
        assert!(!valid_messages(&[invalid]));
    }

    #[test]
    fn oauth_command_requires_a_workspace_and_bound_encrypted_pkce() {
        let mut command = CreateGmailOAuthAuthorization {
            id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            reconnect_account_id: None,
            session_id: Uuid::now_v7(),
            device_id: Uuid::now_v7(),
            state_verifier: vec![7; 32],
            pkce_verifier: EncryptedGmailSecret {
                ciphertext: vec![8; 48],
                nonce: vec![9; 24],
                key_version: 1,
            },
            client_kind: ClientPlatform::Android,
            expires_at: OffsetDateTime::now_utc() + Duration::minutes(10),
        };
        assert!(validate_oauth_command(&command).is_ok());
        command.workspace_id = Uuid::nil();
        assert!(validate_oauth_command(&command).is_err());
    }

    #[test]
    fn failure_codes_accept_the_runtime_taxonomy_without_accepting_unsafe_separators() {
        assert!(valid_failure_code("gmail.authorization_rejected"));
        assert!(valid_failure_code("gmail.provider_unavailable"));
        assert!(valid_failure_code("gmail.required_scope_missing"));
        assert!(!valid_failure_code("gmail.ProviderUnavailable"));
        assert!(!valid_failure_code("gmail.failure-with-dash"));
    }
}

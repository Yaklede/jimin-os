//! Company Google Chat connections and project inflow persistence.
//!
//! Calendar and Chat credentials intentionally live in different tables. A
//! user may keep one personal Calendar connection while linking multiple work
//! Google identities to selected company projects.

use jimin_domain::{ClientPlatform, EmailAddress, GoogleSubject};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    Database, StorageError,
    auth::{append_change, append_delete_change},
    calendar::EncryptedCalendarSecret,
    planning::queue_task_webhook_in_transaction,
};

const STATE_VERIFIER_BYTES: usize = 32;
const XCHACHA_NONCE_BYTES: usize = 24;
const MAX_CIPHERTEXT_BYTES: usize = 8 * 1024;
const MAX_GRANTED_SCOPES: usize = 16;
const MAX_SCOPE_BYTES: usize = 512;
const MAX_FAILURE_CODE_BYTES: usize = 120;
const MAX_SPACE_NAME_BYTES: usize = 256;
const MAX_DISPLAY_NAME_CHARS: usize = 500;
const MAX_MESSAGE_NAME_BYTES: usize = 1_024;
const MAX_MESSAGE_TEXT_CHARS: usize = 32_768;
const MAX_TASK_NOTES_CHARS: usize = 10_000;
const MAX_ASSIGNEE_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleChatAccountStatus {
    Connecting,
    Active,
    ReauthRequired,
    Revoking,
    Revoked,
    Error,
}

impl GoogleChatAccountStatus {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleChatAccount {
    pub id: Uuid,
    pub email: String,
    pub status: GoogleChatAccountStatus,
    pub granted_scopes: Vec<String>,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub last_error_code: Option<String>,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
struct GoogleChatAccountRow {
    id: Uuid,
    email: String,
    status: String,
    granted_scopes: Vec<String>,
    last_successful_sync_at: Option<OffsetDateTime>,
    last_error_code: Option<String>,
    version: i64,
}

impl TryFrom<GoogleChatAccountRow> for GoogleChatAccount {
    type Error = StorageError;

    fn try_from(row: GoogleChatAccountRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            email: row.email,
            status: GoogleChatAccountStatus::parse(&row.status)?,
            granted_scopes: row.granted_scopes,
            last_successful_sync_at: row.last_successful_sync_at,
            last_error_code: row.last_error_code,
            version: row.version,
        })
    }
}

pub struct CreateGoogleChatOAuthAuthorization {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub device_id: Uuid,
    pub state_verifier: Vec<u8>,
    pub pkce_verifier: EncryptedCalendarSecret,
    pub client_kind: ClientPlatform,
    pub expires_at: OffsetDateTime,
}

pub struct ClaimedGoogleChatOAuthAuthorization {
    pub id: Uuid,
    pub user_id: Uuid,
    pub client_kind: ClientPlatform,
    pub pkce_verifier: EncryptedCalendarSecret,
}

#[derive(sqlx::FromRow)]
struct ClaimedGoogleChatOAuthAuthorizationRow {
    id: Uuid,
    user_id: Uuid,
    client_kind: String,
    pkce_verifier_ciphertext: Option<Vec<u8>>,
    pkce_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
}

pub struct CompleteGoogleChatOAuthAuthorization {
    pub authorization_id: Uuid,
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub provider_subject: GoogleSubject,
    pub email: EmailAddress,
    pub granted_scopes: Vec<String>,
    pub refresh_token: Option<EncryptedCalendarSecret>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGoogleChatSource {
    pub id: Uuid,
    pub project_id: Uuid,
    pub account_id: Uuid,
    pub account_email: String,
    pub space_name: String,
    pub display_name: String,
    pub enabled: bool,
    pub acknowledge_with_reaction: bool,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub last_error_code: Option<String>,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
struct ProjectGoogleChatSourceRow {
    id: Uuid,
    project_id: Uuid,
    account_id: Uuid,
    account_email: String,
    space_name: String,
    display_name: String,
    enabled: bool,
    acknowledge_with_reaction: bool,
    last_successful_sync_at: Option<OffsetDateTime>,
    last_error_code: Option<String>,
    version: i64,
}

impl From<ProjectGoogleChatSourceRow> for ProjectGoogleChatSource {
    fn from(row: ProjectGoogleChatSourceRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            account_id: row.account_id,
            account_email: row.account_email,
            space_name: row.space_name,
            display_name: row.display_name,
            enabled: row.enabled,
            acknowledge_with_reaction: row.acknowledge_with_reaction,
            last_successful_sync_at: row.last_successful_sync_at,
            last_error_code: row.last_error_code,
            version: row.version,
        }
    }
}

pub struct NewProjectGoogleChatSource {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub account_id: Uuid,
    pub space_name: String,
    pub display_name: String,
    pub acknowledge_with_reaction: bool,
    pub import_history: bool,
}

pub struct PromoteProjectInflowItem {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub item_id: Uuid,
    pub expected_version: i64,
    pub task_id: Uuid,
    pub title: String,
    pub notes: Option<String>,
    pub assignee_name: Option<String>,
    pub priority: i16,
    pub due_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectInflowStatus {
    Pending,
    Promoted,
    Dismissed,
}

impl ProjectInflowStatus {
    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "pending" => Ok(Self::Pending),
            "promoted" => Ok(Self::Promoted),
            "dismissed" => Ok(Self::Dismissed),
            _ => Err(StorageError::PersistenceUnavailable),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInflowItem {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub source_id: Uuid,
    pub source_name: String,
    pub provider_thread_name: Option<String>,
    pub sender_provider_name: Option<String>,
    pub sender_name: Option<String>,
    pub sent_by_owner: bool,
    pub content_text: String,
    pub received_at: OffsetDateTime,
    pub status: ProjectInflowStatus,
    pub promoted_task_id: Option<Uuid>,
    pub acknowledged_at: Option<OffsetDateTime>,
    pub completion_requested_at: Option<OffsetDateTime>,
    pub completion_reaction_at: Option<OffsetDateTime>,
    pub completion_reply_at: Option<OffsetDateTime>,
    pub completion_delivery_error_code: Option<String>,
    pub completion_delivery_attempt_count: i32,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
struct ProjectInflowItemRow {
    id: Uuid,
    project_id: Uuid,
    project_name: String,
    source_id: Uuid,
    source_name: String,
    provider_thread_name: Option<String>,
    sender_provider_name: Option<String>,
    sender_name: Option<String>,
    sent_by_owner: bool,
    content_text: String,
    received_at: OffsetDateTime,
    status: String,
    promoted_task_id: Option<Uuid>,
    acknowledged_at: Option<OffsetDateTime>,
    completion_requested_at: Option<OffsetDateTime>,
    completion_reaction_at: Option<OffsetDateTime>,
    completion_reply_at: Option<OffsetDateTime>,
    completion_delivery_error_code: Option<String>,
    completion_delivery_attempt_count: i32,
    version: i64,
}

impl TryFrom<ProjectInflowItemRow> for ProjectInflowItem {
    type Error = StorageError;

    fn try_from(row: ProjectInflowItemRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            project_id: row.project_id,
            project_name: row.project_name,
            source_id: row.source_id,
            source_name: row.source_name,
            provider_thread_name: row.provider_thread_name,
            sender_provider_name: row.sender_provider_name,
            sender_name: row.sender_name,
            sent_by_owner: row.sent_by_owner,
            content_text: row.content_text,
            received_at: row.received_at,
            status: ProjectInflowStatus::parse(&row.status)?,
            promoted_task_id: row.promoted_task_id,
            acknowledged_at: row.acknowledged_at,
            completion_requested_at: row.completion_requested_at,
            completion_reaction_at: row.completion_reaction_at,
            completion_reply_at: row.completion_reply_at,
            completion_delivery_error_code: row.completion_delivery_error_code,
            completion_delivery_attempt_count: row.completion_delivery_attempt_count,
            version: row.version,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleChatCompletionDelivery {
    pub inflow_id: Uuid,
    pub user_id: Uuid,
    pub source_id: Uuid,
    pub provider_message_name: String,
    pub provider_thread_name: Option<String>,
    pub task_id: Uuid,
    pub task_title: String,
    pub assignee_name: Option<String>,
    pub due_at: Option<OffsetDateTime>,
    pub reaction_completed: bool,
    pub reply_completed: bool,
    pub attempt_count: i32,
}

#[derive(sqlx::FromRow)]
struct GoogleChatCompletionDeliveryRow {
    inflow_id: Uuid,
    user_id: Uuid,
    source_id: Uuid,
    provider_message_name: String,
    provider_thread_name: Option<String>,
    task_id: Uuid,
    task_title: String,
    assignee_name: Option<String>,
    due_at: Option<OffsetDateTime>,
    reaction_completed: bool,
    reply_completed: bool,
    attempt_count: i32,
}

impl From<GoogleChatCompletionDeliveryRow> for GoogleChatCompletionDelivery {
    fn from(row: GoogleChatCompletionDeliveryRow) -> Self {
        Self {
            inflow_id: row.inflow_id,
            user_id: row.user_id,
            source_id: row.source_id,
            provider_message_name: row.provider_message_name,
            provider_thread_name: row.provider_thread_name,
            task_id: row.task_id,
            task_title: row.task_title,
            assignee_name: row.assignee_name,
            due_at: row.due_at,
            reaction_completed: row.reaction_completed,
            reply_completed: row.reply_completed,
            attempt_count: row.attempt_count,
        }
    }
}

pub struct ProviderGoogleChatMessage {
    pub provider_message_name: String,
    pub provider_thread_name: Option<String>,
    pub sender_provider_name: Option<String>,
    pub sender_name: Option<String>,
    pub content_text: String,
    pub received_at: OffsetDateTime,
}

pub struct GoogleChatSourceSyncConnection {
    pub source_id: Uuid,
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub provider_subject: String,
    pub granted_scopes: Vec<String>,
    pub space_name: String,
    pub acknowledge_with_reaction: bool,
    pub last_provider_message_at: Option<OffsetDateTime>,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub source_had_error: bool,
    pub account_needs_recovery: bool,
    pub refresh_token: EncryptedCalendarSecret,
}

#[derive(sqlx::FromRow)]
struct GoogleChatSourceSyncConnectionRow {
    source_id: Uuid,
    account_id: Uuid,
    user_id: Uuid,
    project_id: Uuid,
    provider_subject: String,
    granted_scopes: Vec<String>,
    space_name: String,
    acknowledge_with_reaction: bool,
    last_provider_message_at: Option<OffsetDateTime>,
    last_successful_sync_at: Option<OffsetDateTime>,
    source_had_error: bool,
    account_needs_recovery: bool,
    refresh_token_ciphertext: Option<Vec<u8>>,
    refresh_token_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
}

pub struct GoogleChatAccountConnection {
    pub account_id: Uuid,
    pub user_id: Uuid,
    pub provider_subject: String,
    pub refresh_token: EncryptedCalendarSecret,
}

#[derive(sqlx::FromRow)]
struct GoogleChatAccountConnectionRow {
    account_id: Uuid,
    user_id: Uuid,
    provider_subject: String,
    refresh_token_ciphertext: Option<Vec<u8>>,
    refresh_token_nonce: Option<Vec<u8>>,
    encryption_key_version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInflowAcknowledgement {
    pub inflow_id: Uuid,
    pub provider_message_name: String,
}

impl Database {
    /// Lists every non-revoked company Chat identity linked by one owner.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the owner ID is invalid or the query fails.
    pub async fn google_chat_accounts_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<GoogleChatAccount>, StorageError> {
        let rows = sqlx::query_as::<_, GoogleChatAccountRow>(
            "SELECT id, email, status, granted_scopes, last_successful_sync_at, last_error_code, version
             FROM google_chat_accounts
             WHERE user_id = $1 AND status <> 'revoked'
             ORDER BY email, id",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(GoogleChatAccount::try_from).collect()
    }

    /// Persists one short-lived, device-bound company Chat OAuth attempt.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the authorization cannot be saved.
    pub async fn create_google_chat_oauth_authorization(
        &self,
        command: &CreateGoogleChatOAuthAuthorization,
    ) -> Result<(), StorageError> {
        validate_oauth_command(command)?;
        sqlx::query(
            "INSERT INTO google_chat_oauth_authorizations (
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
        .map_err(classify)?;
        Ok(())
    }

    /// Claims an unexpired company Chat OAuth attempt exactly once.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the claim cannot be completed atomically.
    pub async fn claim_google_chat_oauth_authorization(
        &self,
        state_verifier: &[u8],
    ) -> Result<Option<ClaimedGoogleChatOAuthAuthorization>, StorageError> {
        if state_verifier.len() != STATE_VERIFIER_BYTES {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, ClaimedGoogleChatOAuthAuthorizationRow>(
            "UPDATE google_chat_oauth_authorizations AS oauth_authorization
             SET status = 'exchanging'
             FROM users
             WHERE oauth_authorization.state_verifier = $1
               AND oauth_authorization.status = 'pending'
               AND oauth_authorization.expires_at > NOW()
               AND users.id = oauth_authorization.user_id
               AND users.status = 'active'
             RETURNING oauth_authorization.id, oauth_authorization.user_id,
                oauth_authorization.client_kind,
                oauth_authorization.pkce_verifier_ciphertext, oauth_authorization.pkce_nonce,
                oauth_authorization.encryption_key_version",
        )
        .bind(state_verifier)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(claimed_authorization).transpose()
    }

    /// Removes PKCE material and records a bounded authorization failure code.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the failure cannot be recorded.
    pub async fn fail_google_chat_oauth_authorization(
        &self,
        authorization_id: Uuid,
        failure_code: &str,
    ) -> Result<(), StorageError> {
        if !is_v7(authorization_id) || !valid_failure_code(failure_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "UPDATE google_chat_oauth_authorizations
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

    /// Atomically consumes an OAuth attempt and stores the encrypted company credential.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when ownership or credential state is invalid.
    pub async fn complete_google_chat_oauth_authorization(
        &self,
        command: &CompleteGoogleChatOAuthAuthorization,
    ) -> Result<GoogleChatAccount, StorageError> {
        validate_completion(command)?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let authorization_user = sqlx::query_scalar::<_, Uuid>(
            "SELECT user_id FROM google_chat_oauth_authorizations
             WHERE id = $1 AND status = 'exchanging' FOR UPDATE",
        )
        .bind(command.authorization_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::InvalidConfiguration)?;
        if authorization_user != command.user_id {
            return Err(StorageError::InvalidConfiguration);
        }
        let existing = sqlx::query_as::<_, (Uuid, Option<Vec<u8>>)>(
            "SELECT id, refresh_token_ciphertext FROM google_chat_accounts
             WHERE user_id = $1 AND provider_subject = $2 FOR UPDATE",
        )
        .bind(command.user_id)
        .bind(command.provider_subject.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if command.refresh_token.is_none()
            && existing
                .as_ref()
                .is_none_or(|(_, refresh)| refresh.is_none())
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let account_id = existing.as_ref().map_or(command.account_id, |row| row.0);
        let row = sqlx::query_as::<_, GoogleChatAccountRow>(
            "INSERT INTO google_chat_accounts (
                id, user_id, provider_subject, email, status, granted_scopes,
                refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
             ) VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, $8)
             ON CONFLICT (user_id, provider_subject) DO UPDATE
             SET email = EXCLUDED.email, status = 'active',
                 granted_scopes = EXCLUDED.granted_scopes,
                 refresh_token_ciphertext = COALESCE(EXCLUDED.refresh_token_ciphertext, google_chat_accounts.refresh_token_ciphertext),
                 refresh_token_nonce = COALESCE(EXCLUDED.refresh_token_nonce, google_chat_accounts.refresh_token_nonce),
                 encryption_key_version = COALESCE(EXCLUDED.encryption_key_version, google_chat_accounts.encryption_key_version),
                 last_error_code = NULL
             RETURNING id, email, status, granted_scopes, last_successful_sync_at, last_error_code, version",
        )
        .bind(account_id)
        .bind(command.user_id)
        .bind(command.provider_subject.as_str())
        .bind(command.email.display())
        .bind(&command.granted_scopes)
        .bind(command.refresh_token.as_ref().map(|value| value.ciphertext.as_slice()))
        .bind(command.refresh_token.as_ref().map(|value| value.nonce.as_slice()))
        .bind(command.refresh_token.as_ref().map(|value| value.key_version))
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        sqlx::query(
            "UPDATE google_chat_oauth_authorizations
             SET status = 'completed', failure_code = NULL,
                 pkce_verifier_ciphertext = NULL, pkce_nonce = NULL,
                 encryption_key_version = NULL
             WHERE id = $1 AND status = 'exchanging'",
        )
        .bind(command.authorization_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        let account = GoogleChatAccount::try_from(row)?;
        append_change(
            &mut transaction,
            command.user_id,
            "google_chat_account",
            account.id,
            account.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(account)
    }

    /// Loads encrypted provider material for one owner-scoped company account.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the credential cannot be loaded or validated.
    pub async fn google_chat_account_connection(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<GoogleChatAccountConnection>, StorageError> {
        let row = sqlx::query_as::<_, GoogleChatAccountConnectionRow>(
            "SELECT id AS account_id, user_id, provider_subject,
                refresh_token_ciphertext, refresh_token_nonce, encryption_key_version
             FROM google_chat_accounts
             WHERE id = $1 AND user_id = $2
               AND status IN ('active', 'error', 'reauth_required')",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(account_connection).transpose()
    }

    /// Deletes one version-matched company account and its project sources locally.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when deletion cannot be completed.
    pub async fn delete_google_chat_account(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        expected_version: i64,
    ) -> Result<bool, StorageError> {
        if !is_v7(user_id) || !is_v7(account_id) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let deleted = sqlx::query_scalar::<_, i64>(
            "DELETE FROM google_chat_accounts
             WHERE id = $1 AND user_id = $2 AND version = $3
             RETURNING version",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(version) = deleted {
            append_delete_change(
                &mut transaction,
                user_id,
                "google_chat_account",
                account_id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(deleted.is_some())
    }

    /// Lists Chat spaces monitored by one owned project.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the source list cannot be loaded.
    pub async fn project_google_chat_sources(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ProjectGoogleChatSource>, StorageError> {
        let rows = sqlx::query_as::<_, ProjectGoogleChatSourceRow>(
            "SELECT source.id, source.project_id, source.account_id,
                account.email AS account_email, source.space_name, source.display_name,
                source.enabled, source.acknowledge_with_reaction,
                source.last_successful_sync_at, source.last_error_code, source.version
             FROM project_google_chat_sources AS source
             JOIN google_chat_accounts AS account ON account.id = source.account_id
             WHERE source.user_id = $1 AND source.project_id = $2
             ORDER BY source.display_name, source.id",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(rows
            .into_iter()
            .map(ProjectGoogleChatSource::from)
            .collect())
    }

    /// Connects an active company account and visible Chat space to an owned project.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when ownership or input is invalid.
    pub async fn create_project_google_chat_source(
        &self,
        command: &NewProjectGoogleChatSource,
    ) -> Result<ProjectGoogleChatSource, StorageError> {
        validate_source(command)?;
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let row = sqlx::query_as::<_, ProjectGoogleChatSourceRow>(
            "INSERT INTO project_google_chat_sources (
                id, user_id, project_id, account_id, space_name, display_name,
                acknowledge_with_reaction, last_provider_message_at
             )
             SELECT $1, $2, project.id, account.id, $5, $6, $7,
                CASE WHEN $8 THEN NULL ELSE NOW() END
             FROM projects AS project
             JOIN google_chat_accounts AS account
               ON account.id = $4 AND account.user_id = $2 AND account.status = 'active'
             WHERE project.id = $3 AND project.user_id = $2
             RETURNING id, project_id, account_id,
                (SELECT email FROM google_chat_accounts WHERE id = account_id) AS account_email,
                space_name, display_name, enabled, acknowledge_with_reaction,
                last_successful_sync_at, last_error_code, version",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.account_id)
        .bind(command.space_name.trim())
        .bind(command.display_name.trim())
        .bind(command.acknowledge_with_reaction)
        .bind(command.import_history)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::InvalidConfiguration)?;
        let source = ProjectGoogleChatSource::from(row);
        append_change(
            &mut transaction,
            command.user_id,
            "project_google_chat_source",
            source.id,
            source.version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(source)
    }

    /// Removes one version-matched project Chat source and its captured inflow.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when deletion cannot be completed.
    pub async fn delete_project_google_chat_source(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        source_id: Uuid,
        expected_version: i64,
    ) -> Result<bool, StorageError> {
        if ![user_id, project_id, source_id].into_iter().all(is_v7) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let deleted = sqlx::query_scalar::<_, i64>(
            "DELETE FROM project_google_chat_sources
             WHERE id = $1 AND user_id = $2 AND project_id = $3 AND version = $4
             RETURNING version",
        )
        .bind(source_id)
        .bind(user_id)
        .bind(project_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(version) = deleted {
            append_delete_change(
                &mut transaction,
                user_id,
                "project_google_chat_source",
                source_id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(deleted.is_some())
    }

    /// Lists enabled source IDs for the bounded background synchronization loop.
    ///
    /// # Errors
    ///
    /// Returns a storage error when active sources cannot be loaded.
    pub async fn active_google_chat_source_ids(&self) -> Result<Vec<Uuid>, StorageError> {
        sqlx::query_scalar(
            "SELECT source.id
             FROM project_google_chat_sources AS source
             JOIN google_chat_accounts AS account ON account.id = source.account_id
             WHERE source.enabled = TRUE AND account.status IN ('active', 'error')
             ORDER BY source.last_successful_sync_at NULLS FIRST, source.id",
        )
        .fetch_all(self.pool())
        .await
        .map_err(classify)
    }

    /// Loads one source and its encrypted account credential for server-side synchronization.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the source or encrypted credential is invalid.
    pub async fn google_chat_source_sync_connection(
        &self,
        source_id: Uuid,
    ) -> Result<Option<GoogleChatSourceSyncConnection>, StorageError> {
        let row = sqlx::query_as::<_, GoogleChatSourceSyncConnectionRow>(
            "SELECT source.id AS source_id, account.id AS account_id,
                source.user_id, source.project_id,
                account.provider_subject, account.granted_scopes,
                source.space_name, source.acknowledge_with_reaction,
                source.last_provider_message_at,
                source.last_successful_sync_at,
                source.last_error_code IS NOT NULL AS source_had_error,
                (account.status <> 'active' OR account.last_error_code IS NOT NULL)
                    AS account_needs_recovery,
                account.refresh_token_ciphertext,
                account.refresh_token_nonce, account.encryption_key_version
             FROM project_google_chat_sources AS source
             JOIN google_chat_accounts AS account ON account.id = source.account_id
             WHERE source.id = $1 AND source.enabled = TRUE
               AND account.status IN ('active', 'error')",
        )
        .bind(source_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        row.map(source_sync_connection).transpose()
    }

    /// Deduplicates provider messages and commits new project inflow atomically.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when provider data cannot be applied safely.
    #[allow(
        clippy::too_many_lines,
        reason = "The ingestion transaction keeps message validation, deduplication, sender recovery, and acknowledgement projection atomic."
    )]
    pub async fn apply_google_chat_messages(
        &self,
        connection: &GoogleChatSourceSyncConnection,
        messages: &[ProviderGoogleChatMessage],
    ) -> Result<Vec<NewInflowAcknowledgement>, StorageError> {
        if !is_v7(connection.source_id)
            || messages
                .iter()
                .any(|message| !valid_provider_message(message))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let mut acknowledgements = Vec::new();
        for message in messages {
            let inflow_id = Uuid::now_v7();
            let inserted = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO project_inflow_items (
                    id, user_id, project_id, source_id, provider_message_name,
                    provider_thread_name, sender_provider_name, sender_name,
                    content_text, received_at
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (source_id, provider_message_name) DO NOTHING
                 RETURNING id",
            )
            .bind(inflow_id)
            .bind(connection.user_id)
            .bind(connection.project_id)
            .bind(connection.source_id)
            .bind(&message.provider_message_name)
            .bind(&message.provider_thread_name)
            .bind(&message.sender_provider_name)
            .bind(&message.sender_name)
            .bind(message.content_text.trim())
            .bind(message.received_at)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?;
            if let Some(inflow_id) = inserted {
                append_change(
                    &mut transaction,
                    connection.user_id,
                    "project_inflow_item",
                    inflow_id,
                    1,
                )
                .await?;
                acknowledgements.push(NewInflowAcknowledgement {
                    inflow_id,
                    provider_message_name: message.provider_message_name.clone(),
                });
            } else if message.sender_provider_name.is_some() || message.sender_name.is_some() {
                let repaired = sqlx::query_as::<_, (Uuid, i64)>(
                    "UPDATE project_inflow_items
                     SET sender_provider_name = COALESCE(sender_provider_name, $3),
                         sender_name = COALESCE(sender_name, $4)
                     WHERE source_id = $1 AND provider_message_name = $2
                       AND ((sender_provider_name IS NULL AND $3::TEXT IS NOT NULL)
                         OR (sender_name IS NULL AND $4::TEXT IS NOT NULL))
                     RETURNING id, version",
                )
                .bind(connection.source_id)
                .bind(&message.provider_message_name)
                .bind(&message.sender_provider_name)
                .bind(&message.sender_name)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(classify)?;
                if let Some((inflow_id, version)) = repaired {
                    append_change(
                        &mut transaction,
                        connection.user_id,
                        "project_inflow_item",
                        inflow_id,
                        version,
                    )
                    .await?;
                }
            }
        }
        let latest = messages.iter().map(|message| message.received_at).max();
        let source_version = sqlx::query_scalar::<_, i64>(
            "UPDATE project_google_chat_sources
             SET last_provider_message_at = CASE
                     WHEN $2::timestamptz IS NULL THEN last_provider_message_at
                     ELSE GREATEST(COALESCE(last_provider_message_at, $2), $2)
                 END,
                 last_successful_sync_at = NOW(), last_error_code = NULL
             WHERE id = $1
             RETURNING version",
        )
        .bind(connection.source_id)
        .bind(latest)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        let account_version = sqlx::query_scalar::<_, i64>(
            "UPDATE google_chat_accounts AS account
             SET status = 'active', last_successful_sync_at = NOW(), last_error_code = NULL
             FROM project_google_chat_sources AS source
             WHERE source.id = $1 AND source.account_id = account.id
             RETURNING account.version",
        )
        .bind(connection.source_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        if connection.source_had_error {
            append_change(
                &mut transaction,
                connection.user_id,
                "project_google_chat_source",
                connection.source_id,
                source_version,
            )
            .await?;
        }
        if connection.account_needs_recovery {
            append_change(
                &mut transaction,
                connection.user_id,
                "google_chat_account",
                connection.account_id,
                account_version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(acknowledgements)
    }

    /// Records that the provider message received the configured acknowledgement reaction.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the acknowledgement timestamp cannot be stored.
    pub async fn mark_google_chat_inflow_acknowledged(
        &self,
        user_id: Uuid,
        inflow_id: Uuid,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE project_inflow_items SET acknowledged_at = NOW()
             WHERE id = $1 AND user_id = $2 AND acknowledged_at IS NULL",
        )
        .bind(inflow_id)
        .bind(user_id)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }

    /// Lists bounded completion deliveries that are ready for an idempotent
    /// provider retry. A caller can narrow the read to the item promoted in the
    /// current request while the periodic source sync drains the remaining
    /// queue.
    ///
    /// # Errors
    ///
    /// Returns a storage error when identifiers, bounds, or persistence are invalid.
    pub async fn pending_google_chat_completion_deliveries(
        &self,
        source_id: Uuid,
        inflow_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<GoogleChatCompletionDelivery>, StorageError> {
        if !is_v7(source_id)
            || inflow_id.is_some_and(|value| !is_v7(value))
            || !(1..=20).contains(&limit)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, GoogleChatCompletionDeliveryRow>(
            "SELECT item.id AS inflow_id, item.user_id, item.source_id,
                item.provider_message_name, item.provider_thread_name,
                task.id AS task_id, task.title AS task_title,
                task.assignee_name, task.due_at,
                item.completion_reaction_at IS NOT NULL AS reaction_completed,
                item.completion_reply_at IS NOT NULL AS reply_completed,
                item.completion_delivery_attempt_count AS attempt_count
             FROM project_inflow_items AS item
             JOIN tasks AS task
               ON task.id = item.promoted_task_id
              AND task.user_id = item.user_id
             WHERE item.source_id = $1
               AND ($2::UUID IS NULL OR item.id = $2)
               AND item.status = 'promoted'
               AND item.completion_requested_at IS NOT NULL
               AND (
                   item.completion_reaction_at IS NULL
                   OR item.completion_reply_at IS NULL
               )
               AND item.completion_delivery_attempt_count < 10
               AND item.completion_delivery_next_attempt_at <= NOW()
             ORDER BY item.completion_delivery_next_attempt_at, item.id
             LIMIT $3",
        )
        .bind(source_id)
        .bind(inflow_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Persists one provider attempt without clearing a part that already
    /// succeeded. Incomplete deliveries receive bounded exponential backoff
    /// and are retried by the source sync worker.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the result shape or persistence is invalid.
    pub async fn record_google_chat_completion_delivery(
        &self,
        delivery: &GoogleChatCompletionDelivery,
        reaction_completed: bool,
        reply_completed: bool,
        failure_code: Option<&str>,
    ) -> Result<(), StorageError> {
        let complete = (delivery.reaction_completed || reaction_completed)
            && (delivery.reply_completed || reply_completed);
        if ![
            delivery.inflow_id,
            delivery.user_id,
            delivery.source_id,
            delivery.task_id,
        ]
        .into_iter()
        .all(is_v7)
            || (!complete && !failure_code.is_some_and(valid_failure_code))
            || failure_code.is_some_and(|code| !valid_failure_code(code))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let next_attempt_seconds = i64::from(30_u32.saturating_mul(
            2_u32.saturating_pow(u32::try_from(delivery.attempt_count.clamp(0, 7)).unwrap_or(7)),
        ))
        .min(3_600);
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE project_inflow_items
             SET completion_reaction_at = CASE
                     WHEN $3 THEN COALESCE(completion_reaction_at, NOW())
                     ELSE completion_reaction_at
                 END,
                 completion_reply_at = CASE
                     WHEN $4 THEN COALESCE(completion_reply_at, NOW())
                     ELSE completion_reply_at
                 END,
                 completion_delivery_attempt_count =
                     completion_delivery_attempt_count + 1,
                 completion_delivery_error_code = CASE
                     WHEN (completion_reaction_at IS NOT NULL OR $3)
                      AND (completion_reply_at IS NOT NULL OR $4)
                     THEN NULL
                     ELSE $5
                 END,
                 completion_delivery_next_attempt_at = CASE
                     WHEN (completion_reaction_at IS NOT NULL OR $3)
                      AND (completion_reply_at IS NOT NULL OR $4)
                     THEN NULL
                     ELSE NOW() + make_interval(secs => $6)
                 END
             WHERE id = $1 AND user_id = $2
               AND source_id = $7 AND promoted_task_id = $8
               AND completion_requested_at IS NOT NULL
             RETURNING version",
        )
        .bind(delivery.inflow_id)
        .bind(delivery.user_id)
        .bind(reaction_completed)
        .bind(reply_completed)
        .bind(failure_code)
        .bind(next_attempt_seconds)
        .bind(delivery.source_id)
        .bind(delivery.task_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some(version) = version else {
            transaction.rollback().await.map_err(classify)?;
            return Err(StorageError::InvalidConfiguration);
        };
        append_change(
            &mut transaction,
            delivery.user_id,
            "project_inflow_item",
            delivery.inflow_id,
            version,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(())
    }

    /// Records a sanitized source failure and moves its account into the matching health state.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the failure state cannot be saved.
    pub async fn mark_google_chat_source_failure(
        &self,
        source_id: Uuid,
        failure_code: &str,
        reauth_required: bool,
    ) -> Result<(), StorageError> {
        if !is_v7(source_id) || !valid_failure_code(failure_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let source = sqlx::query_as::<_, (Uuid, i64)>(
            "UPDATE project_google_chat_sources AS source
             SET last_error_code = $2
             FROM google_chat_accounts AS account
             WHERE source.id = $1 AND account.id = source.account_id
               AND source.last_error_code IS DISTINCT FROM $2
             RETURNING source.user_id, source.version",
        )
        .bind(source_id)
        .bind(failure_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let status = if reauth_required {
            "reauth_required"
        } else {
            "error"
        };
        let account = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "UPDATE google_chat_accounts AS account
             SET status = $3, last_error_code = $2
             FROM project_google_chat_sources AS source
             WHERE source.id = $1 AND source.account_id = account.id
               AND (account.status IS DISTINCT FROM $3
                    OR account.last_error_code IS DISTINCT FROM $2)
             RETURNING account.id, account.user_id, account.version",
        )
        .bind(source_id)
        .bind(failure_code)
        .bind(status)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some((user_id, version)) = source {
            append_change(
                &mut transaction,
                user_id,
                "project_google_chat_source",
                source_id,
                version,
            )
            .await?;
        }
        if let Some((account_id, user_id, version)) = account {
            append_change(
                &mut transaction,
                user_id,
                "google_chat_account",
                account_id,
                version,
            )
            .await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(())
    }

    /// Lists bounded inflow history for one owner-scoped project and optional status.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the inflow list cannot be loaded.
    pub async fn project_inflow_items(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        status: Option<ProjectInflowStatus>,
    ) -> Result<Vec<ProjectInflowItem>, StorageError> {
        let status = status.map(status_name);
        let rows = sqlx::query_as::<_, ProjectInflowItemRow>(
            "SELECT item.id, item.project_id, project.title AS project_name,
                item.source_id, source.display_name AS source_name,
                item.provider_thread_name, item.sender_provider_name,
                COALESCE(item.sender_name, (
                    SELECT mention.name
                    FROM project_webhooks AS sender_webhook
                    CROSS JOIN LATERAL jsonb_each_text(
                        sender_webhook.mention_directory -> 'users'
                    ) AS mention(name, resource_name)
                    WHERE sender_webhook.project_id = item.project_id
                      AND mention.resource_name = item.sender_provider_name
                    ORDER BY sender_webhook.created_at, mention.name
                    LIMIT 1
                )) AS sender_name,
                COALESCE(
                    item.sender_provider_name = CONCAT('users/', account.provider_subject),
                    FALSE
                ) AS sent_by_owner,
                item.content_text,
                item.received_at, item.status, item.promoted_task_id,
                item.acknowledged_at, item.completion_requested_at,
                item.completion_reaction_at, item.completion_reply_at,
                item.completion_delivery_error_code,
                item.completion_delivery_attempt_count, item.version
             FROM project_inflow_items AS item
             JOIN project_google_chat_sources AS source ON source.id = item.source_id
             JOIN google_chat_accounts AS account ON account.id = source.account_id
             JOIN projects AS project ON project.id = item.project_id
             WHERE item.user_id = $1 AND item.project_id = $2
               AND ($3::TEXT IS NULL OR item.status = $3)
             ORDER BY item.received_at DESC, item.id DESC
             LIMIT 200",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(status)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(ProjectInflowItem::try_from).collect()
    }

    /// Lists pending Chat inflow across every owned project for the home brief.
    /// Provider identifiers remain server-side; sender labels are resolved from
    /// the project's registered Google Chat mention directory when available.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the bounded owner-scoped read fails.
    pub async fn pending_project_inflow_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ProjectInflowItem>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, ProjectInflowItemRow>(
            "SELECT item.id, item.project_id, project.title AS project_name,
                item.source_id, source.display_name AS source_name,
                item.provider_thread_name, item.sender_provider_name,
                COALESCE(item.sender_name, (
                    SELECT mention.name
                    FROM project_webhooks AS sender_webhook
                    CROSS JOIN LATERAL jsonb_each_text(
                        sender_webhook.mention_directory -> 'users'
                    ) AS mention(name, resource_name)
                    WHERE sender_webhook.project_id = item.project_id
                      AND mention.resource_name = item.sender_provider_name
                    ORDER BY sender_webhook.created_at, mention.name
                    LIMIT 1
                )) AS sender_name,
                COALESCE(
                    item.sender_provider_name = CONCAT('users/', account.provider_subject),
                    FALSE
                ) AS sent_by_owner,
                item.content_text,
                item.received_at, item.status, item.promoted_task_id,
                item.acknowledged_at, item.completion_requested_at,
                item.completion_reaction_at, item.completion_reply_at,
                item.completion_delivery_error_code,
                item.completion_delivery_attempt_count, item.version
             FROM project_inflow_items AS item
             JOIN project_google_chat_sources AS source ON source.id = item.source_id
             JOIN google_chat_accounts AS account ON account.id = source.account_id
             JOIN projects AS project ON project.id = item.project_id
             WHERE item.user_id = $1 AND item.status = 'pending'
             ORDER BY item.received_at DESC, item.id DESC
             LIMIT 200",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(ProjectInflowItem::try_from).collect()
    }

    /// Lists the most recently handled Chat conversations for the home brief.
    /// This keeps a promoted or dismissed source message visible after it
    /// leaves the decision queue without exposing provider resource names.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the bounded owner-scoped read fails.
    pub async fn recent_project_inflow_decisions_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ProjectInflowItem>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, ProjectInflowItemRow>(
            "SELECT item.id, item.project_id, project.title AS project_name,
                item.source_id, source.display_name AS source_name,
                item.provider_thread_name, item.sender_provider_name,
                COALESCE(item.sender_name, (
                    SELECT mention.name
                    FROM project_webhooks AS sender_webhook
                    CROSS JOIN LATERAL jsonb_each_text(
                        sender_webhook.mention_directory -> 'users'
                    ) AS mention(name, resource_name)
                    WHERE sender_webhook.project_id = item.project_id
                      AND mention.resource_name = item.sender_provider_name
                    ORDER BY sender_webhook.created_at, mention.name
                    LIMIT 1
                )) AS sender_name,
                COALESCE(
                    item.sender_provider_name = CONCAT('users/', account.provider_subject),
                    FALSE
                ) AS sent_by_owner,
                item.content_text,
                item.received_at, item.status, item.promoted_task_id,
                item.acknowledged_at, item.completion_requested_at,
                item.completion_reaction_at, item.completion_reply_at,
                item.completion_delivery_error_code,
                item.completion_delivery_attempt_count, item.version
             FROM project_inflow_items AS item
             JOIN project_google_chat_sources AS source ON source.id = item.source_id
             JOIN google_chat_accounts AS account ON account.id = source.account_id
             JOIN projects AS project ON project.id = item.project_id
             WHERE item.user_id = $1 AND item.status <> 'pending'
             ORDER BY item.updated_at DESC, item.id DESC
             LIMIT 12",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(ProjectInflowItem::try_from).collect()
    }

    /// Dismisses one pending, version-matched inflow item within its owned project.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the decision cannot be committed.
    pub async fn dismiss_project_inflow_item(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        item_id: Uuid,
        expected_version: i64,
    ) -> Result<Option<ProjectInflowItem>, StorageError> {
        if ![user_id, project_id, item_id].into_iter().all(is_v7) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let group = sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT source_id, provider_thread_name
             FROM project_inflow_items
             WHERE id = $1 AND user_id = $2 AND project_id = $3
               AND version = $4 AND status = 'pending'
             FOR UPDATE",
        )
        .bind(item_id)
        .bind(user_id)
        .bind(project_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((source_id, thread_name)) = group else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        let rows = sqlx::query_as::<_, ProjectInflowItemRow>(
            "UPDATE project_inflow_items AS item
             SET status = 'dismissed'
             FROM project_google_chat_sources AS source
             WHERE item.user_id = $2 AND item.project_id = $3
               AND item.status = 'pending' AND source.id = item.source_id
               AND (($4::TEXT IS NULL AND item.id = $1)
                 OR ($4::TEXT IS NOT NULL AND item.source_id = $5
                   AND item.provider_thread_name = $4))
             RETURNING item.id, item.project_id,
                (SELECT title FROM projects WHERE id = item.project_id) AS project_name,
                item.source_id, source.display_name AS source_name,
                item.provider_thread_name, item.sender_provider_name,
                COALESCE(item.sender_name, (
                    SELECT mention.name
                    FROM project_webhooks AS sender_webhook
                    CROSS JOIN LATERAL jsonb_each_text(
                        sender_webhook.mention_directory -> 'users'
                    ) AS mention(name, resource_name)
                    WHERE sender_webhook.project_id = item.project_id
                      AND mention.resource_name = item.sender_provider_name
                    ORDER BY sender_webhook.created_at, mention.name
                    LIMIT 1
                )) AS sender_name,
                COALESCE(item.sender_provider_name = (
                    SELECT CONCAT('users/', account.provider_subject)
                    FROM project_google_chat_sources AS owner_source
                    JOIN google_chat_accounts AS account
                      ON account.id = owner_source.account_id
                    WHERE owner_source.id = item.source_id
                ), FALSE) AS sent_by_owner,
                item.content_text,
                item.received_at, item.status, item.promoted_task_id,
                item.acknowledged_at, item.completion_requested_at,
                item.completion_reaction_at, item.completion_reply_at,
                item.completion_delivery_error_code,
                item.completion_delivery_attempt_count, item.version",
        )
        .bind(item_id)
        .bind(user_id)
        .bind(project_id)
        .bind(&thread_name)
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        let mut selected = None;
        for row in rows {
            let item = ProjectInflowItem::try_from(row)?;
            append_change(
                &mut transaction,
                user_id,
                "project_inflow_item",
                item.id,
                item.version,
            )
            .await?;
            if item.id == item_id {
                selected = Some(item);
            }
        }
        transaction.commit().await.map_err(classify)?;
        Ok(selected)
    }

    /// Promotes one pending inflow item into an owned project task atomically.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the task and decision cannot commit together.
    pub async fn promote_project_inflow_item(
        &self,
        command: &PromoteProjectInflowItem,
    ) -> Result<Option<ProjectInflowItem>, StorageError> {
        if ![
            command.user_id,
            command.project_id,
            command.item_id,
            command.task_id,
        ]
        .into_iter()
        .all(is_v7)
            || command.expected_version <= 0
            || !valid_text(&command.title, 300, false)
            || !command
                .notes
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_TASK_NOTES_CHARS, true))
            || !command
                .assignee_name
                .as_deref()
                .is_none_or(|value| valid_text(value, MAX_ASSIGNEE_NAME_CHARS, false))
            || !(0..=3).contains(&command.priority)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let locked = sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT source_id, provider_thread_name
             FROM project_inflow_items
             WHERE id = $1 AND user_id = $2 AND project_id = $3
               AND version = $4 AND status = 'pending'
             FOR UPDATE",
        )
        .bind(command.item_id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((source_id, thread_name)) = locked else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        let source_messages =
            inflow_group_messages(&mut transaction, command, source_id, thread_name.as_deref())
                .await?;
        let task_notes = command
            .notes
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(
                || default_google_chat_task_notes(&command.title, &source_messages),
                str::to_owned,
            );
        insert_promoted_task(&mut transaction, command, &task_notes).await?;
        let selected = mark_inflow_group_promoted(
            &mut transaction,
            command,
            source_id,
            thread_name.as_deref(),
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(selected)
    }

    /// Retargets an incomplete Google Chat completion delivery to one promoted
    /// message in the same thread and makes it immediately retryable.
    ///
    /// # Errors
    ///
    /// Returns a validation or persistence error when the promoted item cannot
    /// be locked or the retry state cannot be committed.
    #[allow(
        clippy::too_many_lines,
        reason = "The atomic retry keeps the ownership lock, delivery retarget, and returned read model in one transaction."
    )]
    pub async fn retry_project_inflow_completion(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        item_id: Uuid,
        expected_version: i64,
    ) -> Result<Option<ProjectInflowItem>, StorageError> {
        if ![user_id, project_id, item_id].into_iter().all(is_v7) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let group = sqlx::query_as::<_, (Uuid, Option<String>, Uuid)>(
            "SELECT source_id, provider_thread_name, promoted_task_id
             FROM project_inflow_items
             WHERE id = $1 AND user_id = $2 AND project_id = $3
               AND version = $4 AND status = 'promoted'
               AND promoted_task_id IS NOT NULL
             FOR UPDATE",
        )
        .bind(item_id)
        .bind(user_id)
        .bind(project_id)
        .bind(expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((source_id, thread_name, promoted_task_id)) = group else {
            transaction.rollback().await.map_err(classify)?;
            return Ok(None);
        };
        let rows = sqlx::query_as::<_, ProjectInflowItemRow>(
            "UPDATE project_inflow_items AS item
             SET completion_requested_at = CASE
                     WHEN item.id = $1 THEN NOW()
                     ELSE NULL
                 END,
                 completion_reaction_at = CASE
                     WHEN item.id = $1 THEN NULL
                     ELSE item.completion_reaction_at
                 END,
                 completion_reply_at = CASE
                     WHEN item.id = $1 THEN NULL
                     ELSE item.completion_reply_at
                 END,
                 completion_delivery_error_code = CASE
                     WHEN item.id = $1 THEN NULL
                     ELSE item.completion_delivery_error_code
                 END,
                 completion_delivery_attempt_count = CASE
                     WHEN item.id = $1 THEN 0
                     ELSE item.completion_delivery_attempt_count
                 END,
                 completion_delivery_next_attempt_at = CASE
                     WHEN item.id = $1 THEN NOW()
                     ELSE item.completion_delivery_next_attempt_at
                 END
             FROM project_google_chat_sources AS source
             WHERE item.user_id = $2 AND item.project_id = $3
               AND item.status = 'promoted'
               AND item.promoted_task_id = $4
               AND source.id = item.source_id
               AND (($5::TEXT IS NULL AND item.id = $1)
                 OR ($5::TEXT IS NOT NULL AND item.source_id = $6
                   AND item.provider_thread_name = $5))
             RETURNING item.id, item.project_id,
                (SELECT title FROM projects WHERE id = item.project_id) AS project_name,
                item.source_id, source.display_name AS source_name,
                item.provider_thread_name, item.sender_provider_name,
                COALESCE(item.sender_name, (
                    SELECT mention.name
                    FROM project_webhooks AS sender_webhook
                    CROSS JOIN LATERAL jsonb_each_text(
                        sender_webhook.mention_directory -> 'users'
                    ) AS mention(name, resource_name)
                    WHERE sender_webhook.project_id = item.project_id
                      AND mention.resource_name = item.sender_provider_name
                    ORDER BY sender_webhook.created_at, mention.name
                    LIMIT 1
                )) AS sender_name,
                COALESCE(item.sender_provider_name = (
                    SELECT CONCAT('users/', account.provider_subject)
                    FROM project_google_chat_sources AS owner_source
                    JOIN google_chat_accounts AS account
                      ON account.id = owner_source.account_id
                    WHERE owner_source.id = item.source_id
                ), FALSE) AS sent_by_owner,
                item.content_text,
                item.received_at, item.status, item.promoted_task_id,
                item.acknowledged_at, item.completion_requested_at,
                item.completion_reaction_at, item.completion_reply_at,
                item.completion_delivery_error_code,
                item.completion_delivery_attempt_count, item.version",
        )
        .bind(item_id)
        .bind(user_id)
        .bind(project_id)
        .bind(promoted_task_id)
        .bind(&thread_name)
        .bind(source_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        let mut selected = None;
        for row in rows {
            let item = ProjectInflowItem::try_from(row)?;
            append_change(
                &mut transaction,
                user_id,
                "project_inflow_item",
                item.id,
                item.version,
            )
            .await?;
            if item.id == item_id {
                selected = Some(item);
            }
        }
        transaction.commit().await.map_err(classify)?;
        Ok(selected)
    }
}

type InflowMessageEvidence = (Uuid, Option<String>, String, OffsetDateTime);

async fn inflow_group_messages(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &PromoteProjectInflowItem,
    source_id: Uuid,
    thread_name: Option<&str>,
) -> Result<Vec<InflowMessageEvidence>, StorageError> {
    sqlx::query_as::<_, InflowMessageEvidence>(
        "SELECT id, sender_name, content_text, received_at
         FROM project_inflow_items
         WHERE user_id = $1 AND project_id = $2 AND status = 'pending'
           AND (($3::TEXT IS NULL AND id = $4)
             OR ($3::TEXT IS NOT NULL AND source_id = $5
               AND provider_thread_name = $3))
         ORDER BY received_at ASC, id ASC
         FOR UPDATE",
    )
    .bind(command.user_id)
    .bind(command.project_id)
    .bind(thread_name)
    .bind(command.item_id)
    .bind(source_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)
}

async fn insert_promoted_task(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &PromoteProjectInflowItem,
    task_notes: &str,
) -> Result<(), StorageError> {
    let row = sqlx::query_as::<_, PromotedTaskRow>(
        "INSERT INTO tasks (
            id, user_id, project_id, title, notes, assignee_name,
            status, priority, due_at
         ) VALUES ($1, $2, $3, $4, $5, $6, 'open', $7, $8)
         RETURNING id, project_id, title, notes, assignee_name,
            status, priority, due_at, completed_at, version",
    )
    .bind(command.task_id)
    .bind(command.user_id)
    .bind(command.project_id)
    .bind(command.title.trim())
    .bind(task_notes)
    .bind(command.assignee_name.as_deref().map(str::trim))
    .bind(command.priority)
    .bind(command.due_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)?;
    let task = row.into_task()?;
    append_change(transaction, command.user_id, "task", task.id, task.version).await?;
    queue_task_webhook_in_transaction(transaction, command.user_id, &task, "task.created").await
}

async fn mark_inflow_group_promoted(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &PromoteProjectInflowItem,
    source_id: Uuid,
    thread_name: Option<&str>,
) -> Result<Option<ProjectInflowItem>, StorageError> {
    let rows = sqlx::query_as::<_, ProjectInflowItemRow>(
        "UPDATE project_inflow_items AS item
         SET status = 'promoted', promoted_task_id = $2,
             completion_requested_at = CASE
                 WHEN item.id = $1 THEN NOW()
                 ELSE item.completion_requested_at
             END,
             completion_delivery_next_attempt_at = CASE
                 WHEN item.id = $1 THEN NOW()
                 ELSE item.completion_delivery_next_attempt_at
             END
         FROM project_google_chat_sources AS source
         WHERE item.user_id = $3 AND item.project_id = $4
           AND item.status = 'pending' AND source.id = item.source_id
           AND (($5::TEXT IS NULL AND item.id = $1)
             OR ($5::TEXT IS NOT NULL AND item.source_id = $6
               AND item.provider_thread_name = $5))
         RETURNING item.id, item.project_id,
            (SELECT title FROM projects WHERE id = item.project_id) AS project_name,
            item.source_id, source.display_name AS source_name,
            item.provider_thread_name, item.sender_provider_name,
            COALESCE(item.sender_name, (
                SELECT mention.name
                FROM project_webhooks AS sender_webhook
                CROSS JOIN LATERAL jsonb_each_text(
                    sender_webhook.mention_directory -> 'users'
                ) AS mention(name, resource_name)
                WHERE sender_webhook.project_id = item.project_id
                  AND mention.resource_name = item.sender_provider_name
                ORDER BY sender_webhook.created_at, mention.name
                LIMIT 1
            )) AS sender_name,
            COALESCE(item.sender_provider_name = (
                SELECT CONCAT('users/', account.provider_subject)
                FROM project_google_chat_sources AS owner_source
                JOIN google_chat_accounts AS account
                  ON account.id = owner_source.account_id
                WHERE owner_source.id = item.source_id
            ), FALSE) AS sent_by_owner,
            item.content_text,
            item.received_at, item.status, item.promoted_task_id,
            item.acknowledged_at, item.completion_requested_at,
            item.completion_reaction_at, item.completion_reply_at,
            item.completion_delivery_error_code,
            item.completion_delivery_attempt_count, item.version",
    )
    .bind(command.item_id)
    .bind(command.task_id)
    .bind(command.user_id)
    .bind(command.project_id)
    .bind(thread_name)
    .bind(source_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)?;
    let mut selected = None;
    for row in rows {
        let item = ProjectInflowItem::try_from(row)?;
        append_change(
            transaction,
            command.user_id,
            "project_inflow_item",
            item.id,
            item.version,
        )
        .await?;
        if item.id == command.item_id {
            selected = Some(item);
        }
    }
    Ok(selected)
}

#[derive(sqlx::FromRow)]
struct PromotedTaskRow {
    id: Uuid,
    project_id: Option<Uuid>,
    title: String,
    notes: Option<String>,
    assignee_name: Option<String>,
    status: String,
    priority: i16,
    due_at: Option<OffsetDateTime>,
    completed_at: Option<OffsetDateTime>,
    version: i64,
}

impl PromotedTaskRow {
    fn into_task(self) -> Result<crate::planning::Task, StorageError> {
        if self.status != "open" {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(crate::planning::Task {
            id: self.id,
            project_id: self.project_id,
            title: self.title,
            notes: self.notes,
            assignee_name: self.assignee_name,
            status: crate::planning::TaskStatus::Open,
            priority: self.priority,
            due_at: self.due_at,
            completed_at: self.completed_at,
            version: self.version,
        })
    }
}

fn validate_oauth_command(
    command: &CreateGoogleChatOAuthAuthorization,
) -> Result<(), StorageError> {
    if ![
        command.id,
        command.user_id,
        command.session_id,
        command.device_id,
    ]
    .into_iter()
    .all(is_v7)
        || command.state_verifier.len() != STATE_VERIFIER_BYTES
        || !valid_secret(&command.pkce_verifier)
        || command.expires_at <= OffsetDateTime::now_utc()
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn validate_completion(command: &CompleteGoogleChatOAuthAuthorization) -> Result<(), StorageError> {
    if ![
        command.authorization_id,
        command.account_id,
        command.user_id,
    ]
    .into_iter()
    .all(is_v7)
        || !valid_scopes(&command.granted_scopes)
        || command
            .refresh_token
            .as_ref()
            .is_some_and(|secret| !valid_secret(secret))
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn validate_source(command: &NewProjectGoogleChatSource) -> Result<(), StorageError> {
    if ![
        command.id,
        command.user_id,
        command.project_id,
        command.account_id,
    ]
    .into_iter()
    .all(is_v7)
        || !valid_space_name(&command.space_name)
        || !valid_text(&command.display_name, MAX_DISPLAY_NAME_CHARS, false)
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn claimed_authorization(
    row: ClaimedGoogleChatOAuthAuthorizationRow,
) -> Result<ClaimedGoogleChatOAuthAuthorization, StorageError> {
    let pkce_verifier = EncryptedCalendarSecret {
        ciphertext: row
            .pkce_verifier_ciphertext
            .ok_or(StorageError::PersistenceUnavailable)?,
        nonce: row.pkce_nonce.ok_or(StorageError::PersistenceUnavailable)?,
        key_version: row
            .encryption_key_version
            .ok_or(StorageError::PersistenceUnavailable)?,
    };
    if !valid_secret(&pkce_verifier) {
        return Err(StorageError::PersistenceUnavailable);
    }
    Ok(ClaimedGoogleChatOAuthAuthorization {
        id: row.id,
        user_id: row.user_id,
        client_kind: parse_client_platform(&row.client_kind)?,
        pkce_verifier,
    })
}

fn account_connection(
    row: GoogleChatAccountConnectionRow,
) -> Result<GoogleChatAccountConnection, StorageError> {
    let refresh_token = encrypted_secret(
        row.refresh_token_ciphertext,
        row.refresh_token_nonce,
        row.encryption_key_version,
    )?;
    Ok(GoogleChatAccountConnection {
        account_id: row.account_id,
        user_id: row.user_id,
        provider_subject: row.provider_subject,
        refresh_token,
    })
}

fn source_sync_connection(
    row: GoogleChatSourceSyncConnectionRow,
) -> Result<GoogleChatSourceSyncConnection, StorageError> {
    let refresh_token = encrypted_secret(
        row.refresh_token_ciphertext,
        row.refresh_token_nonce,
        row.encryption_key_version,
    )?;
    Ok(GoogleChatSourceSyncConnection {
        source_id: row.source_id,
        account_id: row.account_id,
        user_id: row.user_id,
        project_id: row.project_id,
        provider_subject: row.provider_subject,
        granted_scopes: row.granted_scopes,
        space_name: row.space_name,
        acknowledge_with_reaction: row.acknowledge_with_reaction,
        last_provider_message_at: row.last_provider_message_at,
        last_successful_sync_at: row.last_successful_sync_at,
        source_had_error: row.source_had_error,
        account_needs_recovery: row.account_needs_recovery,
        refresh_token,
    })
}

fn encrypted_secret(
    ciphertext: Option<Vec<u8>>,
    nonce: Option<Vec<u8>>,
    key_version: Option<i32>,
) -> Result<EncryptedCalendarSecret, StorageError> {
    let secret = EncryptedCalendarSecret {
        ciphertext: ciphertext.ok_or(StorageError::PersistenceUnavailable)?,
        nonce: nonce.ok_or(StorageError::PersistenceUnavailable)?,
        key_version: key_version.ok_or(StorageError::PersistenceUnavailable)?,
    };
    if !valid_secret(&secret) {
        return Err(StorageError::PersistenceUnavailable);
    }
    Ok(secret)
}

fn valid_provider_message(message: &ProviderGoogleChatMessage) -> bool {
    valid_text(
        &message.provider_message_name,
        MAX_MESSAGE_NAME_BYTES,
        false,
    ) && message
        .provider_thread_name
        .as_deref()
        .is_none_or(|value| valid_text(value, MAX_MESSAGE_NAME_BYTES, false))
        && message
            .sender_provider_name
            .as_deref()
            .is_none_or(valid_google_chat_user_name)
        && message
            .sender_name
            .as_deref()
            .is_none_or(|value| valid_text(value, MAX_DISPLAY_NAME_CHARS, false))
        && valid_text(&message.content_text, MAX_MESSAGE_TEXT_CHARS, true)
}

fn valid_google_chat_user_name(value: &str) -> bool {
    value.strip_prefix("users/").is_some_and(|identifier| {
        identifier == "app"
            || (!identifier.is_empty()
                && identifier.len() <= 40
                && identifier.bytes().all(|byte| byte.is_ascii_digit()))
    })
}

fn default_google_chat_task_notes(
    title: &str,
    _messages: &[(Uuid, Option<String>, String, OffsetDateTime)],
) -> String {
    truncate_chars(
        &format!(
            "업무 목적\n{}\n\n완료 기준\n요청 범위를 확인하고 처리 결과를 관계자에게 공유합니다.",
            title.trim()
        ),
        MAX_TASK_NOTES_CHARS,
    )
}

fn valid_secret(secret: &EncryptedCalendarSecret) -> bool {
    !secret.ciphertext.is_empty()
        && secret.ciphertext.len() <= MAX_CIPHERTEXT_BYTES
        && secret.nonce.len() == XCHACHA_NONCE_BYTES
        && secret.key_version > 0
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

fn valid_space_name(value: &str) -> bool {
    let value = value.trim();
    value.len() <= MAX_SPACE_NAME_BYTES
        && value.strip_prefix("spaces/").is_some_and(|id| {
            !id.is_empty()
                && id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        })
}

fn valid_text(value: &str, max_chars: usize, multiline: bool) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= max_chars
        && !trimmed.chars().any(|character| {
            character.is_control() && !(multiline && matches!(character, '\n' | '\r' | '\t'))
        })
}

fn valid_failure_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn parse_client_platform(value: &str) -> Result<ClientPlatform, StorageError> {
    match value {
        "macos" => Ok(ClientPlatform::Macos),
        "ios" => Ok(ClientPlatform::Ios),
        "android" => Ok(ClientPlatform::Android),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

const fn status_name(status: ProjectInflowStatus) -> &'static str {
    match status {
        ProjectInflowStatus::Pending => "pending",
        ProjectInflowStatus::Promoted => "promoted",
        ProjectInflowStatus::Dismissed => "dismissed",
    }
}

fn is_v7(id: Uuid) -> bool {
    id.get_version_num() == 7
}

fn classify(_error: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_space_names_are_fixed_provider_resources() {
        assert!(valid_space_name("spaces/AAA-Bbb_123"));
        assert!(!valid_space_name(
            "https://chat.googleapis.com/v1/spaces/AAA"
        ));
        assert!(!valid_space_name("spaces/../../token"));
    }

    #[test]
    fn inflow_status_rejects_unknown_database_values() {
        assert!(matches!(
            ProjectInflowStatus::parse("pending"),
            Ok(ProjectInflowStatus::Pending)
        ));
        assert!(ProjectInflowStatus::parse("unknown").is_err());
    }

    #[test]
    fn provider_messages_accept_google_chat_multiline_text() {
        let message = ProviderGoogleChatMessage {
            provider_message_name: "spaces/AAAAAAAAAAA/messages/BBBBBBBBBBB.BBBBBBBBBBB".to_owned(),
            provider_thread_name: Some("spaces/AAAAAAAAAAA/threads/CCCCCCCCCCC".to_owned()),
            sender_provider_name: Some("users/123456789012345678901".to_owned()),
            sender_name: Some("업무 담당자".to_owned()),
            content_text: "첫 번째 요청\n\t후속 확인 사항".to_owned(),
            received_at: OffsetDateTime::UNIX_EPOCH,
        };

        assert!(valid_provider_message(&message));
    }

    #[test]
    fn provider_messages_still_reject_unsafe_control_characters() {
        let message = ProviderGoogleChatMessage {
            provider_message_name: "spaces/AAAAAAAAAAA/messages/BBBBBBBBBBB.BBBBBBBBBBB".to_owned(),
            provider_thread_name: None,
            sender_provider_name: None,
            sender_name: None,
            content_text: "업무 요청\u{0000}".to_owned(),
            received_at: OffsetDateTime::UNIX_EPOCH,
        };

        assert!(!valid_provider_message(&message));
    }
}

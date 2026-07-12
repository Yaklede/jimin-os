//! Durable, server-owned agent conversation queues. The runtime claims these
//! jobs later; API requests never connect to Codex directly.

use std::time::Duration;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

const MAX_CONTENT_CHARS: usize = 24_000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_RUNNER_ID_CHARS: usize = 200;
const MAX_AUTH_URL_CHARS: usize = 2_048;
const MAX_AUTH_USER_CODE_CHARS: usize = 256;
const MIN_CLAIM_LEASE: Duration = Duration::from_secs(5);
const MAX_CLAIM_LEASE: Duration = Duration::from_mins(5);

pub struct NewConversation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
}

impl NewConversation {
    /// Validates bounded client-visible conversation metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs or text.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.id)
            || !is_v7(self.user_id)
            || !self
                .title
                .as_deref()
                .is_none_or(|title| valid_text(title, MAX_TITLE_CHARS, false))
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

pub struct NewAgentTurn {
    pub job_id: Uuid,
    pub message_id: Uuid,
    pub client_message_id: Uuid,
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub content: String,
}

/// A local planning action extracted from a conversational request. The action
/// is persisted with its job in `waiting_approval` state and is never sent to
/// the agent runner before the owner makes a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAgentAction {
    CreateTask {
        title: String,
    },
    CreateSchedule {
        title: String,
        starts_at: OffsetDateTime,
        ends_at: OffsetDateTime,
        time_zone: String,
    },
}

impl PendingAgentAction {
    fn validate(&self) -> Result<(), StorageError> {
        match self {
            Self::CreateTask { title } => {
                if valid_text(title, MAX_TITLE_CHARS, false) {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
            Self::CreateSchedule {
                title,
                starts_at,
                ends_at,
                time_zone,
            } => {
                if valid_text(title, MAX_TITLE_CHARS, false)
                    && valid_time_zone(time_zone)
                    && ends_at > starts_at
                {
                    Ok(())
                } else {
                    Err(StorageError::InvalidConfiguration)
                }
            }
        }
    }

    fn action_type(&self) -> &'static str {
        match self {
            Self::CreateTask { .. } => "create_task",
            Self::CreateSchedule { .. } => "create_schedule",
        }
    }

    fn title(&self) -> &str {
        match self {
            Self::CreateTask { title } | Self::CreateSchedule { title, .. } => title,
        }
    }

    fn schedule_values(&self) -> (Option<OffsetDateTime>, Option<OffsetDateTime>, Option<&str>) {
        match self {
            Self::CreateTask { .. } => (None, None, None),
            Self::CreateSchedule {
                starts_at,
                ends_at,
                time_zone,
                ..
            } => (Some(*starts_at), Some(*ends_at), Some(time_zone)),
        }
    }

    fn completion_message(&self) -> String {
        match self {
            Self::CreateTask { title } => format!("{title} 할 일을 추가했어요."),
            Self::CreateSchedule {
                title, starts_at, ..
            } => format!(
                "{:02}:{:02}에 {title} 일정을 등록했어요.",
                starts_at.hour(),
                starts_at.minute()
            ),
        }
    }

    fn decline_message(&self) -> String {
        match self {
            Self::CreateTask { title } => format!("{title} 할 일 추가를 취소했어요."),
            Self::CreateSchedule { title, .. } => format!("{title} 일정 등록을 취소했어요."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAgentActionDecision {
    Approve,
    Decline,
}

impl NewAgentTurn {
    /// Validates a single bounded user turn before it is atomically queued.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs or text.
    pub fn validate(&self) -> Result<(), StorageError> {
        if !is_v7(self.job_id)
            || !is_v7(self.message_id)
            || !is_v7(self.client_message_id)
            || !is_v7(self.user_id)
            || !is_v7(self.conversation_id)
            || !valid_text(&self.content, MAX_CONTENT_CHARS, false)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub id: Uuid,
    pub title: Option<String>,
    pub status: ConversationStatus,
    pub last_message_at: Option<OffsetDateTime>,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedAgentTurn {
    pub job_id: Uuid,
    pub message_id: Uuid,
    pub conversation_id: Uuid,
    pub state: AgentJobState,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedAgentJob {
    pub id: Uuid,
    pub user_id: Uuid,
    pub conversation_id: Uuid,
    pub input_message_id: Uuid,
    pub input_content: String,
    pub codex_thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentJobState {
    Queued,
    Claimed,
    Running,
    WaitingApproval,
    RetryWait,
    Completed,
    Failed,
    Cancelled,
    Declined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub role: ConversationMessageRole,
    pub content: String,
    pub status: ConversationMessageStatus,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationMessageRole {
    User,
    Assistant,
    SystemEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationMessageStatus {
    Pending,
    Streaming,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentJob {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub state: AgentJobState,
    pub created_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub version: i64,
    pub pending_action: Option<PendingAgentAction>,
}

/// The safe, client-visible state of the managed `ChatGPT` sign-in ceremony.
/// OAuth access and refresh tokens remain in the Codex runtime and are never
/// represented in this persistence model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAuthentication {
    pub id: Uuid,
    pub state: AgentAuthenticationState,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentAuthenticationState {
    Requested,
    AwaitingAuthorization,
    Ready,
    Failed,
}

/// A request the agent runtime may turn into a Codex-managed device-code
/// login. It deliberately excludes the presentable code so the agent never
/// needs to log or cache it after persisting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedAgentAuthentication {
    pub id: Uuid,
    pub user_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct ConversationRow {
    id: Uuid,
    title: Option<String>,
    status: String,
    last_message_at: Option<OffsetDateTime>,
    version: i64,
}

impl TryFrom<ConversationRow> for Conversation {
    type Error = StorageError;

    fn try_from(row: ConversationRow) -> Result<Self, Self::Error> {
        let status = match row.status.as_str() {
            "active" => ConversationStatus::Active,
            "archived" => ConversationStatus::Archived,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            title: row.title,
            status,
            last_message_at: row.last_message_at,
            version: row.version,
        })
    }
}

#[derive(sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    input_message_id: Uuid,
    conversation_id: Uuid,
    state: String,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct ClaimedJobRow {
    id: Uuid,
    user_id: Uuid,
    conversation_id: Uuid,
    input_message_id: Uuid,
    input_content: String,
    codex_thread_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ExistingAgentTurnRow {
    id: Uuid,
    input_message_id: Uuid,
    conversation_id: Uuid,
    state: String,
    version: i64,
    content: String,
}

#[derive(sqlx::FromRow)]
struct ConversationMessageRow {
    id: Uuid,
    role: String,
    content: String,
    status: String,
    created_at: OffsetDateTime,
    completed_at: Option<OffsetDateTime>,
    version: i64,
}

#[derive(sqlx::FromRow)]
struct AgentJobReadRow {
    id: Uuid,
    conversation_id: Uuid,
    state: String,
    created_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
    version: i64,
    pending_action_type: Option<String>,
    pending_action_title: Option<String>,
    pending_action_starts_at: Option<OffsetDateTime>,
    pending_action_ends_at: Option<OffsetDateTime>,
    pending_action_time_zone: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PendingActionJobRow {
    conversation_id: Uuid,
    state: String,
    pending_action_type: Option<String>,
    pending_action_title: Option<String>,
    pending_action_starts_at: Option<OffsetDateTime>,
    pending_action_ends_at: Option<OffsetDateTime>,
    pending_action_time_zone: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AgentAuthenticationRow {
    id: Uuid,
    state: String,
    verification_url: Option<String>,
    user_code: Option<String>,
}

#[derive(sqlx::FromRow)]
struct RequestedAgentAuthenticationRow {
    id: Uuid,
    user_id: Uuid,
}

impl TryFrom<JobRow> for QueuedAgentTurn {
    type Error = StorageError;

    fn try_from(row: JobRow) -> Result<Self, Self::Error> {
        let state = parse_job_state(&row.state)?;
        Ok(Self {
            job_id: row.id,
            message_id: row.input_message_id,
            conversation_id: row.conversation_id,
            state,
            version: row.version,
        })
    }
}

impl From<ClaimedJobRow> for ClaimedAgentJob {
    fn from(row: ClaimedJobRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            conversation_id: row.conversation_id,
            input_message_id: row.input_message_id,
            input_content: row.input_content,
            codex_thread_id: row.codex_thread_id,
        }
    }
}

impl TryFrom<ExistingAgentTurnRow> for QueuedAgentTurn {
    type Error = StorageError;

    fn try_from(row: ExistingAgentTurnRow) -> Result<Self, Self::Error> {
        Ok(Self {
            job_id: row.id,
            message_id: row.input_message_id,
            conversation_id: row.conversation_id,
            state: parse_job_state(&row.state)?,
            version: row.version,
        })
    }
}

impl TryFrom<ConversationMessageRow> for ConversationMessage {
    type Error = StorageError;

    fn try_from(row: ConversationMessageRow) -> Result<Self, Self::Error> {
        let role = match row.role.as_str() {
            "user" => ConversationMessageRole::User,
            "assistant" => ConversationMessageRole::Assistant,
            "system_event" => ConversationMessageRole::SystemEvent,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        let status = match row.status.as_str() {
            "pending" => ConversationMessageStatus::Pending,
            "streaming" => ConversationMessageStatus::Streaming,
            "completed" => ConversationMessageStatus::Completed,
            "failed" => ConversationMessageStatus::Failed,
            "cancelled" => ConversationMessageStatus::Cancelled,
            _ => return Err(StorageError::PersistenceUnavailable),
        };
        Ok(Self {
            id: row.id,
            role,
            content: row.content,
            status,
            created_at: row.created_at,
            completed_at: row.completed_at,
            version: row.version,
        })
    }
}

impl TryFrom<AgentJobReadRow> for AgentJob {
    type Error = StorageError;

    fn try_from(row: AgentJobReadRow) -> Result<Self, Self::Error> {
        let pending_action = pending_action_from_fields(
            row.pending_action_type.as_deref(),
            row.pending_action_title,
            row.pending_action_starts_at,
            row.pending_action_ends_at,
            row.pending_action_time_zone,
        )?;
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            state: parse_job_state(&row.state)?,
            created_at: row.created_at,
            finished_at: row.finished_at,
            version: row.version,
            pending_action,
        })
    }
}

impl TryFrom<AgentAuthenticationRow> for AgentAuthentication {
    type Error = StorageError;

    fn try_from(row: AgentAuthenticationRow) -> Result<Self, Self::Error> {
        let state = parse_agent_authentication_state(&row.state)?;
        let has_device_code = row.verification_url.is_some() && row.user_code.is_some();
        if matches!(state, AgentAuthenticationState::AwaitingAuthorization) != has_device_code {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(Self {
            id: row.id,
            state,
            verification_url: row.verification_url,
            user_code: row.user_code,
        })
    }
}

impl From<RequestedAgentAuthenticationRow> for RequestedAgentAuthentication {
    fn from(row: RequestedAgentAuthenticationRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
        }
    }
}

impl Database {
    /// Returns the newest authentication state owned by this personal user.
    /// The row carries only presentation fields for the official device-code
    /// flow; Codex owns the actual OAuth credentials.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for a non-version-seven
    /// user identifier and a classified storage error when persistence fails.
    pub async fn agent_authentication_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AgentAuthentication>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<_, AgentAuthenticationRow>(
            "\
            SELECT id, state, verification_url, user_code
            FROM agent_auth_attempts
            WHERE user_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        row.map(AgentAuthentication::try_from).transpose()
    }

    /// Starts or returns the current personal sign-in request. A successful
    /// `ready` row is stable across agent restarts because Codex persists the
    /// managed credential separately in `CODEX_HOME`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed identifiers
    /// and a classified storage error when persistence fails.
    pub async fn request_agent_authentication(
        &self,
        user_id: Uuid,
        attempt_id: Uuid,
    ) -> Result<AgentAuthentication, StorageError> {
        if !is_v7(user_id) || !is_v7(attempt_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let existing = sqlx::query_as::<_, AgentAuthenticationRow>(
            "\
            SELECT id, state, verification_url, user_code
            FROM agent_auth_attempts
            WHERE user_id = $1
              AND state IN ('requested', 'awaiting_authorization', 'ready')
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        if let Some(existing) = existing {
            let authentication = AgentAuthentication::try_from(existing)?;
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(authentication);
        }

        let row = sqlx::query_as::<_, AgentAuthenticationRow>(
            "\
            INSERT INTO agent_auth_attempts (id, user_id, state)
            VALUES ($1, $2, 'requested')
            RETURNING id, state, verification_url, user_code",
        )
        .bind(attempt_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        AgentAuthentication::try_from(row)
    }

    /// Finds one requested ceremony for the single trusted agent process.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence fails.
    pub async fn next_requested_agent_authentication(
        &self,
    ) -> Result<Option<RequestedAgentAuthentication>, StorageError> {
        let row = sqlx::query_as::<_, RequestedAgentAuthenticationRow>(
            "\
            SELECT id, user_id
            FROM agent_auth_attempts
            WHERE state = 'requested'
            ORDER BY created_at ASC, id ASC
            LIMIT 1",
        )
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(row.map(RequestedAgentAuthentication::from))
    }

    /// A device-code ceremony is bound to one App Server process. If that
    /// process restarted before completion, discard the stale presentation
    /// code and let the agent issue a fresh official code.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence fails.
    pub async fn restart_pending_agent_authentication(&self) -> Result<(), StorageError> {
        sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'requested',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL
            WHERE state = 'awaiting_authorization'",
        )
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(())
    }

    /// A persisted ready marker is only valid while the managed Codex process
    /// can still read its `ChatGPT` account. When startup reports no account,
    /// clear that stale marker so the paired user can start a fresh ceremony.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence fails.
    pub async fn invalidate_ready_agent_authentication(&self) -> Result<(), StorageError> {
        sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'failed',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL,
                error_code = 'agent_authentication_required',
                completed_at = NOW()
            WHERE state = 'ready'",
        )
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(())
    }

    /// Stores only the URL and one-time user code returned by the managed
    /// runtime. Both values are intentionally bounded and never emitted by
    /// server logs.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed values and
    /// a classified storage error when persistence fails.
    pub async fn begin_agent_authentication(
        &self,
        attempt_id: Uuid,
        login_id: &str,
        verification_url: &str,
        user_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id)
            || !valid_external_id(login_id)
            || !valid_text(verification_url, MAX_AUTH_URL_CHARS, false)
            || !valid_text(user_code, MAX_AUTH_USER_CODE_CHARS, false)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'awaiting_authorization',
                login_id = $2,
                verification_url = $3,
                user_code = $4
            WHERE id = $1 AND state = 'requested'",
        )
        .bind(attempt_id)
        .bind(login_id)
        .bind(verification_url)
        .bind(user_code)
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(result.rows_affected() == 1)
    }

    /// Marks the specific owned ceremony complete after Codex reports a
    /// managed `ChatGPT` account. The device code is cleared immediately.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for a malformed attempt
    /// identifier and a classified storage error when persistence fails.
    pub async fn complete_agent_authentication(
        &self,
        attempt_id: Uuid,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'ready',
                verification_url = NULL,
                user_code = NULL,
                completed_at = NOW()
            WHERE id = $1 AND state = 'awaiting_authorization'",
        )
        .bind(attempt_id)
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(result.rows_affected() == 1)
    }

    /// Retains a short safe reason when the runtime cannot start the official
    /// sign-in ceremony. The device code is never retained on a failure path.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed values and
    /// a classified storage error when persistence fails.
    pub async fn fail_agent_authentication(
        &self,
        attempt_id: Uuid,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(attempt_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE agent_auth_attempts
            SET state = 'failed',
                login_id = NULL,
                verification_url = NULL,
                user_code = NULL,
                error_code = $2,
                completed_at = NOW()
            WHERE id = $1 AND state IN ('requested', 'awaiting_authorization')",
        )
        .bind(attempt_id)
        .bind(error_code)
        .execute(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(result.rows_affected() == 1)
    }

    /// Creates an active conversation and emits one sync upsert transactionally.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing conversation text.
    pub async fn create_conversation(
        &self,
        conversation: &NewConversation,
    ) -> Result<Conversation, StorageError> {
        conversation.validate()?;
        let user_id = conversation.user_id;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let normalized_title = conversation
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let inserted = sqlx::query_as::<_, ConversationRow>(
            "\
            INSERT INTO conversations (id, user_id, title)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO NOTHING
            RETURNING id, title, status, last_message_at, version",
        )
        .bind(conversation.id)
        .bind(conversation.user_id)
        .bind(normalized_title)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(row) = inserted else {
            let existing = sqlx::query_as::<_, ConversationRow>(
                "\
                SELECT id, title, status, last_message_at, version
                FROM conversations
                WHERE id = $1 AND user_id = $2",
            )
            .bind(conversation.id)
            .bind(conversation.user_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| classify(&error))?;
            let Some(existing) = existing else {
                return Err(StorageError::IdentityConflict);
            };
            if existing.title.as_deref() != normalized_title {
                return Err(StorageError::IdentityConflict);
            }
            let conversation = Conversation::try_from(existing)?;
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(conversation);
        };
        let conversation = Conversation::try_from(row)?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            conversation.id,
            conversation.version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(conversation)
    }

    /// Lists the owning user's active conversations without their message text.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error when persistence is unavailable.
    pub async fn active_conversations_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Conversation>, StorageError> {
        let rows = sqlx::query_as::<_, ConversationRow>(
            "\
            SELECT id, title, status, last_message_at, version
            FROM conversations
            WHERE user_id = $1 AND status = 'active'
            ORDER BY last_message_at DESC NULLS LAST, created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        rows.into_iter().map(Conversation::try_from).collect()
    }

    /// Returns the ordered message history only when the conversation belongs
    /// to the supplied user. An inaccessible conversation is indistinguishable
    /// from a missing one.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing message text in logs.
    pub async fn conversation_messages_for_user(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<Vec<ConversationMessage>>, StorageError> {
        let owns_conversation = sqlx::query_scalar::<_, bool>(
            "\
            SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = $1 AND user_id = $2
            )",
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_one(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        if !owns_conversation {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, ConversationMessageRow>(
            "\
            SELECT id, role, content, status, created_at, completed_at, version
            FROM messages
            WHERE conversation_id = $1
            ORDER BY created_at ASC, id ASC",
        )
        .bind(conversation_id)
        .fetch_all(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        rows.into_iter()
            .map(ConversationMessage::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    /// Returns one agent job only when it belongs to the supplied user.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing job metadata for
    /// inaccessible conversations.
    pub async fn agent_job_for_user(
        &self,
        user_id: Uuid,
        job_id: Uuid,
    ) -> Result<Option<AgentJob>, StorageError> {
        let row = sqlx::query_as::<_, AgentJobReadRow>(
            "\
            SELECT id, conversation_id, state, created_at, finished_at, version,
                   pending_action_type, pending_action_title,
                   pending_action_starts_at, pending_action_ends_at,
                   pending_action_time_zone
            FROM agent_jobs
            WHERE id = $1 AND user_id = $2",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        row.map(AgentJob::try_from).transpose()
    }

    /// Returns the newest job in an owned conversation so a client that was
    /// restarted or opened on another device can restore its request state.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing another user's job.
    pub async fn latest_agent_job_for_conversation_for_user(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<AgentJob>, StorageError> {
        let row = sqlx::query_as::<_, AgentJobReadRow>(
            "\
            SELECT id, conversation_id, state, created_at, finished_at, version,
                   pending_action_type, pending_action_title,
                   pending_action_starts_at, pending_action_ends_at,
                   pending_action_time_zone
            FROM agent_jobs
            WHERE user_id = $1 AND conversation_id = $2
            ORDER BY created_at DESC, id DESC
            LIMIT 1",
        )
        .bind(user_id)
        .bind(conversation_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        row.map(AgentJob::try_from).transpose()
    }

    /// Atomically records the user message and a queued job for one owned,
    /// active conversation. The unique active-job index turns concurrent turns
    /// into an ordinary conflict rather than a competing provider call.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::IdentityConflict`] for an unknown/foreign or
    /// active-busy conversation and a classified error for storage failures.
    pub async fn enqueue_agent_turn(
        &self,
        turn: &NewAgentTurn,
    ) -> Result<QueuedAgentTurn, StorageError> {
        self.enqueue_agent_turn_inner(turn, None).await
    }

    /// Records a planning action as a conversation turn without executing it.
    /// The caller must later resolve the same job through the explicit
    /// approval operation below.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed action
    /// details and a classified storage error when persistence is unavailable.
    pub async fn enqueue_agent_action_turn(
        &self,
        turn: &NewAgentTurn,
        action: PendingAgentAction,
    ) -> Result<QueuedAgentTurn, StorageError> {
        action.validate()?;
        self.enqueue_agent_turn_inner(turn, Some(&action)).await
    }

    #[allow(clippy::too_many_lines)] // One transaction intentionally owns turn, job, and sync writes.
    async fn enqueue_agent_turn_inner(
        &self,
        turn: &NewAgentTurn,
        pending_action: Option<&PendingAgentAction>,
    ) -> Result<QueuedAgentTurn, StorageError> {
        turn.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        if !owns_active_conversation(&mut transaction, turn.user_id, turn.conversation_id).await? {
            return Err(StorageError::IdentityConflict);
        }

        let existing = existing_agent_turn(
            &mut transaction,
            turn.conversation_id,
            turn.client_message_id,
        )
        .await?;
        if let Some(existing) = existing {
            if existing.content != turn.content.trim() {
                return Err(StorageError::IdentityConflict);
            }
            let queued = QueuedAgentTurn::try_from(existing)?;
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(queued);
        }

        let inserted = sqlx::query(
            "\
            INSERT INTO messages (
                id, conversation_id, role, content, status, client_message_id
            ) VALUES ($1, $2, 'user', $3, 'completed', $4)",
        )
        .bind(turn.message_id)
        .bind(turn.conversation_id)
        .bind(turn.content.trim())
        .bind(turn.client_message_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        if inserted.rows_affected() != 1 {
            return Err(StorageError::PersistenceUnavailable);
        }

        let (action_type, action_title, action_starts_at, action_ends_at, action_time_zone) =
            pending_action.map_or((None, None, None, None, None), |action| {
                let (starts_at, ends_at, time_zone) = action.schedule_values();
                (
                    Some(action.action_type()),
                    Some(action.title()),
                    starts_at,
                    ends_at,
                    time_zone,
                )
            });
        let row = sqlx::query_as::<_, JobRow>(
            "\
            INSERT INTO agent_jobs (
                id, user_id, conversation_id, input_message_id, state,
                pending_action_type, pending_action_title,
                pending_action_starts_at, pending_action_ends_at,
                pending_action_time_zone
            ) VALUES (
                $1, $2, $3, $4,
                CASE WHEN $5::text IS NULL THEN 'queued' ELSE 'waiting_approval' END,
                $5, $6, $7, $8, $9
            )
            RETURNING id, input_message_id, conversation_id, state, version",
        )
        .bind(turn.job_id)
        .bind(turn.user_id)
        .bind(turn.conversation_id)
        .bind(turn.message_id)
        .bind(action_type)
        .bind(action_title)
        .bind(action_starts_at)
        .bind(action_ends_at)
        .bind(action_time_zone)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET last_message_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(turn.conversation_id)
        .bind(turn.user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let queued = QueuedAgentTurn::try_from(row)?;
        append_change(
            &mut transaction,
            turn.user_id,
            "agent_job",
            queued.job_id,
            queued.version,
        )
        .await?;
        append_change(
            &mut transaction,
            turn.user_id,
            "conversation",
            turn.conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(queued)
    }

    /// Resolves one owner-visible planning proposal exactly once. Approval
    /// creates the local planning record and finalizes the conversation in the
    /// same transaction; decline records a clear conversation outcome without
    /// changing the personal plan.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs and a
    /// classified storage error when the action or its audit changes cannot be
    /// persisted.
    #[allow(clippy::too_many_lines)] // Approval must atomically cover plan, job, message, and sync writes.
    pub async fn resolve_agent_action(
        &self,
        user_id: Uuid,
        job_id: Uuid,
        decision: PendingAgentActionDecision,
    ) -> Result<bool, StorageError> {
        if !is_v7(user_id) || !is_v7(job_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, PendingActionJobRow>(
            "\
            SELECT conversation_id, state,
                   pending_action_type, pending_action_title,
                   pending_action_starts_at, pending_action_ends_at,
                   pending_action_time_zone
            FROM agent_jobs
            WHERE id = $1 AND user_id = $2
            FOR UPDATE",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(row) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        if parse_job_state(&row.state)? != AgentJobState::WaitingApproval {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        }
        let action = pending_action_from_fields(
            row.pending_action_type.as_deref(),
            row.pending_action_title,
            row.pending_action_starts_at,
            row.pending_action_ends_at,
            row.pending_action_time_zone,
        )?
        .ok_or(StorageError::PersistenceUnavailable)?;

        let outcome = match decision {
            PendingAgentActionDecision::Approve => {
                persist_approved_agent_action(&mut transaction, user_id, &action).await?;
                action.completion_message()
            }
            PendingAgentActionDecision::Decline => action.decline_message(),
        };
        let state = match decision {
            PendingAgentActionDecision::Approve => "completed",
            PendingAgentActionDecision::Decline => "declined",
        };
        let job_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE agent_jobs
            SET state = $3,
                phase = NULL,
                pending_action_type = NULL,
                pending_action_title = NULL,
                pending_action_starts_at = NULL,
                pending_action_ends_at = NULL,
                pending_action_time_zone = NULL,
                finished_at = NOW()
            WHERE id = $1 AND user_id = $2 AND state = 'waiting_approval'
            RETURNING version",
        )
        .bind(job_id)
        .bind(user_id)
        .bind(state)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(job_version) = job_version else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let assistant_message_id = Uuid::now_v7();
        let message_version = sqlx::query_scalar::<_, i64>(
            "\
            INSERT INTO messages (
                id, conversation_id, agent_job_id, role, content, status, completed_at
            ) VALUES ($1, $2, $3, 'assistant', $4, 'completed', NOW())
            RETURNING version",
        )
        .bind(assistant_message_id)
        .bind(row.conversation_id)
        .bind(job_id)
        .bind(outcome)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET last_message_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(row.conversation_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "message",
            assistant_message_id,
            message_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            row.conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Claims one queued turn for a named runner with a bounded lease.
    ///
    /// An expired `claimed` job is safe to recover because the runner has not
    /// persisted the transition that permits a provider turn yet. `running`
    /// jobs are deliberately not reclaimed automatically: their provider side
    /// effect needs an explicit recovery path.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing message content.
    pub async fn claim_next_agent_job(
        &self,
        runner_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedAgentJob>, StorageError> {
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let row = sqlx::query_as::<_, ClaimedJobRow>(
            "\
            WITH recovered AS (
                UPDATE agent_jobs
                SET state = 'queued', phase = NULL, claim_owner = NULL, claim_expires_at = NULL
                WHERE state = 'claimed' AND claim_expires_at < NOW()
            ), candidate AS (
                SELECT id
                FROM agent_jobs
                WHERE state IN ('queued', 'retry_wait')
                ORDER BY created_at ASC, id ASC
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            ), claimed AS (
                UPDATE agent_jobs AS job
                SET state = 'claimed',
                    phase = 'preparing',
                    claim_owner = $1,
                    claim_expires_at = NOW() + ($2 * INTERVAL '1 millisecond'),
                    attempt_count = attempt_count + 1
                FROM candidate
                WHERE job.id = candidate.id
                RETURNING job.id, job.user_id, job.conversation_id, job.input_message_id
            )
            SELECT job.id,
                   job.user_id,
                   job.conversation_id,
                   job.input_message_id,
                   input.content AS input_content,
                   conversation.codex_thread_id
            FROM claimed AS job
            INNER JOIN messages AS input ON input.id = job.input_message_id
            INNER JOIN conversations AS conversation ON conversation.id = job.conversation_id",
        )
        .bind(runner_id)
        .bind(lease_millis)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| classify(&error))?;
        Ok(row.map(ClaimedAgentJob::from))
    }

    /// Safely finalizes expired work that was interrupted after the provider
    /// turn boundary. The turn is never requeued because the provider may have
    /// received it before the worker stopped. The deployed topology has one
    /// durable personal worker, so an expired lease proves no healthy worker
    /// remains responsible for this turn.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed error
    /// metadata, and a classified storage error when persistence fails.
    pub async fn fail_expired_running_agent_jobs(
        &self,
        error_code: &str,
    ) -> Result<usize, StorageError> {
        if !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let rows = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'failed',
                phase = NULL,
                claim_owner = NULL,
                claim_expires_at = NULL,
                error_code = $1,
                finished_at = NOW()
            WHERE state = 'running'
              AND claim_expires_at < NOW()
            RETURNING id, user_id, version",
        )
        .bind(error_code)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        for (job_id, user_id, version) in &rows {
            append_change(&mut transaction, *user_id, "agent_job", *job_id, *version).await?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(rows.len())
    }

    /// Marks a lease-owned job as running after its Codex thread is available,
    /// but before `turn/start` is sent.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing provider IDs in logs.
    pub async fn start_agent_job(
        &self,
        job_id: Uuid,
        runner_id: &str,
        codex_thread_id: &str,
        lease: Duration,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id) || !valid_runner_id(runner_id) || !valid_external_id(codex_thread_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let lease_millis = claim_lease_millis(runner_id, lease)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'running',
                phase = 'starting_turn',
                codex_thread_id = $3,
                claim_expires_at = NOW() + ($4 * INTERVAL '1 millisecond'),
                started_at = COALESCE(started_at, NOW())
            WHERE id = $1 AND claim_owner = $2 AND state = 'claimed'
            RETURNING user_id, conversation_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(codex_thread_id)
        .bind(lease_millis)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, conversation_id, job_version)) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET codex_thread_id = COALESCE(codex_thread_id, $3)
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(conversation_id)
        .bind(user_id)
        .bind(codex_thread_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Persists the authoritative final assistant message and makes a lease-owned
    /// running job terminal in one transaction.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without exposing assistant content.
    pub async fn complete_agent_job(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        content: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id)
            || !is_v7(assistant_message_id)
            || !valid_runner_id(runner_id)
            || !valid_assistant_output(content)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'completed',
                phase = NULL,
                claim_owner = NULL,
                claim_expires_at = NULL,
                finished_at = NOW()
            WHERE id = $1 AND claim_owner = $2 AND state = 'running'
            RETURNING user_id, conversation_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, conversation_id, job_version)) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let message_version = sqlx::query_scalar::<_, i64>(
            "\
            INSERT INTO messages (
                id, conversation_id, agent_job_id, role, content, status, completed_at
            ) VALUES ($1, $2, $3, 'assistant', $4, 'completed', NOW())
            ON CONFLICT (id) DO UPDATE
            SET content = EXCLUDED.content,
                status = 'completed',
                completed_at = NOW()
            WHERE messages.conversation_id = EXCLUDED.conversation_id
              AND messages.agent_job_id = EXCLUDED.agent_job_id
            RETURNING version",
        )
        .bind(assistant_message_id)
        .bind(conversation_id)
        .bind(job_id)
        .bind(content.trim())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let conversation_version = sqlx::query_scalar::<_, i64>(
            "\
            UPDATE conversations
            SET last_message_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING version",
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "message",
            assistant_message_id,
            message_version,
        )
        .await?;
        append_change(
            &mut transaction,
            user_id,
            "conversation",
            conversation_id,
            conversation_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Appends one safe assistant delta to the message visible to the
    /// conversation owner while the lease-owning turn is still running.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for malformed IDs,
    /// runner metadata, or assistant text, and a classified storage error when
    /// persistence fails.
    pub async fn append_agent_response_delta(
        &self,
        job_id: Uuid,
        runner_id: &str,
        assistant_message_id: Uuid,
        delta: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id)
            || !is_v7(assistant_message_id)
            || !valid_runner_id(runner_id)
            || !valid_assistant_delta(delta)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let job = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET phase = 'streaming'
            WHERE id = $1 AND claim_owner = $2 AND state = 'running'
            RETURNING user_id, conversation_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, conversation_id, job_version)) = job else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        let message_version = sqlx::query_scalar::<_, i64>(
            "\
            INSERT INTO messages (
                id, conversation_id, agent_job_id, role, content, status
            ) VALUES ($1, $2, $3, 'assistant', $4, 'streaming')
            ON CONFLICT (id) DO UPDATE
            SET content = messages.content || EXCLUDED.content,
                status = 'streaming',
                completed_at = NULL
            WHERE messages.conversation_id = EXCLUDED.conversation_id
              AND messages.agent_job_id = EXCLUDED.agent_job_id
              AND char_length(messages.content) + char_length(EXCLUDED.content) <= $5
            RETURNING version",
        )
        .bind(assistant_message_id)
        .bind(conversation_id)
        .bind(job_id)
        .bind(delta)
        .bind(i32::try_from(MAX_CONTENT_CHARS).map_err(|_| StorageError::InvalidConfiguration)?)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some(message_version) = message_version else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Err(StorageError::PersistenceUnavailable);
        };
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        append_change(
            &mut transaction,
            user_id,
            "message",
            assistant_message_id,
            message_version,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }

    /// Marks a lease-owned pre-provider or running job as failed using a
    /// sanitized error code only.
    ///
    /// # Errors
    ///
    /// Returns a classified storage error without retaining provider error text.
    pub async fn fail_agent_job(
        &self,
        job_id: Uuid,
        runner_id: &str,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(job_id) || !valid_runner_id(runner_id) || !valid_error_code(error_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let row = sqlx::query_as::<_, (Uuid, i64)>(
            "\
            UPDATE agent_jobs
            SET state = 'failed',
                phase = NULL,
                claim_owner = NULL,
                claim_expires_at = NULL,
                error_code = $3,
                finished_at = NOW()
            WHERE id = $1
              AND claim_owner = $2
              AND state IN ('claimed', 'running')
            RETURNING user_id, version",
        )
        .bind(job_id)
        .bind(runner_id)
        .bind(error_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        let Some((user_id, job_version)) = row else {
            transaction
                .rollback()
                .await
                .map_err(|error| classify(&error))?;
            return Ok(false);
        };
        append_change(&mut transaction, user_id, "agent_job", job_id, job_version).await?;
        transaction
            .commit()
            .await
            .map_err(|error| classify(&error))?;
        Ok(true)
    }
}

async fn persist_approved_agent_action(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    action: &PendingAgentAction,
) -> Result<(), StorageError> {
    match action {
        PendingAgentAction::CreateTask { title } => {
            let id = Uuid::now_v7();
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO tasks (id, user_id, title, notes, status, priority, due_at)
                VALUES ($1, $2, $3, NULL, 'open', 1, NULL)
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?;
            append_change(transaction, user_id, "task", id, version).await
        }
        PendingAgentAction::CreateSchedule {
            title,
            starts_at,
            ends_at,
            time_zone,
        } => {
            let id = Uuid::now_v7();
            let version = sqlx::query_scalar::<_, i64>(
                "\
                INSERT INTO schedule_entries (
                    id, user_id, title, notes, starts_at, ends_at, time_zone, source, status
                ) VALUES ($1, $2, $3, NULL, $4, $5, $6, 'manual', 'confirmed')
                RETURNING version",
            )
            .bind(id)
            .bind(user_id)
            .bind(title.trim())
            .bind(starts_at)
            .bind(ends_at)
            .bind(time_zone.trim())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| classify(&error))?;
            append_change(transaction, user_id, "schedule_entry", id, version).await
        }
    }
}

async fn existing_agent_turn(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: Uuid,
    client_message_id: Uuid,
) -> Result<Option<ExistingAgentTurnRow>, StorageError> {
    sqlx::query_as::<_, ExistingAgentTurnRow>(
        "\
        SELECT job.id,
               job.input_message_id,
               job.conversation_id,
               job.state,
               job.version,
               message.content
        FROM messages AS message
        INNER JOIN agent_jobs AS job ON job.input_message_id = message.id
        WHERE message.conversation_id = $1
          AND message.client_message_id = $2",
    )
    .bind(conversation_id)
    .bind(client_message_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| classify(&error))
}

async fn owns_active_conversation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    conversation_id: Uuid,
) -> Result<bool, StorageError> {
    sqlx::query_scalar::<_, bool>(
        "\
        SELECT EXISTS(
            SELECT 1 FROM conversations
            WHERE id = $1 AND user_id = $2 AND status = 'active'
        )",
    )
    .bind(conversation_id)
    .bind(user_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| classify(&error))
}

fn parse_job_state(value: &str) -> Result<AgentJobState, StorageError> {
    match value {
        "queued" => Ok(AgentJobState::Queued),
        "claimed" => Ok(AgentJobState::Claimed),
        "running" => Ok(AgentJobState::Running),
        "waiting_approval" => Ok(AgentJobState::WaitingApproval),
        "retry_wait" => Ok(AgentJobState::RetryWait),
        "completed" => Ok(AgentJobState::Completed),
        "failed" => Ok(AgentJobState::Failed),
        "cancelled" => Ok(AgentJobState::Cancelled),
        "declined" => Ok(AgentJobState::Declined),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn parse_agent_authentication_state(value: &str) -> Result<AgentAuthenticationState, StorageError> {
    match value {
        "requested" => Ok(AgentAuthenticationState::Requested),
        "awaiting_authorization" => Ok(AgentAuthenticationState::AwaitingAuthorization),
        "ready" => Ok(AgentAuthenticationState::Ready),
        "failed" => Ok(AgentAuthenticationState::Failed),
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn pending_action_from_fields(
    action_type: Option<&str>,
    title: Option<String>,
    starts_at: Option<OffsetDateTime>,
    ends_at: Option<OffsetDateTime>,
    time_zone: Option<String>,
) -> Result<Option<PendingAgentAction>, StorageError> {
    match (action_type, title, starts_at, ends_at, time_zone) {
        (None, None, None, None, None) => Ok(None),
        (Some("create_task"), Some(title), None, None, None) => {
            let action = PendingAgentAction::CreateTask { title };
            action.validate()?;
            Ok(Some(action))
        }
        (Some("create_schedule"), Some(title), Some(starts_at), Some(ends_at), Some(time_zone)) => {
            let action = PendingAgentAction::CreateSchedule {
                title,
                starts_at,
                ends_at,
                time_zone,
            };
            action.validate()?;
            Ok(Some(action))
        }
        _ => Err(StorageError::PersistenceUnavailable),
    }
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.trim().is_empty())
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

fn valid_time_zone(value: &str) -> bool {
    valid_text(value, 80, false)
}

fn valid_assistant_output(value: &str) -> bool {
    !value.trim().is_empty() && valid_assistant_delta(value)
}

fn valid_assistant_delta(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_CONTENT_CHARS
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn valid_runner_id(value: &str) -> bool {
    valid_text(value, MAX_RUNNER_ID_CHARS, false)
}

fn valid_external_id(value: &str) -> bool {
    valid_text(value, MAX_RUNNER_ID_CHARS, false)
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.')
        })
}

fn claim_lease_millis(runner_id: &str, lease: Duration) -> Result<i64, StorageError> {
    if !valid_runner_id(runner_id) || !(MIN_CLAIM_LEASE..=MAX_CLAIM_LEASE).contains(&lease) {
        return Err(StorageError::InvalidConfiguration);
    }
    i64::try_from(lease.as_millis()).map_err(|_| StorageError::InvalidConfiguration)
}

fn classify(error: &sqlx::Error) -> StorageError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.code().as_deref() == Some("23505")
    {
        return StorageError::IdentityConflict;
    }
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::{NewAgentTurn, NewConversation};
    use crate::StorageError;
    use uuid::Uuid;

    #[test]
    fn conversation_rejects_invalid_title_or_identifier() {
        let invalid = NewConversation {
            id: Uuid::nil(),
            user_id: Uuid::now_v7(),
            title: Some(" ".to_owned()),
        };
        assert!(matches!(
            invalid.validate(),
            Err(StorageError::InvalidConfiguration)
        ));
    }

    #[test]
    fn agent_turn_rejects_blank_or_non_v7_identifiers() {
        let invalid = NewAgentTurn {
            job_id: Uuid::now_v7(),
            message_id: Uuid::now_v7(),
            client_message_id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            conversation_id: Uuid::nil(),
            content: " ".to_owned(),
        };
        assert!(matches!(
            invalid.validate(),
            Err(StorageError::InvalidConfiguration)
        ));
    }
}

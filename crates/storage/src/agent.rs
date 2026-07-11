//! Durable, server-owned agent conversation queues. The runtime claims these
//! jobs later; API requests never connect to Codex directly.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError, auth::append_change};

const MAX_CONTENT_CHARS: usize = 24_000;
const MAX_TITLE_CHARS: usize = 200;

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
        })
    }
}

impl Database {
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
        let row = sqlx::query_as::<_, ConversationRow>(
            "\
            INSERT INTO conversations (id, user_id, title)
            VALUES ($1, $2, $3)
            RETURNING id, title, status, last_message_at, version",
        )
        .bind(conversation.id)
        .bind(conversation.user_id)
        .bind(
            conversation
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
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
        turn.validate()?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|error| classify(&error))?;
        let exists = sqlx::query_scalar::<_, bool>(
            "\
            SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = $1 AND user_id = $2 AND status = 'active'
            )",
        )
        .bind(turn.conversation_id)
        .bind(turn.user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify(&error))?;
        if !exists {
            return Err(StorageError::IdentityConflict);
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

        let row = sqlx::query_as::<_, JobRow>(
            "\
            INSERT INTO agent_jobs (id, user_id, conversation_id, input_message_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, input_message_id, conversation_id, state",
        )
        .bind(turn.job_id)
        .bind(turn.user_id)
        .bind(turn.conversation_id)
        .bind(turn.message_id)
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

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn valid_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.trim().is_empty())
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
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

//! Read-only Gmail inbox metadata used by the private assistant.
//!
//! The database stores only bounded message headers and snippets. OAuth
//! credentials remain in the encrypted Google Calendar account record, while
//! bodies, attachments, and raw Gmail API responses never enter this module.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

const MAX_INBOX_MESSAGES: i64 = 100;
const MAX_PROVIDER_ID_BYTES: usize = 255;
const MAX_SENDER_BYTES: usize = 1_024;
const MAX_SUBJECT_BYTES: usize = 998;
const MAX_SNIPPET_BYTES: usize = 512;

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

/// Compact inbox entry available to the server-side assistant context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailMessage {
    pub id: Uuid,
    pub received_at: Option<OffsetDateTime>,
    pub sender: Option<String>,
    pub subject: Option<String>,
    pub snippet: Option<String>,
    pub is_unread: bool,
}

#[derive(sqlx::FromRow)]
struct GmailMessageRow {
    id: Uuid,
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
            received_at: row.received_at,
            sender: row.sender,
            subject: row.subject,
            snippet: row.snippet,
            is_unread: row.is_unread,
        }
    }
}

impl Database {
    /// Applies one bounded inbox snapshot after every provider record has
    /// already been normalized. This intentionally does not tombstone old
    /// messages: Gmail's first-page inbox list is a window, not a complete
    /// mailbox inventory.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for unsafe provider
    /// values and a classified persistence error without exposing email data.
    pub async fn apply_gmail_inbox_sync(
        &self,
        user_id: Uuid,
        messages: &[ProviderGmailMessage],
    ) -> Result<(), StorageError> {
        if user_id.get_version_num() != 7 || !valid_messages(messages) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        for message in messages {
            sqlx::query(
                "\
                INSERT INTO gmail_messages (
                    id, user_id, provider_message_id, provider_thread_id,
                    received_at, sender, subject, snippet, is_unread,
                    provider_deleted_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL)
                ON CONFLICT (user_id, provider_message_id) DO UPDATE
                SET provider_thread_id = EXCLUDED.provider_thread_id,
                    received_at = EXCLUDED.received_at,
                    sender = EXCLUDED.sender,
                    subject = EXCLUDED.subject,
                    snippet = EXCLUDED.snippet,
                    is_unread = EXCLUDED.is_unread,
                    provider_deleted_at = NULL",
            )
            .bind(Uuid::now_v7())
            .bind(user_id)
            .bind(&message.provider_message_id)
            .bind(&message.provider_thread_id)
            .bind(message.received_at)
            .bind(&message.sender)
            .bind(&message.subject)
            .bind(&message.snippet)
            .bind(message.is_unread)
            .execute(&mut *transaction)
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        }
        sqlx::query(
            "\
            INSERT INTO gmail_sync_states (user_id, status, last_successful_sync_at, last_error_code)
            VALUES ($1, 'idle', NOW(), NULL)
            ON CONFLICT (user_id) DO UPDATE
            SET status = 'idle', last_successful_sync_at = NOW(), last_error_code = NULL",
        )
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| StorageError::PersistenceUnavailable)
    }

    /// Returns recent inbox metadata for server-side assistant grounding. It
    /// never exposes a message body or provider identity to a client.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error on unavailable storage.
    pub async fn recent_gmail_messages_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<GmailMessage>, StorageError> {
        let rows = sqlx::query_as::<_, GmailMessageRow>(
            "\
            SELECT id, received_at, sender, subject, snippet, is_unread
            FROM gmail_messages
            WHERE user_id = $1 AND provider_deleted_at IS NULL
            ORDER BY is_unread DESC, received_at DESC NULLS LAST, id DESC
            LIMIT $2",
        )
        .bind(user_id)
        .bind(MAX_INBOX_MESSAGES)
        .fetch_all(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(rows.into_iter().map(GmailMessage::from).collect())
    }

    /// Records only a safe error code while preserving the last successful
    /// inbox summary for retry and reconnect UI.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for invalid ownership
    /// or an unsafe error code and a classified persistence error otherwise.
    pub async fn mark_gmail_sync_failure(
        &self,
        user_id: Uuid,
        failure_code: &str,
    ) -> Result<(), StorageError> {
        if user_id.get_version_num() != 7 || !valid_failure_code(failure_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "\
            INSERT INTO gmail_sync_states (user_id, status, last_error_code)
            VALUES ($1, 'error', $2)
            ON CONFLICT (user_id) DO UPDATE
            SET status = 'error', last_error_code = EXCLUDED.last_error_code",
        )
        .bind(user_id)
        .bind(failure_code)
        .execute(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;
        Ok(())
    }
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
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.')
}

#[cfg(test)]
mod tests {
    use super::{ProviderGmailMessage, valid_messages};

    #[test]
    fn inbox_metadata_rejects_controls_and_accepts_a_safe_header() {
        let valid = ProviderGmailMessage {
            provider_message_id: "message-1".to_owned(),
            provider_thread_id: "thread-1".to_owned(),
            received_at: None,
            sender: Some("Jimin <jimin@example.com>".to_owned()),
            subject: Some("회의 일정".to_owned()),
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
}

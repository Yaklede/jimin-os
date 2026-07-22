//! Project webhook configuration and durable outbound delivery queue.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{Database, StorageError};

const MAX_EVENT_TYPES: usize = 16;
// XChaCha20-Poly1305 adds an authentication tag to the bounded destination.
const MAX_SECRET_BYTES: usize = 8 * 1024 + 32;
const MAX_DELIVERY_ATTEMPTS: i32 = 5;
const WEBHOOK_DELIVERY_LEASE_SECONDS: i64 = 30;
const MAX_WORKER_ID_BYTES: usize = 200;
const MAX_MESSAGE_CHARS: usize = 1_800;
const MAX_MENTION_USERS: usize = 200;
const MAX_MENTION_NAME_CHARS: usize = 80;
const MAX_GOOGLE_CHAT_USER_ID_DIGITS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookProvider {
    GoogleChat,
    Discord,
}

impl WebhookProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoogleChat => "google_chat",
            Self::Discord => "discord",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "google_chat" => Some(Self::GoogleChat),
            "discord" => Some(Self::Discord),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoogleChatMentionDirectory {
    pub users: BTreeMap<String, String>,
}

impl GoogleChatMentionDirectory {
    #[must_use]
    pub fn is_valid_for(&self, provider: WebhookProvider) -> bool {
        if provider == WebhookProvider::Discord && !self.users.is_empty() {
            return false;
        }
        self.users.len() <= MAX_MENTION_USERS
            && self.users.iter().all(|(name, user_id)| {
                valid_mention_name(name) && valid_google_chat_user_id(user_id)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectWebhook {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: WebhookProvider,
    pub destination_hint: String,
    pub mention_directory: GoogleChatMentionDirectory,
    pub events: Vec<String>,
    pub enabled: bool,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
struct ProjectWebhookRow {
    id: Uuid,
    project_id: Uuid,
    provider: String,
    destination_hint: String,
    #[sqlx(json)]
    mention_directory: GoogleChatMentionDirectory,
    events: Vec<String>,
    enabled: bool,
    version: i64,
}

pub struct NewProjectWebhook {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub provider: WebhookProvider,
    pub destination: EncryptedWebhookSecret,
    pub destination_hint: String,
    pub mention_directory: GoogleChatMentionDirectory,
    pub events: Vec<String>,
}

pub enum WebhookDestinationUpdate {
    Keep,
    Replace {
        provider: WebhookProvider,
        secret: EncryptedWebhookSecret,
        hint: String,
    },
}

pub enum WebhookMentionDirectoryUpdate {
    Keep,
    Replace(GoogleChatMentionDirectory),
}

/// Version-checked replacement of one owned webhook configuration.
pub struct ProjectWebhookUpdate {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub provider: WebhookProvider,
    pub events: Vec<String>,
    pub enabled: bool,
    pub destination: WebhookDestinationUpdate,
    pub mention_directory: WebhookMentionDirectoryUpdate,
    pub expected_version: i64,
}

pub struct EncryptedWebhookSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

/// Result of requesting a manual retry for one durable delivery. A delivery
/// that is already queued is treated as an idempotent replay; active or final
/// deliveries remain conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryWebhookDeliveryOutcome {
    Queued,
    AlreadyQueued,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct ClaimedWebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub project_id: Uuid,
    pub event_type: String,
    pub payload: Value,
    pub attempt_count: i32,
    pub provider: String,
    pub destination_ciphertext: Vec<u8>,
    pub destination_nonce: Vec<u8>,
    #[sqlx(json)]
    pub mention_directory: GoogleChatMentionDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_code: Option<i32>,
    pub last_error_code: Option<String>,
    pub created_at: OffsetDateTime,
    pub delivered_at: Option<OffsetDateTime>,
}

impl Database {
    /// Creates a webhook for one project owned by the current user.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn create_project_webhook(
        &self,
        command: &NewProjectWebhook,
    ) -> Result<ProjectWebhook, StorageError> {
        validate_new_webhook(command)?;
        let row = sqlx::query_as::<_, ProjectWebhookRow>(
            "\
            INSERT INTO project_webhooks (
                id, user_id, project_id, provider,
                destination_ciphertext, destination_nonce, destination_hint,
                mention_directory, events
            )
            SELECT $1, $2, project.id, $4, $5, $6, $7, $8, $9
            FROM projects AS project
            WHERE project.id = $3 AND project.user_id = $2
            RETURNING id, project_id, provider, destination_hint,
                mention_directory, events, enabled, version",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.provider.as_str())
        .bind(command.destination.ciphertext.as_slice())
        .bind(command.destination.nonce.as_slice())
        .bind(command.destination_hint.trim())
        .bind(sqlx::types::Json(&command.mention_directory))
        .bind(&command.events)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?
        .ok_or(StorageError::InvalidConfiguration)?;
        Ok(project_webhook(row))
    }

    /// Lists the safe, non-secret webhook configuration for one project.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn project_webhooks(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ProjectWebhook>, StorageError> {
        if !is_v7(user_id) || !is_v7(project_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, ProjectWebhookRow>(
            "\
            SELECT webhook.id, webhook.project_id, webhook.provider,
                webhook.destination_hint, webhook.mention_directory,
                webhook.events, webhook.enabled, webhook.version
            FROM project_webhooks AS webhook
            INNER JOIN projects AS project ON project.id = webhook.project_id
            WHERE webhook.project_id = $1 AND project.user_id = $2
            ORDER BY webhook.created_at, webhook.id",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(rows.into_iter().map(project_webhook).collect())
    }

    /// Lists safe typed webhook metadata across the current user's projects.
    /// Destination secrets are intentionally omitted.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn user_project_webhooks(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ProjectWebhook>, StorageError> {
        if !is_v7(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<_, ProjectWebhookRow>(
            "\
            SELECT webhook.id, webhook.project_id, webhook.provider,
                webhook.destination_hint, webhook.mention_directory,
                webhook.events, webhook.enabled, webhook.version
            FROM project_webhooks AS webhook
            INNER JOIN projects AS project ON project.id = webhook.project_id
            WHERE project.user_id = $1
            ORDER BY webhook.created_at, webhook.id",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        Ok(rows.into_iter().map(project_webhook).collect())
    }

    /// Updates one version-matched webhook owned by the current user without
    /// exposing or accidentally clearing its encrypted destination.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn update_project_webhook(
        &self,
        command: &ProjectWebhookUpdate,
    ) -> Result<Option<ProjectWebhook>, StorageError> {
        validate_webhook_update(command)?;
        let (
            destination_mode,
            provider,
            destination_ciphertext,
            destination_nonce,
            destination_hint,
        ) = match &command.destination {
            WebhookDestinationUpdate::Keep => ("keep", None, None, None, None),
            WebhookDestinationUpdate::Replace {
                provider,
                secret,
                hint,
            } => (
                "replace",
                Some(provider.as_str()),
                Some(secret.ciphertext.as_slice()),
                Some(secret.nonce.as_slice()),
                Some(hint.trim()),
            ),
        };
        let (mention_mode, mention_directory) = match &command.mention_directory {
            WebhookMentionDirectoryUpdate::Keep => ("keep", None),
            WebhookMentionDirectoryUpdate::Replace(directory) => {
                ("replace", Some(sqlx::types::Json(directory)))
            }
        };
        let row = sqlx::query_as::<_, ProjectWebhookRow>(
            "\
            UPDATE project_webhooks AS webhook
            SET provider = CASE WHEN $4 = 'keep' THEN webhook.provider ELSE $5 END,
                destination_ciphertext = CASE WHEN $4 = 'keep' THEN webhook.destination_ciphertext ELSE $6 END,
                destination_nonce = CASE WHEN $4 = 'keep' THEN webhook.destination_nonce ELSE $7 END,
                destination_hint = CASE WHEN $4 = 'keep' THEN webhook.destination_hint ELSE $8 END,
                mention_directory = CASE WHEN $9 = 'keep' THEN webhook.mention_directory ELSE $10 END,
                events = $11,
                enabled = $12
            FROM projects AS project
            WHERE webhook.id = $1
              AND webhook.project_id = $2
              AND webhook.version = $3
              AND project.id = webhook.project_id
              AND project.user_id = $13
              AND webhook.provider = $14
            RETURNING webhook.id, webhook.project_id, webhook.provider,
                webhook.destination_hint, webhook.mention_directory,
                webhook.events, webhook.enabled, webhook.version",
        )
        .bind(command.id)
        .bind(command.project_id)
        .bind(command.expected_version)
        .bind(destination_mode)
        .bind(provider)
        .bind(destination_ciphertext)
        .bind(destination_nonce)
        .bind(destination_hint)
        .bind(mention_mode)
        .bind(mention_directory)
        .bind(&command.events)
        .bind(command.enabled)
        .bind(command.user_id)
        .bind(command.provider.as_str())
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        Ok(row.map(project_webhook))
    }

    /// Deletes a version-matched webhook owned by the current user.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn delete_project_webhook(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        webhook_id: Uuid,
        expected_version: i64,
    ) -> Result<bool, StorageError> {
        if !is_v7(user_id) || !is_v7(project_id) || !is_v7(webhook_id) || expected_version <= 0 {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            DELETE FROM project_webhooks AS webhook
            USING projects AS project
            WHERE webhook.id = $1 AND webhook.project_id = $2
              AND webhook.version = $3
              AND project.id = webhook.project_id AND project.user_id = $4",
        )
        .bind(webhook_id)
        .bind(project_id)
        .bind(expected_version)
        .bind(user_id)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(result.rows_affected() == 1)
    }

    /// Appends one delivery for every enabled webhook subscribed to an event.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn queue_project_webhook_event(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        event_type: &str,
        payload: &Value,
    ) -> Result<usize, StorageError> {
        if !is_v7(user_id)
            || !is_v7(project_id)
            || !valid_event_type(event_type)
            || !payload.is_object()
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let queued = queue_project_event_in_transaction(
            &mut transaction,
            user_id,
            project_id,
            event_type,
            payload,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(queued)
    }

    /// Queues a test event for one enabled, owned webhook.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn queue_webhook_test(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        webhook_id: Uuid,
        payload: &Value,
    ) -> Result<Option<Uuid>, StorageError> {
        if !is_v7(user_id) || !is_v7(project_id) || !is_v7(webhook_id) || !payload.is_object() {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_scalar(
            "\
            INSERT INTO webhook_deliveries (
                id, user_id, project_id, webhook_id, provider,
                destination_ciphertext, destination_nonce,
                mention_directory, event_type, payload, status
            )
            SELECT $4, $1, webhook.project_id, webhook.id, webhook.provider,
                webhook.destination_ciphertext, webhook.destination_nonce,
                webhook.mention_directory, 'webhook.test', $5, 'queued'
            FROM project_webhooks AS webhook
            INNER JOIN projects AS project ON project.id = webhook.project_id
            WHERE webhook.id = $3 AND webhook.project_id = $2
              AND project.user_id = $1 AND webhook.enabled = TRUE
            RETURNING id",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(webhook_id)
        .bind(Uuid::now_v7())
        .bind(payload)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)
    }

    /// Queues one user-requested chat message for an enabled typed webhook.
    /// The destination remains an encrypted immutable snapshot on the delivery.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn queue_webhook_message(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        webhook_id: Uuid,
        message: &str,
    ) -> Result<Option<Uuid>, StorageError> {
        let message = message.trim();
        if !is_v7(user_id)
            || !is_v7(project_id)
            || !is_v7(webhook_id)
            || message.is_empty()
            || message.chars().count() > MAX_MESSAGE_CHARS
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let payload = serde_json::json!({
            "event": "chat.message",
            "projectId": project_id,
            "message": message,
            "occurredAt": OffsetDateTime::now_utc().format(&Rfc3339).ok(),
        });
        sqlx::query_scalar(
            "\
            INSERT INTO webhook_deliveries (
                id, user_id, project_id, webhook_id, provider,
                destination_ciphertext, destination_nonce,
                mention_directory, event_type, payload, status
            )
            SELECT $4, $1, webhook.project_id, webhook.id, webhook.provider,
                webhook.destination_ciphertext, webhook.destination_nonce,
                webhook.mention_directory, 'chat.message', $5, 'queued'
            FROM project_webhooks AS webhook
            INNER JOIN projects AS project ON project.id = webhook.project_id
            WHERE webhook.id = $3 AND webhook.project_id = $2
              AND project.user_id = $1 AND webhook.enabled = TRUE
            RETURNING id",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(webhook_id)
        .bind(Uuid::now_v7())
        .bind(payload)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)
    }

    /// Claims a bounded delivery batch for one worker using skip-locked rows.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn claim_webhook_deliveries(
        &self,
        worker_id: &str,
        limit: i64,
    ) -> Result<Vec<ClaimedWebhookDelivery>, StorageError> {
        if !valid_worker_id(worker_id) || !(1..=50).contains(&limit) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        sqlx::query(
            "\
            UPDATE webhook_deliveries
            SET status = 'failed',
                last_error_code = 'webhook.worker_lease_expired',
                next_attempt_at = NULL,
                lease_owner = NULL,
                lease_expires_at = NULL
            WHERE status = 'sending'
              AND lease_expires_at <= NOW()
              AND attempt_count >= $1",
        )
        .bind(MAX_DELIVERY_ATTEMPTS)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        let deliveries = sqlx::query_as::<_, ClaimedWebhookDelivery>(
            "\
            WITH candidates AS (
                SELECT delivery.id
                FROM webhook_deliveries AS delivery
                WHERE delivery.attempt_count < $4
                  AND (
                    (delivery.status IN ('queued', 'retry_wait')
                      AND COALESCE(delivery.next_attempt_at, delivery.created_at) <= NOW())
                    OR
                    (delivery.status = 'sending' AND delivery.lease_expires_at <= NOW())
                  )
                ORDER BY CASE
                    WHEN delivery.status = 'sending' THEN delivery.lease_expires_at
                    ELSE COALESCE(delivery.next_attempt_at, delivery.created_at)
                END, delivery.id
                FOR UPDATE SKIP LOCKED
                LIMIT $1
            )
            UPDATE webhook_deliveries AS delivery
            SET status = 'sending',
                attempt_count = delivery.attempt_count + 1,
                lease_owner = $2,
                lease_expires_at = NOW() + ($3 * INTERVAL '1 second')
            FROM candidates
            WHERE delivery.id = candidates.id
            RETURNING delivery.id, delivery.webhook_id, delivery.project_id,
                delivery.event_type, delivery.payload, delivery.attempt_count,
                delivery.provider, delivery.destination_ciphertext,
                delivery.destination_nonce, delivery.mention_directory",
        )
        .bind(limit)
        .bind(worker_id)
        .bind(WEBHOOK_DELIVERY_LEASE_SECONDS)
        .bind(MAX_DELIVERY_ATTEMPTS)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        Ok(deliveries)
    }

    /// Marks a claimed delivery as successfully delivered.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn complete_webhook_delivery(
        &self,
        delivery_id: Uuid,
        worker_id: &str,
        attempt_count: i32,
        response_code: i32,
    ) -> Result<bool, StorageError> {
        if !is_v7(delivery_id)
            || !valid_worker_id(worker_id)
            || !(1..=MAX_DELIVERY_ATTEMPTS).contains(&attempt_count)
            || !(200..=299).contains(&response_code)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let result = sqlx::query(
            "\
            UPDATE webhook_deliveries
            SET status = 'delivered', response_code = $2,
                last_error_code = NULL, delivered_at = NOW(), next_attempt_at = NULL,
                lease_owner = NULL, lease_expires_at = NULL
            WHERE id = $1
              AND status = 'sending'
              AND lease_owner = $3
              AND attempt_count = $4",
        )
        .bind(delivery_id)
        .bind(response_code)
        .bind(worker_id)
        .bind(attempt_count)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(result.rows_affected() == 1)
    }

    /// Records a bounded retry or terminal failure for a claimed delivery.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn fail_webhook_delivery(
        &self,
        delivery_id: Uuid,
        worker_id: &str,
        attempt_count: i32,
        response_code: Option<i32>,
        error_code: &str,
    ) -> Result<bool, StorageError> {
        if !is_v7(delivery_id)
            || !valid_worker_id(worker_id)
            || !(1..=MAX_DELIVERY_ATTEMPTS).contains(&attempt_count)
            || response_code.is_some_and(|code| !(100..=599).contains(&code))
            || !valid_error_code(error_code)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let exhausted = attempt_count >= MAX_DELIVERY_ATTEMPTS;
        let delay_seconds = i64::from(2_i32.pow((attempt_count - 1).cast_unsigned()).min(60));
        let next_attempt =
            (!exhausted).then(|| OffsetDateTime::now_utc() + Duration::seconds(delay_seconds));
        let result = sqlx::query(
            "\
            UPDATE webhook_deliveries
            SET status = CASE WHEN $2 THEN 'failed' ELSE 'retry_wait' END,
                response_code = $3, last_error_code = $4, next_attempt_at = $5,
                lease_owner = NULL, lease_expires_at = NULL
            WHERE id = $1
              AND status = 'sending'
              AND lease_owner = $6
              AND attempt_count = $7",
        )
        .bind(delivery_id)
        .bind(exhausted)
        .bind(response_code)
        .bind(error_code)
        .bind(next_attempt)
        .bind(worker_id)
        .bind(attempt_count)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(result.rows_affected() == 1)
    }

    /// Requeues one terminally failed delivery using its original ID and
    /// immutable destination snapshot. The stable delivery ID also remains the
    /// outbound idempotency key. Pending retries are accepted idempotently,
    /// while sending or delivered rows return a conflict.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn retry_webhook_delivery(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        delivery_id: Uuid,
    ) -> Result<RetryWebhookDeliveryOutcome, StorageError> {
        if !is_v7(user_id) || !is_v7(project_id) || !is_v7(delivery_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let status = sqlx::query_scalar::<_, String>(
            "\
            SELECT delivery.status
            FROM webhook_deliveries AS delivery
            WHERE delivery.id = $1
              AND delivery.project_id = $2
              AND delivery.user_id = $3
            FOR UPDATE",
        )
        .bind(delivery_id)
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let outcome = match status.as_deref() {
            Some("failed") => {
                let result = sqlx::query(
                    "\
                    UPDATE webhook_deliveries
                    SET status = 'queued',
                        attempt_count = 0,
                        next_attempt_at = NULL,
                        response_code = NULL,
                        last_error_code = NULL,
                        delivered_at = NULL,
                        lease_owner = NULL,
                        lease_expires_at = NULL
                    WHERE id = $1 AND status = 'failed'",
                )
                .bind(delivery_id)
                .execute(&mut *transaction)
                .await
                .map_err(classify)?;
                if result.rows_affected() == 1 {
                    RetryWebhookDeliveryOutcome::Queued
                } else {
                    RetryWebhookDeliveryOutcome::Conflict
                }
            }
            Some("queued" | "retry_wait") => RetryWebhookDeliveryOutcome::AlreadyQueued,
            Some("sending" | "delivered") | None => RetryWebhookDeliveryOutcome::Conflict,
            Some(_) => return Err(StorageError::PersistenceUnavailable),
        };
        transaction.commit().await.map_err(classify)?;
        Ok(outcome)
    }

    /// Lists the latest delivery history without payloads or authentication.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn webhook_delivery_history(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<WebhookDelivery>, StorageError> {
        if !is_v7(user_id) || !is_v7(project_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, WebhookDelivery>(
            "\
            SELECT delivery.id, delivery.webhook_id, delivery.event_type,
                delivery.status, delivery.attempt_count, delivery.response_code,
                delivery.last_error_code, delivery.created_at, delivery.delivered_at
            FROM webhook_deliveries AS delivery
            WHERE delivery.project_id = $1 AND delivery.user_id = $2
            ORDER BY delivery.created_at DESC, delivery.id DESC
            LIMIT 50",
        )
        .bind(project_id)
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)
    }
}

pub(crate) async fn queue_project_event_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    project_id: Uuid,
    event_type: &str,
    payload: &Value,
) -> Result<usize, StorageError> {
    if !is_v7(user_id)
        || !is_v7(project_id)
        || !valid_event_type(event_type)
        || !payload.is_object()
    {
        return Err(StorageError::InvalidConfiguration);
    }
    let webhooks = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Vec<u8>,
            Vec<u8>,
            sqlx::types::Json<GoogleChatMentionDirectory>,
        ),
    >(
        "\
        SELECT webhook.id, webhook.provider, webhook.destination_ciphertext,
            webhook.destination_nonce, webhook.mention_directory
        FROM project_webhooks AS webhook
        INNER JOIN projects AS project ON project.id = webhook.project_id
        WHERE webhook.project_id = $2 AND project.user_id = $1
          AND webhook.enabled = TRUE AND $3 = ANY(webhook.events)",
    )
    .bind(user_id)
    .bind(project_id)
    .bind(event_type)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)?;
    for (webhook_id, provider, destination_ciphertext, destination_nonce, mention_directory) in
        &webhooks
    {
        sqlx::query(
            "\
            INSERT INTO webhook_deliveries (
                id, user_id, project_id, webhook_id, provider,
                destination_ciphertext, destination_nonce,
                mention_directory, event_type, payload, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'queued')",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(project_id)
        .bind(webhook_id)
        .bind(provider)
        .bind(destination_ciphertext)
        .bind(destination_nonce)
        .bind(mention_directory)
        .bind(event_type)
        .bind(payload)
        .execute(&mut **transaction)
        .await
        .map_err(classify)?;
    }
    Ok(webhooks.len())
}

pub(crate) fn project_event_payload(
    event_type: &str,
    project_id: Uuid,
    entity_id: Uuid,
) -> Result<Value, StorageError> {
    let occurred_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| StorageError::PersistenceUnavailable)?;
    Ok(serde_json::json!({
        "event": event_type,
        "projectId": project_id,
        "entityId": entity_id,
        "occurredAt": occurred_at,
    }))
}

fn validate_new_webhook(command: &NewProjectWebhook) -> Result<(), StorageError> {
    if !is_v7(command.id)
        || !is_v7(command.user_id)
        || !is_v7(command.project_id)
        || invalid_encrypted_secret(&command.destination)
        || !valid_destination_hint(&command.destination_hint)
        || !command.mention_directory.is_valid_for(command.provider)
        || command.events.is_empty()
        || command.events.len() > MAX_EVENT_TYPES
        || has_invalid_events(&command.events)
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn validate_webhook_update(command: &ProjectWebhookUpdate) -> Result<(), StorageError> {
    let invalid_destination = match &command.destination {
        WebhookDestinationUpdate::Keep => false,
        WebhookDestinationUpdate::Replace {
            provider: _,
            secret,
            hint,
        } => invalid_encrypted_secret(secret) || !valid_destination_hint(hint),
    };
    let invalid_mention_directory = match &command.mention_directory {
        WebhookMentionDirectoryUpdate::Keep => false,
        WebhookMentionDirectoryUpdate::Replace(directory) => {
            !directory.is_valid_for(command.provider)
        }
    };
    if !is_v7(command.id)
        || !is_v7(command.user_id)
        || !is_v7(command.project_id)
        || command.events.is_empty()
        || command.events.len() > MAX_EVENT_TYPES
        || has_invalid_events(&command.events)
        || invalid_destination
        || invalid_mention_directory
        || command.expected_version <= 0
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn has_invalid_events(events: &[String]) -> bool {
    events.iter().enumerate().any(|(index, event)| {
        !valid_event_type(event) || events[..index].iter().any(|seen| seen == event)
    })
}

fn invalid_encrypted_secret(secret: &EncryptedWebhookSecret) -> bool {
    secret.ciphertext.is_empty()
        || secret.ciphertext.len() > MAX_SECRET_BYTES
        || secret.nonce.len() != 24
}

fn valid_destination_hint(value: &str) -> bool {
    let length = value.trim().chars().count();
    (1..=120).contains(&length)
}

fn valid_mention_name(value: &str) -> bool {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    value == trimmed
        && (1..=MAX_MENTION_NAME_CHARS).contains(&length)
        && !trimmed.chars().any(|character| {
            character.is_control() || matches!(character, '@' | '<' | '>' | '{' | '}')
        })
}

fn valid_google_chat_user_id(value: &str) -> bool {
    let Some(digits) = value.strip_prefix("users/") else {
        return false;
    };
    (1..=MAX_GOOGLE_CHAT_USER_ID_DIGITS).contains(&digits.len())
        && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn project_webhook(row: ProjectWebhookRow) -> ProjectWebhook {
    ProjectWebhook {
        id: row.id,
        project_id: row.project_id,
        provider: WebhookProvider::parse(&row.provider)
            .expect("the webhook provider database constraint must stay in sync"),
        destination_hint: row.destination_hint,
        mention_directory: row.mention_directory,
        events: row.events,
        enabled: row.enabled,
        version: row.version,
    }
}

fn valid_event_type(value: &str) -> bool {
    matches!(
        value,
        "project.created"
            | "project.updated"
            | "project.deleted"
            | "task.created"
            | "task.updated"
            | "task.completed"
            | "task.restored"
            | "task.deleted"
            | "webhook.test"
            | "chat.message"
    )
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_')
        })
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_WORKER_ID_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_event_payload_uses_parseable_rfc3339_time() {
        let project_id = Uuid::now_v7();
        let entity_id = Uuid::now_v7();
        let payload = project_event_payload("task.created", project_id, entity_id)
            .expect("the current UTC time should always format as RFC 3339");
        let occurred_at = payload
            .get("occurredAt")
            .and_then(Value::as_str)
            .expect("occurredAt should be a string");
        let expected_project_id = project_id.to_string();
        let expected_entity_id = entity_id.to_string();

        assert!(OffsetDateTime::parse(occurred_at, &Rfc3339).is_ok());
        assert_eq!(payload.get("event"), Some(&Value::from("task.created")));
        assert_eq!(
            payload.get("projectId").and_then(Value::as_str),
            Some(expected_project_id.as_str())
        );
        assert_eq!(
            payload.get("entityId").and_then(Value::as_str),
            Some(expected_entity_id.as_str())
        );
    }
}

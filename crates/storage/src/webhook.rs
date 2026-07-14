//! Project webhook configuration and durable outbound delivery queue.

use serde_json::Value;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{Database, StorageError};

const MAX_URL_BYTES: usize = 4_096;
const MAX_EVENT_TYPES: usize = 16;
const MAX_SECRET_BYTES: usize = 8 * 1024;
const MAX_DELIVERY_ATTEMPTS: i32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectWebhook {
    pub id: Uuid,
    pub project_id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub has_authentication: bool,
    pub enabled: bool,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
struct ProjectWebhookRow {
    id: Uuid,
    project_id: Uuid,
    url: String,
    events: Vec<String>,
    has_authentication: bool,
    enabled: bool,
    version: i64,
}

pub struct NewProjectWebhook {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub authentication: Option<EncryptedWebhookSecret>,
}

pub struct EncryptedWebhookSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct ClaimedWebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub project_id: Uuid,
    pub event_type: String,
    pub payload: Value,
    pub attempt_count: i32,
    pub url: String,
    pub auth_header_ciphertext: Option<Vec<u8>>,
    pub auth_header_nonce: Option<Vec<u8>>,
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
                id, user_id, project_id, url, events,
                auth_header_ciphertext, auth_header_nonce
            )
            SELECT $1, $2, project.id, $4, $5, $6, $7
            FROM projects AS project
            WHERE project.id = $3 AND project.user_id = $2
            RETURNING id, project_id, url, events,
                auth_header_ciphertext IS NOT NULL AS has_authentication,
                enabled, version",
        )
        .bind(command.id)
        .bind(command.user_id)
        .bind(command.project_id)
        .bind(command.url.trim())
        .bind(&command.events)
        .bind(
            command
                .authentication
                .as_ref()
                .map(|secret| secret.ciphertext.as_slice()),
        )
        .bind(
            command
                .authentication
                .as_ref()
                .map(|secret| secret.nonce.as_slice()),
        )
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
            SELECT webhook.id, webhook.project_id, webhook.url, webhook.events,
                webhook.auth_header_ciphertext IS NOT NULL AS has_authentication,
                webhook.enabled, webhook.version
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
                id, user_id, project_id, webhook_id, destination_url,
                auth_header_ciphertext, auth_header_nonce,
                event_type, payload, status
            )
            SELECT $4, $1, webhook.project_id, webhook.id, webhook.url,
                webhook.auth_header_ciphertext, webhook.auth_header_nonce,
                'webhook.test', $5, 'queued'
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
        limit: i64,
    ) -> Result<Vec<ClaimedWebhookDelivery>, StorageError> {
        if !(1..=50).contains(&limit) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, ClaimedWebhookDelivery>(
            "\
            WITH candidates AS (
                SELECT delivery.id
                FROM webhook_deliveries AS delivery
                WHERE delivery.status IN ('queued', 'retry_wait')
                  AND COALESCE(delivery.next_attempt_at, delivery.created_at) <= NOW()
                ORDER BY COALESCE(delivery.next_attempt_at, delivery.created_at), delivery.id
                FOR UPDATE SKIP LOCKED
                LIMIT $1
            )
            UPDATE webhook_deliveries AS delivery
            SET status = 'sending', attempt_count = delivery.attempt_count + 1
            FROM candidates
            WHERE delivery.id = candidates.id
            RETURNING delivery.id, delivery.webhook_id, delivery.project_id,
                delivery.event_type, delivery.payload, delivery.attempt_count,
                delivery.destination_url AS url,
                delivery.auth_header_ciphertext, delivery.auth_header_nonce",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(classify)
    }

    /// Marks a claimed delivery as successfully delivered.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn complete_webhook_delivery(
        &self,
        delivery_id: Uuid,
        response_code: i32,
    ) -> Result<(), StorageError> {
        if !is_v7(delivery_id) || !(200..=299).contains(&response_code) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "\
            UPDATE webhook_deliveries
            SET status = 'delivered', response_code = $2,
                last_error_code = NULL, delivered_at = NOW(), next_attempt_at = NULL
            WHERE id = $1 AND status = 'sending'",
        )
        .bind(delivery_id)
        .bind(response_code)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }

    /// Records a bounded retry or terminal failure for a claimed delivery.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error or a classified persistence error.
    pub async fn fail_webhook_delivery(
        &self,
        delivery_id: Uuid,
        attempt_count: i32,
        response_code: Option<i32>,
        error_code: &str,
    ) -> Result<(), StorageError> {
        if !is_v7(delivery_id)
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
        sqlx::query(
            "\
            UPDATE webhook_deliveries
            SET status = CASE WHEN $2 THEN 'failed' ELSE 'retry_wait' END,
                response_code = $3, last_error_code = $4, next_attempt_at = $5
            WHERE id = $1 AND status = 'sending'",
        )
        .bind(delivery_id)
        .bind(exhausted)
        .bind(response_code)
        .bind(error_code)
        .bind(next_attempt)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
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
            INNER JOIN projects AS project ON project.id = delivery.project_id
            WHERE delivery.project_id = $1 AND project.user_id = $2
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
    let webhooks = sqlx::query_as::<_, (Uuid, String, Option<Vec<u8>>, Option<Vec<u8>>)>(
        "\
        SELECT webhook.id, webhook.url,
            webhook.auth_header_ciphertext, webhook.auth_header_nonce
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
    for (webhook_id, url, ciphertext, nonce) in &webhooks {
        sqlx::query(
            "\
            INSERT INTO webhook_deliveries (
                id, user_id, project_id, webhook_id, destination_url,
                auth_header_ciphertext, auth_header_nonce,
                event_type, payload, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'queued')",
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(project_id)
        .bind(webhook_id)
        .bind(url)
        .bind(ciphertext)
        .bind(nonce)
        .bind(event_type)
        .bind(payload)
        .execute(&mut **transaction)
        .await
        .map_err(classify)?;
    }
    Ok(webhooks.len())
}

fn validate_new_webhook(command: &NewProjectWebhook) -> Result<(), StorageError> {
    if !is_v7(command.id)
        || !is_v7(command.user_id)
        || !is_v7(command.project_id)
        || command.url.trim().len() > MAX_URL_BYTES
        || command.url.trim().len() < 8
        || command.events.is_empty()
        || command.events.len() > MAX_EVENT_TYPES
        || command.events.iter().any(|event| !valid_event_type(event))
        || command.authentication.as_ref().is_some_and(|secret| {
            secret.ciphertext.is_empty()
                || secret.ciphertext.len() > MAX_SECRET_BYTES
                || secret.nonce.len() != 24
        })
    {
        return Err(StorageError::InvalidConfiguration);
    }
    Ok(())
}

fn project_webhook(row: ProjectWebhookRow) -> ProjectWebhook {
    ProjectWebhook {
        id: row.id,
        project_id: row.project_id,
        url: row.url,
        events: row.events,
        has_authentication: row.has_authentication,
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
    )
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_')
        })
}

fn is_v7(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

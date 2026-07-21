use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{Database, StorageError};

const MAX_TOKEN_CIPHERTEXT_BYTES: usize = 8 * 1024;
const MAX_ERROR_CODE_BYTES: usize = 120;
const MAX_WORKER_ID_BYTES: usize = 200;
const MAX_ATTEMPTS: i32 = 8;
const REMINDER_LEAD_TIME: Duration = Duration::minutes(15);
const DELIVERY_LEASE_SECONDS: i32 = 120;
const _: () = assert!(DELIVERY_LEASE_SECONDS > 15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedPushToken {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub fingerprint: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRegistrationState {
    pub enabled: bool,
    pub last_seen_at: Option<OffsetDateTime>,
    pub last_delivered_at: Option<OffsetDateTime>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ClaimedPushDelivery {
    pub id: Uuid,
    pub device_id: Uuid,
    pub item_type: String,
    pub item_id: Uuid,
    pub destination: String,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    pub target_at: OffsetDateTime,
    pub attempt_count: i32,
    pub token_ciphertext: Vec<u8>,
    pub token_nonce: Vec<u8>,
}

#[derive(sqlx::FromRow)]
struct ReminderCandidate {
    user_id: Uuid,
    item_type: String,
    item_id: Uuid,
    item_version: i64,
    project_id: Option<Uuid>,
    raw_title: String,
    target_at: OffsetDateTime,
}

impl Database {
    /// Stores one encrypted FCM token for the authenticated Android device.
    /// A token that moved to another device is invalidated before the current
    /// registration is committed.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for malformed identifiers or ciphertext,
    /// and a persistence error when the device is not an active Android device.
    pub async fn register_push_token(
        &self,
        registration_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
        token: &EncryptedPushToken,
    ) -> Result<PushRegistrationState, StorageError> {
        if !is_v7(registration_id)
            || !is_v7(user_id)
            || !is_v7(device_id)
            || invalid_encrypted_token(token)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let android_device_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1 FROM devices
                WHERE id = $1 AND user_id = $2 AND platform = 'android' AND status = 'active'
            )",
        )
        .bind(device_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(classify)?;
        if !android_device_exists {
            return Err(StorageError::InvalidConfiguration);
        }

        sqlx::query(
            "UPDATE push_registrations
             SET status = 'invalidated', invalidated_at = NOW(),
                 last_error_code = 'push.token_rotated'
             WHERE token_fingerprint = $1 AND device_id <> $2 AND status = 'active'",
        )
        .bind(token.fingerprint.as_slice())
        .bind(device_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;

        sqlx::query(
            "INSERT INTO push_registrations (
                id, user_id, device_id, token_ciphertext, token_nonce,
                token_fingerprint, status, last_seen_at
             ) VALUES ($1, $2, $3, $4, $5, $6, 'active', NOW())
             ON CONFLICT (device_id) DO UPDATE SET
                token_ciphertext = EXCLUDED.token_ciphertext,
                token_nonce = EXCLUDED.token_nonce,
                token_fingerprint = EXCLUDED.token_fingerprint,
                status = 'active', last_error_code = NULL,
                invalidated_at = NULL, last_seen_at = NOW()",
        )
        .bind(registration_id)
        .bind(user_id)
        .bind(device_id)
        .bind(token.ciphertext.as_slice())
        .bind(token.nonce.as_slice())
        .bind(token.fingerprint.as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        self.push_registration_state(user_id, device_id).await
    }

    /// Returns only safe registration metadata for the authenticated device.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    pub async fn push_registration_state(
        &self,
        user_id: Uuid,
        device_id: Uuid,
    ) -> Result<PushRegistrationState, StorageError> {
        if !is_v7(user_id) || !is_v7(device_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let row = sqlx::query_as::<
            _,
            (
                String,
                OffsetDateTime,
                Option<OffsetDateTime>,
                Option<String>,
            ),
        >(
            "SELECT status, last_seen_at, last_delivered_at, last_error_code
             FROM push_registrations WHERE user_id = $1 AND device_id = $2",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(self.pool())
        .await
        .map_err(classify)?;
        Ok(row.map_or(
            PushRegistrationState {
                enabled: false,
                last_seen_at: None,
                last_delivered_at: None,
                last_error_code: None,
            },
            |(status, last_seen_at, last_delivered_at, last_error_code)| PushRegistrationState {
                enabled: status == "active",
                last_seen_at: Some(last_seen_at),
                last_delivered_at,
                last_error_code,
            },
        ))
    }

    /// Disables push delivery for the authenticated device without deleting
    /// its delivery audit history.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    pub async fn disable_push_registration(
        &self,
        user_id: Uuid,
        device_id: Uuid,
    ) -> Result<(), StorageError> {
        if !is_v7(user_id) || !is_v7(device_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query(
            "UPDATE push_registrations
             SET status = 'invalidated', invalidated_at = NOW(),
                 last_error_code = NULL
             WHERE user_id = $1 AND device_id = $2",
        )
        .bind(user_id)
        .bind(device_id)
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }

    /// Reconciles due task and schedule reminders into a durable, per-device
    /// queue. The unique item-version key makes the operation idempotent.
    ///
    /// # Errors
    ///
    /// Returns a persistence error when reconciliation cannot complete.
    pub async fn queue_due_push_reminders(
        &self,
        now: OffsetDateTime,
    ) -> Result<usize, StorageError> {
        self.cancel_stale_push_deliveries().await?;
        let candidates = sqlx::query_as::<_, ReminderCandidate>(
            "SELECT task.user_id, 'task'::TEXT AS item_type, task.id AS item_id,
                    task.version AS item_version, task.project_id,
                    task.title AS raw_title, task.due_at AS target_at
             FROM tasks AS task
             WHERE task.status = 'open' AND task.due_at IS NOT NULL
               AND task.due_at > $1 AND task.due_at - INTERVAL '15 minutes' <= $1
             UNION ALL
             SELECT entry.user_id, 'schedule'::TEXT AS item_type, entry.id AS item_id,
                    entry.version AS item_version, NULL::UUID AS project_id,
                    entry.title AS raw_title, entry.starts_at AS target_at
             FROM schedule_entries AS entry
             WHERE entry.status = 'confirmed' AND entry.starts_at > $1
               AND entry.starts_at - INTERVAL '15 minutes' <= $1
             ORDER BY target_at, item_id
             LIMIT 500",
        )
        .bind(now)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;

        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let mut queued = 0_usize;
        for candidate in candidates {
            queued += queue_candidate(&mut transaction, &candidate).await?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(queued)
    }

    /// Claims ready deliveries with a bounded lease. Expired leases are made
    /// retryable before the claim, allowing safe process restart recovery.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    pub async fn claim_push_deliveries(
        &self,
        worker_id: &str,
        limit: i64,
    ) -> Result<Vec<ClaimedPushDelivery>, StorageError> {
        if !valid_worker_id(worker_id) || !(1..=50).contains(&limit) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        sqlx::query(
            "UPDATE push_deliveries
             SET status = 'retry_wait', next_attempt_at = NOW(),
                 lease_owner = NULL, lease_expires_at = NULL,
                 last_error_code = 'push.worker_lease_expired'
             WHERE status = 'sending' AND lease_expires_at <= NOW()",
        )
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;

        let rows = sqlx::query_as::<_, ClaimedPushDelivery>(
            "WITH claimable AS (
                SELECT delivery.id
                FROM push_deliveries AS delivery
                INNER JOIN push_registrations AS registration
                    ON registration.device_id = delivery.device_id
                   AND registration.user_id = delivery.user_id
                   AND registration.status = 'active'
                WHERE delivery.status IN ('queued', 'retry_wait')
                  AND COALESCE(delivery.next_attempt_at, delivery.notify_at) <= NOW()
                  AND delivery.target_at > NOW()
                ORDER BY COALESCE(delivery.next_attempt_at, delivery.notify_at), delivery.id
                FOR UPDATE OF delivery SKIP LOCKED
                LIMIT $1
             ), claimed AS (
                UPDATE push_deliveries AS delivery
                SET status = 'sending', attempt_count = delivery.attempt_count + 1,
                    lease_owner = $2, lease_expires_at = NOW() + make_interval(secs => $3),
                    next_attempt_at = NULL
                FROM claimable
                WHERE delivery.id = claimable.id
                RETURNING delivery.id, delivery.device_id, delivery.item_type,
                    delivery.item_id, delivery.destination, delivery.project_id,
                    delivery.title, delivery.body, delivery.target_at,
                    delivery.attempt_count
             )
             SELECT claimed.id, claimed.device_id, claimed.item_type, claimed.item_id,
                    claimed.destination, claimed.project_id, claimed.title, claimed.body,
                    claimed.target_at, claimed.attempt_count,
                    registration.token_ciphertext, registration.token_nonce
             FROM claimed
             INNER JOIN push_registrations AS registration
                 ON registration.device_id = claimed.device_id AND registration.status = 'active'",
        )
        .bind(limit)
        .bind(worker_id)
        .bind(DELIVERY_LEASE_SECONDS)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        Ok(rows)
    }

    /// Marks a leased delivery complete and updates safe device health data.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    pub async fn complete_push_delivery(
        &self,
        delivery_id: Uuid,
        worker_id: &str,
        attempt_count: i32,
        response_code: i32,
    ) -> Result<(), StorageError> {
        if !is_v7(delivery_id)
            || !valid_worker_id(worker_id)
            || !(1..=MAX_ATTEMPTS).contains(&attempt_count)
            || !(100..=599).contains(&response_code)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let device_id = sqlx::query_scalar::<_, Uuid>(
            "UPDATE push_deliveries
             SET status = 'delivered', delivered_at = NOW(), response_code = $4,
                 last_error_code = NULL, lease_owner = NULL, lease_expires_at = NULL
             WHERE id = $1 AND status = 'sending' AND lease_owner = $2
               AND attempt_count = $3
             RETURNING device_id",
        )
        .bind(delivery_id)
        .bind(worker_id)
        .bind(attempt_count)
        .bind(response_code)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(device_id) = device_id {
            sqlx::query(
                "UPDATE push_registrations
                 SET last_delivered_at = NOW(), last_error_code = NULL
                 WHERE device_id = $1 AND status = 'active'",
            )
            .bind(device_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(())
    }

    /// Persists a sanitized provider failure and optionally invalidates a
    /// rotated or unregistered FCM token.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    #[allow(clippy::too_many_arguments)]
    pub async fn fail_push_delivery(
        &self,
        delivery_id: Uuid,
        worker_id: &str,
        attempt_count: i32,
        response_code: Option<i32>,
        error_code: &str,
        retryable: bool,
        invalidate_token: bool,
    ) -> Result<(), StorageError> {
        if !is_v7(delivery_id)
            || !valid_worker_id(worker_id)
            || !(1..=MAX_ATTEMPTS).contains(&attempt_count)
            || response_code.is_some_and(|value| !(100..=599).contains(&value))
            || !valid_error_code(error_code)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let should_retry = retryable && attempt_count < MAX_ATTEMPTS && !invalidate_token;
        let next_attempt_at = should_retry.then(|| {
            let exponent = (attempt_count - 1).min(5).cast_unsigned();
            let seconds = i64::from(5 * 2_i32.pow(exponent));
            OffsetDateTime::now_utc() + Duration::seconds(seconds)
        });
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let device_id = sqlx::query_scalar::<_, Uuid>(
            "UPDATE push_deliveries
             SET status = CASE WHEN $6 THEN 'retry_wait' ELSE 'failed' END,
                 next_attempt_at = $7, response_code = $4, last_error_code = $5,
                 lease_owner = NULL, lease_expires_at = NULL
             WHERE id = $1 AND status = 'sending' AND lease_owner = $2
               AND attempt_count = $3
             RETURNING device_id",
        )
        .bind(delivery_id)
        .bind(worker_id)
        .bind(attempt_count)
        .bind(response_code)
        .bind(error_code)
        .bind(should_retry)
        .bind(next_attempt_at)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        if let Some(device_id) = device_id {
            let status = if invalidate_token {
                "invalidated"
            } else {
                "active"
            };
            sqlx::query(
                "UPDATE push_registrations
                 SET status = $2, last_error_code = $3,
                     invalidated_at = CASE WHEN $2 = 'invalidated' THEN NOW() ELSE invalidated_at END
                 WHERE device_id = $1",
            )
            .bind(device_id)
            .bind(status)
            .bind(error_code)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        transaction.commit().await.map_err(classify)?;
        Ok(())
    }

    async fn cancel_stale_push_deliveries(&self) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE push_deliveries AS delivery
             SET status = 'cancelled', next_attempt_at = NULL,
                 lease_owner = NULL, lease_expires_at = NULL,
                 last_error_code = 'push.reminder_stale'
             WHERE delivery.status IN ('queued', 'retry_wait')
               AND (
                   NOT EXISTS (
                       SELECT 1 FROM push_registrations AS registration
                       WHERE registration.device_id = delivery.device_id
                         AND registration.user_id = delivery.user_id
                         AND registration.status = 'active'
                   )
                   OR NOT (
                       (delivery.item_type = 'task' AND EXISTS (
                           SELECT 1 FROM tasks AS task
                           WHERE task.id = delivery.item_id AND task.user_id = delivery.user_id
                             AND task.status = 'open' AND task.version = delivery.item_version
                             AND task.due_at = delivery.target_at
                       ))
                       OR
                       (delivery.item_type = 'schedule' AND EXISTS (
                           SELECT 1 FROM schedule_entries AS entry
                           WHERE entry.id = delivery.item_id AND entry.user_id = delivery.user_id
                             AND entry.status = 'confirmed' AND entry.version = delivery.item_version
                             AND entry.starts_at = delivery.target_at
                       ))
                   )
               )",
        )
        .execute(self.pool())
        .await
        .map_err(classify)?;
        Ok(())
    }
}

async fn queue_candidate(
    transaction: &mut Transaction<'_, Postgres>,
    candidate: &ReminderCandidate,
) -> Result<usize, StorageError> {
    let device_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT device_id FROM push_registrations
         WHERE user_id = $1 AND status = 'active'
         ORDER BY device_id",
    )
    .bind(candidate.user_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)?;
    let (destination, title, body) = reminder_copy(candidate);
    let mut queued = 0_usize;
    for device_id in device_ids {
        let result = sqlx::query(
            "INSERT INTO push_deliveries (
                id, user_id, device_id, item_type, item_id, item_version,
                destination, project_id, title, body, target_at, notify_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (device_id, item_type, item_id, item_version) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(candidate.user_id)
        .bind(device_id)
        .bind(&candidate.item_type)
        .bind(candidate.item_id)
        .bind(candidate.item_version)
        .bind(destination)
        .bind(candidate.project_id)
        .bind(&title)
        .bind(body)
        .bind(candidate.target_at)
        .bind(candidate.target_at - REMINDER_LEAD_TIME)
        .execute(&mut **transaction)
        .await
        .map_err(classify)?;
        queued += usize::from(result.rows_affected() > 0);
    }
    Ok(queued)
}

fn reminder_copy(candidate: &ReminderCandidate) -> (&'static str, String, &'static str) {
    let raw_title = candidate.raw_title.trim();
    let title = match candidate.item_type.as_str() {
        "task" => format!("곧 마감해요 · {raw_title}"),
        _ => format!("곧 시작해요 · {raw_title}"),
    };
    let title = title.chars().take(120).collect();
    match (candidate.item_type.as_str(), candidate.project_id) {
        ("task", Some(_)) => (
            "projects",
            title,
            "할 일 기한이 다가왔어요. 지금 진행 상황을 확인해 보세요.",
        ),
        ("task", None) => (
            "calendar",
            title,
            "할 일 기한이 다가왔어요. 지금 진행 상황을 확인해 보세요.",
        ),
        _ => ("calendar", title, "일정 내용을 확인하고 준비해 주세요."),
    }
}

fn invalid_encrypted_token(token: &EncryptedPushToken) -> bool {
    token.ciphertext.is_empty()
        || token.ciphertext.len() > MAX_TOKEN_CIPHERTEXT_BYTES
        || token.nonce.len() != 24
        || token.fingerprint.len() != 32
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_WORKER_ID_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ERROR_CODE_BYTES
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_copy_keeps_notification_contract_bounded() {
        let candidate = ReminderCandidate {
            user_id: Uuid::now_v7(),
            item_type: "task".to_owned(),
            item_id: Uuid::now_v7(),
            item_version: 1,
            project_id: Some(Uuid::now_v7()),
            raw_title: "가".repeat(200),
            target_at: OffsetDateTime::now_utc(),
        };
        let (destination, title, body) = reminder_copy(&candidate);
        assert_eq!(destination, "projects");
        assert_eq!(title.chars().count(), 120);
        assert!(body.chars().count() <= 240);
    }

    #[test]
    fn encrypted_tokens_must_include_a_full_nonce_and_fingerprint() {
        let valid = EncryptedPushToken {
            ciphertext: vec![1; 32],
            nonce: vec![2; 24],
            fingerprint: vec![3; 32],
        };
        assert!(!invalid_encrypted_token(&valid));
        assert!(invalid_encrypted_token(&EncryptedPushToken {
            nonce: vec![2; 12],
            ..valid
        }));
    }

    #[test]
    fn retry_error_codes_are_safe_for_logs_and_storage() {
        assert!(valid_error_code("push.provider_unavailable"));
        assert!(!valid_error_code("push token=secret"));
    }
}

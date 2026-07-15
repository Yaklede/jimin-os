//! Durable Google Calendar mutations for server-owned schedule entries.
//!
//! The journal is committed in the same transaction as the local schedule.
//! Workers receive only routing identifiers and validated event fields; OAuth
//! credentials remain in the Calendar connection store.

use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{Database, StorageError, calendar::PrimaryCalendarMutationTarget};

const MAX_ATTEMPTS: i32 = 8;
const CLAIM_LEASE_SECONDS: i64 = 60;
const EXHAUSTED_MUTATION_ERROR_CODE: &str = "calendar.provider_unavailable";
const UNAVAILABLE_DESTINATION_ERROR_CODE: &str = "calendar.destination_unavailable";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleCalendarMutationOperation {
    Create,
    Update,
    Delete,
}

impl ScheduleCalendarMutationOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "create" => Ok(Self::Create),
            "update" => Ok(Self::Update),
            "delete" => Ok(Self::Delete),
            _ => Err(StorageError::PersistenceUnavailable),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleCalendarMutationPayload {
    pub title: String,
    pub notes: Option<String>,
    pub starts_at: OffsetDateTime,
    pub ends_at: OffsetDateTime,
    pub time_zone: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedScheduleCalendarMutation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub account_id: Uuid,
    pub calendar_id: Uuid,
    pub provider_calendar_id: String,
    pub provider_event_id: String,
    pub provider_etag: Option<String>,
    pub schedule_entry_id: Uuid,
    pub operation: ScheduleCalendarMutationOperation,
    pub payload: ScheduleCalendarMutationPayload,
    pub schedule_version: i64,
    pub attempt_count: i32,
    pub lease_owner: String,
}

#[derive(sqlx::FromRow)]
struct ClaimedScheduleCalendarMutationRow {
    id: Uuid,
    user_id: Uuid,
    account_id: Uuid,
    calendar_id: Uuid,
    provider_calendar_id: String,
    provider_event_id: String,
    provider_etag: Option<String>,
    schedule_entry_id: Uuid,
    operation: String,
    desired_payload: serde_json::Value,
    expected_event_version: i64,
    attempt_count: i32,
    lease_owner: String,
}

impl TryFrom<ClaimedScheduleCalendarMutationRow> for ClaimedScheduleCalendarMutation {
    type Error = StorageError;

    fn try_from(row: ClaimedScheduleCalendarMutationRow) -> Result<Self, Self::Error> {
        let payload = serde_json::from_value(row.desired_payload)
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        let mutation = Self {
            id: row.id,
            user_id: row.user_id,
            account_id: row.account_id,
            calendar_id: row.calendar_id,
            provider_calendar_id: row.provider_calendar_id,
            provider_event_id: row.provider_event_id,
            provider_etag: row.provider_etag,
            schedule_entry_id: row.schedule_entry_id,
            operation: ScheduleCalendarMutationOperation::parse(&row.operation)?,
            payload,
            schedule_version: row.expected_event_version,
            attempt_count: row.attempt_count,
            lease_owner: row.lease_owner,
        };
        if !mutation.valid() {
            return Err(StorageError::PersistenceUnavailable);
        }
        Ok(mutation)
    }
}

impl ClaimedScheduleCalendarMutation {
    fn valid(&self) -> bool {
        [
            self.id,
            self.user_id,
            self.account_id,
            self.calendar_id,
            self.schedule_entry_id,
        ]
        .into_iter()
        .all(|id| id.get_version_num() == 7)
            && !self.provider_calendar_id.is_empty()
            && self.provider_calendar_id.len() <= 1_024
            && valid_provider_event_id(&self.provider_event_id)
            && self.schedule_version > 0
            && self.attempt_count > 0
            && !self.lease_owner.is_empty()
            && self.lease_owner.len() <= 200
            && !self.lease_owner.chars().any(char::is_control)
            && self.payload.ends_at > self.payload.starts_at
    }
}

pub(crate) async fn attach_schedule_and_queue_create(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    schedule_entry_id: Uuid,
    schedule_version: i64,
    target: &PrimaryCalendarMutationTarget,
    payload: &ScheduleCalendarMutationPayload,
) -> Result<(), StorageError> {
    let provider_event_id = provider_event_id_for_schedule(schedule_entry_id);
    let attached = sqlx::query(
        "\
        INSERT INTO schedule_calendar_links (
            schedule_entry_id, user_id, account_id, calendar_id, provider_event_id
        )
        SELECT $1, $2, account.id, calendar.id, $5
        FROM calendar_accounts AS account
        INNER JOIN calendars AS calendar ON calendar.account_id = account.id
        WHERE account.id = $3
          AND account.user_id = $2
          AND account.status = 'active'
          AND calendar.id = $4
          AND calendar.is_primary = TRUE
          AND calendar.sync_enabled = TRUE
          AND calendar.provider_deleted_at IS NULL
          AND calendar.access_role IN ('owner', 'writer')",
    )
    .bind(schedule_entry_id)
    .bind(user_id)
    .bind(target.account_id)
    .bind(target.calendar_id)
    .bind(&provider_event_id)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    if attached.rows_affected() != 1 {
        return Err(StorageError::InvalidConfiguration);
    }
    queue_mutation(
        transaction,
        user_id,
        schedule_entry_id,
        schedule_version,
        ScheduleCalendarMutationOperation::Create,
        &provider_event_id,
        None,
        payload,
    )
    .await
}

pub(crate) async fn attach_schedule_to_active_primary_and_queue_create(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    schedule_entry_id: Uuid,
    schedule_version: i64,
    payload: &ScheduleCalendarMutationPayload,
) -> Result<bool, StorageError> {
    let target = sqlx::query_as::<_, (Uuid, Uuid, String, String)>(
        "\
        SELECT account.id, calendar.id, calendar.provider_calendar_id, calendar.time_zone
        FROM calendar_accounts AS account
        INNER JOIN calendars AS calendar ON calendar.account_id = account.id
        WHERE account.user_id = $1
          AND account.status = 'active'
          AND calendar.is_primary = TRUE
          AND calendar.sync_enabled = TRUE
          AND calendar.provider_deleted_at IS NULL
          AND calendar.access_role IN ('owner', 'writer')
        LIMIT 1
        FOR UPDATE OF account, calendar",
    )
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)?;
    let Some((account_id, calendar_id, provider_calendar_id, time_zone)) = target else {
        return Ok(false);
    };
    let target = PrimaryCalendarMutationTarget {
        account_id,
        calendar_id,
        provider_calendar_id,
        time_zone,
    };
    attach_schedule_and_queue_create(
        transaction,
        user_id,
        schedule_entry_id,
        schedule_version,
        &target,
        payload,
    )
    .await?;
    Ok(true)
}

pub(crate) async fn queue_linked_schedule_mutation(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    schedule_entry_id: Uuid,
    schedule_version: i64,
    operation: ScheduleCalendarMutationOperation,
    payload: &ScheduleCalendarMutationPayload,
) -> Result<(), StorageError> {
    let link = sqlx::query_as::<_, (String, Option<String>)>(
        "\
        SELECT link.provider_event_id, link.provider_etag
        FROM schedule_calendar_links AS link
        INNER JOIN calendar_accounts AS account ON account.id = link.account_id
        INNER JOIN calendars AS calendar ON calendar.id = link.calendar_id
        WHERE link.schedule_entry_id = $1
          AND link.user_id = $2
          AND account.user_id = $2
          AND account.status = 'active'
          AND calendar.account_id = account.id
          AND calendar.access_role IN ('owner', 'writer')
        FOR UPDATE OF link",
    )
    .bind(schedule_entry_id)
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)?;
    let Some((provider_event_id, provider_etag)) = link else {
        return Ok(());
    };
    queue_mutation(
        transaction,
        user_id,
        schedule_entry_id,
        schedule_version,
        operation,
        &provider_event_id,
        provider_etag.as_deref(),
        payload,
    )
    .await
}

pub(crate) async fn resolve_unavailable_schedule_calendar_mutations(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: Option<Uuid>,
) -> Result<u64, StorageError> {
    let idempotency_ids = sqlx::query_scalar::<_, Uuid>(
        "\
        UPDATE calendar_mutations AS mutation
        SET status = 'failed', next_attempt_at = NULL, lease_owner = NULL,
            lease_expires_at = NULL,
            last_error_code = 'calendar.destination_unavailable',
            resolved_at = NOW()
        FROM schedule_calendar_links AS link, calendars AS calendar
        WHERE mutation.schedule_entry_id = link.schedule_entry_id
          AND calendar.id = link.calendar_id
          AND ($1::UUID IS NULL OR calendar.account_id = $1)
          AND (
              calendar.sync_enabled = FALSE
              OR calendar.provider_deleted_at IS NOT NULL
              OR calendar.access_role NOT IN ('owner', 'writer')
          )
          AND (
              mutation.status IN ('queued', 'retry_wait', 'claimed')
              OR (
                  mutation.status = 'sending'
                  AND mutation.lease_expires_at <= NOW()
              )
          )
        RETURNING mutation.idempotency_record_id",
    )
    .bind(account_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)?;
    if idempotency_ids.is_empty() {
        return Ok(0);
    }
    sqlx::query(
        "\
        UPDATE idempotency_records
        SET state = 'failed', locked_until = NULL, response_status = 409,
            response_body = '{}'::JSONB
        WHERE id = ANY($1) AND state = 'pending'",
    )
    .bind(&idempotency_ids)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    sqlx::query(
        "\
        UPDATE calendar_accounts AS account
        SET last_error_code = 'calendar.destination_unavailable'
        WHERE account.status = 'active'
          AND ($1::UUID IS NULL OR account.id = $1)
          AND EXISTS (
              SELECT 1 FROM schedule_calendar_links AS link
              WHERE link.account_id = account.id
                AND link.schedule_entry_id IN (
                    SELECT mutation.schedule_entry_id
                    FROM calendar_mutations AS mutation
                    WHERE mutation.idempotency_record_id = ANY($2)
                )
          )",
    )
    .bind(account_id)
    .bind(&idempotency_ids)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    u64::try_from(idempotency_ids.len()).map_err(|_| StorageError::PersistenceUnavailable)
}

#[allow(clippy::too_many_arguments)]
async fn queue_mutation(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    schedule_entry_id: Uuid,
    schedule_version: i64,
    operation: ScheduleCalendarMutationOperation,
    provider_event_id: &str,
    provider_etag: Option<&str>,
    payload: &ScheduleCalendarMutationPayload,
) -> Result<(), StorageError> {
    let mutation_id = Uuid::now_v7();
    let idempotency_id = Uuid::now_v7();
    let key = format!(
        "calendar:schedule:{schedule_entry_id}:{}:{schedule_version}",
        operation.as_str()
    );
    let request_hash = format!(
        "{schedule_entry_id}:{}:{schedule_version}:{provider_event_id}",
        operation.as_str()
    );
    sqlx::query(
        "\
        INSERT INTO idempotency_records (
            id, user_id, idempotency_key, operation, request_hash, state
        ) VALUES ($1, $2, $3, $4, $5, 'pending')",
    )
    .bind(idempotency_id)
    .bind(user_id)
    .bind(key)
    .bind(format!("calendar.schedule.{}", operation.as_str()))
    .bind(request_hash.as_bytes())
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    sqlx::query(
        "\
        INSERT INTO calendar_mutations (
            id, user_id, event_id, schedule_entry_id, operation, status,
            idempotency_record_id, desired_payload, expected_event_version,
            expected_provider_etag, provider_event_id
        ) VALUES ($1, $2, NULL, $3, $4, 'queued', $5, $6, $7, $8, $9)",
    )
    .bind(mutation_id)
    .bind(user_id)
    .bind(schedule_entry_id)
    .bind(operation.as_str())
    .bind(idempotency_id)
    .bind(serde_json::to_value(payload).map_err(|_| StorageError::InvalidConfiguration)?)
    .bind(schedule_version)
    .bind(provider_etag)
    .bind(provider_event_id)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    Ok(())
}

impl Database {
    /// Terminally resolves mutations whose provider calendar can no longer be
    /// written, preventing disabled or deleted destinations from accumulating
    /// permanent retry rows.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when cleanup cannot commit.
    pub async fn resolve_unavailable_schedule_calendar_mutations(
        &self,
    ) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let resolved =
            resolve_unavailable_schedule_calendar_mutations(&mut transaction, None).await?;
        transaction.commit().await.map_err(classify)?;
        Ok(resolved)
    }

    /// Claims due schedule mutations in stable per-schedule order. Lease
    /// recovery remains bounded; an expired final attempt is terminal before
    /// any replacement worker can receive it.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration error for an unsafe worker or limit,
    /// and a classified persistence error when a claim cannot be committed.
    pub async fn claim_schedule_calendar_mutations(
        &self,
        worker_id: &str,
        limit: i64,
    ) -> Result<Vec<ClaimedScheduleCalendarMutation>, StorageError> {
        if worker_id.is_empty()
            || worker_id.len() > 200
            || worker_id.chars().any(char::is_control)
            || !(1..=25).contains(&limit)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        terminalize_exhausted_schedule_calendar_mutations(&mut transaction).await?;
        let rows = sqlx::query_as::<_, ClaimedScheduleCalendarMutationRow>(
            "\
            WITH candidates AS (
                SELECT mutation.id
                FROM calendar_mutations AS mutation
                INNER JOIN schedule_calendar_links AS link
                    ON link.schedule_entry_id = mutation.schedule_entry_id
                INNER JOIN schedule_entries AS schedule
                    ON schedule.id = mutation.schedule_entry_id
                   AND schedule.user_id = mutation.user_id
                INNER JOIN calendar_accounts AS account ON account.id = link.account_id
                INNER JOIN calendars AS calendar ON calendar.id = link.calendar_id
                WHERE mutation.schedule_entry_id IS NOT NULL
                  AND mutation.status IN ('queued', 'retry_wait', 'claimed', 'sending')
                  AND mutation.attempt_count < $4
                  AND (mutation.next_attempt_at IS NULL OR mutation.next_attempt_at <= NOW())
                  AND (mutation.lease_expires_at IS NULL OR mutation.lease_expires_at <= NOW())
                  AND account.user_id = mutation.user_id
                  AND account.status = 'active'
                  AND calendar.account_id = account.id
                  AND calendar.sync_enabled = TRUE
                  AND calendar.provider_deleted_at IS NULL
                  AND calendar.access_role IN ('owner', 'writer')
                  AND NOT EXISTS (
                      SELECT 1
                      FROM calendar_mutations AS earlier
                      WHERE earlier.schedule_entry_id = mutation.schedule_entry_id
                        AND earlier.status IN ('queued', 'claimed', 'sending', 'retry_wait')
                        AND (earlier.created_at, earlier.id) < (mutation.created_at, mutation.id)
                  )
                ORDER BY mutation.created_at, mutation.id
                LIMIT $2
                FOR UPDATE OF account SKIP LOCKED
            ), claimed AS (
                UPDATE calendar_mutations AS mutation
                SET status = 'claimed', attempt_count = mutation.attempt_count + 1,
                    lease_owner = $1,
                    lease_expires_at = NOW() + ($3 * INTERVAL '1 second'),
                    next_attempt_at = NULL,
                    last_error_code = NULL
                FROM candidates
                WHERE mutation.id = candidates.id
                RETURNING mutation.*
            )
            SELECT claimed.id, claimed.user_id, link.account_id, link.calendar_id,
                calendar.provider_calendar_id, claimed.provider_event_id,
                link.provider_etag,
                claimed.schedule_entry_id, claimed.operation, claimed.desired_payload,
                claimed.expected_event_version, claimed.attempt_count, claimed.lease_owner
            FROM claimed
            INNER JOIN schedule_calendar_links AS link
                ON link.schedule_entry_id = claimed.schedule_entry_id
            INNER JOIN calendars AS calendar ON calendar.id = link.calendar_id
            ORDER BY claimed.created_at, claimed.id",
        )
        .bind(worker_id)
        .bind(limit)
        .bind(CLAIM_LEASE_SECONDS)
        .bind(MAX_ATTEMPTS)
        .fetch_all(&mut *transaction)
        .await
        .map_err(classify)?;
        let claimed = rows
            .into_iter()
            .map(ClaimedScheduleCalendarMutation::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(classify)?;
        Ok(claimed)
    }

    /// Completes one claimed mutation and advances its provider link and
    /// idempotency record atomically.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration error for unsafe completion metadata,
    /// or a persistence error when the terminal transaction cannot commit.
    pub async fn complete_schedule_calendar_mutation(
        &self,
        mutation_id: Uuid,
        attempt_count: i32,
        worker_id: &str,
        provider_etag: Option<&str>,
    ) -> Result<bool, StorageError> {
        if mutation_id.get_version_num() != 7
            || attempt_count <= 0
            || !valid_worker_id(worker_id)
            || provider_etag.is_some_and(|value| value.is_empty() || value.len() > 2_048)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let completed = sqlx::query_as::<_, (Uuid, Uuid, String)>(
            "\
            UPDATE calendar_mutations
            SET status = 'completed', resolved_at = NOW(), lease_owner = NULL,
                lease_expires_at = NULL, next_attempt_at = NULL, last_error_code = NULL
            WHERE id = $1
              AND attempt_count = $2
              AND lease_owner = $3
              AND status IN ('claimed', 'sending')
            RETURNING idempotency_record_id, schedule_entry_id, operation",
        )
        .bind(mutation_id)
        .bind(attempt_count)
        .bind(worker_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?;
        let Some((idempotency_id, schedule_entry_id, operation)) = completed else {
            transaction.commit().await.map_err(classify)?;
            return Ok(false);
        };
        if operation == "delete" {
            sqlx::query(
                "UPDATE schedule_calendar_links SET provider_etag = NULL WHERE schedule_entry_id = $1",
            )
            .bind(schedule_entry_id)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        } else {
            sqlx::query(
                "UPDATE schedule_calendar_links SET provider_etag = $2 WHERE schedule_entry_id = $1",
            )
            .bind(schedule_entry_id)
            .bind(provider_etag)
            .execute(&mut *transaction)
            .await
            .map_err(classify)?;
        }
        sqlx::query(
            "\
            UPDATE calendar_accounts AS account
            SET last_error_code = NULL
            FROM schedule_calendar_links AS link
            WHERE link.schedule_entry_id = $1
              AND account.id = link.account_id
              AND account.status = 'active'
              AND account.last_error_code IN (
                  'calendar.provider_unavailable',
                  'calendar.event_conflict',
                  'calendar.event_not_found',
                  'calendar.event_rejected',
                  'calendar.destination_unavailable',
                  'calendar.authorization_failed'
              )",
        )
        .bind(schedule_entry_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        sqlx::query(
            "\
            UPDATE idempotency_records
            SET state = 'completed', locked_until = NULL, response_status = 200,
                response_body = '{}'::JSONB
            WHERE id = $1 AND state = 'pending'",
        )
        .bind(idempotency_id)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }

    /// Releases a claimed mutation with bounded exponential retry or records a
    /// terminal sanitized failure. Event text and provider responses are never
    /// persisted as failure data.
    ///
    /// # Errors
    ///
    /// Returns an invalid-configuration error for unsafe failure metadata, or
    /// a persistence error when retry and connection state cannot commit.
    pub async fn fail_schedule_calendar_mutation(
        &self,
        mutation_id: Uuid,
        attempt_count: i32,
        worker_id: &str,
        error_code: &str,
        retryable: bool,
    ) -> Result<bool, StorageError> {
        if mutation_id.get_version_num() != 7
            || attempt_count <= 0
            || !valid_worker_id(worker_id)
            || error_code.is_empty()
            || error_code.len() > 120
            || error_code.chars().any(char::is_control)
        {
            return Err(StorageError::InvalidConfiguration);
        }
        let delay = retry_delay(attempt_count);
        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let destination_available =
            schedule_mutation_destination_available(&mut transaction, mutation_id).await?;
        let should_retry = retryable && attempt_count < MAX_ATTEMPTS && destination_available;
        let error_code = if destination_available {
            error_code
        } else {
            "calendar.destination_unavailable"
        };
        let failed = transition_schedule_mutation_failure(
            &mut transaction,
            mutation_id,
            attempt_count,
            worker_id,
            error_code,
            should_retry,
            delay,
        )
        .await?;
        let Some((idempotency_id, schedule_entry_id)) = failed else {
            transaction.commit().await.map_err(classify)?;
            return Ok(false);
        };
        record_schedule_mutation_failure(
            &mut transaction,
            idempotency_id,
            schedule_entry_id,
            error_code,
            should_retry,
        )
        .await?;
        transaction.commit().await.map_err(classify)?;
        Ok(true)
    }
}

async fn terminalize_exhausted_schedule_calendar_mutations(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), StorageError> {
    let exhausted = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "\
        UPDATE calendar_mutations
        SET status = 'failed', next_attempt_at = NULL, lease_owner = NULL,
            lease_expires_at = NULL,
            last_error_code = CASE WHEN EXISTS (
                SELECT 1
                FROM schedule_calendar_links AS link
                INNER JOIN calendar_accounts AS account ON account.id = link.account_id
                INNER JOIN calendars AS calendar ON calendar.id = link.calendar_id
                WHERE link.schedule_entry_id = calendar_mutations.schedule_entry_id
                  AND account.status = 'active'
                  AND calendar.sync_enabled = TRUE
                  AND calendar.provider_deleted_at IS NULL
                  AND calendar.access_role IN ('owner', 'writer')
            ) THEN $2 ELSE $3 END,
            resolved_at = NOW()
        WHERE schedule_entry_id IS NOT NULL
          AND status IN ('claimed', 'sending')
          AND attempt_count >= $1
          AND lease_expires_at <= NOW()
        RETURNING idempotency_record_id, schedule_entry_id, last_error_code",
    )
    .bind(MAX_ATTEMPTS)
    .bind(EXHAUSTED_MUTATION_ERROR_CODE)
    .bind(UNAVAILABLE_DESTINATION_ERROR_CODE)
    .fetch_all(&mut **transaction)
    .await
    .map_err(classify)?;
    for (idempotency_id, schedule_entry_id, error_code) in exhausted {
        record_schedule_mutation_failure(
            transaction,
            idempotency_id,
            schedule_entry_id,
            &error_code,
            false,
        )
        .await?;
    }
    Ok(())
}

async fn schedule_mutation_destination_available(
    transaction: &mut Transaction<'_, Postgres>,
    mutation_id: Uuid,
) -> Result<bool, StorageError> {
    sqlx::query_scalar(
        "\
        SELECT EXISTS (
            SELECT 1
            FROM calendar_mutations AS mutation
            INNER JOIN schedule_calendar_links AS link
                ON link.schedule_entry_id = mutation.schedule_entry_id
            INNER JOIN calendar_accounts AS account ON account.id = link.account_id
            INNER JOIN calendars AS calendar ON calendar.id = link.calendar_id
            WHERE mutation.id = $1
              AND account.status = 'active'
              AND calendar.sync_enabled = TRUE
              AND calendar.provider_deleted_at IS NULL
              AND calendar.access_role IN ('owner', 'writer')
        )",
    )
    .bind(mutation_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(classify)
}

#[allow(clippy::too_many_arguments)]
async fn transition_schedule_mutation_failure(
    transaction: &mut Transaction<'_, Postgres>,
    mutation_id: Uuid,
    attempt_count: i32,
    worker_id: &str,
    error_code: &str,
    should_retry: bool,
    delay: Duration,
) -> Result<Option<(Uuid, Uuid)>, StorageError> {
    sqlx::query_as(
        "\
        UPDATE calendar_mutations
        SET status = CASE WHEN $3 THEN 'retry_wait' ELSE
                CASE WHEN $4 IN ('calendar.event_conflict', 'calendar.event_not_found')
                    THEN 'conflict' ELSE 'failed' END
            END,
            next_attempt_at = CASE WHEN $3 THEN NOW() + ($5 * INTERVAL '1 second') ELSE NULL END,
            lease_owner = NULL, lease_expires_at = NULL, last_error_code = $4,
            resolved_at = CASE WHEN $3 THEN NULL ELSE NOW() END
        WHERE id = $1
          AND attempt_count = $2
          AND lease_owner = $6
          AND status IN ('claimed', 'sending')
        RETURNING idempotency_record_id, schedule_entry_id",
    )
    .bind(mutation_id)
    .bind(attempt_count)
    .bind(should_retry)
    .bind(error_code)
    .bind(delay.whole_seconds())
    .bind(worker_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(classify)
}

async fn record_schedule_mutation_failure(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_id: Uuid,
    schedule_entry_id: Uuid,
    error_code: &str,
    should_retry: bool,
) -> Result<(), StorageError> {
    sqlx::query(
        "\
        UPDATE calendar_accounts AS account
        SET status = CASE
                WHEN $2 = 'calendar.authorization_failed' THEN 'error'
                ELSE account.status
            END,
            last_error_code = $2
        FROM schedule_calendar_links AS link
        WHERE link.schedule_entry_id = $1
          AND account.id = link.account_id
          AND account.status = 'active'",
    )
    .bind(schedule_entry_id)
    .bind(error_code)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    if should_retry {
        return Ok(());
    }
    sqlx::query(
        "\
        UPDATE idempotency_records
        SET state = 'failed', locked_until = NULL,
            response_status = CASE
                WHEN $2 IN (
                    'calendar.event_conflict',
                    'calendar.event_not_found',
                    'calendar.destination_unavailable'
                ) THEN 409
                WHEN $2 = 'calendar.event_rejected' THEN 400
                ELSE 503
            END,
            response_body = '{}'::JSONB
        WHERE id = $1 AND state = 'pending'",
    )
    .bind(idempotency_id)
    .bind(error_code)
    .execute(&mut **transaction)
    .await
    .map_err(classify)?;
    Ok(())
}

#[must_use]
pub fn provider_event_id_for_schedule(schedule_entry_id: Uuid) -> String {
    format!("jos{}", schedule_entry_id.simple())
}

fn valid_provider_event_id(value: &str) -> bool {
    (5..=1_024).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'v'))
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 200 && !value.chars().any(char::is_control)
}

fn retry_delay(attempt_count: i32) -> Duration {
    let exponent = u32::try_from(attempt_count.saturating_sub(1).min(8)).unwrap_or(0);
    Duration::seconds(i64::from(2_u32.pow(exponent).min(300)))
}

fn classify(error: sqlx::Error) -> StorageError {
    match error {
        sqlx::Error::Database(database)
            if database.constraint().is_some_and(|constraint| {
                constraint == "calendar_mutations_schedule_version_idx"
                    || constraint == "idempotency_records_user_id_idempotency_key_operation_key"
            }) =>
        {
            StorageError::InvalidConfiguration
        }
        _ => StorageError::PersistenceUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_id_is_stable_and_google_safe() {
        let schedule_id = Uuid::now_v7();
        let first = provider_event_id_for_schedule(schedule_id);
        assert_eq!(first, provider_event_id_for_schedule(schedule_id));
        assert_eq!(first.len(), 35);
        assert!(valid_provider_event_id(&first));
    }

    #[test]
    fn retry_delay_is_bounded() {
        assert_eq!(retry_delay(1), Duration::seconds(1));
        assert_eq!(retry_delay(20), Duration::seconds(256));
    }
}

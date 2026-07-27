//! Private Android device-signal persistence.
//!
//! Device signal commands are always scoped by the authenticated user and
//! device. Raw call data must never be written to application logs.

use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{Database, StorageError};

const MAX_CALLS_PER_SYNC: usize = 200;
const MAX_SOURCE_EVENT_ID_BYTES: usize = 120;
const MAX_CALLER_NAME_BYTES: usize = 120;
const MAX_PHONE_NUMBER_BYTES: usize = 64;
const MAX_VERSION_BYTES: usize = 120;
const RETENTION_DAYS: i64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallLogPermission {
    NotDetermined,
    Granted,
    Denied,
    Unavailable,
}

impl CallLogPermission {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotDetermined => "not_determined",
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Unavailable => "unavailable",
        }
    }

    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "not_determined" => Ok(Self::NotDetermined),
            "granted" => Ok(Self::Granted),
            "denied" => Ok(Self::Denied),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(StorageError::PersistenceUnavailable),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMissedCallSignal {
    pub id: Uuid,
    pub source_event_id: String,
    pub occurred_at: OffsetDateTime,
    pub caller_name: Option<String>,
    pub phone_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MissedCallSignal {
    pub id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub occurred_at: OffsetDateTime,
    pub caller_name: Option<String>,
    pub phone_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSignalState {
    pub device_id: Uuid,
    pub device_name: String,
    pub permission: CallLogPermission,
    pub platform_version: Option<String>,
    pub app_version: Option<String>,
    pub last_synced_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSignalSync {
    pub inserted_count: usize,
    pub state: DeviceSignalState,
}

#[derive(Debug, Clone, Copy)]
pub struct MissedCallSyncRequest<'a> {
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub permission: CallLogPermission,
    pub platform_version: Option<&'a str>,
    pub app_version: Option<&'a str>,
    pub synced_at: OffsetDateTime,
    pub calls: &'a [NewMissedCallSignal],
}

impl Database {
    /// Stores a bounded, idempotent missed-call snapshot for the authenticated
    /// Android device and updates its permission/sync health.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for malformed values or when the session
    /// does not belong to an active Android device.
    pub async fn sync_missed_calls(
        &self,
        request: MissedCallSyncRequest<'_>,
    ) -> Result<DeviceSignalSync, StorageError> {
        if !valid_uuid(request.user_id)
            || !valid_uuid(request.device_id)
            || request.calls.len() > MAX_CALLS_PER_SYNC
            || !valid_optional(request.platform_version, MAX_VERSION_BYTES)
            || !valid_optional(request.app_version, MAX_VERSION_BYTES)
            || request.permission != CallLogPermission::Granted && !request.calls.is_empty()
            || request
                .calls
                .iter()
                .any(|call| !valid_new_call(call, request.synced_at))
        {
            return Err(StorageError::InvalidConfiguration);
        }

        let mut transaction = self.pool().begin().await.map_err(classify)?;
        let device_name = sqlx::query_scalar::<_, String>(
            "SELECT name
             FROM devices
             WHERE id = $1 AND user_id = $2
               AND platform = 'android' AND status = 'active'",
        )
        .bind(request.device_id)
        .bind(request.user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(classify)?
        .ok_or(StorageError::InvalidConfiguration)?;

        let mut inserted_count = 0_usize;
        for call in request.calls {
            let inserted = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO device_missed_calls (
                    id, user_id, device_id, source_event_id, occurred_at,
                    caller_name, phone_number
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (device_id, source_event_id) DO NOTHING
                 RETURNING id",
            )
            .bind(call.id)
            .bind(request.user_id)
            .bind(request.device_id)
            .bind(call.source_event_id.trim())
            .bind(call.occurred_at)
            .bind(trimmed_optional(call.caller_name.as_deref()))
            .bind(trimmed_optional(call.phone_number.as_deref()))
            .fetch_optional(&mut *transaction)
            .await
            .map_err(classify)?;
            inserted_count += usize::from(inserted.is_some());
        }

        sqlx::query(
            "INSERT INTO device_signal_states (
                device_id, user_id, call_log_permission, platform_version,
                app_version, last_synced_at
             ) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (device_id) DO UPDATE SET
                call_log_permission = EXCLUDED.call_log_permission,
                platform_version = EXCLUDED.platform_version,
                app_version = EXCLUDED.app_version,
                last_synced_at = EXCLUDED.last_synced_at",
        )
        .bind(request.device_id)
        .bind(request.user_id)
        .bind(request.permission.as_str())
        .bind(trimmed_optional(request.platform_version))
        .bind(trimmed_optional(request.app_version))
        .bind(request.synced_at)
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;

        sqlx::query(
            "DELETE FROM device_missed_calls
             WHERE user_id = $1 AND occurred_at < $2",
        )
        .bind(request.user_id)
        .bind(request.synced_at - Duration::days(RETENTION_DAYS))
        .execute(&mut *transaction)
        .await
        .map_err(classify)?;

        transaction.commit().await.map_err(classify)?;
        Ok(DeviceSignalSync {
            inserted_count,
            state: DeviceSignalState {
                device_id: request.device_id,
                device_name,
                permission: request.permission,
                platform_version: trimmed_optional(request.platform_version).map(str::to_owned),
                app_version: trimmed_optional(request.app_version).map(str::to_owned),
                last_synced_at: Some(request.synced_at),
            },
        })
    }

    /// Returns signal health for all active Android devices owned by a user.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    pub async fn device_signal_states_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<DeviceSignalState>, StorageError> {
        if !valid_uuid(user_id) {
            return Err(StorageError::InvalidConfiguration);
        }
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<OffsetDateTime>,
            ),
        >(
            "SELECT device.id, device.name, state.call_log_permission,
                    state.platform_version, state.app_version, state.last_synced_at
             FROM devices AS device
             LEFT JOIN device_signal_states AS state ON state.device_id = device.id
             WHERE device.user_id = $1
               AND device.platform = 'android' AND device.status = 'active'
             ORDER BY state.last_synced_at DESC NULLS LAST, device.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter()
            .map(
                |(
                    device_id,
                    device_name,
                    permission,
                    platform_version,
                    app_version,
                    last_synced_at,
                )| {
                    Ok(DeviceSignalState {
                        device_id,
                        device_name,
                        permission: permission.as_deref().map_or(
                            Ok(CallLogPermission::NotDetermined),
                            CallLogPermission::parse,
                        )?,
                        platform_version,
                        app_version,
                        last_synced_at,
                    })
                },
            )
            .collect()
    }

    /// Returns recent missed calls for assistant context or an authenticated
    /// user-facing read. The result is newest-first and bounded.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or persistence error.
    pub async fn recent_missed_calls_for_user(
        &self,
        user_id: Uuid,
        since: OffsetDateTime,
        limit: i64,
    ) -> Result<Vec<MissedCallSignal>, StorageError> {
        if !valid_uuid(user_id) || !(1..=200).contains(&limit) {
            return Err(StorageError::InvalidConfiguration);
        }
        sqlx::query_as::<_, MissedCallSignal>(
            "SELECT call.id, call.device_id, device.name AS device_name,
                    call.occurred_at, call.caller_name, call.phone_number
             FROM device_missed_calls AS call
             INNER JOIN devices AS device
                 ON device.id = call.device_id AND device.user_id = call.user_id
             WHERE call.user_id = $1 AND call.occurred_at >= $2
             ORDER BY call.occurred_at DESC, call.id DESC
             LIMIT $3",
        )
        .bind(user_id)
        .bind(since)
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(classify)
    }
}

fn valid_new_call(call: &NewMissedCallSignal, synced_at: OffsetDateTime) -> bool {
    valid_uuid(call.id)
        && valid_required(&call.source_event_id, MAX_SOURCE_EVENT_ID_BYTES)
        && valid_optional(call.caller_name.as_deref(), MAX_CALLER_NAME_BYTES)
        && valid_optional(call.phone_number.as_deref(), MAX_PHONE_NUMBER_BYTES)
        && call.occurred_at <= synced_at + Duration::minutes(5)
        && call.occurred_at >= synced_at - Duration::days(RETENTION_DAYS)
}

fn valid_required(value: &str, maximum_bytes: usize) -> bool {
    let value = value.trim();
    !value.is_empty() && value.len() <= maximum_bytes
}

fn valid_optional(value: Option<&str>, maximum_bytes: usize) -> bool {
    value.is_none_or(|value| valid_required(value, maximum_bytes))
}

fn trimmed_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn valid_uuid(value: Uuid) -> bool {
    value.get_version_num() == 7
}

fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_values_are_stable_for_api_and_database_contracts() {
        for (permission, value) in [
            (CallLogPermission::NotDetermined, "not_determined"),
            (CallLogPermission::Granted, "granted"),
            (CallLogPermission::Denied, "denied"),
            (CallLogPermission::Unavailable, "unavailable"),
        ] {
            assert_eq!(permission.as_str(), value);
            assert_eq!(
                CallLogPermission::parse(value).expect("known permission"),
                permission
            );
        }
    }

    #[test]
    fn rejects_calls_outside_the_private_retention_window() {
        let synced_at = OffsetDateTime::now_utc();
        let call = NewMissedCallSignal {
            id: Uuid::now_v7(),
            source_event_id: "42".to_owned(),
            occurred_at: synced_at - Duration::days(91),
            caller_name: None,
            phone_number: Some("010-0000-0000".to_owned()),
        };

        assert!(!valid_new_call(&call, synced_at));
    }
}

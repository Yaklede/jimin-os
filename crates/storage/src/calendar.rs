//! Google Calendar connection metadata and durable sync records.
//!
//! Provider credentials are intentionally represented only as encrypted SQL
//! columns in the migration. This module exposes the safe, client-visible
//! connection summary without returning refresh tokens, sync tokens, or
//! provider event payloads.

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

/// Safe state of the single Google Calendar account linked to a personal user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarAccountStatus {
    Connecting,
    Active,
    ReauthRequired,
    Revoking,
    Revoked,
    Error,
}

impl CalendarAccountStatus {
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

/// Calendar account metadata that may be shown to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarAccount {
    pub id: Uuid,
    pub email: String,
    pub status: CalendarAccountStatus,
    pub granted_scopes: Vec<String>,
    pub last_successful_sync_at: Option<OffsetDateTime>,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
struct CalendarAccountRow {
    id: Uuid,
    email: String,
    status: String,
    granted_scopes: Vec<String>,
    last_successful_sync_at: Option<OffsetDateTime>,
    version: i64,
}

impl TryFrom<CalendarAccountRow> for CalendarAccount {
    type Error = StorageError;

    fn try_from(row: CalendarAccountRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            email: row.email,
            status: CalendarAccountStatus::parse(&row.status)?,
            granted_scopes: row.granted_scopes,
            last_successful_sync_at: row.last_successful_sync_at,
            version: row.version,
        })
    }
}

impl Database {
    /// Returns the owner's Google Calendar connection without credential data.
    ///
    /// # Errors
    ///
    /// Returns a classified persistence error when the database cannot be
    /// queried or an unknown status is found in a persisted row.
    pub async fn calendar_account_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<CalendarAccount>, StorageError> {
        let row = sqlx::query_as::<_, CalendarAccountRow>(
            "\
            SELECT id, email, status, granted_scopes, last_successful_sync_at, version
            FROM calendar_accounts
            WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        row.map(CalendarAccount::try_from).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::CalendarAccountStatus;

    #[test]
    fn calendar_account_status_rejects_unknown_values() {
        assert!(CalendarAccountStatus::parse("active").is_ok());
        assert!(CalendarAccountStatus::parse("unexpected").is_err());
    }
}

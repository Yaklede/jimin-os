//! Durable cross-device change feed.
//!
//! The feed is an invalidation journal, not a second source of truth. Clients
//! use it to decide which server projections must be fetched again.

use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Database, StorageError};

const MAX_SYNC_PAGE_SIZE: i64 = 200;

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SyncChange {
    pub sequence: i64,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub operation: String,
    pub entity_version: i64,
    pub changed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncChangePage {
    pub items: Vec<SyncChange>,
    pub next_cursor: i64,
    pub current_cursor: i64,
    pub has_more: bool,
}

impl Database {
    /// Returns one ordered page of invalidations after an acknowledged cursor.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidConfiguration`] for an invalid owner,
    /// cursor, or page size, and [`StorageError::PersistenceUnavailable`] when
    /// `PostgreSQL` cannot serve a consistent page.
    pub async fn sync_changes_for_user(
        &self,
        user_id: Uuid,
        after: i64,
        limit: i64,
    ) -> Result<SyncChangePage, StorageError> {
        if user_id.get_version_num() != 7 || after < 0 || !(1..=MAX_SYNC_PAGE_SIZE).contains(&limit)
        {
            return Err(StorageError::InvalidConfiguration);
        }

        let fetch_limit = limit
            .checked_add(1)
            .ok_or(StorageError::InvalidConfiguration)?;
        let mut items = sqlx::query_as::<_, SyncChange>(
            "SELECT sequence, entity_type, entity_id, operation, entity_version, changed_at
             FROM sync_changes
             WHERE user_id = $1 AND sequence > $2
             ORDER BY sequence ASC
             LIMIT $3",
        )
        .bind(user_id)
        .bind(after)
        .bind(fetch_limit)
        .fetch_all(self.pool())
        .await
        .map_err(|_| StorageError::PersistenceUnavailable)?;

        let page_overflow = i64::try_from(items.len()).is_ok_and(|count| count > limit);
        if page_overflow {
            items.pop();
        }
        let next_cursor = items.last().map_or(after, |change| change.sequence);
        let current_cursor = self.current_sync_cursor_for_user(user_id).await?;

        Ok(SyncChangePage {
            items,
            next_cursor,
            current_cursor,
            has_more: page_overflow || current_cursor > next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;
    use std::time::Duration;

    fn database() -> Database {
        Database::connect_lazy(
            &SecretString::from("postgres://example.invalid/jimin".to_owned()),
            1,
            Duration::from_secs(1),
        )
        .expect("test database configuration should be valid")
    }

    #[tokio::test]
    async fn rejects_invalid_change_feed_boundaries_before_querying() {
        let database = database();
        let user_id = Uuid::now_v7();

        assert!(matches!(
            database.sync_changes_for_user(user_id, -1, 10).await,
            Err(StorageError::InvalidConfiguration)
        ));
        assert!(matches!(
            database.sync_changes_for_user(user_id, 0, 0).await,
            Err(StorageError::InvalidConfiguration)
        ));
        assert!(matches!(
            database.sync_changes_for_user(user_id, 0, 201).await,
            Err(StorageError::InvalidConfiguration)
        ));
    }
}

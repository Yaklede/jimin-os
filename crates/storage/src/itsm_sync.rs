//! Explicit ticket links and a durable, version-fenced outbound sync lane.
//! No task notes, upstream descriptions or credentials enter this queue.
use crate::{Database, StorageError, inflow_analysis::InflowReferenceDocument};
use sqlx::{Postgres, Transaction, types::Json};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub enum ItsmLinkChoice<'a> {
    #[default]
    Automatic,
    Disabled,
    Selected(&'a str),
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ItsmSyncDelivery {
    pub id: Uuid,
    pub issue_url: String,
    pub issue_id: i64,
    pub version: i64,
    pub sync_deadline: bool,
    pub due_at: Option<OffsetDateTime>,
    pub should_complete: bool,
    pub linked_count: i64,
    pub attempt_count: i32,
}

/// Only verified ITSM evidence is eligible, never an arbitrary URL in task notes.
#[must_use]
pub fn verified_issue_references(documents: &[InflowReferenceDocument]) -> Vec<(String, i64)> {
    let mut output = Vec::new();
    for document in documents {
        if document.provider != "itsm" || document.error_code.is_some() {
            continue;
        }
        let Some((url, id)) = canonical_issue_reference(&document.url) else {
            continue;
        };
        if document.external_id != id.to_string() {
            continue;
        }
        if !output.iter().any(|(known, _)| known == &url) {
            output.push((url, id));
        }
    }
    output
}

fn canonical_issue_reference(value: &str) -> Option<(String, i64)> {
    if value.len() > 2048 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let remainder = value.strip_prefix("https://")?;
    let authority = remainder.split('/').next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let clean = value.split(['?', '#']).next()?;
    let (prefix, id) = clean.rsplit_once("/issues/")?;
    if id.is_empty() || !id.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let id: i64 = id.parse().ok()?;
    (id > 0).then(|| (format!("{prefix}/issues/{id}"), id))
}

pub(crate) async fn bind_promoted_itsm_task(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    task: Uuid,
    documents: &[InflowReferenceDocument],
    choice: ItsmLinkChoice<'_>,
) -> Result<(), StorageError> {
    let references = verified_issue_references(documents);
    let selected = match choice {
        ItsmLinkChoice::Automatic if references.len() <= 1 => references.first(),
        // Older clients cannot choose among multiple tickets. Preserve their
        // task/Chat flow, but never guess a ticket for an external write.
        ItsmLinkChoice::Automatic | ItsmLinkChoice::Disabled => None,
        ItsmLinkChoice::Selected(url) => {
            let (canonical, _) =
                canonical_issue_reference(url).ok_or(StorageError::InvalidConfiguration)?;
            Some(
                references
                    .iter()
                    .find(|(candidate, _)| candidate == &canonical)
                    .ok_or(StorageError::InvalidConfiguration)?,
            )
        }
    };
    // Absence of evidence is not an opt-out: a later verified source can link it.
    if selected.is_none() && !matches!(choice, ItsmLinkChoice::Disabled) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO task_itsm_links(task_id,user_id,enabled,issue_url,issue_id)
        SELECT id,user_id,$3,$4,$5 FROM tasks WHERE id=$1 AND user_id=$2
        ON CONFLICT(task_id) DO NOTHING",
    )
    .bind(task)
    .bind(user)
    .bind(selected.is_some())
    .bind(selected.map(|(url, _)| url))
    .bind(selected.map(|(_, id)| id))
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

pub(crate) async fn queue_task_itsm_sync(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    task: Uuid,
) -> Result<(), StorageError> {
    let has_link: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM task_itsm_links WHERE task_id=$1 AND user_id=$2)",
    )
    .bind(task)
    .bind(user)
    .fetch_one(&mut **tx)
    .await
    .map_err(classify)?;
    if !has_link {
        let documents: Vec<Json<Vec<InflowReferenceDocument>>> = sqlx::query_scalar(
            "SELECT reference_documents FROM project_inflow_analyses WHERE linked_task_id=$1 AND user_id=$2 ORDER BY updated_at DESC LIMIT 10")
            .bind(task).bind(user).fetch_all(&mut **tx).await.map_err(classify)?;
        let references = documents
            .into_iter()
            .flat_map(|documents| documents.0)
            .collect::<Vec<_>>();
        // Historical ambiguous links must not block OS completion or close arbitrary issues.
        if verified_issue_references(&references).len() == 1 {
            bind_promoted_itsm_task(tx, user, task, &references, ItsmLinkChoice::Automatic).await?;
        }
    }
    let link: Option<(String,i64)> = sqlx::query_as(
        "SELECT issue_url,issue_id FROM task_itsm_links WHERE task_id=$1 AND user_id=$2 AND enabled")
        .bind(task).bind(user).fetch_optional(&mut **tx).await.map_err(classify)?;
    let Some((url, id)) = link else {
        return Ok(());
    };
    let due_at: Option<OffsetDateTime> = sqlx::query_scalar(
        "SELECT MAX(t.due_at) FROM task_itsm_links l JOIN tasks t ON t.id=l.task_id AND t.user_id=l.user_id
         WHERE l.enabled AND l.issue_url=$1 AND t.status<>'cancelled'")
        .bind(&url).fetch_one(&mut **tx).await.map_err(classify)?;
    sqlx::query("INSERT INTO itsm_issue_sync_jobs(id,issue_url,issue_id,sync_deadline,last_due_at)
        VALUES($1,$2,$3,$4,$5) ON CONFLICT(issue_url) DO UPDATE SET
        version=itsm_issue_sync_jobs.version+1,
        sync_deadline=itsm_issue_sync_jobs.sync_deadline OR EXCLUDED.sync_deadline
            OR itsm_issue_sync_jobs.last_due_at IS DISTINCT FROM EXCLUDED.last_due_at,
        last_due_at=EXCLUDED.last_due_at,
        status=CASE WHEN itsm_issue_sync_jobs.status='sending' THEN 'sending' ELSE 'queued' END,
        attempt_count=CASE WHEN itsm_issue_sync_jobs.status='sending' THEN itsm_issue_sync_jobs.attempt_count ELSE 0 END,
        next_attempt_at=NULL,last_error_code=NULL,updated_at=NOW()")
        .bind(Uuid::now_v7()).bind(url).bind(id).bind(due_at.is_some()).bind(due_at)
        .execute(&mut **tx).await.map_err(classify)?;
    Ok(())
}

impl Database {
    /// Leases at most one ticket lane; expired workers cannot acknowledge a newer lease.
    /// # Errors
    /// Returns validation or persistence failures.
    pub async fn claim_itsm_sync(
        &self,
        worker: &str,
    ) -> Result<Option<ItsmSyncDelivery>, StorageError> {
        if worker.is_empty() || worker.len() > 120 || worker.chars().any(char::is_control) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let id: Option<Uuid> = sqlx::query_scalar(
            "UPDATE itsm_issue_sync_jobs SET status='sending',lease_owner=$1,leased_version=version,
             lease_until=NOW()+INTERVAL '120 seconds',attempt_count=attempt_count+1,updated_at=NOW()
             WHERE id=(SELECT id FROM itsm_issue_sync_jobs WHERE status='queued'
               OR (status='retry_wait' AND next_attempt_at<=NOW())
               OR (status='sending' AND lease_until<NOW())
               ORDER BY updated_at,id FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING id")
            .bind(worker).fetch_optional(&mut *tx).await.map_err(classify)?;
        let Some(id) = id else {
            tx.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let delivery = sqlx::query_as::<_,ItsmSyncDelivery>(
            "SELECT j.id,j.issue_url,j.issue_id,j.leased_version AS version,j.sync_deadline,j.attempt_count,
                MAX(t.due_at) FILTER(WHERE t.status<>'cancelled') AS due_at,
                COUNT(t.id) FILTER(WHERE t.status<>'cancelled') AS linked_count,
                (COUNT(t.id)>0
                  AND COUNT(t.id) FILTER(WHERE t.status='completed')=COUNT(t.id)
                  AND NOT EXISTS(SELECT 1 FROM tasks child JOIN task_itsm_links parent ON parent.task_id=child.parent_task_id
                    WHERE parent.issue_url=j.issue_url AND parent.enabled AND child.status='open')) AS should_complete
             FROM itsm_issue_sync_jobs j LEFT JOIN task_itsm_links l ON l.issue_url=j.issue_url AND l.enabled
             LEFT JOIN tasks t ON t.id=l.task_id AND t.user_id=l.user_id
             WHERE j.id=$1 GROUP BY j.id")
            .bind(id).fetch_one(&mut *tx).await.map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(Some(delivery))
    }

    /// Rechecks local state immediately before an external write.
    /// # Errors
    /// Returns persistence failures.
    pub async fn itsm_sync_is_current(
        &self,
        job: &ItsmSyncDelivery,
        worker: &str,
    ) -> Result<bool, StorageError> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM itsm_issue_sync_jobs WHERE id=$1 AND version=$2
            AND leased_version=$2 AND lease_owner=$3 AND attempt_count=$4 AND status='sending' AND lease_until>NOW())",
        )
        .bind(job.id)
        .bind(job.version)
        .bind(worker)
        .bind(job.attempt_count)
        .fetch_one(self.pool())
        .await
        .map_err(classify)
    }

    /// Acknowledges only the leased revision; newer mutations remain queued.
    /// # Errors
    /// Returns validation or persistence failures.
    pub async fn finish_itsm_sync(
        &self,
        job: &ItsmSyncDelivery,
        worker: &str,
        error: Option<&str>,
        retryable: bool,
    ) -> Result<bool, StorageError> {
        if error.is_some_and(|code| {
            !code.starts_with("itsm.")
                || code.len() > 100
                || !code
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c == b'.' || c == b'_')
        }) {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let result=sqlx::query("UPDATE itsm_issue_sync_jobs SET
            status=CASE WHEN version<>$2 THEN 'queued' WHEN $4::TEXT IS NULL THEN 'succeeded'
                WHEN $5 AND attempt_count<5 THEN 'retry_wait' ELSE 'failed' END,
            next_attempt_at=CASE WHEN version=$2 AND $4::TEXT IS NOT NULL AND $5 AND attempt_count<5
                THEN NOW()+make_interval(secs=>LEAST(900,15*attempt_count*attempt_count)) ELSE NULL END,
            attempt_count=CASE WHEN version<>$2 THEN 0 ELSE attempt_count END,
            last_error_code=CASE WHEN version=$2 THEN $4 ELSE NULL END,
            last_synced_at=CASE WHEN version=$2 AND $4::TEXT IS NULL THEN NOW() ELSE last_synced_at END,
            lease_owner=NULL,leased_version=NULL,lease_until=NULL,updated_at=NOW()
            WHERE id=$1 AND leased_version=$2 AND lease_owner=$3 AND attempt_count=$6 AND status='sending' AND lease_until>NOW()")
            .bind(job.id).bind(job.version).bind(worker).bind(error).bind(retryable)
            .bind(job.attempt_count)
            .execute(&mut *tx).await.map_err(classify)?;
        if result.rows_affected() == 1 {
            append_sync_change(&mut tx, job.id).await?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(result.rows_affected() == 1)
    }

    /// Retries an owned ticket lane without duplicating successful Chat delivery.
    /// # Errors
    /// Returns validation or persistence failures.
    pub async fn retry_itsm_sync(&self, user: Uuid, id: Uuid) -> Result<bool, StorageError> {
        if user.get_version_num() != 7 || id.get_version_num() != 7 {
            return Err(StorageError::InvalidConfiguration);
        }
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let result=sqlx::query("UPDATE itsm_issue_sync_jobs j SET status='queued',attempt_count=0,next_attempt_at=NULL,last_error_code=NULL,updated_at=NOW()
            WHERE j.id=$1 AND j.status='failed' AND EXISTS(SELECT 1 FROM task_itsm_links l JOIN tasks t ON t.id=l.task_id AND t.user_id=l.user_id
                WHERE l.issue_url=j.issue_url AND l.enabled AND l.user_id=$2)")
            .bind(id).bind(user).execute(&mut *tx).await.map_err(classify)?;
        if result.rows_affected() == 1 {
            append_sync_change(&mut tx, id).await?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(result.rows_affected() == 1)
    }
}
async fn append_sync_change(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO sync_changes(user_id,entity_type,entity_id,operation,entity_version)
        SELECT DISTINCT l.user_id,'itsm_issue_sync',j.id,'upsert',j.version
        FROM itsm_issue_sync_jobs j JOIN task_itsm_links l ON l.issue_url=j.issue_url AND l.enabled
        JOIN tasks t ON t.id=l.task_id AND t.user_id=l.user_id WHERE j.id=$1",
    )
    .bind(id)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}
fn classify(_: sqlx::Error) -> StorageError {
    StorageError::PersistenceUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_verified_canonical_issue_evidence_can_be_bound() {
        let document = InflowReferenceDocument {
            provider: "itsm".into(),
            url: "https://itsm.example/issues/12?tab=history".into(),
            external_id: "12".into(),
            title: None,
            original_content: None,
            error_code: None,
        };
        assert_eq!(
            verified_issue_references(&[document.clone(), document.clone()]),
            vec![("https://itsm.example/issues/12".into(), 12)]
        );
        let mut invalid = document;
        for url in [
            "http://itsm.example/issues/12",
            "https://credential@itsm.example/issues/12",
            "https://itsm.example/issues/0",
            "https://itsm.example/issues/not-an-id",
        ] {
            invalid.url = url.into();
            assert!(verified_issue_references(&[invalid.clone()]).is_empty());
        }
        invalid.url = "https://itsm.example/issues/12".into();
        invalid.error_code = Some("itsm.authentication_required".into());
        assert!(verified_issue_references(&[invalid]).is_empty());
    }
}

use super::{ItsmClient, MAX_RESPONSE_BYTES};
use futures_util::StreamExt;
use jimin_storage::{Database, itsm_sync::ItsmSyncDelivery};
use reqwest::{RequestBuilder, Response};
use secrecy::ExposeSecret;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use std::time::Duration;
use time::UtcOffset;

#[derive(Debug, Clone, Copy)]
struct SyncError {
    code: &'static str,
    retryable: bool,
}
impl SyncError {
    const fn permanent(code: &'static str) -> Self {
        Self {
            code,
            retryable: false,
        }
    }
    const fn transient() -> Self {
        Self {
            code: "itsm.unavailable",
            retryable: true,
        }
    }
}
#[derive(Deserialize)]
struct IssueEnvelope {
    issue: IssueState,
}
#[derive(Deserialize)]
struct IssueState {
    id: u64,
    status: Status,
    due_date: Option<String>,
    allowed_statuses: Option<Vec<Status>>,
}
#[derive(Debug, Clone, Deserialize)]
struct Status {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    is_closed: bool,
}
#[derive(Deserialize)]
struct StatusEnvelope {
    issue_statuses: Vec<Status>,
}

pub(crate) struct SyncWorker(tokio::task::JoinHandle<()>);
impl Drop for SyncWorker {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) fn spawn_sync_worker(database: Database, client: ItsmClient) -> SyncWorker {
    let worker = format!("itsm-{}", uuid::Uuid::now_v7());
    SyncWorker(tokio::spawn(async move {
        loop {
            let result = process_once(&database, &client, &worker).await;
            if result.is_err() {
                crate::write_single_json_line(&json!({"event":"itsm_sync_storage_unavailable"}));
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }))
}

async fn process_once(
    database: &Database,
    client: &ItsmClient,
    worker: &str,
) -> Result<(), jimin_storage::StorageError> {
    let Some(job) = database.claim_itsm_sync(worker).await? else {
        return Ok(());
    };
    let result = client
        .sync_ticket(&job, || async {
            database
                .itsm_sync_is_current(&job, worker)
                .await
                .map_err(|_| SyncError::transient())
        })
        .await;
    let (error, retryable) = match result {
        Ok(()) => (None, false),
        Err(error) => (Some(error.code), error.retryable),
    };
    database
        .finish_itsm_sync(&job, worker, error, retryable)
        .await?;
    Ok(())
}

impl ItsmClient {
    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        request.header("X-Redmine-API-Key", self.api_token.expose_secret())
    }

    async fn issue_state(&self, id: u64) -> Result<IssueState, SyncError> {
        let url = self
            .base_url
            .join(&format!("issues/{id}.json?include=allowed_statuses"))
            .map_err(|_| SyncError::permanent("itsm.invalid_reference"))?;
        let envelope: IssueEnvelope = read_json(self.authenticated(self.client.get(url))).await?;
        if envelope.issue.id != id {
            return Err(SyncError::permanent("itsm.invalid_response"));
        }
        Ok(envelope.issue)
    }

    async fn statuses(&self) -> Result<Vec<Status>, SyncError> {
        let url = self
            .base_url
            .join("issue_statuses.json")
            .map_err(|_| SyncError::permanent("itsm.invalid_reference"))?;
        let response: StatusEnvelope = read_json(self.authenticated(self.client.get(url))).await?;
        if response.issue_statuses.is_empty() || response.issue_statuses.len() > 200 {
            return Err(SyncError::permanent("itsm.invalid_response"));
        }
        Ok(response.issue_statuses)
    }

    async fn sync_ticket<F, Fut>(
        &self,
        job: &ItsmSyncDelivery,
        is_current: F,
    ) -> Result<(), SyncError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<bool, SyncError>>,
    {
        if job.linked_count == 0 {
            return Ok(());
        }
        let id = u64::try_from(job.issue_id)
            .map_err(|_| SyncError::permanent("itsm.invalid_reference"))?;
        // The admin credential is never sent to a link supplied by a task.
        if self.issue_id(&job.issue_url) != Some(id) {
            return Err(SyncError::permanent("itsm.invalid_reference"));
        }
        if !is_current().await? {
            return Err(SyncError::permanent("itsm.superseded"));
        }
        let current = self.issue_state(id).await?;
        let deadline = job.due_at.map(|value| {
            value
                .to_offset(UtcOffset::from_hms(9, 0, 0).expect("Seoul offset"))
                .date()
                .to_string()
        });
        let mut changes = Map::new();
        if job.sync_deadline
            && current.due_date.as_deref().filter(|date| !date.is_empty()) != deadline.as_deref()
        {
            changes.insert("due_date".into(), json!(deadline));
        }
        let closed_statuses = if job.should_complete {
            self.statuses().await?
        } else {
            Vec::new()
        };
        let already_closed = closed_statuses
            .iter()
            .any(|status| status.id == current.status.id && status.is_closed);
        if job.should_complete && !already_closed {
            let target = select_completion_status(
                &closed_statuses,
                current.allowed_statuses.as_deref(),
                self.completion_status_id,
            )?;
            changes.insert("status_id".into(), json!(target));
        }
        if changes.is_empty() {
            return Ok(());
        }
        if !is_current().await? {
            return Err(SyncError::permanent("itsm.superseded"));
        }
        let url = self
            .base_url
            .join(&format!("issues/{id}.json"))
            .map_err(|_| SyncError::permanent("itsm.invalid_reference"))?;
        // Only these two attributes can be emitted. No notes, assignee,
        // description, project transfer or comment creates are sent on retry.
        let response = self
            .authenticated(self.client.put(url))
            .json(&json!({"issue":changes}))
            .send()
            .await
            .map_err(|_| SyncError::transient())?;
        validate_response(&response)?;
        let verified = self.issue_state(id).await?;
        if changes.contains_key("due_date")
            && verified.due_date.as_deref().filter(|date| !date.is_empty()) != deadline.as_deref()
        {
            return Err(SyncError::permanent("itsm.verification_failed"));
        }
        if let Some(target) = changes.get("status_id").and_then(Value::as_u64)
            && verified.status.id != target
        {
            return Err(SyncError::permanent("itsm.verification_failed"));
        }
        Ok(())
    }
}

fn select_completion_status(
    statuses: &[Status],
    allowed: Option<&[Status]>,
    configured: Option<u64>,
) -> Result<u64, SyncError> {
    let candidates = statuses
        .iter()
        .filter(|status| status.is_closed)
        .collect::<Vec<_>>();
    let target = if let Some(id) = configured {
        candidates
            .iter()
            .find(|status| status.id == id)
            .copied()
            .ok_or(SyncError::permanent("itsm.completion_status_unavailable"))?
    } else {
        let candidates = candidates
            .into_iter()
            .filter(|status| {
                allowed.is_none_or(|values| values.iter().any(|value| value.id == status.id))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty()
            && allowed.is_some()
            && statuses.iter().any(|status| status.is_closed)
        {
            return Err(SyncError::permanent("itsm.workflow_blocked"));
        }
        if candidates.len() == 1 {
            candidates[0]
        } else {
            let preferred = candidates
                .into_iter()
                .filter(|status| {
                    matches!(
                        status.name.trim().to_lowercase().as_str(),
                        "완료" | "종료" | "closed" | "done" | "completed"
                    )
                })
                .collect::<Vec<_>>();
            if preferred.len() != 1 {
                return Err(SyncError::permanent("itsm.completion_status_unavailable"));
            }
            preferred[0]
        }
    };
    if allowed.is_some_and(|values| !values.iter().any(|value| value.id == target.id)) {
        return Err(SyncError::permanent("itsm.workflow_blocked"));
    }
    Ok(target.id)
}

fn validate_response(response: &Response) -> Result<(), SyncError> {
    match response.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(SyncError::permanent("itsm.authentication_required")),
        404 => Err(SyncError::permanent("itsm.issue_not_found")),
        400 | 409 | 422 => Err(SyncError::permanent("itsm.workflow_blocked")),
        429 | 500..=599 => Err(SyncError::transient()),
        _ => Err(SyncError::permanent("itsm.request_rejected")),
    }
}
async fn read_json<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, SyncError> {
    let response = request.send().await.map_err(|_| SyncError::transient())?;
    validate_response(&response)?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err(SyncError::permanent("itsm.invalid_response"));
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| SyncError::transient())?;
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(SyncError::permanent("itsm.invalid_response"));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| SyncError::permanent("itsm.invalid_response"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn delivery(client: &ItsmClient, complete: bool) -> ItsmSyncDelivery {
        ItsmSyncDelivery {
            id: uuid::Uuid::now_v7(),
            issue_url: client.base_url.join("issues/42").unwrap().to_string(),
            issue_id: 42,
            version: 1,
            sync_deadline: true,
            due_at: Some(
                time::OffsetDateTime::parse(
                    "2026-10-20T15:30:00Z",
                    &time::format_description::well_known::Rfc3339,
                )
                .unwrap(),
            ),
            should_complete: complete,
            linked_count: 1,
            attempt_count: 1,
        }
    }
    async fn server(
        responses: Vec<(u16, String)>,
    ) -> (ItsmClient, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) =
                    tokio::time::timeout(Duration::from_secs(3), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let count =
                        tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buffer))
                            .await
                            .unwrap()
                            .unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some((header, _)) = text.split_once("\r\n\r\n") {
                        let length = header
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|value| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= header.len() + 4 + length {
                            break;
                        }
                    }
                    assert!(request.len() < 16000);
                }
                requests.push(String::from_utf8(request).unwrap());
                let response = format!(
                    "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        // Only this private test constructor permits a loopback HTTP mock.
        let client = ItsmClient {
            base_url: reqwest::Url::parse(&format!("http://{address}/")).unwrap(),
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            api_token: secrecy::SecretString::from("fixture-only-value"),
            completion_status_id: None,
        };
        (client, handle)
    }
    fn issue(date: Option<&str>, status: u64) -> String {
        json!({"issue":{"id":42,"status":{"id":status},"due_date":date,"allowed_statuses":[{"id":7,"name":"완료"}]}}).to_string()
    }
    fn catalog() -> String {
        json!({"issue_statuses":[{"id":1,"name":"신규","is_closed":false},{"id":7,"name":"완료","is_closed":true}]}).to_string()
    }

    #[tokio::test]
    async fn actual_http_write_contains_only_deadline_and_completion_then_verifies() {
        let (client, server) = server(vec![
            (200, issue(Some("2026-10-20"), 1)),
            (200, catalog()),
            (204, String::new()),
            (200, issue(Some("2026-10-21"), 7)),
        ])
        .await;
        client
            .sync_ticket(&delivery(&client, true), || async { Ok(true) })
            .await
            .unwrap();
        let requests = server.await.unwrap();
        assert!(requests[2].starts_with("PUT /issues/42.json "));
        let body: Value =
            serde_json::from_str(requests[2].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            body,
            json!({"issue":{"due_date":"2026-10-21","status_id":7}})
        );
        assert!(requests[3].starts_with("GET /issues/42.json"));
        assert!(requests.iter().all(|request| {
            request
                .to_lowercase()
                .contains("x-redmine-api-key: fixture-only-value")
        }));
    }
    #[tokio::test]
    async fn shared_open_tasks_do_not_emit_a_completion_change() {
        let (client, server) = server(vec![
            (200, issue(None, 1)),
            (204, String::new()),
            (200, issue(Some("2026-10-21"), 1)),
        ])
        .await;
        client
            .sync_ticket(&delivery(&client, false), || async { Ok(true) })
            .await
            .unwrap();
        let requests = server.await.unwrap();
        let body: Value =
            serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body, json!({"issue":{"due_date":"2026-10-21"}}));
    }
    #[tokio::test]
    async fn matching_remote_state_is_idempotent_without_another_put() {
        let (client, server) =
            server(vec![(200, issue(Some("2026-10-21"), 7)), (200, catalog())]).await;
        client
            .sync_ticket(&delivery(&client, true), || async { Ok(true) })
            .await
            .unwrap();
        assert!(
            server
                .await
                .unwrap()
                .iter()
                .all(|request| request.starts_with("GET "))
        );
    }
    #[tokio::test]
    async fn initial_no_deadline_preserves_remote_but_explicit_clear_sends_null() {
        let (client, server) = server(vec![
            (200, issue(Some("2026-10-21"), 1)),
            (200, issue(Some("2026-10-21"), 1)),
            (204, String::new()),
            (200, issue(None, 1)),
        ])
        .await;
        let mut job = delivery(&client, false);
        job.due_at = None;
        job.sync_deadline = false;
        client
            .sync_ticket(&job, || async { Ok(true) })
            .await
            .unwrap();
        job.sync_deadline = true;
        client
            .sync_ticket(&job, || async { Ok(true) })
            .await
            .unwrap();
        let requests = server.await.unwrap();
        let body: Value =
            serde_json::from_str(requests[2].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body, json!({"issue":{"due_date":null}}));
    }
    #[tokio::test]
    async fn forbidden_and_workflow_errors_are_not_blindly_retried() {
        for (responses, code) in [
            (vec![(403, "{}".into())], "itsm.authentication_required"),
            (
                vec![(200, issue(None, 1)), (422, "{}".into())],
                "itsm.workflow_blocked",
            ),
            (vec![(503, "{}".into())], "itsm.unavailable"),
        ] {
            let (client, server) = server(responses).await;
            let error = client
                .sync_ticket(&delivery(&client, false), || async { Ok(true) })
                .await
                .unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(error.retryable, code == "itsm.unavailable");
            server.await.unwrap();
        }
    }
    #[tokio::test]
    async fn ignored_server_changes_are_reported_not_claimed_as_success() {
        let (client, server) = server(vec![
            (200, issue(None, 1)),
            (204, String::new()),
            (200, issue(None, 1)),
        ])
        .await;
        let error = client
            .sync_ticket(&delivery(&client, false), || async { Ok(true) })
            .await
            .unwrap_err();
        assert_eq!(error.code, "itsm.verification_failed");
        server.await.unwrap();
    }
    #[tokio::test]
    async fn stale_local_revision_and_foreign_urls_send_no_request() {
        let (client, server) = server(vec![]).await;
        let mut job = delivery(&client, false);
        job.linked_count = 0;
        client
            .sync_ticket(&job, || async { Ok(true) })
            .await
            .unwrap();
        job.linked_count = 1;
        assert_eq!(
            client
                .sync_ticket(&job, || async { Ok(false) })
                .await
                .unwrap_err()
                .code,
            "itsm.superseded"
        );
        job.issue_url = "https://foreign.example/issues/42".into();
        assert_eq!(
            client
                .sync_ticket(&job, || async { Ok(true) })
                .await
                .unwrap_err()
                .code,
            "itsm.invalid_reference"
        );
        assert!(server.await.unwrap().is_empty());
    }
    fn status(id: u64, name: &str, is_closed: bool) -> Status {
        Status {
            id,
            name: name.into(),
            is_closed,
        }
    }
    #[test]
    fn completion_respects_actual_closed_status_and_workflow() {
        let values = vec![
            status(1, "신규", false),
            status(7, "완료", true),
            status(9, "거절", true),
        ];
        assert_eq!(select_completion_status(&values, None, None).unwrap(), 7);
        assert_eq!(select_completion_status(&values, None, Some(9)).unwrap(), 9);
        assert_eq!(
            select_completion_status(&values, Some(&[status(1, "신규", false)]), Some(7))
                .unwrap_err()
                .code,
            "itsm.workflow_blocked"
        );
        assert!(select_completion_status(&values, None, Some(1)).is_err());
        assert!(
            select_completion_status(
                &[status(7, "완료", true), status(8, "종료", true)],
                None,
                None
            )
            .is_err()
        );
    }
    #[test]
    fn deadline_uses_seoul_calendar_date_not_utc_date() {
        let value = time::OffsetDateTime::parse(
            "2026-10-20T15:30:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap();
        assert_eq!(
            value
                .to_offset(UtcOffset::from_hms(9, 0, 0).unwrap())
                .date()
                .to_string(),
            "2026-10-21"
        );
    }
}

//! Read-only enrichment for trusted ITSM issue links.
//!
//! Chat messages are untrusted input. This client never follows a URL from a
//! message directly: it extracts only numeric issue identifiers that match the
//! configured ITSM origin, then builds the Redmine-compatible API URL from the
//! trusted base URL.

use std::{collections::BTreeSet, time::Duration};

use futures_util::StreamExt;
use jimin_storage::inflow_analysis::InflowAnalysisMessage;
use reqwest::{Client, Url, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const TOTAL_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_REFERENCE_DOCUMENTS: usize = 4;
const MAX_ORIGINAL_CONTENT_CHARS: usize = 40_000;
const MAX_TITLE_CHARS: usize = 200;

pub(crate) struct ItsmClient {
    base_url: Url,
    client: Client,
    api_token: Option<SecretString>,
    allowed_source_ids: BTreeSet<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItsmReferenceSnapshot {
    pub url: String,
    pub external_id: String,
    pub title: Option<String>,
    pub original_content: Option<String>,
    pub error_code: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItsmClientError {
    InvalidConfiguration,
}

#[derive(Debug, Deserialize)]
struct RedmineIssueEnvelope {
    issue: RedmineIssue,
}

#[derive(Debug, Deserialize)]
struct RedmineIssue {
    id: u64,
    subject: String,
    #[serde(default)]
    description: String,
    status: Option<RedmineNamedValue>,
    priority: Option<RedmineNamedValue>,
    author: Option<RedmineNamedValue>,
    assigned_to: Option<RedmineNamedValue>,
    due_date: Option<String>,
    created_on: Option<String>,
    updated_on: Option<String>,
    #[serde(default)]
    journals: Vec<RedmineJournal>,
    #[serde(default)]
    attachments: Vec<RedmineAttachment>,
}

#[derive(Debug, Deserialize)]
struct RedmineNamedValue {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RedmineJournal {
    user: Option<RedmineNamedValue>,
    #[serde(default)]
    notes: String,
    created_on: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedmineAttachment {
    filename: String,
    content_url: Option<String>,
    description: Option<String>,
}

impl ItsmClient {
    pub(crate) fn new(
        base_url: &str,
        api_token: Option<SecretString>,
        allowed_source_ids: BTreeSet<Uuid>,
    ) -> Result<Self, ItsmClientError> {
        if allowed_source_ids.is_empty()
            || allowed_source_ids
                .iter()
                .any(|source_id| source_id.get_version_num() != 7)
        {
            return Err(ItsmClientError::InvalidConfiguration);
        }
        if api_token.as_ref().is_some_and(|token| {
            let value = token.expose_secret();
            value.is_empty() || value.len() > 16 * 1024 || value.chars().any(char::is_control)
        }) {
            return Err(ItsmClientError::InvalidConfiguration);
        }
        let mut base_url =
            Url::parse(base_url).map_err(|_| ItsmClientError::InvalidConfiguration)?;
        if base_url.scheme() != "https"
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(ItsmClientError::InvalidConfiguration);
        }
        base_url.set_path(&format!("{}/", base_url.path().trim_end_matches('/')));
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| ItsmClientError::InvalidConfiguration)?;
        Ok(Self {
            base_url,
            client,
            api_token,
            allowed_source_ids,
        })
    }

    pub(crate) async fn resolve_messages(
        &self,
        source_id: Uuid,
        messages: &[InflowAnalysisMessage],
    ) -> Vec<ItsmReferenceSnapshot> {
        if !self.source_is_allowed(source_id) {
            return Vec::new();
        }
        let mut issue_ids = BTreeSet::new();
        for message in messages {
            for candidate in http_links(&message.content_text) {
                if let Some(issue_id) = self.issue_id(&candidate) {
                    issue_ids.insert(issue_id);
                    if issue_ids.len() == MAX_REFERENCE_DOCUMENTS {
                        break;
                    }
                }
            }
            if issue_ids.len() == MAX_REFERENCE_DOCUMENTS {
                break;
            }
        }

        let mut snapshots = Vec::with_capacity(issue_ids.len());
        let deadline = tokio::time::Instant::now() + TOTAL_RESOLUTION_TIMEOUT;
        for issue_id in issue_ids {
            let Ok(snapshot) = tokio::time::timeout_at(deadline, self.fetch_issue(issue_id)).await
            else {
                snapshots.push(self.failed_issue_snapshot(issue_id, "itsm.unavailable"));
                break;
            };
            snapshots.push(snapshot);
        }
        snapshots
    }

    fn source_is_allowed(&self, source_id: Uuid) -> bool {
        self.allowed_source_ids.contains(&source_id)
    }

    fn issue_id(&self, candidate: &str) -> Option<u64> {
        let url = Url::parse(candidate).ok()?;
        if url.scheme() != self.base_url.scheme()
            || url.host_str() != self.base_url.host_str()
            || url.port_or_known_default() != self.base_url.port_or_known_default()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return None;
        }
        let base_segments = self
            .base_url
            .path_segments()?
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        let segments = url
            .path_segments()?
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.len() != base_segments.len() + 2
            || segments[..base_segments.len()] != base_segments
            || segments[base_segments.len()] != "issues"
        {
            return None;
        }
        let id = segments[base_segments.len() + 1];
        if id.is_empty() || id.len() > 20 || !id.bytes().all(|value| value.is_ascii_digit()) {
            return None;
        }
        id.parse().ok()
    }

    fn issue_page_url(&self, issue_id: u64) -> Url {
        self.base_url
            .join(&format!("issues/{issue_id}"))
            .expect("validated base URL must accept a relative issue path")
    }

    fn failed_issue_snapshot(&self, issue_id: u64, code: &'static str) -> ItsmReferenceSnapshot {
        failed_snapshot(self.issue_page_url(issue_id), issue_id, code)
    }

    async fn fetch_issue(&self, issue_id: u64) -> ItsmReferenceSnapshot {
        let page_url = self.issue_page_url(issue_id);
        let mut api_url = self
            .base_url
            .join(&format!("issues/{issue_id}.json"))
            .expect("validated base URL must accept a relative API path");
        api_url
            .query_pairs_mut()
            .append_pair("include", "journals,attachments");
        let mut request = self.client.get(api_url);
        if let Some(token) = self.api_token.as_ref() {
            request = request.header("X-Redmine-API-Key", token.expose_secret());
        }
        let Ok(response) = request.send().await else {
            return failed_snapshot(page_url, issue_id, "itsm.unavailable");
        };
        if !response.status().is_success() {
            let code = match response.status().as_u16() {
                401 | 403 => "itsm.authentication_required",
                404 => "itsm.issue_not_found",
                _ => "itsm.request_rejected",
            };
            return failed_snapshot(page_url, issue_id, code);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return failed_snapshot(page_url, issue_id, "itsm.response_too_large");
        }
        let mut body = Vec::with_capacity(
            response
                .content_length()
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or(8 * 1024)
                .min(MAX_RESPONSE_BYTES),
        );
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else {
                return failed_snapshot(page_url, issue_id, "itsm.unavailable");
            };
            if !append_bounded_response(&mut body, &chunk) {
                return failed_snapshot(page_url, issue_id, "itsm.response_too_large");
            }
        }
        let envelope: RedmineIssueEnvelope = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => return failed_snapshot(page_url, issue_id, "itsm.invalid_response"),
        };
        if envelope.issue.id != issue_id || envelope.issue.subject.trim().is_empty() {
            return failed_snapshot(page_url, issue_id, "itsm.invalid_response");
        }
        let original_content = render_original_content(&envelope.issue);
        ItsmReferenceSnapshot {
            url: page_url.into(),
            external_id: issue_id.to_string(),
            title: Some(bounded_title(&envelope.issue.subject)),
            original_content: Some(original_content),
            error_code: None,
        }
    }
}

fn append_bounded_response(target: &mut Vec<u8>, chunk: &[u8]) -> bool {
    if target
        .len()
        .checked_add(chunk.len())
        .is_none_or(|length| length > MAX_RESPONSE_BYTES)
    {
        return false;
    }
    target.extend_from_slice(chunk);
    true
}

fn failed_snapshot(url: Url, issue_id: u64, code: &'static str) -> ItsmReferenceSnapshot {
    ItsmReferenceSnapshot {
        url: url.into(),
        external_id: issue_id.to_string(),
        title: None,
        original_content: None,
        error_code: Some(code),
    }
}

fn render_original_content(issue: &RedmineIssue) -> String {
    let mut sections = Vec::new();
    sections.push(format!(
        "ITSM #{}\n제목: {}",
        issue.id,
        issue.subject.trim()
    ));
    push_named_value(&mut sections, "상태", issue.status.as_ref());
    push_named_value(&mut sections, "우선순위", issue.priority.as_ref());
    push_named_value(&mut sections, "등록자", issue.author.as_ref());
    push_named_value(&mut sections, "담당자", issue.assigned_to.as_ref());
    push_optional_value(&mut sections, "마감일", issue.due_date.as_deref());
    push_optional_value(&mut sections, "등록 시각", issue.created_on.as_deref());
    push_optional_value(&mut sections, "수정 시각", issue.updated_on.as_deref());
    if !issue.description.trim().is_empty() {
        sections.push(format!("원문 설명\n{}", issue.description.trim()));
    }
    let attachments = issue
        .attachments
        .iter()
        .map(|attachment| {
            let mut value = format!("- {}", attachment.filename.trim());
            if let Some(description) = attachment
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                value.push_str(": ");
                value.push_str(description);
            }
            if let Some(url) = attachment
                .content_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                value.push_str("\n  ");
                value.push_str(url);
            }
            value
        })
        .collect::<Vec<_>>();
    if !attachments.is_empty() {
        sections.push(format!("첨부 파일\n{}", attachments.join("\n")));
    }
    let journals = issue
        .journals
        .iter()
        .filter_map(|journal| {
            let notes = journal.notes.trim();
            if notes.is_empty() {
                return None;
            }
            let author = journal
                .user
                .as_ref()
                .map_or("작성자 미확인", |value| value.name.trim());
            let created_at = journal.created_on.as_deref().unwrap_or("시각 미확인");
            Some(format!("[{created_at} · {author}]\n{notes}"))
        })
        .collect::<Vec<_>>();
    if !journals.is_empty() {
        sections.push(format!("원문 업데이트\n{}", journals.join("\n\n")));
    }
    bounded_chars(
        &sections.join("\n\n"),
        MAX_ORIGINAL_CONTENT_CHARS,
        "\n[원문이 길어 이후 내용은 ITSM 링크에서 확인해 주세요.]",
    )
}

fn push_named_value(sections: &mut Vec<String>, label: &str, value: Option<&RedmineNamedValue>) {
    if let Some(value) = value
        .map(|value| value.name.trim())
        .filter(|value| !value.is_empty())
    {
        sections.push(format!("{label}: {value}"));
    }
}

fn push_optional_value(sections: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        sections.push(format!("{label}: {value}"));
    }
}

fn bounded_title(value: &str) -> String {
    bounded_chars(value.trim(), MAX_TITLE_CHARS, "…")
}

fn bounded_chars(value: &str, maximum: usize, suffix: &str) -> String {
    if value.chars().count() <= maximum {
        return value.to_owned();
    }
    let suffix = suffix.chars().take(maximum).collect::<String>();
    let retained = maximum.saturating_sub(suffix.chars().count());
    let mut result = value.chars().take(retained).collect::<String>();
    result.push_str(&suffix);
    result
}

fn http_links(value: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = value;
    while let Some(index) = next_http_link_index(remaining) {
        let candidate = remaining[index..]
            .split(|character: char| {
                character.is_whitespace() || matches!(character, '<' | '>' | '"' | '\'' | ']')
            })
            .next()
            .unwrap_or_default()
            .trim_end_matches(|character: char| {
                matches!(character, '.' | ',' | ';' | ':' | '!' | '?' | ')')
            });
        if candidate.len() <= 2_048 && Url::parse(candidate).is_ok() {
            links.push(candidate.to_owned());
        }
        remaining = &remaining[index + candidate.len().max(1)..];
    }
    links
}

fn next_http_link_index(value: &str) -> Option<usize> {
    match (value.find("https://"), value.find("http://")) {
        (Some(https), Some(http)) => Some(https.min(http)),
        (Some(https), None) => Some(https),
        (None, Some(http)) => Some(http),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ItsmClient, MAX_ORIGINAL_CONTENT_CHARS, MAX_RESPONSE_BYTES, MAX_TITLE_CHARS,
        RedmineIssueEnvelope, append_bounded_response, bounded_title, render_original_content,
    };
    use secrecy::SecretString;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    fn client(base_url: &str) -> ItsmClient {
        ItsmClient::new(
            base_url,
            Some(SecretString::from("secret")),
            [Uuid::now_v7()].into_iter().collect(),
        )
        .expect("valid client")
    }

    #[test]
    fn accepts_only_numeric_issue_links_on_the_configured_origin() {
        let client = client("https://itsm.bix.bz/");
        assert_eq!(
            client.issue_id("https://itsm.bix.bz/issues/3876"),
            Some(3_876)
        );
        assert_eq!(
            client.issue_id("https://itsm.bix.bz/issues/3876?tab=history"),
            Some(3_876)
        );
        assert_eq!(client.issue_id("https://evil.example/issues/3876"), None);
        assert_eq!(
            client.issue_id("https://itsm.bix.bz/issues/not-a-number"),
            None
        );
        assert_eq!(
            client.issue_id("https://itsm.bix.bz/projects/1/issues/3876"),
            None
        );
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_base_urls() {
        let allowed = || [Uuid::now_v7()].into_iter().collect::<BTreeSet<_>>();
        assert!(ItsmClient::new("http://itsm.bix.bz", None, allowed()).is_err());
        assert!(ItsmClient::new("https://user@itsm.bix.bz", None, allowed()).is_err());
        assert!(ItsmClient::new("https://itsm.bix.bz?redirect=1", None, allowed()).is_err());
        assert!(
            ItsmClient::new(
                "https://itsm.bix.bz",
                Some(SecretString::from("unsafe\nsecret")),
                allowed(),
            )
            .is_err()
        );
        assert!(
            ItsmClient::new("https://itsm.bix.bz", None, BTreeSet::new()).is_err(),
            "an enabled ITSM client must never have an empty source allowlist"
        );
    }

    #[test]
    fn resolves_only_explicitly_allowed_google_chat_sources() {
        let allowed_source_id = Uuid::now_v7();
        let denied_source_id = Uuid::now_v7();
        let client = ItsmClient::new(
            "https://itsm.bix.bz",
            None,
            [allowed_source_id].into_iter().collect(),
        )
        .expect("valid scoped client");

        assert!(client.source_is_allowed(allowed_source_id));
        assert!(!client.source_is_allowed(denied_source_id));
    }

    #[test]
    fn preserves_description_updates_and_attachment_links() {
        let envelope: RedmineIssueEnvelope = serde_json::from_str(
            r#"{
              "issue": {
                "id": 3876,
                "subject": "거래내역 정산방식 표기",
                "description": "API와 화면에 정산방식을 표시합니다.",
                "status": {"name": "신규"},
                "author": {"name": "조지민"},
                "attachments": [{
                  "filename": "요청서.pdf",
                  "content_url": "https://itsm.bix.bz/attachments/download/1"
                }],
                "journals": [{
                  "user": {"name": "이의현"},
                  "notes": "대표님 요청으로 우선 처리합니다.",
                  "created_on": "2026-07-31T10:00:00+09:00"
                }]
              }
            }"#,
        )
        .expect("valid fixture");
        let content = render_original_content(&envelope.issue);
        assert!(content.contains("API와 화면에 정산방식을 표시합니다."));
        assert!(content.contains("대표님 요청으로 우선 처리합니다."));
        assert!(content.contains("https://itsm.bix.bz/attachments/download/1"));
    }

    #[test]
    fn response_chunks_are_rejected_before_crossing_the_byte_limit() {
        let mut body = vec![b'a'; MAX_RESPONSE_BYTES - 2];
        assert!(append_bounded_response(&mut body, b"bc"));
        assert_eq!(body.len(), MAX_RESPONSE_BYTES);
        assert!(!append_bounded_response(&mut body, b"d"));
        assert_eq!(body.len(), MAX_RESPONSE_BYTES);
    }

    #[test]
    fn long_titles_and_original_content_honor_storage_contracts() {
        let title = bounded_title(&"제목".repeat(MAX_TITLE_CHARS));
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
        assert!(title.ends_with('…'));

        let envelope: RedmineIssueEnvelope = serde_json::from_str(&format!(
            r#"{{
              "issue": {{
                "id": 3876,
                "subject": "긴 원문",
                "description": "{}"
              }}
            }}"#,
            "상세".repeat(MAX_ORIGINAL_CONTENT_CHARS)
        ))
        .expect("valid long fixture");
        let content = render_original_content(&envelope.issue);
        assert_eq!(content.chars().count(), MAX_ORIGINAL_CONTENT_CHARS);
        assert!(content.ends_with("ITSM 링크에서 확인해 주세요.]"));
    }
}

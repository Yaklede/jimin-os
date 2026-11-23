//! Read-only completed project history. The legacy task list stays unchanged.
use crate::{
    ApiState, TaskResponse, auth, invalid_request_response, no_store_json, storage_error_response,
    task_response, unavailable_response,
};
use axum::{
    Extension,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Response,
};
use jimin_observability::RequestId;
use jimin_storage::planning::{PROJECT_HISTORY_PAGE_SIZE, ProjectHistoryQuery};
use serde::{Deserialize, Serialize};
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, IntoParams, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub(super) struct HistoryQuery {
    page: Option<u32>,
    query: Option<String>,
    assignee: Option<String>,
    /// Inclusive completion date in Asia/Seoul (YYYY-MM-DD).
    completed_from: Option<String>,
    /// Inclusive completion date in Asia/Seoul (YYYY-MM-DD).
    completed_to: Option<String>,
    /// Open the page containing this completed task when following a direct link.
    anchor_task_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryPage {
    items: Vec<TaskResponse>,
    page: u32,
    page_size: i64,
    total_count: i64,
    filtered_count: i64,
    assignees: Vec<String>,
}

#[utoipa::path(
    get, path="/v1/projects/{project_id}/completed-tasks", tag="planning",
    params(("project_id"=Uuid, Path), HistoryQuery),
    responses((status=200, body=HistoryPage),(status=400),(status=401),(status=503))
)]
pub(super) async fn list(
    State(state): State<ApiState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let principal = match auth::authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(error) => return error.into_response(request_id),
    };
    let Ok(query) = query.validate() else {
        return invalid_request_response(request_id);
    };
    let Some(db) = state.planning() else {
        return unavailable_response(request_id);
    };
    match db
        .project_completed_history(principal.identity().user_id(), project_id, &query)
        .await
    {
        Ok(case) => match case.tasks.into_iter().map(task_response).collect() {
            Ok(items) => no_store_json(HistoryPage {
                items,
                page: case.page,
                page_size: PROJECT_HISTORY_PAGE_SIZE,
                total_count: case.total_count,
                filtered_count: case.filtered_count,
                assignees: case.assignees,
            }),
            Err(()) => unavailable_response(request_id),
        },
        Err(error) => storage_error_response(&error, request_id),
    }
}

impl HistoryQuery {
    fn validate(self) -> Result<ProjectHistoryQuery, ()> {
        let page = self.page.unwrap_or(1);
        let search = trimmed(self.query, 240)?;
        let assignee = trimmed(self.assignee, 80)?;
        let from = self.completed_from.as_deref().map(parse_date).transpose()?;
        let to = self.completed_to.as_deref().map(parse_date).transpose()?;
        if !(1..=1_000_000).contains(&page)
            || matches!((from, to), (Some(start), Some(end)) if start > end)
            || self
                .anchor_task_id
                .is_some_and(|id| id.get_version_num() != 7)
            || (self.anchor_task_id.is_some()
                && (search.is_some() || assignee.is_some() || from.is_some() || to.is_some()))
        {
            return Err(());
        }
        let offset = UtcOffset::from_hms(9, 0, 0).map_err(|_| ())?;
        let midnight = |date| PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_offset(offset);
        let issue = search.as_deref().and_then(issue_number);
        Ok(ProjectHistoryQuery {
            page,
            issue_id: issue,
            title: search.filter(|_| issue.is_none()),
            assignee,
            completed_from: from.map(midnight),
            completed_before: to
                .map(|date| date.next_day().ok_or(()))
                .transpose()?
                .map(midnight),
            anchor_task_id: self.anchor_task_id,
        })
    }
}

fn trimmed(raw: Option<String>, limit: usize) -> Result<Option<String>, ()> {
    let Some(raw) = raw else { return Ok(None) };
    if raw.chars().count() > limit || raw.chars().any(char::is_control) {
        return Err(());
    }
    let value = raw.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

fn parse_date(raw: &str) -> Result<Date, ()> {
    let bytes = raw.as_bytes();
    if bytes.len() != 10
        || !bytes.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return Err(());
    }
    let year = raw[..4].parse::<i32>().map_err(|_| ())?;
    let month = Month::try_from(raw[5..7].parse::<u8>().map_err(|_| ())?).map_err(|_| ())?;
    let day = raw[8..].parse::<u8>().map_err(|_| ())?;
    if year == 0 {
        return Err(());
    }
    Date::from_calendar_date(year, month, day).map_err(|_| ())
}

fn issue_number(search: &str) -> Option<i64> {
    let lower = search.to_ascii_lowercase();
    let rest = lower.strip_prefix("itsm ").unwrap_or(&lower).trim();
    let candidate = if let Ok(url) = reqwest::Url::parse(rest) {
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        let (_, tail) = url.path().rsplit_once("/issues/")?;
        return tail
            .trim_end_matches('/')
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0);
    } else {
        rest.strip_prefix('#').unwrap_or(rest)
    };
    if !candidate.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    candidate.parse::<i64>().ok().filter(|id| *id > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ticket_identifiers_and_leaves_plain_search_to_title_matching() {
        assert_eq!(issue_number("9032"), Some(9032));
        assert_eq!(issue_number("ITSM #9032"), Some(9032));
        assert_eq!(
            issue_number("https://itsm.example/issues/9032?tab=details"),
            Some(9032)
        );
        assert_eq!(issue_number("9032번 상세 업무"), None);
        assert_eq!(issue_number("#0"), None);
    }

    #[test]
    fn validates_dates_and_uses_full_korean_calendar_days() {
        let query = HistoryQuery {
            page: Some(2),
            query: None,
            assignee: Some(" 김경주 ".to_owned()),
            completed_from: Some("2026-11-23".to_owned()),
            completed_to: Some("2026-11-23".to_owned()),
            anchor_task_id: None,
        }
        .validate()
        .unwrap();
        assert_eq!(query.assignee.as_deref(), Some("김경주"));
        assert_eq!(query.completed_from.unwrap().offset().whole_hours(), 9);
        assert_eq!(
            (query.completed_before.unwrap() - query.completed_from.unwrap()).whole_hours(),
            24
        );
        assert!(parse_date("2026-02-29").is_err());
        assert!(parse_date("2026-2-09").is_err());
    }
}

//! Owner-scoped summaries of work awaiting attention, separate from registered tasks.
use std::{cmp::Reverse, fmt::Write as _};

use jimin_storage::{
    Database,
    agent::{ConversationMessage, ConversationMessageRole},
    completion_reviews::CompletionReview,
    gmail_inflow::GmailInflowCandidate,
    inflow_analysis::{InflowAnalysisState, InflowClassification},
    inflow_queue::ProjectInflowCandidate,
    work::Project,
};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

const MAX_ROWS: usize = 100;
const MAX_ANSWER_ROWS: usize = 60;

#[derive(Default)]
pub(crate) struct WorkInbox {
    chat: Option<Vec<Incoming>>,
    mail: Option<Vec<GmailInflowCandidate>>,
    completion: Option<Vec<CompletionReview>>,
}

#[derive(Serialize)]
struct Incoming {
    project_id: Uuid,
    project: String,
    source: String,
    title: String,
    summary: Option<String>,
    state: &'static str,
    priority: Option<i16>,
    suggested_assignee: Option<String>,
    due_at: Option<i64>,
    received_at: i64,
    task_id: Option<Uuid>,
}

impl WorkInbox {
    pub(crate) async fn load(database: &Database, user_id: Uuid) -> Self {
        let (chat, mail, completion) = tokio::join!(
            database.project_inflow_queue_for_user(user_id),
            database.gmail_inflow_attention_for_user(user_id),
            database.completion_reviews(user_id, None),
        );
        Self {
            chat: chat.ok().map(|rows| {
                let mut rows: Vec<_> = rows
                    .iter()
                    .filter(|row| !row.reviewed())
                    .map(Incoming::from)
                    .collect();
                rows.sort_by_key(|row| (Reverse(row.priority), Reverse(row.received_at)));
                rows
            }),
            mail: mail.ok(),
            completion: completion.ok(),
        }
    }

    pub(crate) fn write_prompt(&self, prompt: &mut String) {
        section(
            prompt,
            "unassigned_new_work_requests",
            self.chat.as_ref().map(|rows| {
                rows.iter()
                    .filter(|row| row.state == "new_unassigned")
                    .collect::<Vec<_>>()
            }),
        );
        section(
            prompt,
            "other_chat_requests_for_review",
            self.chat.as_ref().map(|rows| {
                rows.iter()
                    .filter(|row| row.state != "new_unassigned")
                    .collect::<Vec<_>>()
            }),
        );
        section(
            prompt,
            "gmail_work_requests",
            self.mail.as_ref().map(|rows| rows.iter().map(|row| {
                serde_json::json!({
                    "workspace": one_line(&row.workspace_name, 100),
                    "scope": row.workspace_scope,
                    "title": row.suggested_task_title.as_deref().or(row.subject.as_deref()).map(|text| one_line(text, 200)),
                    "classification": row.classification.map(|classification| format!("{classification:?}")),
                    "priority": row.suggested_priority,
                })
            }).collect()),
        );
        section(
            prompt,
            "completion_reports_awaiting_review",
            self.completion.as_ref().map(|rows| {
                rows.iter()
                    .map(|row| {
                        serde_json::json!({
                            "task_id": row.task_id,
                            "project": row.project_title.as_deref().map(|text| one_line(text, 100)),
                            "title": one_line(&row.title, 200),
                            "assignee": row.assignee_name.as_deref().map(|text| one_line(text, 80)),
                        })
                    })
                    .collect()
            }),
        );
    }

    /// Explicit read-only priority reviews use the same queue as Home, so old
    /// conversation answers cannot accidentally substitute registered work.
    pub(crate) fn priority_answer(
        &self,
        input: &str,
        history: &[ConversationMessage],
        projects: &[Project],
    ) -> Option<String> {
        let normalized = normalized_text(input);
        let previous = history
            .iter()
            .rfind(|item| item.role == ConversationMessageRole::User);
        let follows_ranking =
            previous.is_some_and(|message| ranks_new_requests(&normalized_text(&message.content)));
        let corrects_to_unassigned = ["배정이안된", "배정안된", "미배정", "아직배정"]
            .iter()
            .any(|word| normalized.contains(word));
        let correction =
            follows_ranking && corrects_to_unassigned && mentions_new_requests(&normalized);
        if !(ranks_new_requests(&normalized) || correction)
            || has_extra_scope_or_change(&normalized)
        {
            return None;
        }
        let matches = |text| {
            let text = normalized_text(text);
            projects
                .iter()
                .filter(move |project| {
                    let name = normalized_text(&project.title);
                    !name.is_empty() && text.contains(&name)
                })
                .collect::<Vec<_>>()
        };
        let mut scope = matches(input);
        if scope.is_empty() && correction {
            scope = previous.map_or_else(Vec::new, |message| matches(message.content.as_str()));
        }
        if scope.is_empty()
            && normalized.contains("프로젝트에서")
            && !["전체프로젝트", "모든프로젝트"]
                .iter()
                .any(|word| normalized.contains(word))
        {
            return None;
        }
        let project = match scope.as_slice() {
            [] => None,
            [project] => Some(*project),
            _ => return None,
        };
        let scope_name = project.map_or_else(
            || "전체 프로젝트".to_owned(),
            |item| safe_label(&item.title),
        );
        let Some(chat) = &self.chat else {
            return Some(
                "새 업무 요청을 불러오지 못했어요. 잠시 후 이 대화에서 다시 물어봐 주세요."
                    .to_owned(),
            );
        };
        Some(render_priority_answer(
            chat,
            project.map(|item| item.id),
            &scope_name,
        ))
    }
}

fn render_priority_answer(chat: &[Incoming], project_id: Option<Uuid>, scope_name: &str) -> String {
    let matching = chat
        .iter()
        .filter(|row| project_id.is_none_or(|id| id == row.project_id));
    let pending_count = matching
        .clone()
        .filter(|row| row.state == "analysis_pending")
        .count();
    let mut ranked = matching
        .filter(|row| row.state == "new_unassigned")
        .collect::<Vec<_>>();
    ranked.sort_by_key(|row| {
        (
            Reverse(row.priority),
            row.due_at.unwrap_or(i64::MAX),
            Reverse(row.received_at),
        )
    });
    let mut answer = format!(
        "**{scope_name} · 미배정 새 업무 요청 {}건**\n등록된 일감은 제외했어요.",
        ranked.len()
    );
    if let Some(first) = ranked.first() {
        let _ = write!(
            answer,
            "\n먼저 **{}**부터 배정해 보세요.",
            safe_label(&first.title)
        );
    }
    for priority in (0..=3).rev() {
        let mut rows = ranked
            .iter()
            .take(MAX_ANSWER_ROWS)
            .filter(|row| row.priority == Some(priority))
            .peekable();
        if rows.peek().is_some() {
            let _ = writeln!(answer, "\n\n### {}", priority_name(priority));
            for row in rows {
                let project_suffix = if project_id.is_none() {
                    format!(" · {}", safe_label(&row.project))
                } else {
                    String::new()
                };
                let due = row
                    .due_at
                    .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
                    .map_or_else(String::new, |time| {
                        let local = time.to_offset(
                            time::UtcOffset::from_hms(9, 0, 0).unwrap_or(time::UtcOffset::UTC),
                        );
                        format!(
                            " · {}월 {}일 {:02}:{:02} 마감",
                            local.month() as u8,
                            local.day(),
                            local.hour(),
                            local.minute()
                        )
                    });
                let _ = writeln!(answer, "- {}{project_suffix}{due}", safe_label(&row.title));
            }
        }
    }
    if ranked.len() > MAX_ANSWER_ROWS {
        let _ = write!(
            answer,
            "\n우선순위가 높은 {MAX_ANSWER_ROWS}건을 표시했어요. 나머지 요청은 새 업무 요청 목록에서 확인해 주세요."
        );
    }
    if pending_count > 0 {
        let _ = write!(
            answer,
            "\n대화 내용을 정리 중인 요청 {pending_count}건은 아직 순서에 넣지 않았어요."
        );
    }
    answer
}

impl From<&ProjectInflowCandidate> for Incoming {
    fn from(row: &ProjectInflowCandidate) -> Self {
        let item = &row.representative;
        let analysis = row.analysis.as_ref();
        let current_new = analysis.is_some_and(|value| {
            value.state == InflowAnalysisState::Ready
                && value.analyzed_revision == Some(value.source_revision)
                && value.classification == Some(InflowClassification::NewTask)
                && value.suggested_task_title.is_some()
        });
        let state = if item.promoted_task_id.is_some() {
            "existing_task_update"
        } else if current_new {
            "new_unassigned"
        } else if analysis.is_some_and(|value| {
            matches!(
                value.state,
                InflowAnalysisState::Queued
                    | InflowAnalysisState::Claimed
                    | InflowAnalysisState::Running
            )
        }) {
            "analysis_pending"
        } else {
            "needs_review"
        };
        Self {
            project_id: item.project_id,
            project: one_line(&item.project_name, 100),
            source: one_line(&item.source_name, 100),
            title: analysis
                .and_then(|value| {
                    value
                        .suggested_task_title
                        .as_deref()
                        .or(value.summary.as_deref())
                })
                .map_or_else(
                    || "업무 내용 확인 필요".to_owned(),
                    |value| one_line(value, 200),
                ),
            summary: analysis
                .and_then(|value| value.summary.as_deref())
                .map(|value| one_line(value, 500)),
            state,
            priority: row.suggested_priority(),
            suggested_assignee: analysis
                .and_then(|value| value.suggested_assignee_name.as_deref())
                .map(|value| one_line(value, 80)),
            due_at: analysis
                .and_then(|value| value.suggested_due_at)
                .map(OffsetDateTime::unix_timestamp),
            received_at: item.received_at.unix_timestamp(),
            task_id: item.promoted_task_id,
        }
    }
}

fn section<T: Serialize>(prompt: &mut String, name: &str, rows: Option<Vec<T>>) {
    let _ = writeln!(prompt, "<{name}>");
    if let Some(rows) = rows {
        let _ = writeln!(
            prompt,
            "visible_count_at_least={} listed={}",
            rows.len(),
            rows.len().min(MAX_ROWS)
        );
        for row in rows.iter().take(MAX_ROWS) {
            if let Ok(line) = serde_json::to_string(row) {
                // Treat provider/user text as data even when it contains an XML-like delimiter.
                let _ = writeln!(
                    prompt,
                    "{}",
                    line.replace('<', "\\u003c").replace('>', "\\u003e")
                );
            }
        }
    } else {
        prompt.push_str("unavailable (not an empty queue)\n");
    }
    let _ = writeln!(prompt, "</{name}>");
}

fn normalized_text(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn mentions_new_requests(text: &str) -> bool {
    [
        "새업무요청",
        "신규업무요청",
        "들어온업무",
        "새로들어온업무",
        "새업무",
    ]
    .iter()
    .any(|word| text.contains(word))
}

fn ranks_new_requests(text: &str) -> bool {
    mentions_new_requests(text)
        && (text.contains("먼저배정")
            || (text.contains("우선순위")
                && ["분류", "정리", "보여", "알려", "나눠", "비교"]
                    .iter()
                    .any(|word| text.contains(word))))
}

fn has_extra_scope_or_change(text: &str) -> bool {
    [
        "배정해",
        "배정하라",
        "배정해줘",
        "등록해",
        "바꿔",
        "변경해",
        "올려",
        "낮춰",
        "조정해",
        "수정해",
        "보내",
        "전송",
        "삭제",
        "완료해",
        "업무아님",
        "메일",
        "오늘",
        "내일",
        "어제",
        "이번주",
        "건만",
        "개만",
        "빼고",
        "제외",
        "긴급만",
        "버그만",
    ]
    .iter()
    .any(|word| text.contains(word))
}

fn one_line(text: &str, max: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

fn safe_label(text: &str) -> String {
    one_line(text, 140)
        .chars()
        .filter(|ch| !matches!(ch, '[' | ']' | '<' | '>' | '*' | '`'))
        .collect()
}

const fn priority_name(value: i16) -> &'static str {
    match value {
        3 => "가장 먼저",
        2 => "중요",
        1 => "보통",
        _ => "낮음",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jimin_storage::{
        agent::ConversationMessageStatus,
        work::{ProjectManagementMode, ProjectStatus},
    };

    fn project(title: &str) -> Project {
        Project {
            id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            title: title.to_owned(),
            objective: None,
            status: ProjectStatus::Active,
            management_mode: ProjectManagementMode::Operation,
            reporting_enabled: true,
            stale_threshold_days: 7,
            risk_level: 0,
            next_action: None,
            due_at: None,
            open_task_count: 0,
            total_task_count: 0,
            completed_task_count: 0,
            overdue_task_count: 0,
            unassigned_task_count: 0,
            progress_percent: 0,
            weekly_created_task_count: 0,
            weekly_completed_task_count: 0,
            backlog_delta: 0,
            stale_task_count: 0,
            average_cycle_time_hours: 0,
            on_time_completion_percent: None,
            version: 1,
        }
    }

    fn row(project: &Project, title: &str, priority: Option<i16>, state: &'static str) -> Incoming {
        Incoming {
            project_id: project.id,
            project: project.title.clone(),
            source: "Google Chat".into(),
            title: title.into(),
            summary: None,
            state,
            priority,
            // A suggested name is not a recorded assignment.
            suggested_assignee: Some("예시 담당자".into()),
            due_at: None,
            received_at: 1,
            task_id: (state == "existing_task_update").then(Uuid::now_v7),
        }
    }

    #[test]
    fn latest_production_pattern_ranks_unregistered_requests_and_keeps_project_on_correction() {
        let requested = project("예시 결제");
        let other = project("다른 회사");
        let inbox = WorkInbox {
            chat: Some(vec![
                row(&requested, "신규 기능 개선", Some(1), "new_unassigned"),
                row(&other, "다른 회사 오류", Some(3), "new_unassigned"),
                row(&requested, "결제 오류 수정", Some(2), "new_unassigned"),
                row(
                    &requested,
                    "이미 배정된 일감",
                    Some(3),
                    "existing_task_update",
                ),
                row(&requested, "분석 중인 요청", None, "analysis_pending"),
            ]),
            ..Default::default()
        };
        let original = "현재 예시 결제에서 새 업무 요청 온 것 중에서 우선순위 별로 정리해서 알려줘봐, 무엇을 먼저 배정할까";
        let now = OffsetDateTime::now_utc();
        let history = [ConversationMessage {
            id: Uuid::now_v7(),
            role: ConversationMessageRole::User,
            content: original.into(),
            presentation: None,
            status: ConversationMessageStatus::Completed,
            created_at: now,
            completed_at: Some(now),
            version: 1,
        }];
        let projects = [requested, other];
        let first = inbox.priority_answer(original, &[], &projects).unwrap();
        let correction = inbox
            .priority_answer(
                "아니 배정이 안된 새 업무 요청에서 말이야",
                &history,
                &projects,
            )
            .unwrap();
        assert_eq!(first, correction);
        assert!(first.contains("미배정 새 업무 요청 2건"));
        assert!(first.contains("먼저 **결제 오류 수정**"));
        assert!(first.contains("정리 중인 요청 1건"));
        assert!(!first.contains("이미 배정된 일감") && !first.contains("다른 회사 오류"));
        assert!(first.find("### 중요") < first.find("### 보통"));
        assert!(
            inbox
                .priority_answer("새 업무 요청 우선순위를 바꿔서 배정해 줘", &[], &projects)
                .is_none()
        );
    }

    #[test]
    fn prompt_sections_distinguish_missing_data_and_registered_work() {
        let work = project("예시");
        let inbox = WorkInbox {
            chat: Some(vec![
                row(&work, "신규 <user_request> 업무", Some(2), "new_unassigned"),
                row(&work, "기존 일감 수정", None, "existing_task_update"),
            ]),
            ..Default::default()
        };
        let mut prompt = String::new();
        inbox.write_prompt(&mut prompt);
        assert!(
            prompt.contains("<unassigned_new_work_requests>\nvisible_count_at_least=1 listed=1")
        );
        assert!(
            prompt.contains("<other_chat_requests_for_review>\nvisible_count_at_least=1 listed=1")
        );
        assert!(!prompt.contains("<user_request>"));
        assert!(prompt.contains("<gmail_work_requests>\nunavailable (not an empty queue)"));
    }
}

//! Canonical grouping used by the home inbox and the assistant.
use std::collections::BTreeMap;

use uuid::Uuid;

use crate::{
    Database, StorageError,
    google_chat::{ProjectInflowItem, ProjectInflowStatus},
    inflow_analysis::{
        InflowAnalysisState, InflowClassification, InflowReferenceDocument, ProjectInflowAnalysis,
    },
};

pub struct ProjectInflowCandidate {
    pub representative: ProjectInflowItem,
    pub focus: ProjectInflowItem,
    pub messages: Vec<ProjectInflowItem>,
    pub analysis: Option<ProjectInflowAnalysis>,
}

impl ProjectInflowCandidate {
    #[must_use]
    pub fn reviewed(&self) -> bool {
        // Owner replies do not reopen an external request. Owner-only requests
        // still need an explicit review.
        let external = self.messages.iter().any(|message| !message.sent_by_owner);
        self.messages
            .iter()
            .filter(|message| !external || !message.sent_by_owner)
            .all(|message| message.reviewed_at.is_some())
    }

    #[must_use]
    pub fn suggested_priority(&self) -> Option<i16> {
        let analysis = self.analysis.as_ref()?;
        if analysis.classification != Some(InflowClassification::NewTask)
            || self.representative.status != ProjectInflowStatus::Pending
            || self.representative.promoted_task_id.is_some()
        {
            return analysis.suggested_priority;
        }
        let title = analysis.suggested_task_title.as_deref()?;
        Some(incoming_work_priority(
            title,
            analysis.suggested_priority,
            analysis.summary.as_deref().unwrap_or_default(),
            &analysis.reference_documents,
        ))
    }
}

impl Database {
    /// Loads the owner-scoped unread projection also used by Home.
    ///
    /// # Errors
    /// Returns a storage error if the queue or its analyses cannot be read.
    pub async fn project_inflow_queue_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ProjectInflowCandidate>, StorageError> {
        let items = self.pending_project_inflow_for_user(user_id).await?;
        let ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
        let analyses = self
            .project_inflow_analyses_for_items(user_id, &ids)
            .await?;
        Ok(group_project_inflow_candidates(items, analyses))
    }
}

/// Policy for a suggested value, not for a registered or manually edited task.
/// Explicit deferral takes precedence over inferred urgency.
#[must_use]
pub fn incoming_work_priority(
    title: &str,
    proposed: Option<i16>,
    summary: &str,
    references: &[InflowReferenceDocument],
) -> i16 {
    let mut itsm_tickets = references
        .iter()
        .filter(|doc| doc.provider == "itsm" && doc.error_code.is_none())
        .filter_map(|doc| doc.original_content.as_deref())
        .peekable();
    // A low-priority related ticket alone must not downgrade a multi-ticket request.
    let deferred_tickets =
        itsm_tickets.peek().is_some() && itsm_tickets.all(itsm_ticket_is_deferred);
    if deferred_tickets || explicitly_deferred(title) || explicitly_deferred(summary) {
        return 0;
    }

    let title = title.to_lowercase();
    let improvement = [
        "개선",
        "추가",
        "신규",
        "고도화",
        "enhancement",
        "feature request",
    ];
    let incident = [
        "버그",
        "오류",
        "작동 안",
        "동작 안",
        "먹통",
        "실패",
        "장애",
        "crash",
        "regression",
        "not working",
    ];
    let urgent = [
        "긴급",
        "즉시",
        "서비스 중단",
        "업무 불가",
        "결제 불가",
        "데이터 유실",
        "outage",
        "asap",
    ];
    let proposed = proposed.filter(|value| (0..=3).contains(value));
    if proposed == Some(3) || urgent.iter().any(|term| title.contains(term)) {
        3
    } else if improvement.iter().any(|term| title.contains(term))
        && !["오류 수정", "버그 수정", "장애 복구", "crash fix"]
            .iter()
            .any(|term| title.contains(term))
    {
        1
    } else if incident.iter().any(|term| title.contains(term)) {
        2
    } else {
        proposed.unwrap_or(1)
    }
}

fn itsm_ticket_is_deferred(content: &str) -> bool {
    // The ITSM snapshot renders metadata first, then separate sections for
    // description, attachments and journals. A quoted field in a comment is not metadata.
    let mut sections = content.split("\n\n");
    let Some(header) = sections.next() else {
        return false;
    };
    if !header.starts_with("ITSM #") {
        return false;
    }
    let mut in_description = false;
    for section in sections {
        if let Some(description) = section.strip_prefix("원문 설명\n") {
            in_description = true;
            if explicitly_deferred(description) {
                return true;
            }
            continue;
        }
        if section.starts_with("첨부 파일\n") || section.starts_with("원문 업데이트\n") {
            break;
        }
        if in_description {
            if explicitly_deferred(section) {
                return true;
            }
        } else if section.strip_prefix("우선순위:").is_some_and(|priority| {
            matches!(
                priority.trim().to_ascii_lowercase().as_str(),
                "후순위" | "낮음" | "low"
            )
        }) {
            return true;
        }
    }
    false
}

fn explicitly_deferred(text: &str) -> bool {
    text.split(['\n', '.', '!', '?', ';']).any(|sentence| {
        let normalized = sentence
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        matches!(
            normalized.trim_matches(['-', '*', '(', ')', '[', ']']),
            "후순위"
                | "후순위로진행"
                | "후순위로진행해주세요"
                | "후순위로진행부탁드립니다"
                | "후순위로진행합니다"
                | "후순위로진행하는요청입니다"
                | "우선순위:후순위"
        )
    })
}

pub fn group_project_inflow_candidates(
    items: Vec<ProjectInflowItem>,
    analysis_rows: Vec<ProjectInflowAnalysis>,
) -> Vec<ProjectInflowCandidate> {
    let mut analyses_by_conversation = BTreeMap::new();
    let mut analyses_by_representative = BTreeMap::new();
    for analysis in analysis_rows {
        analyses_by_representative.insert(analysis.representative_item_id, analysis.clone());
        analyses_by_conversation.insert(
            (analysis.source_id, analysis.conversation_key.clone()),
            analysis,
        );
    }
    let mut groups = BTreeMap::<(Uuid, String, &str, Option<Uuid>), Vec<_>>::new();
    for item in items {
        let group = item.provider_thread_name.clone().map_or_else(
            || format!("message:{}", item.id),
            |thread| format!("thread:{thread}"),
        );
        let status = match item.status {
            ProjectInflowStatus::Pending => "pending",
            ProjectInflowStatus::Promoted => "promoted",
            ProjectInflowStatus::Dismissed => "dismissed",
        };
        groups
            .entry((item.source_id, group, status, item.promoted_task_id))
            .or_default()
            .push(item);
    }

    let mut candidates = groups
        .into_iter()
        .filter_map(|((source_id, conversation_key, _, _), mut messages)| {
            messages.sort_by_key(|item| (item.received_at, item.id));
            let focus = messages
                .iter()
                .rfind(|item| !item.sent_by_owner)
                .cloned()
                .or_else(|| messages.last().cloned())?;
            let representative = messages.last()?.clone();
            let analysis = analyses_by_conversation
                .get(&(source_id, conversation_key))
                .cloned()
                .or_else(|| analyses_by_representative.get(&representative.id).cloned());
            let linked_attention_is_visible = representative.promoted_task_id.is_some()
                && !focus.sent_by_owner
                && focus.sender_provider_name.as_deref() != Some("users/app")
                && analysis.as_ref().is_some_and(|analysis| {
                    matches!(
                        analysis.classification,
                        Some(InflowClassification::FollowUp | InflowClassification::Question)
                    )
                });
            if representative.status == ProjectInflowStatus::Pending
                && analysis.as_ref().is_some_and(|analysis| {
                    analysis.state == InflowAnalysisState::Ready
                        && analysis.classification != Some(InflowClassification::NewTask)
                        && !linked_attention_is_visible
                        && (representative.promoted_task_id.is_some()
                            || focus.sender_provider_name.as_deref() == Some("users/app"))
                })
            {
                return None;
            }
            Some(ProjectInflowCandidate {
                representative,
                focus,
                messages,
                analysis,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.representative.received_at));
    candidates
}

#[cfg(test)]
mod tests {
    use super::{InflowReferenceDocument, incoming_work_priority};

    #[test]
    fn suggests_important_for_defects_normal_for_improvements_and_preserves_urgent() {
        assert_eq!(
            incoming_work_priority("결제 화면 오류 수정", Some(1), "", &[]),
            2
        );
        assert_eq!(
            incoming_work_priority("오류 알림 개선", Some(2), "", &[]),
            1
        );
        assert_eq!(
            incoming_work_priority("정산 화면 필터 추가", None, "", &[]),
            1
        );
        assert_eq!(
            incoming_work_priority("긴급: 앱 로그인 불가", Some(1), "", &[]),
            3
        );
        assert_eq!(incoming_work_priority("일반 개선", Some(3), "", &[]), 3);
    }

    fn ticket(id: &str, content: &str) -> InflowReferenceDocument {
        InflowReferenceDocument {
            provider: "itsm".to_owned(),
            url: format!("https://itsm.example.test/issues/{id}"),
            external_id: id.to_owned(),
            title: None,
            original_content: Some(format!("ITSM #{id}\n제목: 정산 오류\n\n{content}")),
            error_code: None,
        }
    }

    #[test]
    fn explicit_deferral_in_ticket_metadata_or_description_beats_model_urgency() {
        for content in [
            "우선순위: 후순위\n\n원문 설명\n정산 오류 수정",
            "우선순위: 보통\n\n원문 설명\n정산 오류입니다.\n\n후순위로진행",
        ] {
            assert_eq!(
                incoming_work_priority("정산 오류", Some(3), "", &[ticket("42", content)]),
                0
            );
        }
        assert_eq!(
            incoming_work_priority(
                "정산 오류",
                Some(3),
                "정산 오류입니다. 후순위로 진행해 주세요.",
                &[]
            ),
            0
        );
    }

    #[test]
    fn negation_comments_failed_references_and_mixed_tickets_do_not_defer_urgency() {
        let mut failed = ticket("42", "우선순위: 후순위");
        failed.error_code = Some("itsm.unavailable".to_owned());
        for refs in [
            vec![failed],
            vec![ticket(
                "42",
                "원문 설명\n후순위로 진행하지 않습니다.\n\n원문 업데이트\n우선순위: 후순위",
            )],
            vec![
                ticket("42", "우선순위: 후순위"),
                ticket("43", "우선순위: 긴급"),
            ],
        ] {
            assert_eq!(
                incoming_work_priority(
                    "긴급: 정산 오류",
                    Some(3),
                    "후순위로 진행하지 않습니다.",
                    &refs
                ),
                3
            );
        }
    }
}

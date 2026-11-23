//! Canonical grouping used by the home inbox and the assistant.
use std::collections::BTreeMap;

use uuid::Uuid;

use crate::{
    Database, StorageError,
    google_chat::{ProjectInflowItem, ProjectInflowStatus},
    inflow_analysis::{InflowAnalysisState, InflowClassification, ProjectInflowAnalysis},
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
            || self.representative.promoted_task_id.is_some()
        {
            return analysis.suggested_priority;
        }
        let title = analysis.suggested_task_title.as_deref()?;
        Some(incoming_work_priority(title, analysis.suggested_priority))
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
/// A recorded urgent recommendation is preserved.
#[must_use]
pub fn incoming_work_priority(title: &str, proposed: Option<i16>) -> i16 {
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
    use super::incoming_work_priority;

    #[test]
    fn suggests_important_for_defects_normal_for_improvements_and_preserves_urgent() {
        assert_eq!(incoming_work_priority("결제 화면 오류 수정", Some(1)), 2);
        assert_eq!(incoming_work_priority("오류 알림 개선", Some(2)), 1);
        assert_eq!(incoming_work_priority("정산 화면 필터 추가", None), 1);
        assert_eq!(incoming_work_priority("긴급: 앱 로그인 불가", Some(1)), 3);
        assert_eq!(incoming_work_priority("일반 개선", Some(3)), 3);
    }
}

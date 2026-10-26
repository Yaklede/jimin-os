use super::{group_project_inflow_candidates, project_inflow_item_response};
use jimin_storage::{
    google_chat::{ProjectInflowItem, ProjectInflowStatus},
    inflow_analysis::{InflowAnalysisState, InflowClassification, ProjectInflowAnalysis},
};
use time::OffsetDateTime;
use uuid::Uuid;

fn owner_request(
    classification: InflowClassification,
) -> (ProjectInflowItem, ProjectInflowAnalysis) {
    let new_task = classification == InflowClassification::NewTask;
    let item = ProjectInflowItem {
        id: Uuid::now_v7(),
        project_id: Uuid::now_v7(),
        project_name: "회귀 테스트 프로젝트".to_owned(),
        source_id: Uuid::now_v7(),
        source_name: "업무 요청".to_owned(),
        provider_thread_name: Some("spaces/test/threads/owner-request".to_owned()),
        sender_provider_name: Some("users/100".to_owned()),
        sender_name: None,
        sent_by_owner: true,
        content_text: "토큰에 인증 범위를 추가하고 위변조를 차단해 주세요.".to_owned(),
        received_at: OffsetDateTime::UNIX_EPOCH,
        status: ProjectInflowStatus::Pending,
        promoted_task_id: None,
        acknowledged_at: Some(OffsetDateTime::UNIX_EPOCH),
        reviewed_at: None,
        dismissal_reason: None,
        dismissal_reply_status: None,
        completion_requested_at: None,
        completion_reaction_at: None,
        completion_reply_at: None,
        completion_delivery_error_code: None,
        completion_delivery_attempt_count: 0,
        version: 1,
    };
    let analysis = ProjectInflowAnalysis {
        id: Uuid::now_v7(),
        project_id: item.project_id,
        source_id: item.source_id,
        conversation_key: "thread:spaces/test/threads/owner-request".to_owned(),
        representative_item_id: item.id,
        source_revision: 1,
        analyzed_revision: Some(1),
        state: InflowAnalysisState::Ready,
        classification: Some(classification),
        confidence: Some(99),
        summary: Some("토큰의 인증 범위를 추가하고 위변조를 차단한다.".to_owned()),
        suggested_task_title: new_task.then(|| "토큰 인증 범위 추가 및 위변조 차단".to_owned()),
        suggested_action_items: if new_task {
            vec!["인증 범위 검증을 적용한다.".to_owned()]
        } else {
            Vec::new()
        },
        suggested_completion_criteria: new_task
            .then(|| "위변조된 인증 정보가 차단된다.".to_owned()),
        suggested_assignee_name: None,
        suggested_due_at: None,
        suggested_priority: new_task.then_some(1),
        reference_documents: Vec::new(),
        linked_task_id: None,
        error_code: None,
        version: 1,
    };
    (item, analysis)
}

#[test]
fn owner_new_task_is_unread_without_any_external_reply() {
    let (item, analysis) = owner_request(InflowClassification::NewTask);
    let candidates = group_project_inflow_candidates(vec![item.clone()], vec![analysis]);
    assert_eq!(candidates.len(), 1);
    let response = project_inflow_item_response(candidates.into_iter().next().unwrap()).unwrap();
    assert_eq!(response.id, item.id);
    assert_eq!(
        response.analysis_classification.as_deref(),
        Some("new_task")
    );
    assert!(response.sent_by_owner);
    assert!(!response.reviewed, "posting a request is not reviewing it");
    assert_eq!(response.message_count, 1);
}

#[test]
fn owner_first_analysis_is_visible_and_unread() {
    let (item, _) = owner_request(InflowClassification::NewTask);
    let candidates = group_project_inflow_candidates(vec![item], vec![]);
    let response = project_inflow_item_response(candidates.into_iter().next().unwrap()).unwrap();
    assert_eq!(response.analysis_status, "queued");
    assert!(!response.reviewed);
}

#[test]
fn owner_uncertain_request_remains_available_for_review() {
    for classification in [
        InflowClassification::Question,
        InflowClassification::FollowUp,
        InflowClassification::StatusUpdate,
        InflowClassification::Noise,
        InflowClassification::Duplicate,
    ] {
        let (item, analysis) = owner_request(classification);
        let candidates = group_project_inflow_candidates(vec![item], vec![analysis]);
        assert_eq!(
            candidates.len(),
            1,
            "owner request was lost: {classification:?}"
        );
        let response =
            project_inflow_item_response(candidates.into_iter().next().unwrap()).unwrap();
        assert!(!response.reviewed);
    }
}

#[test]
fn owner_explicit_review_is_preserved_and_new_owner_message_is_unread() {
    let (mut item, mut analysis) = owner_request(InflowClassification::NewTask);
    item.reviewed_at = Some(item.received_at);
    let candidates = group_project_inflow_candidates(vec![item.clone()], vec![analysis.clone()]);
    let response = project_inflow_item_response(candidates.into_iter().next().unwrap()).unwrap();
    assert!(response.reviewed);

    let mut next = item.clone();
    next.id = Uuid::now_v7();
    next.received_at += time::Duration::seconds(1);
    next.reviewed_at = None;
    analysis.representative_item_id = next.id;
    analysis.source_revision = 2;
    analysis.analyzed_revision = Some(2);
    let candidates = group_project_inflow_candidates(vec![item, next], vec![analysis]);
    let response = project_inflow_item_response(candidates.into_iter().next().unwrap()).unwrap();
    assert!(!response.reviewed);
}

#[test]
fn owner_existing_task_follow_up_is_not_reintroduced_as_new_work() {
    for classification in [
        InflowClassification::FollowUp,
        InflowClassification::Question,
    ] {
        let (mut item, mut analysis) = owner_request(classification);
        let task_id = Uuid::now_v7();
        item.promoted_task_id = Some(task_id);
        analysis.linked_task_id = Some(task_id);
        assert!(group_project_inflow_candidates(vec![item], vec![analysis]).is_empty());
    }
}

#[test]
fn owner_reply_does_not_reopen_a_reviewed_external_request() {
    let (mut external, mut analysis) = owner_request(InflowClassification::NewTask);
    external.sent_by_owner = false;
    external.sender_provider_name = Some("users/200".to_owned());
    external.reviewed_at = Some(external.received_at);
    let mut reply = external.clone();
    reply.id = Uuid::now_v7();
    reply.sent_by_owner = true;
    reply.received_at += time::Duration::seconds(1);
    reply.reviewed_at = None;
    analysis.representative_item_id = reply.id;
    let candidates = group_project_inflow_candidates(vec![external.clone(), reply], vec![analysis]);
    let response = project_inflow_item_response(candidates.into_iter().next().unwrap()).unwrap();
    assert!(response.reviewed);
    assert_eq!(response.id, external.id);
}

#[test]
fn app_non_task_message_is_still_filtered() {
    let (mut item, analysis) = owner_request(InflowClassification::StatusUpdate);
    item.sent_by_owner = false;
    item.sender_provider_name = Some("users/app".to_owned());
    assert!(group_project_inflow_candidates(vec![item], vec![analysis]).is_empty());
}

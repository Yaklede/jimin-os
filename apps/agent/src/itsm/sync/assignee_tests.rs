use super::{
    ItsmClient, Value, json,
    tests::{catalog, delivery, server},
};
use jimin_storage::itsm_sync::ItsmSyncDelivery;
use std::cell::Cell;

fn issue(date: Option<&str>, status: u64, assignee: Option<u64>) -> (u16, String) {
    let mut value = json!({"issue":{"id":42,"project":{"id":17},"status":{"id":status},"due_date":date,
        "allowed_statuses":[{"id":7}]}});
    if let Some(id) = assignee {
        let name = if id == 81 {
            "조 지민"
        } else {
            "이전 담당자"
        };
        value["issue"]["assigned_to"] = json!({"id":id,"name":name});
    }
    (200, value.to_string())
}

fn member(id: u64, name: &str) -> Value {
    json!({"id":id+100,"project":{"id":17},"user":{"id":id,"name":name},"roles":[{"id":4}]})
}

fn page(members: &[Value], offset: usize, limit: usize, total_count: usize) -> (u16, String) {
    (
        200,
        json!({"memberships":members,"offset":offset,"limit":limit,"total_count":total_count})
            .to_string(),
    )
}

fn role(assignable: bool) -> (u16, String) {
    (
        200,
        json!({"role":{"id":4,"assignable":assignable}}).to_string(),
    )
}

fn updates(requests: &[String]) -> Vec<Value> {
    requests
        .iter()
        .filter(|request| request.starts_with("PUT /issues/42.json "))
        .map(|request| serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap())
        .collect()
}

fn job(client: &ItsmClient, complete: bool) -> ItsmSyncDelivery {
    let mut job = delivery(client, complete);
    job.assignee_names = vec!["조지민".into()];
    job
}

#[tokio::test]
async fn sends_assignee_id_for_a_spaced_redmine_name() {
    let date = Some("2026-10-21");
    let (client, server) = server(vec![
        issue(None, 1, Some(80)),
        page(&[member(82, "다른 사람")], 0, 1, 2),
        page(&[member(81, "조\u{00A0}지민")], 1, 1, 2),
        role(true),
        issue(None, 1, Some(80)),
        (204, String::new()),
        issue(None, 1, Some(81)),
        (200, catalog()),
        (204, String::new()),
        issue(date, 7, Some(81)),
        // Retry: all remote changes already applied. No role query or PUT required.
        issue(date, 7, Some(81)),
        page(&[member(81, "조 지민")], 0, 100, 1),
        (200, catalog()),
    ])
    .await;
    let mut job = job(&client, true);
    job.assignee_names.push("조 지민".into()); // Shared issue, same person.
    for _ in 0..2 {
        client
            .sync_ticket(&job, || async { Ok(true) })
            .await
            .unwrap();
    }
    let requests = server.await.unwrap();
    assert!(requests[1].starts_with("GET /projects/17/memberships.json?limit=100&offset=0 "));
    assert!(requests[2].starts_with("GET /projects/17/memberships.json?limit=100&offset=1 "));
    assert!(requests[3].starts_with("GET /roles/4.json "));
    assert_eq!(
        updates(&requests),
        vec![
            json!({"issue":{"assigned_to_id":81}}),
            json!({"issue":{"due_date":"2026-10-21","status_id":7}}),
        ]
    );
}

#[tokio::test]
async fn ambiguous_conflicting_or_unassignable_names_do_not_block_deadline() {
    let group =
        json!({"id":91,"project":{"id":17},"group":{"id":85,"name":"조 지민"},"roles":[{"id":4}]});
    for (names, directory, code) in [
        (
            vec!["조지민"],
            vec![
                page(&[member(81, "조지민")], 0, 1, 2),
                page(&[member(82, "조 지민")], 1, 1, 2),
            ],
            "itsm.assignee_ambiguous",
        ),
        (
            vec!["조지민"],
            vec![page(&[group], 0, 100, 1)],
            "itsm.assignee_not_found",
        ),
        (
            vec!["조지민"],
            vec![page(&[member(81, "조 지민")], 0, 100, 1), role(false)],
            "itsm.assignee_not_assignable",
        ),
        (vec!["조지민, 김경주"], vec![], "itsm.assignee_conflict"),
        (
            vec!["조지민", "다른 사람"],
            vec![],
            "itsm.assignee_conflict",
        ),
    ] {
        let mut responses = vec![issue(None, 1, None)];
        responses.extend(directory);
        responses.extend([(204, String::new()), issue(Some("2026-10-21"), 1, None)]);
        let (client, server) = server(responses).await;
        let mut job = job(&client, false);
        job.assignee_names = names.into_iter().map(str::to_owned).collect();
        assert_eq!(
            client
                .sync_ticket(&job, || async { Ok(true) })
                .await
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(
            updates(&server.await.unwrap()),
            vec![json!({"issue":{"due_date":"2026-10-21"}})]
        );
    }
}

#[tokio::test]
async fn ignored_or_rejected_assignment_still_updates_deadline_and_reports_failure() {
    for (status, expected) in [
        (204, "itsm.assignee_verification_failed"),
        (422, "itsm.workflow_blocked"),
    ] {
        let mut responses = vec![
            issue(None, 1, None),
            page(&[member(81, "조 지민")], 0, 100, 1),
            role(true),
            issue(None, 1, None),
            (status, String::new()),
        ];
        if status == 204 {
            responses.push(issue(None, 1, None));
        }
        responses.extend([(204, String::new()), issue(Some("2026-10-21"), 1, None)]);
        let (client, server) = server(responses).await;
        assert_eq!(
            client
                .sync_ticket(&job(&client, false), || async { Ok(true) })
                .await
                .unwrap_err()
                .code,
            expected
        );
        assert_eq!(
            updates(&server.await.unwrap()),
            vec![
                json!({"issue":{"assigned_to_id":81}}),
                json!({"issue":{"due_date":"2026-10-21"}}),
            ]
        );
    }
}

#[tokio::test]
async fn superseded_assignee_during_lookup_sends_no_update() {
    let (client, server) = server(vec![
        issue(None, 1, None),
        page(&[member(81, "조 지민")], 0, 100, 1),
        role(true),
        issue(None, 1, None),
    ])
    .await;
    let checks = Cell::new(0);
    let error = client
        .sync_ticket(&job(&client, false), || {
            let first = checks.get() == 0;
            checks.set(checks.get() + 1);
            async move { Ok(first) }
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "itsm.superseded");
    assert!(updates(&server.await.unwrap()).is_empty());
}

#[tokio::test]
async fn incomplete_or_wrong_project_directory_never_assigns() {
    let mut foreign = member(81, "조 지민");
    foreign["project"]["id"] = json!(18);
    for responses in [
        vec![page(&[foreign], 0, 100, 1)],
        vec![page(&[member(81, "조 지민")], 0, 1, 2), page(&[], 1, 1, 2)],
    ] {
        let date = Some("2026-10-21");
        let mut sequence = vec![issue(date, 1, Some(80))];
        sequence.extend(responses);
        let (client, server) = server(sequence).await;
        let error = client
            .sync_ticket(&job(&client, false), || async { Ok(true) })
            .await
            .unwrap_err();
        assert_eq!(error.code, "itsm.invalid_response");
        assert!(updates(&server.await.unwrap()).is_empty());
    }
}

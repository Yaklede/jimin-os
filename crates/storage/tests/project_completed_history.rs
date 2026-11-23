use jimin_storage::{
    Database,
    planning::{PROJECT_HISTORY_PAGE_SIZE, ProjectHistoryQuery},
};
use secrecy::SecretString;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

fn first_page() -> ProjectHistoryQuery {
    ProjectHistoryQuery {
        page: 1,
        title: None,
        issue_id: None,
        assignee: None,
        completed_from: None,
        completed_before: None,
        anchor_task_id: None,
    }
}

async fn history_fixture() -> (Database, Uuid, Uuid, Uuid, Vec<Uuid>) {
    let url = std::env::var("JIMIN_TEST_DATABASE_URL").expect("isolated database required");
    let db = Database::connect_lazy(&SecretString::from(url.clone()), 4, Duration::from_secs(3))
        .unwrap();
    db.migrate().await.unwrap();
    let pool = PgPool::connect(&url).await.unwrap();
    let owner = Uuid::now_v7();
    let stranger = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let project = Uuid::now_v7();
    let other_project = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,google_sub,email,normalized_email,status,last_login_at) VALUES($1,'owner-history','owner-history@example.test','owner-history@example.test','active',NOW()), ($2,'other-history','other-history@example.test','other-history@example.test','active',NOW())")
        .bind(owner).bind(stranger).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO workspaces(id,user_id,scope,name) VALUES($1,$2,'company','예시 회사')",
    )
    .bind(workspace)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO projects(id,user_id,workspace_id,title) VALUES($1,$2,$3,'검수 프로젝트'),($4,$2,$3,'별도 프로젝트')")
        .bind(project).bind(owner).bind(workspace).bind(other_project).execute(&pool).await.unwrap();
    let ids = (0..390).map(|_| Uuid::now_v7()).collect::<Vec<_>>();
    sqlx::query("INSERT INTO tasks(id,user_id,project_id,title,notes,assignee_name,status,priority,completed_at)
        SELECT task_id,$2,$3,CASE WHEN n=390 THEN '오래된 정산 점검' WHEN n=390-1 THEN '#43900 구분' ELSE '검수 '||n END,
        CASE WHEN n=388 THEN 'https://itsm.example/issues/5500?view=all' END,
        CASE WHEN n%2=0 THEN '김경주, 송천안' ELSE '송천안' END,
        'completed',CASE WHEN n=390 THEN 3 ELSE 1 END,
        TIMESTAMPTZ '2026-11-23 15:00:00+00' - (n-1)*INTERVAL '1 day'
        FROM unnest($1::uuid[]) WITH ORDINALITY AS items(task_id,n)")
        .bind(&ids).bind(owner).bind(project).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO tasks(id,user_id,project_id,title,status,completed_at)
        VALUES($1,$2,$3,'아직 하는 일','open',NULL),($4,$2,$5,'별도 완료','completed',NOW())",
    )
    .bind(Uuid::now_v7())
    .bind(owner)
    .bind(project)
    .bind(Uuid::now_v7())
    .bind(other_project)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task_itsm_links(task_id,user_id,enabled,issue_id,issue_url)
        VALUES($1,$2,true,4390,'https://itsm.example/issues/4390')",
    )
    .bind(ids[389])
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();
    (db, owner, stranger, project, ids)
}

#[tokio::test]
#[ignore = "requires an empty isolated JIMIN_TEST_DATABASE_URL"]
async fn completed_pages_filter_all_records_and_preserve_direct_links() {
    let (db, owner, stranger, project, ids) = history_fixture().await;
    let first = db
        .project_completed_history(owner, project, &first_page())
        .await
        .unwrap();
    assert_eq!(
        (first.page, first.total_count, first.filtered_count),
        (1, 390, 390)
    );
    assert_eq!(
        first.tasks.len(),
        usize::try_from(PROJECT_HISTORY_PAGE_SIZE).unwrap()
    );
    assert_eq!(first.tasks[0].id, ids[0]);
    assert_eq!(first.tasks[29].id, ids[29]);
    assert_eq!(
        db.open_tasks_for_project(owner, project)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(first.assignees.contains(&"김경주".to_owned()));
    assert!(
        db.project_completed_history(stranger, project, &first_page())
            .await
            .is_err()
    );

    let mut query = ProjectHistoryQuery {
        page: 200,
        ..first_page()
    };
    let last = db
        .project_completed_history(owner, project, &query)
        .await
        .unwrap();
    assert_eq!(last.page, 13);
    assert_eq!(last.tasks[29].id, ids[389]);
    query.page = 1;
    query.anchor_task_id = Some(ids[389]);
    assert_eq!(
        db.project_completed_history(owner, project, &query)
            .await
            .unwrap()
            .page,
        13
    );
    query.anchor_task_id = None;
    query.title = Some("오래된 정산".to_owned());
    query.assignee = Some("김경주".to_owned());
    let matched = db
        .project_completed_history(owner, project, &query)
        .await
        .unwrap();
    assert_eq!(
        (
            matched.total_count,
            matched.filtered_count,
            matched.tasks[0].id
        ),
        (390, 1, ids[389])
    );
    query.title = None;
    query.assignee = None;
    query.issue_id = Some(4390);
    let linked = db
        .project_completed_history(owner, project, &query)
        .await
        .unwrap();
    assert_eq!(linked.filtered_count, 1);
    assert_eq!(linked.tasks[0].id, ids[389]);
    query.issue_id = Some(5500);
    assert_eq!(
        db.project_completed_history(owner, project, &query)
            .await
            .unwrap()
            .tasks[0]
            .id,
        ids[387]
    );
    query.issue_id = None;
    // At 15:00 UTC the Seoul date changes; the exclusive end excludes the next day.
    let day = first.tasks[0].completed_at.unwrap();
    query.completed_from = Some(day);
    query.completed_before = Some(day + time::Duration::days(1));
    assert_eq!(
        db.project_completed_history(owner, project, &query)
            .await
            .unwrap()
            .filtered_count,
        1
    );
}

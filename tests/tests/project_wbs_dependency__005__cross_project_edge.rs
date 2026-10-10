//! PROJECT.WBS.DEPENDENCY — cross-project edge rejection (TST-PROJECT-WBS-DEPENDENCY-005).
//!
//! Contract: a dependency edge must connect two items belonging to the same
//! project. The foreign key constraints and the service validation both enforce this.
//!
//! Level: L2 Persistence — exercise the same boundary production uses. Use only
//! an isolated disposable Postgres/Neon test target; assert committed truth and
//! rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_wbs_dependency__005__cross_project_edge

use db::WbsDao;
use model::WbsDependency;
use test_harness::database::TestDatabase;
use uuid::Uuid;

async fn create_test_project(db: &TestDatabase, name: &str) -> String {
    let project_id = Uuid::new_v4().to_string();
    // wbs_dependency FK references project(id), not wbs_project(id)
    sqlx::query(
        r#"
        insert into project (id, name, status, created_at, updated_at)
        values ($1::uuid, $2, 'open', now(), now())
        "#,
    )
    .bind(&project_id)
    .bind(name)
    .execute(db.database().pool())
    .await
    .expect("create project");
    project_id
}

async fn create_test_item(db: &TestDatabase, project_id: &str, title: &str) -> String {
    let item_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        insert into wbs_item (id, project_id, title, category, status, created_at, updated_at)
        values ($1::uuid, $2::uuid, $3, 'task', 'open', now(), now())
        "#,
    )
    .bind(&item_id)
    .bind(project_id)
    .bind(title)
    .execute(db.database().pool())
    .await
    .expect("create wbs item");
    item_id
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): uses disposable Postgres; production is refused"]
async fn project_wbs_dependency_005__cross_project_edge() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database");

    // Create two separate projects
    let project_a = create_test_project(&test_db, "Project A").await;
    let project_b = create_test_project(&test_db, "Project B").await;

    // Create items in each project
    let item_a = create_test_item(&test_db, &project_a, "Item in A").await;
    let item_b = create_test_item(&test_db, &project_b, "Item in B").await;

    let dao = WbsDao::new(test_db.database().clone());

    // Attempt to create a dependency from item in Project A to item in Project B
    // The service layer validates that both items belong to the same project
    // The DAO will fail due to FK constraint: (project_id, source_id) and (project_id, target_id)
    // must both reference wbs_item(project_id, id)

    let cross_edge = WbsDependency {
        project_id: project_a.clone(), // Claims project A
        source_id: item_a.clone(),     // Item in project A - OK
        target_id: item_b.clone(),     // Item in project B - violates FK
        kind: "finish_to_start".into(),
    };

    // The DAO insert should fail due to foreign key constraint
    let result = dao.insert_dependency(&cross_edge).await;
    assert!(
        result.is_err(),
        "cross-project dependency must fail at database level"
    );
    let error = result.unwrap_err();
    let error_str = error.to_string().to_lowercase();
    assert!(
        error_str.contains("foreign key")
            || error_str.contains("fk")
            || error_str.contains("constraint"),
        "error must indicate foreign key violation: {}",
        error
    );

    // Same test with project_id = project_b but source in project_a
    let cross_edge2 = WbsDependency {
        project_id: project_b.clone(),
        source_id: item_a, // Item in project A - violates FK for project B
        target_id: item_b, // Item in project B - OK
        kind: "finish_to_start".into(),
    };

    let result = dao.insert_dependency(&cross_edge2).await;
    assert!(
        result.is_err(),
        "cross-project dependency (v2) must fail at database level"
    );

    // Valid edge within same project must succeed
    let item_a2 = create_test_item(&test_db, &project_a, "Item A2").await;
    let valid_edge = WbsDependency {
        project_id: project_a.clone(),
        source_id: item_a2.clone(),
        target_id: create_test_item(&test_db, &project_a, "Item A3").await,
        kind: "finish_to_start".into(),
    };
    let result = dao.insert_dependency(&valid_edge).await;
    assert!(result.is_ok(), "same-project dependency must succeed");

    let edges = dao.list_dependencies(&project_a).await.expect("list deps");
    assert_eq!(edges.len(), 1, "only the valid dependency must exist");
}

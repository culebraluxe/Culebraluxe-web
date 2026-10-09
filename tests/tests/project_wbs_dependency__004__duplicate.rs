//! PROJECT.WBS.DEPENDENCY — duplicate edge rejection (TST-PROJECT-WBS-DEPENDENCY-004).
//!
//! Contract: inserting the same dependency edge twice is rejected. The composite
//! primary key on (source_id, target_id) in wbs_dependency enforces this at the
//! database level, and the service layer returns a business error for the duplicate.
//!
//! Level: L2 Persistence — exercise the same boundary production uses. Use only
//! an isolated disposable Postgres/Neon test target; assert committed truth and
//! rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_wbs_dependency__004__duplicate

use db::WbsDao;
use model::WbsDependency;
use test_harness::database::TestDatabase;
use uuid::Uuid;

async fn create_test_project(db: &TestDatabase) -> String {
    let project_id = Uuid::new_v4().to_string();
    // wbs_dependency FK references project(id), not wbs_project(id)
    sqlx::query(
        r#"
        insert into project (id, name, status, created_at, updated_at)
        values ($1::uuid, 'Test Project', 'open', now(), now())
        "#,
    )
    .bind(&project_id)
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
async fn project_wbs_dependency_004__duplicate() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database");

    let project_id = create_test_project(&test_db).await;

    let item_a = create_test_item(&test_db, &project_id, "Item A").await;
    let item_b = create_test_item(&test_db, &project_id, "Item B").await;

    let dao = WbsDao::new(test_db.database().clone());

    let edge = WbsDependency {
        project_id: project_id.clone(),
        source_id: item_a.clone(),
        target_id: item_b.clone(),
        kind: "finish_to_start".into(),
    };

    // First insert succeeds
    let result = dao.insert_dependency(&edge).await;
    assert!(result.is_ok(), "first insert of dependency must succeed");

    // Second insert of the SAME edge must fail (composite PK on source_id, target_id)
    let result = dao.insert_dependency(&edge).await;
    assert!(result.is_err(), "duplicate dependency insert must fail");
    let error = result.unwrap_err();
    let error_str = error.to_string().to_lowercase();
    assert!(
        error_str.contains("duplicate")
            || error_str.contains("unique")
            || error_str.contains("conflict"),
        "error must indicate duplicate/unique constraint violation: {}",
        error
    );

    // Verify only one edge exists
    let edges = dao.list_dependencies(&project_id).await.expect("list deps");
    assert_eq!(edges.len(), 1, "exactly one dependency must exist");

    // Negative case: different edge between same items in reverse direction is NOT a duplicate
    // (different source/target)
    let reverse_edge = WbsDependency {
        project_id: project_id.clone(),
        source_id: item_b,
        target_id: item_a,
        kind: "finish_to_start".into(),
    };
    let result = dao.insert_dependency(&reverse_edge).await;
    assert!(
        result.is_ok(),
        "reverse edge is a different dependency and must succeed"
    );

    let edges = dao.list_dependencies(&project_id).await.expect("list deps");
    assert_eq!(
        edges.len(),
        2,
        "both directions must exist as separate edges"
    );
}

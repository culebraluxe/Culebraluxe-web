//! PROJECT.WBS.DEPENDENCY — delete dependency (TST-PROJECT-WBS-DEPENDENCY-006).
//!
//! Contract: a dependency edge can be deleted, and the deletion is visible to
//! subsequent reads. The service layer validates the edge exists before deletion.
//!
//! Level: L2 Persistence — exercise the same boundary production uses. Use only
//! an isolated disposable Postgres/Neon test target; assert committed truth and
//! rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_wbs_dependency__006__delete

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
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): uses disposable Postgres; production is refused"]
async fn project_wbs_dependency_006__delete() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database");

    let project_id = create_test_project(&test_db).await;

    let item_a = create_test_item(&test_db, &project_id, "Item A").await;
    let item_b = create_test_item(&test_db, &project_id, "Item B").await;
    let item_c = create_test_item(&test_db, &project_id, "Item C").await;

    let dao = WbsDao::new(test_db.database().clone());

    // Create two dependencies
    let edge_ab = WbsDependency {
        project_id: project_id.clone(),
        source_id: item_a.clone(),
        target_id: item_b.clone(),
        kind: "finish_to_start".into(),
    };
    let edge_bc = WbsDependency {
        project_id: project_id.clone(),
        source_id: item_b.clone(),
        target_id: item_c.clone(),
        kind: "finish_to_start".into(),
    };

    dao.insert_dependency(&edge_ab).await.expect("insert A->B");
    dao.insert_dependency(&edge_bc).await.expect("insert B->C");

    let edges = dao.list_dependencies(&project_id).await.expect("list deps");
    assert_eq!(edges.len(), 2, "two dependencies must exist");

    // Delete one dependency
    let deleted = dao
        .delete_dependency(&project_id, &item_a, &item_b)
        .await
        .expect("delete dependency");
    assert!(deleted, "delete must return true for existing edge");

    // Verify only the remaining edge exists
    let edges = dao
        .list_dependencies(&project_id)
        .await
        .expect("list deps after delete");
    assert_eq!(edges.len(), 1, "one dependency must remain");
    assert_eq!(edges[0].source_id, item_b);
    assert_eq!(edges[0].target_id, item_c);

    // Deleting a non-existent dependency returns false
    let deleted = dao
        .delete_dependency(&project_id, &item_a, &item_b)
        .await
        .expect("delete non-existent");
    assert!(!deleted, "delete non-existent must return false");

    // Negative case: deleting with wrong project_id returns false
    let wrong_project = Uuid::new_v4().to_string();
    let deleted = dao
        .delete_dependency(&wrong_project, &item_b, &item_c)
        .await
        .expect("delete with wrong project");
    assert!(!deleted, "delete with wrong project_id must return false");

    // The remaining edge should still exist
    let edges = dao
        .list_dependencies(&project_id)
        .await
        .expect("list deps final");
    assert_eq!(edges.len(), 1, "remaining edge must still exist");
}

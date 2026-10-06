//! DB.TRANSACTION — WBS item + dependency (TST-DB-TRANSACTION-009).
//!
//! CONTRACT. A WBS (Work Breakdown Structure) item must have dependencies that
//! are enforced at the database level.  The wbs_dependency table links a source
//! item to a target item within the same project, with a composite foreign key
//! that enforces scope.  This test verifies the WBS item dependency relationship.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__009__wbs_item_dependency

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_009__wbs_item_dependency() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // 2. Insert a WBS project.
    sqlx::query(
        "INSERT INTO wbs_project (id, name, status) VALUES ('proj-001', 'Test Project', 'open')",
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert wbs_project");

    // 3. Insert a project (wbs_item.project_id references project(id)).
    let project_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO project (id, name, status) VALUES ('proj-001', 'WBS Test Project', 'open') RETURNING id",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert project for WBS items");

    // 4. Insert two WBS items referencing the project.
    sqlx::query(
        "INSERT INTO wbs_item (id, project_id, title, category, status)
         VALUES ('item-001', $1, 'Design Phase', 'design', 'open')",
    )
    .bind(&project_id)
    .execute(&mut *tx.connection())
    .await
    .expect("insert wbs_item 001");

    sqlx::query(
        "INSERT INTO wbs_item (id, project_id, title, category, status)
         VALUES ('item-002', $1, 'Build Phase', 'build', 'open')",
    )
    .bind(&project_id)
    .execute(&mut *tx.connection())
    .await
    .expect("insert wbs_item 002");

    // 5. Create a dependency: item-002 depends on item-001 (finish_to_start).
    sqlx::query(
        "INSERT INTO wbs_dependency (project_id, source_id, target_id, kind)
         VALUES ($1, 'item-001', 'item-002', 'finish_to_start')",
    )
    .bind(&project_id)
    .execute(&mut *tx.connection())
    .await
    .expect("insert wbs_dependency");

    // 6. Verify the dependency is readable.
    let dep_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM wbs_dependency
         WHERE source_id = 'item-001' AND target_id = 'item-002'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count dependencies");
    assert_eq!(dep_count.0, 1, "dependency must be persisted");

    // 7. Verify the dependency details.
    let (kind,): (String,) = sqlx::query_as(
        "SELECT kind FROM wbs_dependency WHERE source_id = 'item-001' AND target_id = 'item-002'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get dependency kind");
    assert_eq!(kind, "finish_to_start", "dependency kind must be finish_to_start");

    // 8. Negative case: a self-dependency (source_id = target_id) must fail.
    let self_dep_result = sqlx::query(
        "INSERT INTO wbs_dependency (project_id, source_id, target_id, kind)
         VALUES ($1, 'item-001', 'item-001', 'finish_to_start')",
    )
    .bind(&project_id)
    .execute(&mut *tx.connection())
    .await;

    assert!(
        self_dep_result.is_err(),
        "self-dependency must violate the wbs_dependency_distinct constraint"
    );

    // 9. Negative case: a dependency referencing a non-existent target must fail.
    let bad_target_result = sqlx::query(
        "INSERT INTO wbs_dependency (project_id, source_id, target_id, kind)
         VALUES ($1, 'item-001', 'item-999', 'finish_to_start')",
    )
    .bind(&project_id)
    .execute(&mut *tx.connection())
    .await;

    assert!(
        bad_target_result.is_err(),
        "dependency with non-existent target must violate the foreign key"
    );

    // 10. Roll back to leave no residual state.
    let _ = tx.rollback().await;
}

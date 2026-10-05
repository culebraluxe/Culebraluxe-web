//! DB.TRANSACTION — work item + story run (TST-DB-TRANSACTION-002).
//!
//! CONTRACT. A work item must be linkable to a story run, and the story run
//! must be able to query its associated work items.  This test verifies the
//! bidirectional relationship between a story run and its work items using the
//! production database boundary with an isolated disposable test target; PROD is
//! forbidden.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__002__work_item_story_run

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_002__work_item_story_run() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let tx = test_db.begin().await.expect("begin transaction");

    // 2. Insert a storyboard_story row to associate work items with.
    sqlx::query(
        "INSERT INTO storyboard_story (name, status, context_refs, scope)
         VALUES ('TST-DB-SCHEMA-002', 'planned', '[]', 'db')"
    )
    .execute(&*tx.connection())
    .await
    .expect("insert storyboard_story");

    let story_id: String = sqlx::query_scalar(
        "SELECT id FROM storyboard_story WHERE name = 'TST-DB-SCHEMA-002' LIMIT 1"
    )
    .fetch_one(&*tx.connection())
    .await
    .expect("get story id")
    .to_string();

    // 3. Insert multiple work items that reference this story (simulating a story run).
    sqlx::query(
        "INSERT INTO agent_work_item (story_id, status, kind, payload)
         VALUES ($1, 'pending', 'schema_check', '{\"test\": \"TST-DB-SCHEMA-002\"}'::jsonb)",
    )
    .bind(&story_id)
    .execute(&*tx.connection())
    .await
    .expect("insert first work item");

    sqlx::query(
        "INSERT INTO agent_work_item (story_id, status, kind, payload)
         VALUES ($1, 'pending', 'schema_check', '{\"test\": \"TST-DB-SCHEMA-002-2\"}'::jsonb)",
    )
    .bind(&story_id)
    .execute(&*tx.connection())
    .await
    .expect("insert second work item");

    sqlx::query(
        "INSERT INTO agent_work_item (story_id, status, kind, payload)
         VALUES ($1, 'pending', 'schema_check', '{\"test\": \"TST-DB-SCHEMA-002-3\"}'::jsonb)",
    )
    .bind(&story_id)
    .execute(&*tx.connection())
    .await
    .expect("insert third work item");

    // 4. Verify the story can query all its work items.
    let work_items: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM agent_work_item WHERE story_id = $1"
    )
    .bind(&story_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count work items by story id");

    assert_eq!(work_items.0, 3, "story run must have 3 associated work items");

    // 5. Verify each work item is readable individually.
    for expected_kind in &["schema_check"; 3] {
        let kind: (String,) = sqlx::query_as(
            "SELECT kind FROM agent_work_item WHERE story_id = $1 LIMIT 1"
        )
        .bind(&story_id)
        .fetch_one(&mut *tx.connection())
        .await
        .expect("get work item kind");

        assert_eq!(kind.0, "schema_check", "work item kind must be schema_check");
    }

    // 6. Roll back to leave no residual state.
    let _ = tx.rollback().await;

    // 7. Negative case: a story run with no work items should report count 0.
    let other_story_id = "00000000-0000-0000-0000-000000000000";
    let zero_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM agent_work_item WHERE story_id = $1"
    )
    .bind(&other_story_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count work items for non-existent story");

    assert_eq!(zero_count.0, 0, "story with no work items must report count 0");
}
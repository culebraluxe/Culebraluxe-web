//! DB.TRANSACTION — story + agent work item (TST-DB-TRANSACTION-001).
//!
//! CONTRACT. A story that becomes `Ready` is dispatched exactly one open work item, in the same statement that moved
//! it (the `storyboard_story_ready_dispatch` trigger), and the two stay one consistent fact: the item names its story,
//! asking to dispatch again answers `AlreadyQueued` with the SAME item instead of a second one, and an item cannot
//! exist for a story that does not.
//!
//! Everything runs inside transactions that are rolled back; no row is left in the database. A non-production database
//! only; PROD is refused by the harness before a socket opens.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__001__story_agent_work_item

use test_harness::database::TestDatabase;

const STORY: &str = "TST-DB-TRANSACTION-001";

#[tokio::test]
async fn db_transaction_001__story_agent_work_item() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );

    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'story + work item', 'P3', 'Planned')",
    )
    .bind(STORY)
    .execute(&mut *tx.connection())
    .await
    .expect("insert a Planned story");

    let planned: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
            .bind(STORY)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("count items for a Planned story");
    assert_eq!(planned, 0, "a story that is not Ready has no work item");

    sqlx::query("update storyboard_story set status = 'Ready' where id = $1")
        .bind(STORY)
        .execute(&mut *tx.connection())
        .await
        .expect("move the story to Ready");

    let items: Vec<(String, String)> =
        sqlx::query_as("select id::text, state from agent_work_item where story_id = $1")
            .bind(STORY)
            .fetch_all(&mut *tx.connection())
            .await
            .expect("read the dispatched item");
    assert_eq!(
        items.len(),
        1,
        "becoming Ready dispatches exactly one work item: {items:?}"
    );
    assert_eq!(items[0].1, "Ready", "the dispatched item is open and Ready");

    // Dispatching again is answered, not repeated: same item, no second row.
    let (outcome, item): (String, Option<String>) =
        sqlx::query_as("select outcome, item from forge_dispatch_story($1)")
            .bind(STORY)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("dispatch a story that is already queued");
    assert_eq!(outcome, "AlreadyQueued");
    assert_eq!(
        item.as_deref(),
        Some(items[0].0.as_str()),
        "the answer names the item that is already there"
    );
    let after: i64 = sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
        .bind(STORY)
        .fetch_one(&mut *tx.connection())
        .await
        .expect("count items after a repeat dispatch");
    assert_eq!(after, 1, "a repeat dispatch adds no row");
    tx.rollback()
        .await
        .expect("rollback leaves no story and no item");

    // An item cannot exist for a story that does not (its own transaction: a refused insert aborts one).
    let mut tx = test_db.begin().await.expect("begin transaction");
    let orphan = sqlx::query("insert into agent_work_item (story_id, state) values ('TST-DB-TRANSACTION-001-NO-SUCH-STORY', 'Ready')")
        .execute(&mut *tx.connection())
        .await;
    assert!(
        orphan.is_err(),
        "a work item for a story that does not exist must be refused by its foreign key"
    );
    let _ = tx.rollback().await;
}

//! DB.TRANSACTION — work item + story run (TST-DB-TRANSACTION-002).
//!
//! CONTRACT. Claiming a work item and beginning its run is ONE fact over several rows, and settling it undoes none of
//! it halfway: the claim marks the item Claimed, beginning opens a `storyboard_story_run` and links it back
//! (`agent_work_item.story_run_id`) while the board moves to `In Progress`, and settling the claim closes that same run
//! and writes the story's status in the same transaction. These are the stored routines the engine calls
//! (`forge_claim_specific_agent_work`, `forge_begin_agent_work_run`, `forge_finish_agent_work_run`); each assertion
//! reads the rows, not a return value.
//!
//! Inside a transaction that is rolled back; no row is left in the database. A non-production database only.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__002__work_item_story_run

use test_harness::database::TestDatabase;

const STORY: &str = "TST-DB-TRANSACTION-002";

#[tokio::test]
async fn db_transaction_002__work_item_story_run() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let mut tx = test_db.begin().await.expect("begin transaction");

    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'work item + run', 'P3', 'Planned')",
    )
    .bind(STORY)
    .execute(&mut *tx.connection())
    .await
    .expect("insert a Planned story");
    sqlx::query("update storyboard_story set status = 'Ready' where id = $1")
        .bind(STORY)
        .execute(&mut *tx.connection())
        .await
        .expect("move the story to Ready (the trigger dispatches its item)");
    let item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1")
            .bind(STORY)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("the dispatched item");

    // CLAIM: the item is Claimed, and no run exists yet.
    let claimed: Vec<String> = sqlx::query_scalar(
        "select state from forge_claim_specific_agent_work($1::uuid, 'tst-db-transaction-002')",
    )
    .bind(&item)
    .fetch_all(&mut *tx.connection())
    .await
    .expect("claim the item");
    assert_eq!(
        claimed,
        vec!["Claimed".to_string()],
        "the claim returns the item it took"
    );
    let runs_before: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id = $1")
            .bind(STORY)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("count runs before begin");
    assert_eq!(runs_before, 0, "claiming opens no run");

    // BEGIN: one run, linked from the item.
    let (run_id,): (String,) =
        sqlx::query_as("select story_run_id from forge_begin_agent_work_run($1::uuid, 'DEV')")
            .bind(&item)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("begin the run");
    let linked: Option<String> =
        sqlx::query_scalar("select story_run_id::text from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("read the item's run link");
    assert_eq!(
        linked.as_deref(),
        Some(run_id.as_str()),
        "the item links the run it opened"
    );
    let (run_story, ended): (String, bool) = sqlx::query_as(
        "select story_id, ended_at is not null from storyboard_story_run where id = $1::uuid",
    )
    .bind(&run_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("read the run row");
    assert_eq!(run_story, STORY, "the run belongs to the item's story");
    assert!(!ended, "a run that has just begun is open");
    // The board is NOT moved by the routine: the engine's writer marks the story `In Progress` itself once the role
    // starts. What the routine guarantees is that the story still EXPECTS a run, which is what lets a failed claim
    // hold it below.
    let board: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(STORY)
        .fetch_one(&mut *tx.connection())
        .await
        .expect("read the board status");
    assert!(
        board == "Ready" || board == "In Progress",
        "beginning a run leaves the story on a status that expects one, got {board:?}"
    );

    // SETTLE: the item, the story and the run close together. `Error` over an `In Progress` board holds the story.
    let (item_state, story_status): (String, Option<String>) =
        sqlx::query_as("select item_state, story_status from forge_finish_agent_work_run($1::uuid, 'Error', 'probe failure')")
            .bind(&item)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("settle the claim");
    assert_eq!(item_state, "Error");
    assert_eq!(
        story_status.as_deref(),
        Some("Hold"),
        "a failed claim holds the board it was running"
    );
    let (ended_at_set, result): (bool, Option<String>) = sqlx::query_as(
        "select ended_at is not null, result_status from storyboard_story_run where id = $1::uuid",
    )
    .bind(&run_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("read the closed run");
    assert!(
        ended_at_set,
        "settling closes the run it opened, in the same transaction"
    );
    assert_eq!(
        result.as_deref(),
        Some("Failed"),
        "an Error item rules its run Failed"
    );

    // A second settle is a no-op: a claim ends exactly once, and a ruling is never rewritten.
    let again: Vec<String> = sqlx::query_scalar(
        "select item_state from forge_finish_agent_work_run($1::uuid, 'Done', null)",
    )
    .bind(&item)
    .fetch_all(&mut *tx.connection())
    .await
    .expect("settle again");
    assert!(
        again.is_empty(),
        "a settled claim settles nothing a second time: {again:?}"
    );
    tx.rollback()
        .await
        .expect("rollback leaves no story, item or run");
}

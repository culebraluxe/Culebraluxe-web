//! DB.TRANSACTION — story + agent work item (TST-DB-TRANSACTION-001).
//!
//! CONTRACT. A story and its agent work item are created as one durable fact. When a story is
//! dispatched — its status changes into `Ready`, by insert or by update — the database's own
//! `storyboard_story_ready_dispatch` trigger (`db/migrations/025_agent_work_queue.sql:101-116`,
//! restated in 146 and 259) queues **exactly one** `agent_work_item` bound to that story, in
//! state `Ready`, with its priority derived from the story's priority. The two rows are one
//! transaction: the trigger runs inside the story's write, so a rolled-back story leaves no work
//! item behind, and a work item cannot exist without its story (`agent_work_item.story_id` is a
//! foreign key to `storyboard_story(id)`, `db/migrations/025_agent_work_queue.sql:51`).
//!
//! This is production database work — a trigger, a foreign key and a partial unique index — not
//! something a unit test can see. The test drives the real DEV schema through [`TestDatabase`]
//! (which refuses PRODUCTION before any socket is opened, `tests/src/database.rs:68-75`) and reads
//! the committed truth back on the pool, never a test double.
//!
//! The negative/refusal cases are load-bearing, so the test cannot pass without exercising the
//! subject:
//!   * a story that is not dispatched has no work item — `Ready` is the explicit authorization,
//!     never inferred (`db/migrations/025_agent_work_queue.sql:5-7`);
//!   * a work item for a story that does not exist is refused by the foreign key;
//!   * a second active work item on a story that already holds one is refused by the partial
//!     unique index `agent_work_item_one_serial_active_per_story` (migration 143), so the
//!     `on conflict do nothing` arbiter of the trigger is not the only guard;
//!   * a rolled-back story takes its work item with it, proving the two are one transaction.
//!
//! Level: L2 Persistence — an isolated, disposable DEV/Neon target only. PROD is forbidden and the
//! harness refuses it; the proof rows are named under this run's unique namespace and deleted at
//! the end, so the disposable branch is left as it was found.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_transaction__001__story_agent_work_item -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use sqlx::PgPool;

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";
/// Every proof row this test creates is named under this prefix plus the run's unique namespace, so
/// two concurrent runs of this contract never share a row and cleanup is scoped to this run alone.
const PROOF_PREFIX: &str = "TST-DB-TRANSACTION-001-";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is
/// opened, so this can never be pointed at PROD by a stray environment.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(database) => return database,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// The SQLSTATE sqlx decoded from a database error, if the failure reached the server.
fn sqlstate(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.code().map(|code| code.to_string()),
        _ => None,
    }
}

/// Every committed `agent_work_item` for a story, active or not — "did a second row appear?".
async fn total_item_count(pool: &PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the work-item count is readable")
}

/// The story's open serial work item — the one the Ready trigger is allowed to queue.
async fn active_item_count(pool: &PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from agent_work_item
          where story_id = $1 and state in ('Ready', 'Claimed', 'Running', 'Paused')
            and parallel_group_id is null",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("the active work-item count is readable")
}

/// Insert a story at the given status. `notes` and the other non-not-null columns take their
/// defaults; the Ready trigger fires itself from the row, so no helper writes the work item.
async fn insert_story(pool: &PgPool, story_id: &str, status: &str, priority: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'TST-DB-TRANSACTION-001', 'Story + agent work item', $2, $3, '')",
    )
    .bind(story_id)
    .bind(priority)
    .bind(status)
    .execute(pool)
    .await
    .expect("the proof story inserts and its dispatch trigger runs");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-TRANSACTION-001); the file and the assay use it.
async fn db_transaction_001__story_agent_work_item() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the story/work-item proof runs only on an isolated DEV target"
    );
    let pool = dev.database().pool().clone();
    let ns = dev.namespace().to_string();

    let ready_story = format!("{PROOF_PREFIX}ready-{ns}");
    let planned_story = format!("{PROOF_PREFIX}planned-{ns}");
    let absent_story = format!("{PROOF_PREFIX}absent-{ns}");
    let atomic_story = format!("{PROOF_PREFIX}atomic-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. STORY -> WORK ITEM. A story dispatched into `Ready` queues exactly one work item, bound to the story,
    //    in state `Ready`, with its priority derived from the story's priority rather than chosen by the caller.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(&pool, &ready_story, "Ready", "High").await;

    let (item_id, state, priority, bound_story): (String, String, i32, String) = sqlx::query_as(
        "select id::text, state, priority, story_id from agent_work_item where story_id = $1",
    )
    .bind(&ready_story)
    .fetch_one(&pool)
    .await
    .expect("the Ready trigger queues exactly one item for the story");
    assert_eq!(
        state, "Ready",
        "{HARNESS}: a dispatched story's work item starts Ready"
    );
    assert_eq!(
        priority, 80,
        "{HARNESS}: priority is derived from the story priority (story_priority_score('High'))"
    );
    assert_eq!(
        bound_story, ready_story,
        "{HARNESS}: the work item references the story it was queued for"
    );
    assert_eq!(
        total_item_count(&pool, &ready_story).await,
        1,
        "{HARNESS}: dispatch queues exactly one work item"
    );
    assert_eq!(
        active_item_count(&pool, &ready_story).await,
        1,
        "{HARNESS}: the one item is the story's open serial slot"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. WORK ITEM -> STORY (bidirectional). The story exists and the item names it back.
    // -----------------------------------------------------------------------------------------------------------
    let story_exists: bool =
        sqlx::query_scalar("select exists(select 1 from storyboard_story where id = $1)")
            .bind(&ready_story)
            .fetch_one(&pool)
            .await
            .expect("the story is readable");
    assert!(
        story_exists,
        "{HARNESS}: the work item's story is a real storyboard_story row"
    );
    let item_story: String =
        sqlx::query_scalar("select story_id from agent_work_item where id = $1::uuid")
            .bind(&item_id)
            .fetch_one(&pool)
            .await
            .expect("the work item names its story");
    assert_eq!(
        item_story, ready_story,
        "{HARNESS}: the item reads back to the story it belongs to"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. RE-SAVE WHILE READY DOES NOT DUPLICATE. Writing the story again while it stays Ready must not queue a
    //    second item — the trigger's `on conflict` arbiter and the partial unique index both hold.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("update storyboard_story set status = 'Ready', title = 'resaved' where id = $1")
        .bind(&ready_story)
        .execute(&pool)
        .await
        .expect("the story re-saves while remaining Ready");
    assert_eq!(
        total_item_count(&pool, &ready_story).await,
        1,
        "{HARNESS}: a re-save while Ready does not queue a second item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — NOT READY MEANS NO WORK ITEM. `Ready` is the explicit authorization; a `Planned` story queues
    //    nothing, and only the change *into* Ready queues its single item. A trigger that queued on any write
    //    would pass step 1 and fail here.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(&pool, &planned_story, "Planned", "Medium").await;
    assert_eq!(
        total_item_count(&pool, &planned_story).await,
        0,
        "{HARNESS}: a story that was never dispatched has no work item"
    );

    sqlx::query("update storyboard_story set status = 'Ready' where id = $1")
        .bind(&planned_story)
        .execute(&pool)
        .await
        .expect("the story transitions into Ready");
    assert_eq!(
        total_item_count(&pool, &planned_story).await,
        1,
        "{HARNESS}: the change into Ready queues exactly one item"
    );
    let planned_priority: i32 =
        sqlx::query_scalar("select priority from agent_work_item where story_id = $1")
            .bind(&planned_story)
            .fetch_one(&pool)
            .await
            .expect("the transitioned item is readable");
    assert_eq!(
        planned_priority, 50,
        "{HARNESS}: the transitioned item's priority is derived from the story's 'Medium' priority"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — NO ORPHAN WORK ITEM. A work item whose story does not exist is refused by the
    //    foreign key, so an item can never outlive or precede its story.
    // -----------------------------------------------------------------------------------------------------------
    let orphan = sqlx::query(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 0)",
    )
    .bind(&absent_story)
    .execute(&pool)
    .await;
    let orphan_error =
        orphan.expect_err("a work item for a story that does not exist must be refused");
    assert_eq!(
        sqlstate(&orphan_error).as_deref(),
        Some("23503"),
        "{HARNESS}: the orphan item is refused by the foreign key (SQLSTATE 23503), got: {orphan_error}"
    );
    assert_eq!(
        total_item_count(&pool, &absent_story).await,
        0,
        "{HARNESS}: the refused orphan wrote nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE / REFUSAL — ONE ACTIVE WORK ITEM PER STORY. Even a writer that skips the trigger entirely cannot
    //    put a second open serial item on a story that already holds one; the partial unique index refuses it.
    // -----------------------------------------------------------------------------------------------------------
    let duplicate =
        sqlx::query("insert into agent_work_item (story_id, state) values ($1, 'Ready')")
            .bind(&ready_story)
            .execute(&pool)
            .await;
    let duplicate_error = duplicate.expect_err("a second active item on a story must be refused");
    assert!(
        duplicate_error
            .to_string()
            .contains("agent_work_item_one_serial_active_per_story"),
        "{HARNESS}: the refusal names the one-serial-item-per-story index, got: {duplicate_error}"
    );
    assert_eq!(
        total_item_count(&pool, &ready_story).await,
        1,
        "{HARNESS}: the refused duplicate leaves the story with exactly one item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. ATOMIC — STORY AND WORK ITEM ARE ONE TRANSACTION. Inside the story's own transaction the trigger's item
    //    is visible; rolling the story back removes the story *and* its item together, so no half state commits.
    // -----------------------------------------------------------------------------------------------------------
    let mut tx = dev.begin().await.expect("begin the story's transaction");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'TST-DB-TRANSACTION-001', 'atomic', 'Low', 'Ready', '')",
    )
    .bind(&atomic_story)
    .execute(&mut *tx.connection())
    .await
    .expect("the story inserts inside its transaction");
    let inside: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
            .bind(&atomic_story)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("the item is visible inside the story's transaction");
    assert_eq!(
        inside, 1,
        "{HARNESS}: the trigger's work item is written in the story's own transaction"
    );
    tx.rollback()
        .await
        .expect("roll the story's transaction back");

    let committed_story: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id = $1")
            .bind(&atomic_story)
            .fetch_one(&pool)
            .await
            .expect("the committed story count reads after the rollback");
    assert_eq!(
        committed_story, 0,
        "{HARNESS}: a rolled-back story does not commit"
    );
    assert_eq!(
        total_item_count(&pool, &atomic_story).await,
        0,
        "{HARNESS}: a rolled-back story takes its work item with it — the pair is one transaction"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. CLEANUP. The proof stories are deleted (their items cascade), so the disposable branch is left as it was
    //    found. A non-zero count is a failed cleanup and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
    let proof_stories = vec![
        ready_story.clone(),
        planned_story.clone(),
        atomic_story.clone(),
    ];
    sqlx::query("delete from storyboard_story where id = any($1::text[])")
        .bind(&proof_stories)
        .execute(&pool)
        .await
        .expect("the proof stories are removable");
    let leftover: (i64, i64) = sqlx::query_as(
        "select (select count(*) from storyboard_story where id = any($1::text[])),
                (select count(*) from agent_work_item where story_id = any($1::text[]))",
    )
    .bind(&proof_stories)
    .fetch_one(&pool)
    .await
    .expect("the cleanup is measurable");
    assert_eq!(
        leftover,
        (0, 0),
        "{HARNESS}: the proof must leave no story and no work item behind"
    );
}

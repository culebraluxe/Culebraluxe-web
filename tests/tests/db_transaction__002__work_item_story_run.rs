//! DB.TRANSACTION — work item + Story Run (TST-DB-TRANSACTION-002).
//!
//! CONTRACT. Opening a Story Run is ONE transaction with the work item it belongs to. A live
//! `Claimed` `agent_work_item` and the `storyboard_story_run` row opened for it are written
//! together, and the item is stamped with `story_run_id` and moved `Claimed → Running` in the same
//! statement (`forge_begin_agent_work_run`,
//! `db/migrations/264_forge_agent_work_run.sql:27-70`), driven here through the production
//! `ForgeEngineDao::begin_agent_work_run` (`db/src/forge_engine.rs:299-315`). The pair can therefore
//! never be half-moved: an item `Running` beside a run that does not exist, or a Story Run held by
//! no work item, is exactly what this contract forbids.
//!
//! WHY IT MATTERS. `agent_work_item` "stores NO story specification; the authoritative spec lives on
//! `storyboard_story` and is snapshotted into `storyboard_story_run` when execution begins"
//! (`db/migrations/025_agent_work_queue.sql:8-11`; the FK is `:59`). The port shipped for a week
//! with `story_run_id` null on every claim, so a lane executed with nothing durable behind it
//! (`db/src/forge_engine.rs:70-74`). This test pins the repair: the run is opened where execution
//! begins, the item names it, and the run carries the story's own snapshot.
//!
//! NEGATIVE / REFUSAL. A begin on an item that is not the live `Claimed` claim — a second begin on an
//! item already `Running`, or one that was never claimed — returns no row, opens no second Story Run
//! and commits nothing. An invalid call cannot manufacture the pair or bypass it.
//!
//! GREENFIELD RUST. This is not a port of any TypeScript test and shares no code with a test-only
//! copy of the seam: the real `ForgeEngineDao` is driven and the committed rows are read back. Raw
//! SQL here is fixture setup/teardown and a read-back, never a second implementation of the boundary.
//!
//! Level: L2 Persistence (DatabaseHarness) — an isolated, disposable DEV/Neon target only.
//! [`TestDatabase`] refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`).
//! The proof leaves nothing behind: this run's proof stories are deleted by unique namespace (their
//! items and runs cascade, `db/migrations/023_storyboard_execution_history.sql:74`,
//! `db/migrations/025_agent_work_queue.sql:51`) and a zero-leftover count is asserted.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_transaction__002__work_item_story_run -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";
/// The owner the proof claims under.
const OWNER: &str = "tst-db-transaction-002";
/// The namespace every proof row in this file is named under. Each run appends its own
/// `TestDatabase` namespace, so two concurrent runs of this contract never share a row and cleanup
/// is scoped to this run alone.
const PROOF_PREFIX: &str = "TST-DB-TRANSACTION-002-";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is
/// opened, and the declaration is supplied explicitly rather than read from the environment so a
/// stray `APP_ENV` cannot change which database is targeted.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; TestDatabase refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Put one disposable story on the board at `Ready`. The board's own dispatch trigger
/// (`db/migrations/025_agent_work_queue.sql:101-111`) creates exactly one `Ready` work item; this
/// returns its id — the work item that exists BEFORE any Story Run is opened.
async fn insert_ready_story(pool: &PgPool, story_id: &str, goal: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, scope)
         values ($1, 'PROOF', 'Work item + Story Run', 'High', 'Ready', '', $2, 'db')",
    )
    .bind(story_id)
    .bind(goal)
    .execute(pool)
    .await
    .expect("insert the proof story at Ready");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the board's Ready trigger created exactly one work item")
}

/// Delete this run's proof stories; their work items and Story Runs cascade with the story. Scoped
/// to `namespace` on purpose: a concurrent run of this same contract has its own namespace.
async fn reap_namespace(pool: &PgPool, namespace: &str) -> u64 {
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{namespace}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows")
        .rows_affected()
}

/// The committed shape of a work item: `(state, story_run_id)`. Read on the pool, not the DAO's
/// return value, so the assertion is about committed database truth.
async fn item_row(pool: &PgPool, item_id: &str) -> (String, Option<String>) {
    sqlx::query_as("select state, story_run_id::text from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item is readable")
}

/// The whole durable work-item row a begin writes: `(state, story_run_id, started_at, updated_at)`.
async fn durable_item_row(
    pool: &PgPool,
    item_id: &str,
) -> (String, Option<String>, String, String) {
    sqlx::query_as(
        "select state, story_run_id::text, coalesce(started_at::text, ''), updated_at::text
           from agent_work_item where id=$1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the work item's durable row is readable")
}

/// Every Story Run opened for a story: `(id, environment, run_type, started?, open?, goal_snapshot)`.
async fn run_rows(
    pool: &PgPool,
    story_id: &str,
) -> Vec<(String, String, String, bool, bool, String)> {
    sqlx::query_as(
        "select id::text, coalesce(execution_environment, ''), coalesce(run_type, ''),
                started_at is not null, ended_at is null, coalesce(goal_snapshot, '')
           from storyboard_story_run where story_id=$1 order by created_at",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("the story's runs are readable")
}

/// Every work item bound to one Story Run. The link is only sound if the run is held by exactly the
/// item that opened it and no other.
async fn run_holders(pool: &PgPool, run_id: &str) -> Vec<String> {
    sqlx::query_scalar(
        "select id::text from agent_work_item where story_run_id=$1::uuid order by id",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .expect("the run's holders are readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-TRANSACTION-002); the file and the assay use it.
async fn db_transaction_002__work_item_story_run() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let test_db = connect_dev().await;
    assert_eq!(
        test_db.target(),
        DbTarget::Dev,
        "{HARNESS}: the work-item/Story-Run proof runs only on an isolated DEV target"
    );
    let pool = test_db.database().pool().clone();
    let engine = ForgeEngineDao::new(test_db.database().clone());

    let ns = test_db.namespace().to_string();
    let story = format!("{PROOF_PREFIX}main-{ns}");
    let goal = format!("prove a work item and its Story Run open together ({ns})");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE WORK ITEM COMES FIRST. A `Ready` story has a `Ready` work item and NO Story Run. The pair starts
    //    unopened: a work item with nothing behind it yet, which is the state the begin must resolve.
    // -----------------------------------------------------------------------------------------------------------
    let item = insert_ready_story(&pool, &story, &goal).await;
    let (state, story_run_id) = item_row(&pool, &item).await;
    assert_eq!(state, "Ready", "{HARNESS}: a fresh work item starts Ready");
    assert_eq!(
        story_run_id, None,
        "{HARNESS}: a Ready work item opens no Story Run yet"
    );
    assert!(
        run_rows(&pool, &story).await.is_empty(),
        "{HARNESS}: the story has no Story Run before execution begins"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. A CLAIM ALONE STILL OPENS NO RUN. `Claimed` names an owner and nothing else; the run is opened where
    //    execution begins, not where the claim is taken.
    // -----------------------------------------------------------------------------------------------------------
    let claimed = engine
        .claim_specific_agent_work(&item, OWNER)
        .await
        .unwrap()
        .expect("a Ready item is claimable");
    assert_eq!(
        claimed.state, "Claimed",
        "{HARNESS}: a claim parks the item at Claimed"
    );
    assert!(
        run_rows(&pool, &story).await.is_empty(),
        "{HARNESS}: a claim alone opens no Story Run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE PAIR OPENS TOGETHER. `begin_agent_work_run` (production DAO) opens exactly one `storyboard_story_run`
    //    and stamps the item with it, `Claimed -> Running`, in one statement.
    // -----------------------------------------------------------------------------------------------------------
    let begun = engine
        .begin_agent_work_run(&item)
        .await
        .unwrap()
        .expect("a live Claimed item must open its Story Run");

    // Committed database truth, read back through the pool — not the DAO's return value.
    let (state, story_run_id) = item_row(&pool, &item).await;
    assert_eq!(
        state, "Running",
        "{HARNESS}: beginning the run moves the item Claimed -> Running"
    );
    assert_eq!(
        story_run_id.as_deref(),
        Some(begun.story_run_id.as_str()),
        "{HARNESS}: the work item is stamped with the Story Run it opened"
    );

    let runs = run_rows(&pool, &story).await;
    assert_eq!(
        runs.len(),
        1,
        "{HARNESS}: beginning an execution opens exactly one Story Run"
    );
    let (run_id, environment, run_type, started, open, goal_snapshot) = runs[0].clone();
    assert_eq!(
        run_id, begun.story_run_id,
        "{HARNESS}: the committed run is the run the boundary returned"
    );
    assert_eq!(
        environment, "DEV",
        "{HARNESS}: the run records the target it actually ran on"
    );
    assert_eq!(
        run_type, "dispatch",
        "{HARNESS}: the run_type is read off the item's envelope, not the caller"
    );
    assert!(started, "{HARNESS}: the Story Run carries a start instant");
    assert!(
        open,
        "{HARNESS}: a run that just began is open (ended_at is null)"
    );
    assert_eq!(
        goal_snapshot, goal,
        "{HARNESS}: the run snapshots the story's own specification, not a caller's copy"
    );

    // The link is bidirectional: from the run we find exactly the one item that opened it, and no other work
    // item in the database carries this run id. A run shared with a second item would be a run two workers
    // could drive, which is not "work item + Story Run".
    assert_eq!(
        run_holders(&pool, &run_id).await,
        vec![item.clone()],
        "{HARNESS}: the Story Run is bound to exactly the work item that opened it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3b. COMMITTED TRUTH SURVIVES A ROLLBACK. The `Claimed -> Running` move and the stamp are durable, not a
    //     snapshot the writer's connection happened to hold: a transaction that rewrites the item back to `Ready`
    //     sees its own uncommitted write inside the transaction, and rolling back restores exactly what the
    //     production begin committed. This is the "assert committed database truth and rollback" half of the L2 rule.
    // -----------------------------------------------------------------------------------------------------------
    let probe_item = item.clone();
    let inside_probe = test_db
        .with_rollback(|conn| {
            Box::pin(async move {
                sqlx::query("update agent_work_item set state = 'Ready' where id = $1::uuid")
                    .bind(&probe_item)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx(
                            "test-harness.db_transaction_002.rollback_probe_update",
                            &error,
                        )
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_item)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "test-harness.db_transaction_002.rollback_probe_read",
                                &error,
                            )
                        })?;
                Ok(state)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(
        inside_probe, "Ready",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    let committed_after_rollback: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(&pool)
            .await
            .expect("the committed item is readable after the rollback");
    assert_eq!(
        committed_after_rollback, "Running",
        "{HARNESS}: rollback restored the committed truth the begin wrote"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — A SECOND BEGIN IS REFUSED. The single live `Claimed` moment is spent; any further
    //    begin must return no row, open no second Story Run, and commit nothing. This is the "exactly once" half of
    //    the pair: an invalid call cannot manufacture a second run.
    // -----------------------------------------------------------------------------------------------------------
    let durable_before_second = durable_item_row(&pool, &item).await;
    assert_eq!(
        durable_before_second.0, "Running",
        "{HARNESS}: the begin left the item Running"
    );
    assert!(
        engine.begin_agent_work_run(&item).await.unwrap().is_none(),
        "{HARNESS}: a work item that has left Claimed must never open a second Story Run"
    );
    assert_eq!(
        run_rows(&pool, &story).await.len(),
        1,
        "{HARNESS}: the refused second begin opened no Story Run"
    );
    assert_eq!(
        durable_item_row(&pool, &item).await,
        durable_before_second,
        "{HARNESS}: a refused second begin commits nothing — state, run id and timestamps are unchanged"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — AN ITEM THAT WAS NEVER CLAIMED HAS NO RUN TO OPEN. A `Ready` work item (and an id that does
    //    not exist at all) is refused, so the pair cannot be opened without the live claim that owns it.
    // -----------------------------------------------------------------------------------------------------------
    let unclaimed_story = format!("{PROOF_PREFIX}unclaimed-{ns}");
    let unclaimed_item = insert_ready_story(&pool, &unclaimed_story, "never claimed").await;
    let unclaimed_durable = durable_item_row(&pool, &unclaimed_item).await;
    assert!(
        engine
            .begin_agent_work_run(&unclaimed_item)
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: an unclaimed Ready item must not open a Story Run"
    );
    assert!(
        engine
            .begin_agent_work_run(&uuid::Uuid::new_v4().to_string())
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: an unknown work item is refused, not an error and not a run"
    );
    assert!(
        run_rows(&pool, &unclaimed_story).await.is_empty(),
        "{HARNESS}: neither refusal opened a Story Run"
    );
    assert_eq!(
        durable_item_row(&pool, &unclaimed_item).await,
        unclaimed_durable,
        "{HARNESS}: a refused begin on an unclaimed item commits nothing — state, run id and timestamps are unchanged"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / ROLLBACK. This run's proof stories are deleted; the work items and Story Runs are gone with
    //    them, so DEV is left as it was found. A non-zero count is a failed rollback and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
    reap_namespace(&pool, &ns).await;
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    let leftover_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_items, 0,
        "{HARNESS}: the proof must leave no work item behind"
    );
    let leftover_runs: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_runs, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

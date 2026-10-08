//! FORGE.DISPATCH — an explicit transition to Ready dispatches exactly once (TST-FORGE-DISPATCH-002).
//!
//! Contract: when a story's status is explicitly changed to `Ready` (not merely found at `Ready` by a sweep),
//! the database's dispatch trigger (`agent_work_item_dispatch()`, migration 025/146) fires exactly once and
//! creates exactly one serial work item (`parallel_group_id IS NULL`). A second explicit transition to `Ready`
//! on the same story must not duplicate the dispatch — the unique index `agent_work_item_one_serial_active_per_story`
//! (migration 143) ensures at most one serial item exists, and `forge_dispatch_story` returns `AlreadyQueued`
//! when the story already holds a live serial slot.
//!
//! The subject is the real dispatch path, not a re-declaration of it. `ForgeHarness` wraps the production
//! `ForgeEngineDao` on an isolated, disposable DEV database, the production `ensure_story_dispatched` is used to
//! trigger the dispatch, and every assertion is read back on the pool the DAO committed to.
//!
//! Level: L2 Persistence, harness `ForgeHarness`. The boundary rule is an isolated disposable DEV/Neon target only;
//! `TestDatabase` refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`) and the
//! harness asserts the target is DEV. The proof leaves nothing behind: its stories are deleted by namespace (items
//! and runs cascade) and a zero-leftover count is asserted.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__002__explicit_transition_to_ready_dispatches_exactly_once -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget, EnsureDispatch};
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The namespace every proof row in this file is named under. Each run appends its own `TestDatabase` namespace, so
/// two concurrent runs of this contract never share a row, and cleanup is scoped to this run's namespace alone.
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-002-";

/// Connect to the disposable DEV branch, tolerating a cold-pool / cold-Neon handshake under concurrent test load.
///
/// This is infrastructure, not the contract: a cold handshake can drop the first socket ("unexpected end of file"),
/// and several contract tests plus the engine can be opening pools against the same DEV branch at once. The retry
/// changes nothing about which database is targeted — the harness still refuses PRODUCTION before any socket is
/// opened.
async fn connect_dev() -> ForgeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ForgeHarness::connect_from_env().await {
            Ok(harness) => return harness,
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

/// Insert one disposable board row at `status`.
async fn insert_story(pool: &PgPool, story_id: &str, status: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'FORGE-DISPATCH-CONTRACT', 'Forge dispatch contract', 'High', $2, '')",
    )
    .bind(story_id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert the proof story");
}

/// The story's committed board status.
async fn story_status(pool: &PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id=$1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the story status")
}

/// Every serial work item the story holds (parallel_group_id IS NULL): `(state, claimed_by)`.
/// A dispatch is a change in this count.
async fn serial_work_items(pool: &PgPool, story_id: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as(
        "select state, claimed_by from agent_work_item
         where story_id=$1 and parallel_group_id is null
         order by queued_at, id",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read the story's serial work items")
}

/// The number of open Story Runs Forge opened for the story and never closed.
async fn open_runs(pool: &PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id=$1 and ended_at is null",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("count the story's open runs")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)]
async fn forge_dispatch_002__explicit_transition_to_ready_dispatches_exactly_once() {
    // 0. A disposable DEV target, and only a DEV target.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    // Every row this run creates is named under this unique namespace.
    let explicit_ready = format!("{PROOF_PREFIX}explicit-ready-{ns}");
    let second_explicit = format!("{PROOF_PREFIX}second-explicit-{ns}");
    let sweep_dispatch = format!("{PROOF_PREFIX}sweep-dispatch-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. EXPLICIT TRANSITION TO READY — the board (or API) explicitly changes status to Ready.
    //    This fires the database trigger and creates exactly one serial work item.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &explicit_ready, "Planned").await;
    assert_eq!(
        story_status(pool, &explicit_ready).await,
        "Planned",
        "{HARNESS}: story starts at Planned"
    );
    assert!(
        serial_work_items(pool, &explicit_ready).await.is_empty(),
        "{HARNESS}: no serial item before explicit Ready"
    );

    // Explicitly transition to Ready via the production dispatch verb.
    let result = harness.engine().ensure_story_dispatched(&explicit_ready).await;
    assert!(matches!(result, Ok(EnsureDispatch::Queued { .. })),
        "{HARNESS}: explicit transition to Ready queues exactly one item");

    // Read back committed truth: exactly one serial Ready item exists.
    let items = serial_work_items(pool, &explicit_ready).await;
    assert_eq!(
        items.len(),
        1,
        "{HARNESS}: explicit Ready creates exactly one serial work item"
    );
    assert_eq!(
        items[0].0, "Ready",
        "{HARNESS}: the dispatched item is in Ready state"
    );
    assert_eq!(
        items[0].1, None,
        "{HARNESS}: the dispatched item is unclaimed"
    );
    assert_eq!(
        story_status(pool, &explicit_ready).await,
        "Ready",
        "{HARNESS}: board status is Ready"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. SECOND EXPLICIT TRANSITION — calling ensure_story_dispatched again on the same Ready story
    //    must NOT duplicate the dispatch. It returns AlreadyQueued with the existing item.
    // -----------------------------------------------------------------------------------------------------------
    let result2 = harness.engine().ensure_story_dispatched(&explicit_ready).await;
    assert!(matches!(result2, Ok(EnsureDispatch::AlreadyQueued { .. })),
        "{HARNESS}: second explicit transition returns AlreadyQueued");

    // Committed truth unchanged: still exactly one serial item.
    let items2 = serial_work_items(pool, &explicit_ready).await;
    assert_eq!(
        items2.len(),
        1,
        "{HARNESS}: second explicit transition does NOT duplicate the dispatch"
    );
    assert_eq!(
        items2[0].0, "Ready",
        "{HARNESS}: the original item remains Ready"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. SWEEP DISPATCH — a story already at Ready with no serial item (e.g., item went terminal)
    //    is repaired by reconcile_dispatch_queue through forge_dispatch_story.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &sweep_dispatch, "Ready").await;
    // Manually remove any serial item to simulate a terminal item (simulating what reconcile does).
    sqlx::query("delete from agent_work_item where story_id=$1 and parallel_group_id is null")
        .bind(&sweep_dispatch)
        .execute(pool)
        .await
        .expect("remove serial item for sweep test");

    assert!(
        serial_work_items(pool, &sweep_dispatch).await.is_empty(),
        "{HARNESS}: sweep test story has no serial item"
    );

    // Run the sweep - this should dispatch the story.
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    assert_eq!(
        report.queued,
        1,
        "{HARNESS}: sweep dispatched exactly one story"
    );

    // Read back: sweep created exactly one serial item.
    let items3 = serial_work_items(pool, &sweep_dispatch).await;
    assert_eq!(
        items3.len(),
        1,
        "{HARNESS}: sweep dispatch creates exactly one serial work item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. FAULT CASE — a story at Ready that ALREADY holds a serial item must not be dispatched again
    //    by the sweep. The unique index and forge_dispatch_story guard this.
    // -----------------------------------------------------------------------------------------------------------
    let items_before_sweep = serial_work_items(pool, &explicit_ready).await.clone();
    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("second sweep");
    // The sweep should not queue the explicit_ready story again (AlreadyQueued).
    assert_eq!(
        report2.queued,
        0,
        "{HARNESS}: sweep does not re-dispatch story that already has a serial item"
    );

    let items_after_sweep = serial_work_items(pool, &explicit_ready).await;
    assert_eq!(
        items_after_sweep.len(),
        items_before_sweep.len(),
        "{HARNESS}: serial item count unchanged after sweep on already-dispatched story"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. COMMITTED TRUTH SURVIVES A ROLLBACK. A probe that rewrites the status sees its own write
    //    inside its transaction, and rolling back restores the committed truth.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = explicit_ready.clone();
    let inside_probe = harness
        .database()
        .with_rollback(|conn| {
            Box::pin(async move {
                sqlx::query(
                    "update storyboard_story set status='Planned', updated_at=now() where id=$1",
                )
                .bind(&probe_story)
                .execute(&mut *conn)
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx(
                        "test-harness.forge_dispatch.rollback_probe_update",
                        &error,
                    )
                })?;
                let status: String =
                    sqlx::query_scalar("select status from storyboard_story where id=$1")
                        .bind(&probe_story)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "test-harness.forge_dispatch.rollback_probe_read",
                                &error,
                            )
                        })?;
                Ok(status)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(
        inside_probe, "Planned",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    assert_eq!(
        story_status(pool, &explicit_ready).await,
        "Ready",
        "{HARNESS}: rollback restored the committed truth — the story is still Ready"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / ROLLBACK. This run's proof stories are deleted by namespace; items and runs cascade.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{ns}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows");
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let leftover_stories: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .expect("count leftover stories");
    let leftover_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .expect("count leftover work items");
    let leftover_runs: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .expect("count leftover runs");
    assert_eq!(
        leftover_stories + leftover_items + leftover_runs,
        0,
        "{HARNESS}: the proof must leave no story, work item or run behind"
    );
}
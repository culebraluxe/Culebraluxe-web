//! FORGE.DISPATCH — an In Progress human OPEN card never dispatches (TST-FORGE-DISPATCH-001).
//!
//! Contract: a `storyboard_story` that says `In Progress` while nothing Forge owns holds it — no open work item and
//! no open Story Run — is a human's OPEN card, and `ForgeEngineDao::reconcile_dispatch_queue`
//! (`db/src/forge_engine.rs:1186`) must leave it exactly as a human left it. It must never be restated to
//! `Ready`, because a change into `Ready` is what fires the database's dispatch trigger
//! (`agent_work_item_dispatch()`, `db/migrations/025_agent_work_queue.sql:101`, restated in
//! `db/migrations/146_fix_storyboard_ready_dispatch_arbiter.sql:36`), and a manufactured `Ready` would open a work
//! item nobody asked for. Moving something into the engine queue is the explicit handoff; the sweep may repair Forge
//! state, it must never invent it.
//!
//! The subject is the real sweep, not a re-declaration of it. `reconcile_dispatch_queue` restates an `In Progress`
//! story only on **positive evidence** that Forge owned it: an existing `Ready`/`Paused` work item or a Story Run
//! that is still open (`ended_at is null`). The doc comment at `db/src/forge_engine.rs:1192-1206` names the
//! failure this fences as *manufacturing authorization*: a sweep that treated every `In Progress` row the same would
//! turn a human's sticky note into `Ready`, let the trigger fire, and the engine would claim work nobody requested.
//! That is precisely the boundary this contract proves.
//!
//! This is greenfield Rust: it is not a port of any TypeScript test. It exercises the production boundary — the real
//! `ForgeHarness` wraps the production `ForgeEngineDao` on an isolated, disposable DEV database, the production claim
//! path (`claim_specific_agent_work`) is used to manufacture a live run, the seed insert goes through the board's own
//! dispatch trigger, and every assertion is read back on the pool the DAO committed to. Raw SQL here is fixture setup
//! and teardown (disposable stories named under this run's namespace) and a committed-truth read-back, never a second
//! implementation of the sweep under test.
//!
//! Level: L2 Persistence, harness `ForgeHarness`. The boundary rule is an isolated disposable DEV/Neon target only;
//! `TestDatabase` refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`) and the
//! harness asserts the target is DEV. The proof leaves nothing behind: its stories are deleted by namespace (items
//! and runs cascade) and a zero-leftover count is asserted.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__001__in_progress_human_open_never_dispatches -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The namespace every proof row in this file is named under. Each run appends its own `TestDatabase` namespace, so
/// two concurrent runs of this contract never share a row, and cleanup is scoped to this run's namespace alone.
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-001-";
/// The worker that holds the live-claim fault control.
const LIVE_WORKER: &str = "forge-dispatch-001:live";

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

/// Insert one disposable board row at `status`. A non-`Ready` insert opens no work item: the dispatch trigger fires
/// only on a change INTO `Ready`, which is the whole reason an `In Progress` human card starts with nothing holding
/// it.
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

/// Every work item the story holds, whatever its state: `(state, claimed_by)`. A dispatch is a change in this count,
/// so a manufactured `Ready` would show up here even though nothing was ever claimed.
async fn work_items(pool: &PgPool, story_id: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as(
        "select state, claimed_by from agent_work_item where story_id=$1 order by queued_at, id",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read the story's work items")
}

/// The number of Story Runs Forge opened for the story and never closed — the second form of positive ownership
/// evidence the sweep consults.
async fn open_runs(pool: &PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id=$1 and ended_at is null",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("count the story's open runs")
}

/// A worker may start a run only through a claim (`Begin`), never a raw hand-off. This helper is fixture setup: it
/// takes the board's own trigger-created `Ready` item through the production claim path so a *live* claim exists.
async fn claim_ready_item(harness: &ForgeHarness, story_id: &str, worker: &str) -> String {
    let item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id=$1 and state='Ready'",
    )
    .bind(story_id)
    .fetch_one(harness.pool())
    .await
    .expect("the Ready trigger queued exactly one item for the live-claim control");
    harness
        .engine()
        .claim_specific_agent_work(&item, worker)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable by one worker");
    item
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-DISPATCH-001); the file and the assay use it.
async fn forge_dispatch_001__in_progress_human_open_never_dispatches() {
    // 0. A disposable DEV target, and only a DEV target. `connect_from_env` resolves the declaration exactly as
    //    production does and refuses PRODUCTION before any socket is opened, so this test cannot be pointed at PROD
    //    by a stray VERCEL_ENV/APP_ENV.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    // Every row this run creates is named under this unique namespace, so it can never collide with a concurrent run
    // of this same contract on the shared DEV branch.
    let human_open = format!("{PROOF_PREFIX}human-open-{ns}");
    let closed_run = format!("{PROOF_PREFIX}closed-run-{ns}");
    let owned_open_run = format!("{PROOF_PREFIX}owned-open-run-{ns}");
    let live_claim = format!("{PROOF_PREFIX}live-claim-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT SHAPE — A BARE In Progress CARD. No work item, no Story Run, no live process instance: this is
    //    the board's OPEN card (human work, no engine run), the deliberate meaning of `In Progress`. It starts with
    //    nothing holding it because the dispatch trigger fires only on a change INTO `Ready`.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &human_open, "In Progress").await;
    assert_eq!(
        story_status(pool, &human_open).await,
        "In Progress",
        "{HARNESS}: the open card is In Progress"
    );
    assert!(
        work_items(pool, &human_open).await.is_empty(),
        "{HARNESS}: an In Progress human card holds no engine work item"
    );
    assert_eq!(
        open_runs(pool, &human_open).await,
        0,
        "{HARNESS}: an In Progress human card has no open Forge run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. FAULT CONTROL — FORGE'S OWN RUN ALREADY ENDED. An `In Progress` story carrying a Story Run that is *closed*
    //    is not owned any more: the sweep's positive evidence is an OPEN run (`ended_at is null`). A boundary that
    //    accepted any run row, closed or not, would restate this card and manufacture a dispatch.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &closed_run, "In Progress").await;
    sqlx::query("insert into storyboard_story_run (story_id, started_at, ended_at) values ($1, now(), now())")
        .bind(&closed_run)
        .execute(pool)
        .await
        .expect("insert a closed proof run");
    assert!(
        work_items(pool, &closed_run).await.is_empty(),
        "{HARNESS}: a closed-run card holds no work item"
    );
    assert_eq!(
        open_runs(pool, &closed_run).await,
        0,
        "{HARNESS}: the fault control's only run is closed (ended_at is set)"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. POSITIVE CONTROL — FORGE OWNED IT. An `In Progress` story with an OPEN Story Run and no item is the
    //    stranded shape the sweep exists to repair: Forge started a run and nothing holds it any more. This control
    //    is load-bearing — it proves the sweep actually restates and dispatches owned work, so step 1 cannot pass
    //    because the sweep is a no-op.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &owned_open_run, "In Progress").await;
    sqlx::query("insert into storyboard_story_run (story_id, started_at) values ($1, now())")
        .bind(&owned_open_run)
        .execute(pool)
        .await
        .expect("insert an open proof run");
    assert_eq!(
        open_runs(pool, &owned_open_run).await,
        1,
        "{HARNESS}: the positive control is Forge-owned — an open run exists"
    );
    assert!(
        work_items(pool, &owned_open_run).await.is_empty(),
        "{HARNESS}: and no item holds it — exactly the stranded shape the sweep repairs"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. FAULT CONTROL — A LIVE RUN. A story `In Progress` beside a `Claimed` item is Forge's half of a run that is
    //    happening right now. The sweep must not restate it and must not dispatch a second engine over it.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &live_claim, "Ready").await;
    claim_ready_item(&harness, &live_claim, LIVE_WORKER).await;
    sqlx::query("update storyboard_story set status='In Progress', updated_at=now() where id=$1")
        .bind(&live_claim)
        .execute(pool)
        .await
        .expect("put the board's half of the live run In Progress");
    let live_items_before = work_items(pool, &live_claim).await;
    assert_eq!(
        live_items_before.len(),
        1,
        "{HARNESS}: the live run holds exactly one work item"
    );
    assert_eq!(
        live_items_before[0].0, "Claimed",
        "{HARNESS}: and that item is a live claim"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE SWEEP — the one production pass, run once over every control at the same time.
    // -----------------------------------------------------------------------------------------------------------
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    println!(
        "reconcile_dispatch_queue: queued={} restated={} cleared={}",
        report.queued, report.restated, report.cleared
    );

    // 5a. THE CONTRACT — the human OPEN card is left exactly as a human left it. It is not restated to `Ready`, so
    //     the dispatch trigger never fires, no work item is ever written, and no run is opened. A sweep that treated
    //     every `In Progress` row alike would fail every one of these three assertions.
    assert_eq!(
        story_status(pool, &human_open).await,
        "In Progress",
        "{HARNESS}: an In Progress human OPEN card must never be restated to Ready"
    );
    assert!(
        work_items(pool, &human_open).await.is_empty(),
        "{HARNESS}: and must never be dispatched — no work item exists"
    );
    assert_eq!(
        open_runs(pool, &human_open).await,
        0,
        "{HARNESS}: and no run is opened for it"
    );

    // 5b. FAULT — a closed run is not ownership. The card stays `In Progress` and dispatches nothing.
    assert_eq!(
        story_status(pool, &closed_run).await,
        "In Progress",
        "{HARNESS}: a story whose only run is closed is not Forge-owned and must not be restated"
    );
    assert!(
        work_items(pool, &closed_run).await.is_empty(),
        "{HARNESS}: a closed run must not manufacture a dispatch"
    );

    // 5c. POSITIVE CONTROL — Forge owned the stranded story, so the sweep restores the dispatch: the board goes back
    //     to `Ready` and the database's trigger opens exactly one queue slot. This is what makes 5a a real refusal.
    assert_eq!(
        story_status(pool, &owned_open_run).await,
        "Ready",
        "{HARNESS}: a stranded In Progress story Forge owned goes back to Ready"
    );
    let repaired = work_items(pool, &owned_open_run).await;
    assert_eq!(
        repaired.len(),
        1,
        "{HARNESS}: the repaired story is dispatched exactly once"
    );
    assert_eq!(
        repaired[0].0, "Ready",
        "{HARNESS}: the repaired dispatch is a queue slot, not a claim"
    );
    assert_eq!(
        repaired[0].1, None,
        "{HARNESS}: and nothing holds it until the engine claims it"
    );

    // 5d. FAULT — a live run is never touched. The board half is not restated and no second item appears; the claim
    //     is exactly the item that was already claimed, so the sweep cannot start a second engine over a story that
    //     is still running.
    assert_eq!(
        story_status(pool, &live_claim).await,
        "In Progress",
        "{HARNESS}: the board half of a live run must not be restated"
    );
    let live_items_after = work_items(pool, &live_claim).await;
    assert_eq!(
        live_items_after.len(),
        1,
        "{HARNESS}: a live run must not be dispatched a second time"
    );
    assert_eq!(
        live_items_after[0].0, "Claimed",
        "{HARNESS}: the live claim is untouched"
    );
    assert_eq!(
        live_items_after[0].1.as_deref(),
        Some(LIVE_WORKER),
        "{HARNESS}: the live claim still belongs to its owner"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. COMMITTED TRUTH SURVIVES A ROLLBACK. The status step 5a asserted is the row the sweep committed, not a
    //    transaction-local artifact: a probe that rewrites the open card to `Ready` sees its own write inside its
    //    transaction, and rolling back restores the committed `In Progress`. This is the "assert committed database
    //    truth and rollback" half of the L2 boundary rule.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = human_open.clone();
    let inside_probe = harness
        .database()
        .with_rollback(|conn| {
            Box::pin(async move {
                sqlx::query(
                    "update storyboard_story set status='Ready', updated_at=now() where id=$1",
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
        inside_probe, "Ready",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    assert_eq!(
        story_status(pool, &human_open).await,
        "In Progress",
        "{HARNESS}: rollback restored the committed truth — the open card was never restated"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. CLEANUP / ROLLBACK. This run's proof stories are deleted by namespace; items and runs cascade with them, so
    //    DEV is left as it was found. A non-zero leftover count is a failed rollback and fails the proof.
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

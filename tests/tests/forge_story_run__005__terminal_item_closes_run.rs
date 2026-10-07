//! FORGE.STORY_RUN — terminal item closes run (TST-FORGE-STORY-RUN-005).
//!
//! Contract: the Story Run a claim opened closes in the same transaction that settles the claim, whatever the
//! settlement is. `ForgeEngineDao::finish_agent_work_run` (`db/src/forge_engine.rs:366-381`) drives
//! `forge_finish_agent_work_run` (`db/migrations/263_forge_agent_work_settlement.sql:100-166`), which performs
//! `forge_close_story_run` (same migration, lines 80-96) guarded by `ended_at is null` — so the moment an item
//! reaches a terminal state, its run has `ended_at`; a run that outlives a terminal item is the "queue and board
//! half-moved" defect this contract exists to prevent.
//!
//! What this file proves:
//!
//!   1. **`Done` closes the run** — with the board confirming (`Complete`), the settle returns `Done`, the item is
//!      terminal and the run carries `ended_at`, `result_status='Complete'` and `completion=100`;
//!   2. **a refused verdict still closes** — `Done` against a board that does not confirm it is REFUSED (item
//!      `Error`, story `Hold`, ruling `Failed`), and the run closes all the same: closure follows the terminal
//!      state, not the happy verdict;
//!   3. **negative / fault — no phantom close** — settling an item that was never claimed returns no row and opens
//!      no run: there is nothing to close, and nothing is invented;
//!   4. **negative / fault — open before settle** — between `begin` and `finish` the run is open (`ended_at` null,
//!      `result_status` null), so the close above is really the settle's doing and not a fixture that started
//!      closed.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim, the run open and the settle are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! raw SQL here is fixture setup and read-back only. No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__005__terminal_item_closes_run -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-005-";
const OWNER: &str = "forge-story-run-005-owner";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
async fn connect_dev() -> ForgeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ForgeHarness::connect_declared(Some("dev"), Some("dev")).await {
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

/// Put one disposable story on the board at `Ready` and return the exactly-one work item its dispatch trigger
/// queued — the same queue production dispatches from.
async fn seed(harness: &ForgeHarness, story_id: &str) -> String {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match harness.seed_ready_story(story_id).await {
            Ok(item) => return item,
            Err(error) => {
                let message = error.to_string();
                let landed: Option<String> = sqlx::query_scalar(
                    "select id::text from agent_work_item where story_id=$1 and state='Ready'",
                )
                .bind(story_id)
                .fetch_optional(harness.pool())
                .await
                .ok()
                .flatten();
                if let Some(item) = landed {
                    return item;
                }
                eprintln!("proof: seed attempt {attempt} failed: {message}");
                last = Some(message);
                tokio::time::sleep(std::time::Duration::from_millis(250 * attempt)).await;
            }
        }
    }
    panic!(
        "the board's Ready trigger must queue exactly one work item for {story_id}: {}",
        last.unwrap_or_default()
    );
}

/// Claim the item and open its run through the production seam, returning the run id.
async fn begin(harness: &ForgeHarness, item: &str) -> String {
    harness
        .engine()
        .claim_specific_agent_work(item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .engine()
        .begin_agent_work_run(item)
        .await
        .expect("the production begin runs")
        .expect("the claim opens the run")
        .story_run_id
}

/// Move the board to `status` before the settle — the settle reads the board, not this test.
async fn set_board(pool: &PgPool, story_id: &str, status: &str) {
    sqlx::query("update storyboard_story set status=$2 where id=$1")
        .bind(story_id)
        .bind(status)
        .execute(pool)
        .await
        .expect("the board can be moved");
}

/// The run's closing facts, read off the committed row.
async fn run_state(pool: &PgPool, run_id: &str) -> (Option<String>, Option<String>, Option<i32>) {
    sqlx::query_as(
        "select ended_at::text, result_status, completion
           from storyboard_story_run where id=$1::uuid",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("the run row is readable")
}

/// The item's committed state, read on the pool the DAO wrote to.
async fn item_state(pool: &PgPool, item_id: &str) -> String {
    sqlx::query_scalar("select state from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-005).
async fn forge_story_run_005__terminal_item_closes_run() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine: ForgeEngineDao = harness.engine().clone();
    let ns = harness.database().namespace().to_string();

    let story_done = format!("{PROOF_PREFIX}done-{ns}");
    let story_refused = format!("{PROOF_PREFIX}refused-{ns}");
    let story_unclaimed = format!("{PROOF_PREFIX}unclaimed-{ns}");

    // 1. THE OPEN RUN — control: between `begin` and `finish` the run is open, so any close below is the settle's
    //    doing.
    let item_done = seed(&harness, &story_done).await;
    let run_done = begin(&harness, &item_done).await;
    let (open_ended, open_ruling, _open_completion) = run_state(&pool, &run_done).await;

    // 2. `Done` over a board that confirms it: the terminal settle.
    set_board(&pool, &story_done, "Complete").await;
    let settled_done = engine
        .finish_agent_work_run(&item_done, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs")
        .expect("a Running item must settle");
    let (done_ended, done_ruling, done_completion) = run_state(&pool, &run_done).await;
    let done_item_state = item_state(&pool, &item_done).await;

    // 3. A REFUSED VERDICT STILL CLOSES. `Done` over a board that does not confirm it: the settle refuses the
    //    verdict (item `Error`, story `Hold`), and the run closes as `Failed` — closure follows the terminal state.
    let item_refused = seed(&harness, &story_refused).await;
    let run_refused = begin(&harness, &item_refused).await;
    set_board(&pool, &story_refused, "In Progress").await;
    let settled_refused = engine
        .finish_agent_work_run(&item_refused, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs")
        .expect("a Running item must settle");
    let (refused_ended, refused_ruling, refused_completion) = run_state(&pool, &run_refused).await;
    let refused_item_state = item_state(&pool, &item_refused).await;
    let refused_story_status: Option<String> =
        sqlx::query_scalar("select status from storyboard_story where id=$1")
            .bind(&story_refused)
            .fetch_one(&pool)
            .await
            .expect("the story row is readable");

    // 4. NEGATIVE / FAULT — NO PHANTOM CLOSE: settling an item that was never claimed settles nothing and opens
    //    no run. Ownership, not a call counter, is what gives a run to close.
    let item_unclaimed = seed(&harness, &story_unclaimed).await;
    let settled_unclaimed = engine
        .finish_agent_work_run(&item_unclaimed, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs");
    let runs_for_unclaimed: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id=$1")
            .bind(&story_unclaimed)
            .fetch_one(&pool)
            .await
            .expect("runs are countable");
    let unclaimed_state = item_state(&pool, &item_unclaimed).await;

    // 5. CLEANUP / ROLLBACK: the proof stories are deleted; their items and runs go with them.
    for story in [&story_done, &story_refused, &story_unclaimed] {
        harness
            .cleanup_story(story)
            .await
            .expect("reap the proof story");
    }
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let stories_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    let items_left: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    let runs_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();

    // ---------------------------------------------------------------------------------------------------------
    // 6. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert!(
        open_ended.is_none() && open_ruling.is_none(),
        "{HARNESS}: control — the run is open before the settle, so the close below is the settle's doing"
    );
    assert_eq!(
        settled_done.item_state, "Done",
        "{HARNESS}: a confirmed `Done` settles the item Done"
    );
    assert!(
        done_ended.is_some(),
        "{HARNESS}: a terminal item closes its Story Run — ended_at is set"
    );
    assert_eq!(
        done_ruling.as_deref(),
        Some("Complete"),
        "{HARNESS}: the confirmed run is ruled Complete"
    );
    assert_eq!(
        done_completion,
        Some(100),
        "{HARNESS}: a Complete run is recorded at 100"
    );
    assert_eq!(
        done_item_state, "Done",
        "{HARNESS}: the settled item is terminal"
    );

    assert_eq!(
        settled_refused.item_state, "Error",
        "{HARNESS}: a `Done` the board does not confirm is refused to the item"
    );
    assert_eq!(
        settled_refused.story_status.as_deref(),
        Some("Hold"),
        "{HARNESS}: the refused verdict holds the board"
    );
    assert_eq!(
        refused_item_state, "Error",
        "{HARNESS}: the refused item is terminal all the same"
    );
    assert_eq!(
        refused_story_status.as_deref(),
        Some("Hold"),
        "{HARNESS}: the board moved with the refusal"
    );
    assert!(
        refused_ended.is_some(),
        "{HARNESS}: a refused verdict still closes its Story Run — terminal state is what closes, not the happy path"
    );
    assert_eq!(
        refused_ruling.as_deref(),
        Some("Failed"),
        "{HARNESS}: the refused run is ruled Failed, never silently Complete"
    );
    assert_ne!(
        refused_completion,
        Some(100),
        "{HARNESS}: a refused run is never recorded at 100"
    );

    // 7. NEGATIVE / FAULT.
    assert!(
        settled_unclaimed.is_none(),
        "{HARNESS}: settling a never-claimed item settles nothing — there is no run to close"
    );
    assert_eq!(
        runs_for_unclaimed, 0,
        "{HARNESS}: no run is invented for a claim that never opened one"
    );
    assert_eq!(
        unclaimed_state, "Ready",
        "{HARNESS}: the unclaimed item is untouched by the refused settle"
    );

    // 8. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        items_left, 0,
        "{HARNESS}: the proof must leave no work item behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

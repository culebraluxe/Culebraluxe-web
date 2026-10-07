//! FORGE.STORY_RUN — Abandoned closes run without verdict (TST-FORGE-STORY-RUN-006).
//!
//! Contract: `Abandoned` is the engine's own fault — nothing about the story was decided — so when the claim is
//! settled `Abandoned` the Story Run still closes (`ended_at` is set, in the same transaction:
//! `forge_finish_agent_work_run` → `forge_close_story_run`, `db/migrations/263_forge_agent_work_settlement.sql:100-166`
//! and `80-96`), but it rules NOTHING: `forge_run_result_status_for('Ready')` answers `NULL` for a cleared claim, so
//! `result_status` stays NULL. An engine fault must never become a story verdict, and a run must never outlive the
//! claim that opened it.
//!
//! What this file proves:
//!
//!   1. **closed without a verdict** — after the `Abandoned` settle the run has `ended_at`, `result_status IS NULL`,
//!      and its `completion` is untouched: closed, unruled;
//!   2. **the claim is cleared, not terminal** — the item goes back to `Ready` with its claim unset, so the queue
//!      is open again while the run it opened is closed;
//!   3. **negative / fault — an unknown outcome is refused** — a settlement outcome outside
//!      `Done | Error | Cancelled | Abandoned` is rejected by `forge_settlement_pair` (SQLSTATE 22023) and leaves
//!      the run exactly as it was: open, unruled, item still `Running`. A bogus ruling cannot close a run;
//!   4. **negative / fault — open before settle** — the control read between `begin` and `finish` shows the run
//!      open, so the close above is really the settle's doing.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim, the run open and the settle are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! raw SQL here is fixture setup and read-back only. The unknown-outcome fault is the database's own settlement
//! routine — the same one the DAO drives — called with a value the DAO's `AgentWorkOutcome` type cannot even
//! express. No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__006__abandoned_closes_run_without_verdict -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-006-";
const OWNER: &str = "forge-story-run-006-owner";

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

/// The run's ruling facts, read off the committed row: `(ended_at, result_status, completion)`.
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-006).
async fn forge_story_run_006__abandoned_closes_run_without_verdict() {
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
    let story = format!("{PROOF_PREFIX}abandoned-{ns}");

    // 1. THE CLAIM AND THE RUN OPEN, then the control read: the run is open and unruled before anything settles it.
    let item = seed(&harness, &story).await;
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    let run_id = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs")
        .expect("the claim opens the run")
        .story_run_id;
    let (open_ended, open_ruling, open_completion) = run_state(&pool, &run_id).await;

    // 2. NEGATIVE / FAULT — AN UNKNOWN OUTCOME IS REFUSED. The database's own settlement routine is handed a word
    //    outside the four the contract allows; `forge_settlement_pair` raises 22023, the statement rolls back, and
    //    the run is left exactly as it was: open, unruled, item still Running.
    let bogus =
        sqlx::query("select item_state from forge_finish_agent_work_run($1::uuid, 'Bogus', null)")
            .bind(&item)
            .fetch_all(&pool)
            .await;
    let bogus_refused = bogus.is_err();
    let (after_bogus_ended, after_bogus_ruling, after_bogus_completion) =
        run_state(&pool, &run_id).await;
    let after_bogus_item: String =
        sqlx::query_scalar("select state from agent_work_item where id=$1::uuid")
            .bind(&item)
            .fetch_one(&pool)
            .await
            .expect("the work item is readable");

    // 3. THE ABANDONED SETTLE through the production seam.
    let settled = engine
        .finish_agent_work_run(&item, AgentWorkOutcome::Abandoned, None)
        .await
        .expect("the production settle runs")
        .expect("a Running item must settle");
    let (closed_ended, closed_ruling, closed_completion) = run_state(&pool, &run_id).await;
    let closed_item: (String, Option<String>, Option<String>) = sqlx::query_as(
        "select state, claimed_by::text, story_run_id::text
           from agent_work_item where id=$1::uuid",
    )
    .bind(&item)
    .fetch_one(&pool)
    .await
    .expect("the work item is readable");
    let story_status: Option<String> =
        sqlx::query_scalar("select status from storyboard_story where id=$1")
            .bind(&story)
            .fetch_one(&pool)
            .await
            .expect("the story row is readable");

    // 4. CLEANUP / ROLLBACK: the proof story is deleted; its item and run go with it.
    harness
        .cleanup_story(&story)
        .await
        .expect("reap the proof story");
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let stories_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
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
    // 5. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert!(
        open_ended.is_none() && open_ruling.is_none(),
        "{HARNESS}: control — the run is open and unruled before any settle"
    );
    assert_eq!(
        settled.item_state, "Ready",
        "{HARNESS}: an Abandoned claim is cleared back into the queue, not made terminal"
    );
    assert_eq!(
        settled.story_status.as_deref(),
        Some("Ready"),
        "{HARNESS}: the board goes back to Ready with the cleared claim"
    );
    assert!(
        closed_ended.is_some(),
        "{HARNESS}: Abandoned closes the Story Run — the run does not outlive the claim that opened it"
    );
    assert_eq!(
        closed_ruling, None,
        "{HARNESS}: a cleared claim rules nothing — the closed run carries NO verdict"
    );
    assert_eq!(
        closed_completion, open_completion,
        "{HARNESS}: an unruled close leaves completion exactly as it was"
    );
    assert_eq!(
        closed_item.0, "Ready",
        "{HARNESS}: the cleared item is back in the queue while its run is closed"
    );
    assert_eq!(
        closed_item.1, None,
        "{HARNESS}: the cleared claim carries no owner anymore"
    );
    assert_eq!(
        closed_item.2.as_deref(),
        Some(run_id.as_str()),
        "{HARNESS}: the item still names the run it opened, for the record"
    );
    assert_eq!(
        story_status.as_deref(),
        Some("Ready"),
        "{HARNESS}: the board is dispatchable again"
    );

    // 6. NEGATIVE / FAULT.
    assert!(
        bogus_refused,
        "{HARNESS}: a settlement outcome outside Done|Error|Cancelled|Abandoned must be refused by the database"
    );
    assert_eq!(
        after_bogus_ended, open_ended,
        "{HARNESS}: the refused outcome left the run open — a bogus ruling cannot close a run"
    );
    assert_eq!(
        after_bogus_ruling, open_ruling,
        "{HARNESS}: the refused outcome wrote no verdict"
    );
    assert_eq!(
        after_bogus_completion, open_completion,
        "{HARNESS}: the refused outcome wrote no completion"
    );
    assert_eq!(
        after_bogus_item, "Running",
        "{HARNESS}: the refused outcome left the claim untouched"
    );

    // 7. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

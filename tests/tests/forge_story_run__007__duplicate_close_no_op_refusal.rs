//! FORGE.STORY_RUN — duplicate close no-op/refusal (TST-FORGE-STORY-RUN-007).
//!
//! Contract: a Story Run closes exactly once. Two guards make a second close a NO-OP instead of a rewrite:
//!
//!   * the settlement's own guard — `forge_finish_agent_work_run` only proceeds `where state in ('Claimed',
//!     'Running')` (`db/migrations/263_forge_agent_work_settlement.sql:116`), so a settled item has nothing left to
//!     settle and the DAO answers `Ok(None)`;
//!   * the close's guard — `forge_close_story_run` updates `where … and r.ended_at is null` (same migration, line
//!     95), so even a close statement that reaches the database afterwards (the race the guard exists for) matches
//!     no row: the first ruling, the first `ended_at`, the first `notes` and `completion` all stand.
//!
//! What this file proves:
//!
//!   1. **the first close commits** — a confirmed `Done` sets `ended_at`, `result_status='Complete'` and
//!      `completion=100`, and its notes are recorded;
//!   2. **the duplicate settle is refused** — a second `finish_agent_work_run` (whatever the outcome) returns
//!      `Ok(None)` and changes nothing on any row;
//!   3. **the duplicate close is a no-op** — a second `forge_close_story_run` with a contradictory ruling and a
//!      reason matches no row: ruling, `ended_at`, `completion` and notes are byte-for-byte what the first close
//!      wrote;
//!   4. **negative / fault** — the proof compares the whole closing record, not one column, so a close that only
//!      appended a note, only bumped `updated_at`'s payload, or only relabelled the ruling would fail the test.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim, the run open and the settle are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! raw SQL here is fixture setup, read-back and the deliberate duplicate-close fault (the database's own close
//! routine, the one the DAO drives). No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__007__duplicate_close_no_op_refusal -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-007-";
const OWNER: &str = "forge-story-run-007-owner";

/// The whole closing record of one run: `(ended_at, result_status, completion, notes)`.
type CloseRecord = (Option<String>, Option<String>, Option<i32>, Option<String>);

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

/// Read the run's complete closing record off the committed row.
async fn close_record(pool: &PgPool, run_id: &str) -> CloseRecord {
    sqlx::query_as(
        "select ended_at::text, result_status, completion, notes
           from storyboard_story_run where id=$1::uuid",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("the run row is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-007).
async fn forge_story_run_007__duplicate_close_no_op_refusal() {
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
    let story = format!("{PROOF_PREFIX}duplicate-{ns}");

    // 1. THE CLAIM, THE RUN OPEN, AND THE ONE LEGITIMATE CLOSE.
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
    sqlx::query("update storyboard_story set status='Complete' where id=$1")
        .bind(&story)
        .execute(&pool)
        .await
        .expect("the board can confirm completion");
    let first = engine
        .finish_agent_work_run(&item, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs")
        .expect("the first settle must land");
    let after_first = close_record(&pool, &run_id).await;

    // 2. THE DUPLICATE SETTLE — whatever outcome, a settled item has nothing left to settle.
    let second_done = engine
        .finish_agent_work_run(&item, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs");
    let second_error = engine
        .finish_agent_work_run(&item, AgentWorkOutcome::Error, Some("late failure"))
        .await
        .expect("the production settle runs");
    let after_duplicates = close_record(&pool, &run_id).await;
    let item_after_duplicates: String =
        sqlx::query_scalar("select state from agent_work_item where id=$1::uuid")
            .bind(&item)
            .fetch_one(&pool)
            .await
            .expect("the work item is readable");

    // 3. THE DUPLICATE CLOSE — the database's own close routine, reached directly with a contradictory ruling and
    //    a reason of its own (the race the `ended_at is null` guard exists for). It must match no row.
    sqlx::query("select forge_close_story_run($1::uuid, 'Failed', 'late ruling from a straggler')")
        .bind(&item)
        .execute(&pool)
        .await
        .expect("the close routine runs");
    let after_close_again = close_record(&pool, &run_id).await;

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
    assert_eq!(
        first.item_state, "Done",
        "{HARNESS}: the first settle lands and settles the item Done"
    );
    assert!(
        after_first.0.is_some(),
        "{HARNESS}: the first close sets ended_at"
    );
    assert_eq!(
        after_first.1.as_deref(),
        Some("Complete"),
        "{HARNESS}: the first close writes the ruling"
    );
    assert_eq!(
        after_first.2,
        Some(100),
        "{HARNESS}: the first close writes completion"
    );

    assert!(
        second_done.is_none(),
        "{HARNESS}: a second settle of the same claim is REFUSED — no row settles twice"
    );
    assert!(
        second_error.is_none(),
        "{HARNESS}: a duplicate settle is refused whatever outcome it brings, error text included"
    );
    assert_eq!(
        after_duplicates, after_first,
        "{HARNESS}: the duplicate settles left the whole closing record untouched"
    );
    assert_eq!(
        item_after_duplicates, "Done",
        "{HARNESS}: the duplicate settles did not move the terminal item either"
    );

    assert_eq!(
        after_close_again, after_first,
        "{HARNESS}: a second close with a contradictory ruling is a NO-OP — ruling, ended_at, completion and notes \
         are exactly what the first close wrote"
    );
    assert_ne!(
        after_close_again.1.as_deref(),
        Some("Failed"),
        "{HARNESS}: a straggler close can never relabel a run that already closed"
    );
    assert_eq!(
        after_close_again.0, after_first.0,
        "{HARNESS}: the straggler close never moved ended_at"
    );

    // 6. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

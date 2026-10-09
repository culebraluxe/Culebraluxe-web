//! DB.CONCURRENCY — complete vs complete (TST-DB-CONCURRENCY-003).
//!
//! Contract: **a run ends exactly once.** `ForgeEngineDao::finish_agent_work_run`
//! (`db/src/forge_engine.rs:359-373`) is "the one terminal write" — it calls `forge_finish_agent_work_run`
//! (migration 263), whose opening statement is the guard that makes the contract true:
//!
//! ```sql
//! select s.status, i.attempts, coalesce(i.max_attempts, 3)
//!   from agent_work_item i join storyboard_story s on s.id = i.story_id
//!  where i.id = p_work_item_id and i.state in ('Claimed','Running')
//!    for update of i;
//! ```
//!
//! Three properties are load-bearing and each is a separate mechanism:
//!
//! 1. **`for update of i`** takes the row lock, so N concurrent settles serialise on it. The first to be granted
//!    the lock sees `Claimed`, moves the row, and returns; every later one re-reads the *already-settled* row, fails
//!    the `state in ('Claimed','Running')` predicate, and `return`s without a row — `Ok(None)`.
//! 2. **The `state in ('Claimed','Running')` guard** is what makes a second settle a no-op instead of an overwrite.
//!    The comment on the DAO says so in as many words: it exists so "the child settling its own run and the worker
//!    settling a failed launch cannot race each other into a wrong verdict".
//! 3. **`v_changed = 0` → return** guards the write itself. Even a caller that reached the update with a stale view
//!    writes nothing, and reports that it settled nothing.
//!
//! So of N concurrent `Done` settles exactly one is answered `Some(settlement)` and the rest are `None` — and,
//! critically, exactly one `storyboard_story_run` is closed and exactly one item reaches a terminal state. The
//! proof reads both back from committed truth.
//!
//! The negative case is the one that matters: a late settle on an already-settled item is refused rather than
//! re-settling it. That is what stops a duplicated child process from overwriting `Done` with `Error` and losing the
//! run's real verdict.
//!
//! Level: L4 Adversarial — `RaceHarness` puts every settler inside the racy region at one `ConcurrencyBarrier`, and
//! refuses PRODUCTION before any socket is opened.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_concurrency__003__complete_vs_complete -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L4 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget};
use test_harness::RaceHarness;

const HARNESS: &str = "RaceHarness/L4 Adversarial";

/// How many processes settle the same run at once. The real race is "the child settles its own run and the worker
/// settles a failed launch", and two is enough to cause it; eight proves it is not a pairwise coincidence.
const SETTLERS: usize = 8;

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: `RaceHarness` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> RaceHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match RaceHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; RaceHarness refuses PROD: {}",
        last.unwrap_or_default()
    )
}

/// Put a story on the board as `Complete`, which is what makes a `Done` settlement legal.
///
/// `forge_settlement_pair` (migration 263) refuses `Done` unless the board already says the run finished — on any
/// other board status a `Done` becomes `Error` and the reason names the refusal. The race is about *how many*
/// settles win, not about whether `Done` is allowed, so the fixture starts from the board state that permits it.
async fn complete_the_board(harness: &RaceHarness, story_id: &str) {
    sqlx::query("update storyboard_story set status = 'Complete' where id = $1")
        .bind(story_id)
        .execute(harness.pool())
        .await
        .expect("the board is moved to Complete so a Done settlement is legal");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); RaceHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-CONCURRENCY-003); the assay uses it.
async fn db_concurrency_003__complete_vs_complete() {
    // 0. L4 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the complete-vs-complete proof runs only on an isolated DEV target"
    );
    let marker = format!(
        "tstdbcon003-{}-{}",
        harness.namespace(),
        uuid::Uuid::new_v4()
    );

    // An isolated story with one `Ready` item, claimed so the item is in `Claimed` — the only state from which a
    // settlement is legal at all.
    let story_id = format!("{marker}-story");
    harness
        .seed_story(&story_id)
        .await
        .expect("the story fixture seeds");
    let item = harness
        .seed_ready_item(&story_id)
        .await
        .expect("the ready work item seeds");
    assert!(
        harness
            .engine()
            .claim_specific_agent_work(&item, "tst-settler")
            .await
            .expect("the claim answers")
            .is_some(),
        "{HARNESS}: the item is claimed, so a run may begin and a settlement is legal"
    );

    // The run is opened for real before anything settles it, because a settlement closes a *run*: `forge_close_story_run`
    // is called inside the same transaction, and there is nothing to close if execution never began. So the sequence
    // under test is the production one — claim, begin, then settle — with the race on the last step.
    let begun = harness
        .begin_claim(&item)
        .await
        .expect("begin answers rather than erroring")
        .expect("the claimed item begins its run");
    assert!(
        !begun.story_run_id.is_empty(),
        "{HARNESS}: execution opens a durable story run for the claim to settle"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE RACE. SETTLERS processes all try to end the same run with `Done`, all inside the racy region. The
    //    row lock in `forge_finish_agent_work_run` grants them one at a time; the `state in ('Claimed','Running')`
    //    guard refuses every one after the first.
    // -----------------------------------------------------------------------------------------------------------
    complete_the_board(&harness, &story_id).await;
    let racing_item = item.clone();
    let settlements = harness
        .race(SETTLERS, move |index, harness| {
            let item = racing_item.clone();
            async move {
                let answer = harness
                    .settle_claim(&item, AgentWorkOutcome::Done, None)
                    .await
                    .expect("a settle answers rather than erroring");
                (index, answer)
            }
        })
        .await;

    // `wrote()` is the exact translation of the old `Some`: the typed answer (migration 278) says WHO ended the run,
    // and only one racer can have written it.
    let winners: Vec<_> = settlements
        .iter()
        .filter(|(_, answer)| answer.wrote())
        .collect();
    let losers = settlements
        .iter()
        .filter(|(_, answer)| !answer.wrote())
        .count();
    assert_eq!(
        winners.len(),
        1,
        "{HARNESS}: exactly one of {SETTLERS} concurrent settles ends the run, got {}",
        winners.len()
    );
    assert_eq!(
        losers,
        SETTLERS - 1,
        "{HARNESS}: every other concurrent settle is refused rather than re-settling the run"
    );

    // The one winner's answer is the real settlement, not a bare state.
    let (_, answer) = winners[0];
    let settlement = answer
        .settlement()
        .expect("the winning settle wrote a pair");
    assert_eq!(
        settlement.item_state, "Done",
        "{HARNESS}: the winning settle recorded the terminal state it was asked for"
    );

    // Committed truth: one item, one terminal state. A settle that had overwritten its predecessor would still read
    // a terminal state here, so the *count* of winners above is what distinguishes "ended once" from "ended last".
    let state = harness
        .work_item(&item)
        .await
        .expect("the work item reads back");
    assert_eq!(
        state.state, "Done",
        "{HARNESS}: the committed item state is the terminal state the winner wrote"
    );

    // Exactly one story run was closed for this item. `forge_close_story_run` runs inside the same transaction as
    // the guarded update, so a refused settle cannot close a second run.
    let runs: i64 = sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id = $1 and ended_at is not null",
    )
    .bind(&story_id)
    .fetch_one(harness.pool())
    .await
    .expect("the run count reads");
    assert_eq!(
        runs, 1,
        "{HARNESS}: {SETTLERS} concurrent settles close exactly one story run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE / REFUSAL — A LATE SETTLE CANNOT OVERWRITE THE VERDICT. This is the duplicated-child case: a
    //    second process that settles after the fact is refused, and `Done` is still what the row says. If the
    //    guard were missing this second settle would succeed and write `Error` over a run that actually finished.
    // -----------------------------------------------------------------------------------------------------------
    let late = harness
        .settle_claim(
            &item,
            AgentWorkOutcome::Error,
            Some("a late duplicate settling"),
        )
        .await
        .expect("the late settle answers rather than erroring");
    assert!(
        !late.wrote(),
        "{HARNESS}: a settle on an already-settled item is refused (got {})",
        late.name()
    );
    assert_eq!(
        harness
            .work_item(&item)
            .await
            .expect("the work item reads back after the late settle")
            .state,
        "Done",
        "{HARNESS}: the late settle cannot overwrite the recorded verdict — `Done` survives an `Error` that arrives late"
    );

    // The run count is unchanged: the refused settle closed nothing.
    let runs_after: i64 = sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id = $1 and ended_at is not null",
    )
    .bind(&story_id)
    .fetch_one(harness.pool())
    .await
    .expect("the run count reads after the late settle");
    assert_eq!(
        runs_after, 1,
        "{HARNESS}: the refused late settle closes no second story run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — A SECOND `Done` IS REFUSED EVEN WHEN THE BOARD WOULD ALLOW IT. The guard is on the
    //    ITEM's state, not on the board, so re-running the whole settle after the board moved again is still a
    //    no-op. This is what makes the settlement idempotent rather than merely guarded once.
    // -----------------------------------------------------------------------------------------------------------
    complete_the_board(&harness, &story_id).await;
    let again = harness
        .settle_claim(&item, AgentWorkOutcome::Done, None)
        .await
        .expect("the repeat settle answers rather than erroring");
    assert!(
        !again.wrote(),
        "{HARNESS}: settling an already-settled run again is refused even with the board in the same state (got {})",
        again.name()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER. Every row this run seeded is removed; a non-zero leftover fails the proof, because
    //    a stray open item is a row the unattended poller could later pick up.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no receipt, run, item, task or story behind"
    );
}

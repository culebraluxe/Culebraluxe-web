//! DB.CONCURRENCY — complete vs cancel (TST-DB-CONCURRENCY-005).
//!
//! Contract: **a run ends in exactly one verdict, and the two verdicts that contradict each other cannot both be
//! written.** This is the race the DAO's own comment names: a child process settling its own run as `Done` while the
//! worker that launched it settles the same run as `Cancelled`. Both go through the *same* terminal write,
//! `ForgeEngineDao::finish_agent_work_run` (`db/src/forge_engine.rs:359-373`) calling
//! `forge_finish_agent_work_run` (migration 263), so the guard that separates them is the opening statement:
//!
//! ```sql
//! select s.status, i.attempts, coalesce(i.max_attempts, 3)
//!   from agent_work_item i join storyboard_story s on s.id = i.story_id
//!  where i.id = p_work_item_id and i.state in ('Claimed','Running')
//!    for update of i;
//! ```
//!
//! `for update of i` grants the two callers one at a time; the `state in ('Claimed','Running')` predicate admits
//! only the first, and `v_changed = 0 → return` guarantees the loser wrote nothing and is told `None`.
//!
//! Both are asserted, because a contract that only pins "one wins" would pass on an implementation that wrote the
//! loser over the winner one row later:
//!
//! 1. **At most one verdict.** Of N racing settles, exactly one is answered. The committed state is the winner's
//!    verdict — `Done` or `Cancelled`, never a blend and never the loser's.
//! 2. **The verdict is final.** Whichever verdict won, re-settling with the *other* one is refused. `Done` that
//!    arrives after a `Cancelled` cannot resurrect a cancelled run as finished, and `Cancelled` after a `Done`
//!    cannot erase a real completion. This is the whole point: the two outcomes contradict, so the loser must have
//!    no write path at all.
//! 3. **One run, closed once.** Both settles close a `storyboard_story_run` inside the same transaction, so the
//!    loser cannot open or close a second run.
//!
//! Note the asymmetry in what each side is *trying* to say, which is why the board state matters: `Done` is only
//! honoured when the board already reads `Complete` or `Hold` — `forge_settlement_pair` refuses it otherwise and
//! records `Error` instead. A cancel has no such precondition. The fixture therefore drives the board to
//! `Complete` so the race is genuinely `Done` against `Cancelled` and not `Done`-refused against `Cancelled`.
//!
//! Level: L4 Adversarial — `RaceHarness` puts every settler inside the racy region at one `ConcurrencyBarrier`, and
//! refuses PRODUCTION before any socket is opened.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_concurrency__005__complete_vs_cancel -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L4 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget};
use test_harness::RaceHarness;

const HARNESS: &str = "RaceHarness/L4 Adversarial";

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

/// Claim one isolated item and open its run, which is the state both verdicts are legal from.
///
/// A run has to exist before it can be settled: `forge_close_story_run` is called inside the settle's transaction,
/// so a settlement with no run behind it closes nothing.
async fn claim_and_begin(harness: &RaceHarness, story_id: &str, item: &str) {
    assert!(
        harness
            .engine()
            .claim_specific_agent_work(item, "tst-completer")
            .await
            .expect("the claim answers")
            .is_some(),
        "{HARNESS}: the item is claimed, so a run may begin"
    );
    assert!(
        harness
            .begin_claim(item)
            .await
            .expect("begin answers rather than erroring")
            .is_some(),
        "{HARNESS}: execution opens a durable run for the two verdicts to race over"
    );
    // The board is moved to `Complete` so a `Done` verdict is legal: `forge_settlement_pair` refuses `Done` unless
    // the board already says the run finished, and would otherwise record `Error` — which would make this a race
    // between `Error` and `Cancelled` rather than the `Done`-against-`Cancelled` this story is about.
    sqlx::query("update storyboard_story set status = 'Complete' where id = $1")
        .bind(story_id)
        .execute(harness.pool())
        .await
        .expect("the board is moved to Complete so a Done verdict is legal");
}

/// How many closed runs a story has.
async fn closed_run_count(harness: &RaceHarness, story_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id = $1 and ended_at is not null",
    )
    .bind(story_id)
    .fetch_one(harness.pool())
    .await
    .expect("the closed-run count reads")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); RaceHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-CONCURRENCY-005); the assay uses it.
async fn db_concurrency_005__complete_vs_cancel() {
    // 0. L4 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the complete-vs-cancel proof runs only on an isolated DEV target"
    );
    let marker = format!(
        "tstdbcon005-{}-{}",
        harness.namespace(),
        uuid::Uuid::new_v4()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE ORDERED RACE — CANCEL THEN DONE. The two verdicts contradict, so the order is what the assertions
    //    are about. Here the cancel lands first and the `Done` that follows is a duplicated child reporting a
    //    completion the world has already declared cancelled.
    // -----------------------------------------------------------------------------------------------------------
    let story_a = format!("{marker}-a");
    harness.seed_story(&story_a).await.expect("story A seeds");
    let item_a = harness
        .seed_ready_item(&story_a)
        .await
        .expect("item A seeds");
    claim_and_begin(&harness, &story_a, &item_a).await;

    let cancel_first = harness
        .settle_claim(
            &item_a,
            AgentWorkOutcome::Cancelled,
            Some("cancelled before the child reported"),
        )
        .await
        .expect("the cancel answers rather than erroring")
        .settlement()
        .cloned()
        .expect("a cancel against a running item is legal");
    assert_eq!(
        cancel_first.item_state, "Cancelled",
        "{HARNESS}: the first verdict is recorded as Cancelled"
    );

    // NEGATIVE: the late `Done` must not overwrite it.
    let late_done = harness
        .settle_claim(&item_a, AgentWorkOutcome::Done, None)
        .await
        .expect("the late Done answers rather than erroring");
    assert!(
        !late_done.wrote(),
        "{HARNESS}: a `Done` arriving after a `Cancelled` is refused — it cannot resurrect a cancelled run (got {})",
        late_done.name()
    );
    assert_eq!(
        harness
            .work_item(&item_a)
            .await
            .expect("item A reads back")
            .state,
        "Cancelled",
        "{HARNESS}: the committed verdict stays `Cancelled`; the contradictory `Done` wrote nothing"
    );
    assert_eq!(
        closed_run_count(&harness, &story_a).await,
        1,
        "{HARNESS}: the refused `Done` closes no second run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE ORDERED RACE — DONE THEN CANCEL. The mirror: the child finished the work and the worker cancelled the
    //    launch behind it. The `Done` is the real verdict and the late `Cancelled` must not erase it.
    // -----------------------------------------------------------------------------------------------------------
    let story_b = format!("{marker}-b");
    harness.seed_story(&story_b).await.expect("story B seeds");
    let item_b = harness
        .seed_ready_item(&story_b)
        .await
        .expect("item B seeds");
    claim_and_begin(&harness, &story_b, &item_b).await;

    let done_first = harness
        .settle_claim(&item_b, AgentWorkOutcome::Done, None)
        .await
        .expect("the Done answers rather than erroring")
        .settlement()
        .cloned()
        .expect("a Done against a board that reads Complete is legal");
    assert_eq!(
        done_first.item_state, "Done",
        "{HARNESS}: the first verdict is recorded as Done"
    );

    // NEGATIVE: the late `Cancelled` must not overwrite the real completion.
    let late_cancel = harness
        .settle_claim(
            &item_b,
            AgentWorkOutcome::Cancelled,
            Some("a cancel that arrived too late"),
        )
        .await
        .expect("the late cancel answers rather than erroring");
    assert!(
        !late_cancel.wrote(),
        "{HARNESS}: a `Cancelled` arriving after a `Done` is refused — it cannot erase a real completion (got {})",
        late_cancel.name()
    );
    assert_eq!(
        harness
            .work_item(&item_b)
            .await
            .expect("item B reads back")
            .state,
        "Done",
        "{HARNESS}: the committed verdict stays `Done`; the contradictory cancel wrote nothing"
    );
    assert_eq!(
        closed_run_count(&harness, &story_b).await,
        1,
        "{HARNESS}: the refused cancel closes no second run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE HEAD-TO-HEAD RACE. Half the participants settle `Done` and half settle `Cancelled`, all released at
    //    one barrier on one fresh item. Which verdict wins is the database's business and is deliberately NOT
    //    pinned — the race is a real race, not a scripted order. What IS pinned is that the two contradictory
    //    verdicts cannot both have been written: exactly one settle is answered, and the committed state is
    //    exactly that verdict.
    // -----------------------------------------------------------------------------------------------------------
    let story_c = format!("{marker}-c");
    harness.seed_story(&story_c).await.expect("story C seeds");
    let item_c = harness
        .seed_ready_item(&story_c)
        .await
        .expect("item C seeds");
    claim_and_begin(&harness, &story_c, &item_c).await;

    let contenders = 8;
    let racing_item = item_c.clone();
    let verdicts = harness
        .race(contenders, move |index, harness| {
            let item = racing_item.clone();
            async move {
                let outcome = if index % 2 == 0 {
                    AgentWorkOutcome::Done
                } else {
                    AgentWorkOutcome::Cancelled
                };
                let settled = harness
                    .settle_claim(&item, outcome, None)
                    .await
                    .expect("a settle answers rather than erroring");
                (
                    outcome,
                    // Which settle WROTE is `wrote()`. A `Conflict` or `Duplicate` answer carries a pair too — it is
                    // a report about the row, not this call's write — so `settlement().is_some()` counts the seven
                    // losers as winners.
                    settled.wrote(),
                )
            }
        })
        .await;

    let winners: Vec<_> = verdicts.iter().filter(|(_, wrote)| *wrote).collect();
    assert_eq!(
        winners.len(),
        1,
        "{HARNESS}: exactly one of {contenders} racing settles ends the run, got {}",
        winners.len()
    );

    // The committed verdict IS the winner's verdict — not merely "some terminal state". This is what distinguishes
    // "one of the two verdicts" from "the last writer's verdict, with the other one lost".
    let (winning_outcome, _) = winners[0];
    let raced = harness
        .work_item(&item_c)
        .await
        .expect("item C reads back after the race");
    assert_eq!(
        raced.state,
        winning_outcome.as_str(),
        "{HARNESS}: the committed verdict is the one settle that was answered, got {}",
        raced.state
    );
    assert!(
        matches!(raced.state.as_str(), "Done" | "Cancelled"),
        "{HARNESS}: the race settled to one of the two verdicts, got {}",
        raced.state
    );
    assert_eq!(
        closed_run_count(&harness, &story_c).await,
        1,
        "{HARNESS}: {contenders} racing settles close exactly one story run — the losers close nothing"
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

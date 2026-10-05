//! DB.CONCURRENCY — duplicate work claim (TST-DB-CONCURRENCY-002).
//!
//! Contract: **one work item, one owner, however many workers reach for it.** A queued item is claimed by
//! `ForgeEngineDao::claim_specific_agent_work`, which calls the database function `forge_claim_specific_agent_work`
//! (migration 262, `db/src/forge_engine.rs:246-260`). Three separate mechanisms have to hold for that to be true,
//! and each is asserted here against committed truth:
//!
//! 1. **The advisory claim lock.** `pg_advisory_xact_lock(9000212)` serialises claim *selection*, released when the
//!    caller's transaction ends. So the losers do not read a stale `Ready` and then all try to update it: they read
//!    the winner's committed `Claimed`.
//! 2. **The guarded update.** The move is `update … where w.id = … and w.state = 'Ready'`. Only the row that is
//!    still `Ready` when the lock is granted moves, so a loser updates zero rows and the function returns no row —
//!    `Ok(None)`, a refusal, not an error.
//! 3. **`attempts` counts claims, not callers.** The winning update is the only one that increments it, so a race
//!    of eight workers leaves `attempts = 1`. This is the assertion that would fail if the lock were removed and
//!    eight updates all matched: `attempts` would read 8, and the retry budget would be spent by callers that never
//!    owned anything.
//!
//! The negative cases are the ones that make the invariant mean something:
//!
//! - **A second serial item on the same story is refused.** The function checks for any open item
//!   (`state in ('Claimed','Running','Paused')`) on the story and returns without claiming. This is "one serial
//!   chain per STORY, not one per system" — and it is why an eighth caller cannot help itself by grabbing a
//!   sibling item on the same story.
//! - **A settled item cannot be re-claimed.** Once an item is terminal (`Done`), it is not `Ready`, so the guarded
//!   update matches nothing. A worker that arrives late cannot resurrect finished work.
//! - **A claimed item is not claimable again** by anyone, including the winner: the item is no longer `Ready`.
//!
//! Level: L4 Adversarial — `RaceHarness` puts every worker inside the racy region at one `ConcurrencyBarrier`, and
//! refuses PRODUCTION before any socket is opened.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_concurrency__002__duplicate_work_claim -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L4 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget};
use test_harness::RaceHarness;

const HARNESS: &str = "RaceHarness/L4 Adversarial";

/// How many workers race for one item. More than two, so "exactly one owner" is not an artifact of a pair.
const WORKERS: usize = 8;

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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); RaceHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-CONCURRENCY-002); the assay uses it.
async fn db_concurrency_002__duplicate_work_claim() {
    // 0. L4 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the duplicate-claim proof runs only on an isolated DEV target"
    );
    let marker = format!(
        "tstdbcon002-{}-{}",
        harness.namespace(),
        uuid::Uuid::new_v4()
    );

    // One isolated story (in `Planned`, so the unattended poller never reads it — see `seed_story`) with a single
    // `Ready` item. No other lane's open work can refuse these claims.
    let story_id = format!("{marker}-story");
    harness
        .seed_story(&story_id)
        .await
        .expect("the story fixture seeds");
    let item = harness
        .seed_ready_item(&story_id)
        .await
        .expect("the ready work item seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE RACE. WORKERS workers meet at one barrier and claim the SAME item. Exactly one may come away owning
    //    it; the rest must be refused with `Ok(None)` — a claim that loses is not an error, and not a second owner.
    // -----------------------------------------------------------------------------------------------------------
    let racing_item = item.clone();
    let claims = harness
        .race(WORKERS, move |index, harness| {
            let item = racing_item.clone();
            let worker = format!("tst-worker-{index}");
            async move {
                let won = harness
                    .engine()
                    .claim_specific_agent_work(&item, &worker)
                    .await
                    .expect("a claim answers rather than erroring");
                (worker, won.is_some())
            }
        })
        .await;

    let owners: Vec<&str> = claims
        .iter()
        .filter(|(_, owns)| *owns)
        .map(|(worker, _)| worker.as_str())
        .collect();
    assert_eq!(
        owners.len(),
        1,
        "{HARNESS}: exactly one of {WORKERS} workers owns {item}, got {owners:?}"
    );

    // Committed truth: the owner recorded on the row is that same worker, and nobody else.
    let state = harness
        .work_item(&item)
        .await
        .expect("the work item reads back");
    assert_eq!(
        state.state, "Claimed",
        "{HARNESS}: the racing item committed exactly one Claimed transition"
    );
    assert_eq!(
        state.claimed_by.as_deref(),
        Some(owners[0]),
        "{HARNESS}: the committed owner is the worker that was told it won, got {:?}",
        state.claimed_by
    );

    // The assertion that makes the advisory lock observable: `attempts` is incremented by the winning update and
    // by nothing else. Eight callers that all matched `state = 'Ready'` would read 8 here and have burned the
    // item's whole retry budget between them.
    assert_eq!(
        state.attempts, 1,
        "{HARNESS}: {WORKERS} racing claims increment attempts exactly once — only the owner consumed the retry budget"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE / REFUSAL — THE OWNER ITSELF CANNOT RE-CLAIM. The item is `Claimed`, not `Ready`, so the guarded
    //    update matches nothing and the same worker that just won is told `None` a moment later.
    // -----------------------------------------------------------------------------------------------------------
    let reclaim = harness
        .engine()
        .claim_specific_agent_work(&item, owners[0])
        .await
        .expect("the re-claim answers rather than erroring");
    assert!(
        reclaim.is_none(),
        "{HARNESS}: the winning worker cannot claim the item a second time"
    );
    assert_eq!(
        harness
            .work_item(&item)
            .await
            .expect("the work item reads back after the re-claim")
            .attempts,
        1,
        "{HARNESS}: a refused re-claim does not consume an attempt"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — ONE SERIAL ACTIVE ITEM PER STORY IS THE DATABASE'S, NOT THE CALLER'S. The same
    //    "one writer per story" the claim function checks is enforced independently by the partial unique index
    //    `agent_work_item_one_serial_active_per_story` (migration 262). So even a writer that skips the DAO
    //    entirely — a migration, a script, a second caller with a stale view — cannot put a second open serial
    //    item on a story. The claim function's early return is the polite half of this rule; the index is the half
    //    that holds when the polite half is bypassed, which is why this test bypasses it on purpose.
    // -----------------------------------------------------------------------------------------------------------
    let second_serial = sqlx::query(
        "insert into agent_work_item (story_id, state) values ($1, 'Ready') returning id::text",
    )
    .bind(&story_id)
    .fetch_one(harness.pool())
    .await;
    assert!(
        second_serial.is_err(),
        "{HARNESS}: the database refuses a second open serial item on a story that already holds one"
    );
    let unique_violation = second_serial
        .expect_err("the second serial insert is refused")
        .to_string();
    assert!(
        unique_violation.contains("agent_work_item_one_serial_active_per_story"),
        "{HARNESS}: the refusal names the one-serial-item-per-story constraint, got {unique_violation}"
    );

    // Committed truth: the refused insert wrote nothing, so the story still has exactly the one item it had.
    assert_eq!(
        harness
            .item_count(&story_id)
            .await
            .expect("the item count reads"),
        1,
        "{HARNESS}: the refused insert leaves the story with exactly one work item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / FAULT — A SETTLED ITEM CANNOT BE RE-CLAIMED. Once the item is terminal it is not `Ready`, so a
    //    worker arriving after the work finished finds nothing to claim. Without the state guard this would be a
    //    resurrection of finished work.
    // -----------------------------------------------------------------------------------------------------------
    let settled_story = format!("{marker}-settled-story");
    harness
        .seed_story(&settled_story)
        .await
        .expect("the settled story fixture seeds");
    let settled = harness
        .seed_ready_item(&settled_story)
        .await
        .expect("the settled run's work item seeds");
    assert!(
        harness
            .engine()
            .claim_specific_agent_work(&settled, "tst-worker-settled")
            .await
            .expect("the first claim of the settled item answers")
            .is_some(),
        "{HARNESS}: the first worker claims the item it is alone on"
    );
    assert!(
        harness
            .engine()
            .finish_agent_work_run(
                &settled,
                AgentWorkOutcome::Cancelled,
                Some("settled by the proof")
            )
            .await
            .expect("the settle answers")
            .is_some(),
        "{HARNESS}: the claimed item settles as Cancelled"
    );
    assert_eq!(
        harness
            .work_item(&settled)
            .await
            .expect("the settled item reads back")
            .state,
        "Cancelled",
        "{HARNESS}: the cancellation is committed"
    );
    let resurrect = harness
        .engine()
        .claim_specific_agent_work(&settled, "tst-worker-late")
        .await
        .expect("the late claim answers rather than erroring");
    assert!(
        resurrect.is_none(),
        "{HARNESS}: a settled item is not claimable — finished work cannot be resurrected"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / NO LEFTOVER. Every row this run seeded is removed; a non-zero leftover fails the proof, because
    //    a stray `Ready` item is a row the unattended poller could later pick up.
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

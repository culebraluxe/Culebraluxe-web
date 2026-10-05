//! DB.CONCURRENCY — claim vs cancel (TST-DB-CONCURRENCY-004).
//!
//! Contract: **a cancel cannot take an item it never owned, and cannot resurrect one it has already settled.**
//! This is the race between the two writers that meet on one `agent_work_item`: a worker calling
//! `ForgeEngineDao::claim_specific_agent_work` (`Ready → Claimed`, migration 262) and a caller calling
//! `ForgeEngineDao::finish_agent_work_run(.., Cancelled, ..)` (the terminal write, migration 263).
//!
//! They are not two arbitrations of the same kind, which is what makes the interleaving worth proving:
//!
//! - the **claim** is guarded by `where w.state = 'Ready'` under `pg_advisory_xact_lock(9000212)`, and it refuses
//!   while any item on the story is already open;
//! - the **cancel** is guarded by `where … i.state in ('Claimed','Running') for update of i` — so it is *illegal
//!   against a `Ready` item*. A cancel aimed at work that has not started is a no-op, not a pre-emptive kill.
//!
//! Every reachable interleaving is asserted, because the contract is not "one of two wins" but a specific claim
//! about which side may do what:
//!
//! 1. **Cancel first, then claim.** The cancel finds nothing in `Claimed`/`Running` and is refused (`None`). The
//!    item is still `Ready`, so the claim afterwards succeeds and owns a live item. A cancel that could land on a
//!    `Ready` item would let anyone delete queued work they never held.
//! 2. **Claim first, then cancel.** The claim owns the item (`Claimed`, `attempts = 1`), and the cancel is then
//!    legal: it takes the terminal state and the claim cannot be replayed over it.
//! 3. **Head to head.** Many claimers and many cancellers released at one barrier. Exactly one claim may own the
//!    item, at most one cancel may settle it, and the committed row must be a legal state — never a resurrection
//!    of a settled item, and never two owners.
//!
//! The negative cases are the refusals: cancelling a `Ready` item, cancelling an already-cancelled item, and
//! re-claiming after the cancel.
//!
//! Level: L4 Adversarial — `RaceHarness` puts every participant inside the racy region at one `ConcurrencyBarrier`,
//! and refuses PRODUCTION before any socket is opened.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_concurrency__004__claim_vs_cancel -- --ignored
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); RaceHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-CONCURRENCY-004); the assay uses it.
async fn db_concurrency_004__claim_vs_cancel() {
    // 0. L4 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the claim-vs-cancel proof runs only on an isolated DEV target"
    );
    let marker = format!(
        "tstdbcon004-{}-{}",
        harness.namespace(),
        uuid::Uuid::new_v4()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. INTERLEAVING A — CANCEL FIRST, THEN CLAIM. A cancel is guarded by `state in ('Claimed','Running')`, so it
    //    is illegal against queued work. It must be refused, and the item must survive it untouched: a cancel
    //    that could land on a `Ready` item would let anyone delete work they never held.
    // -----------------------------------------------------------------------------------------------------------
    let story_a = format!("{marker}-a");
    harness.seed_story(&story_a).await.expect("story A seeds");
    let item_a = harness
        .seed_ready_item(&story_a)
        .await
        .expect("item A seeds");

    let premature_cancel = harness
        .engine()
        .finish_agent_work_run(
            &item_a,
            AgentWorkOutcome::Cancelled,
            Some("aimed at unclaimed work"),
        )
        .await
        .expect("the cancel answers rather than erroring");
    assert!(
        premature_cancel.is_none(),
        "{HARNESS}: a cancel against a `Ready` item is refused — a run that never began cannot be cancelled"
    );
    assert_eq!(
        harness
            .work_item(&item_a)
            .await
            .expect("item A reads back")
            .state,
        "Ready",
        "{HARNESS}: the refused cancel leaves the item `Ready` and claimable"
    );
    assert_eq!(
        harness
            .work_item(&item_a)
            .await
            .expect("item A reads back")
            .attempts,
        0,
        "{HARNESS}: the refused cancel consumes no attempt"
    );

    // The claim that follows now succeeds, which is the point: the cancel did not take the work away.
    assert!(
        harness
            .engine()
            .claim_specific_agent_work(&item_a, "tst-worker-a")
            .await
            .expect("the claim answers")
            .is_some(),
        "{HARNESS}: after a refused premature cancel the item is still claimable"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. INTERLEAVING B — CLAIM FIRST, THEN CANCEL. Now the cancel is legal: the item is `Claimed`, so the guard
    //    admits it and the terminal write takes the row. The claim cannot be replayed over the result.
    // -----------------------------------------------------------------------------------------------------------
    let cancellation = harness
        .engine()
        .finish_agent_work_run(
            &item_a,
            AgentWorkOutcome::Cancelled,
            Some("cancelled by the proof"),
        )
        .await
        .expect("the cancel answers rather than erroring")
        .expect("a cancel against a claimed item is legal and settles it");
    assert_eq!(
        cancellation.item_state, "Cancelled",
        "{HARNESS}: the cancel settles the claimed item as Cancelled"
    );
    assert_eq!(
        harness
            .work_item(&item_a)
            .await
            .expect("item A reads back after the cancel")
            .state,
        "Cancelled",
        "{HARNESS}: the cancellation is committed"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — THE CANCELLED ITEM CANNOT BE RE-CLAIMED. The item is no longer `Ready`, so the
    //    claim's guarded update matches nothing. This is the "no resurrection" half of the contract: a cancel is
    //    final, not a pause.
    // -----------------------------------------------------------------------------------------------------------
    let re_claim = harness
        .engine()
        .claim_specific_agent_work(&item_a, "tst-worker-late")
        .await
        .expect("the re-claim answers rather than erroring");
    assert!(
        re_claim.is_none(),
        "{HARNESS}: a cancelled item is not claimable — a cancel is final, not a pause"
    );
    assert_eq!(
        harness
            .work_item(&item_a)
            .await
            .expect("item A reads back after the re-claim")
            .state,
        "Cancelled",
        "{HARNESS}: the refused re-claim leaves the cancellation in place"
    );

    // And a second cancel is refused too: the terminal write happens once.
    let second_cancel = harness
        .engine()
        .finish_agent_work_run(
            &item_a,
            AgentWorkOutcome::Cancelled,
            Some("a second cancel"),
        )
        .await
        .expect("the second cancel answers rather than erroring");
    assert!(
        second_cancel.is_none(),
        "{HARNESS}: a second cancel on an already-cancelled item is refused"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE HEAD-TO-HEAD RACE. Many claimers and many cancellers, all released at one barrier on one fresh item.
    //    The outcome is deliberately NOT fixed to a single winner: which side lands first is the database's
    //    business. What is fixed — and asserted — is that the committed row is a legal state, that the claim was
    //    never taken twice, and that a cancel which landed did not leave the item claimable again.
    // -----------------------------------------------------------------------------------------------------------
    let story_b = format!("{marker}-b");
    harness.seed_story(&story_b).await.expect("story B seeds");
    let item_b = harness
        .seed_ready_item(&story_b)
        .await
        .expect("item B seeds");

    // Four claimers and four cancellers, all in the racy region together.
    let racing_item = item_b.clone();
    let outcomes = harness
        .race(8, move |index, harness| {
            let item = racing_item.clone();
            async move {
                if index % 2 == 0 {
                    let worker = format!("tst-racer-{index}");
                    let claim = harness
                        .engine()
                        .claim_specific_agent_work(&item, &worker)
                        .await
                        .expect("a claim answers rather than erroring");
                    (true, claim.is_some())
                } else {
                    let cancel = harness
                        .engine()
                        .finish_agent_work_run(
                            &item,
                            AgentWorkOutcome::Cancelled,
                            Some("racing cancel"),
                        )
                        .await
                        .expect("a cancel answers rather than erroring");
                    (false, cancel.is_some())
                }
            }
        })
        .await;

    let claim_wins = outcomes
        .iter()
        .filter(|(is_claim, won)| *is_claim && *won)
        .count();
    let cancel_wins = outcomes
        .iter()
        .filter(|(is_claim, won)| !*is_claim && *won)
        .count();
    assert!(
        claim_wins <= 1,
        "{HARNESS}: the racing item has at most one owner, got {claim_wins}"
    );
    assert!(
        cancel_wins <= 1,
        "{HARNESS}: the racing item is settled at most once, got {cancel_wins}"
    );
    // A cancel can only be legal against a `Claimed`/`Running` item, so if a cancel won, a claim had to have
    // landed first. That ordering is the invariant; neither can happen alone.
    if cancel_wins == 1 {
        assert_eq!(
            claim_wins, 1,
            "{HARNESS}: a cancel only settles an item that was claimed first — the two outcomes cannot both hold for one item"
        );
    }

    // Committed truth: the item is in a legal state, and `attempts` counted only real claims.
    let raced = harness
        .work_item(&item_b)
        .await
        .expect("item B reads back after the race");
    assert!(
        matches!(raced.state.as_str(), "Ready" | "Claimed" | "Cancelled"),
        "{HARNESS}: the raced item committed a legal state, got {}",
        raced.state
    );
    assert_eq!(
        raced.attempts,
        i64::from(claim_wins == 1),
        "{HARNESS}: attempts counts the real owner exactly once, got {}",
        raced.attempts
    );
    // `claimed_by` is the item's audit trail, not its current holder: `forge_finish_agent_work_run` clears it only
    // on the one branch that puts the item BACK in the queue (a cleared claim), and leaves it on a terminal write so
    // a settled item still names who owned it. So the assertion is against the number of real claims, not the
    // state — an owner is recorded if and only if a claim actually landed, and it survives the settlement.
    assert_eq!(
        raced.claimed_by.is_some(),
        claim_wins == 1,
        "{HARNESS}: an owner is recorded if and only if one claim really landed ({claim_wins}), and a terminal \
         settle keeps the attribution for the audit trail"
    );

    // And the raced item is not left in a state a stranger could pick up: if it settled, nothing can claim it.
    if raced.state == "Cancelled" {
        assert!(
            harness
                .engine()
                .claim_specific_agent_work(&item_b, "tst-worker-after")
                .await
                .expect("the post-race claim answers")
                .is_none(),
            "{HARNESS}: an item the race cancelled is not claimable afterwards"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / NO LEFTOVER. Every row this run seeded is removed; a non-zero leftover fails the proof, because
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

//! FORGE.CLAIM-010 — agent work ownership follows Ready -> Claimed -> Running with no skipped or duplicate
//! transition (TST-FORGE-CLAIM-010).
//!
//! CONTRACT. Ownership of a story's work moves through exactly three states in order, and each step is a
//! compare-and-set that admits one predecessor: the claim (`forge_claim_specific_agent_work`, migration 262)
//! moves `Ready → Claimed` and nothing else, and the begin (`forge_begin_agent_work_run`, migration 264) moves
//! `Claimed → Running` and nothing else — opening the Story Run in the same transaction. A skipped transition
//! (`Ready → Running` with no claim) would run work nobody owns; a duplicate (`Claimed → Running` twice) would open
//! two runs for one claim. Both answer `Ok(None)`: the caller does not own the run it is about to start.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__010__agent_work_ownership_follows_ready_claimed_running_with_no_skipped_or_duplicate_transition -- --ignored

use db::{AgentWorkOutcome, DbTarget};
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER: &str = "forge-ownership-owner";

async fn item_state(pool: &sqlx::PgPool, item_id: &str) -> String {
    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("read the committed claim back")
}

async fn run_count(pool: &sqlx::PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from storyboard_story_run where story_id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the run ledger is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_010__agent_work_ownership_follows_ready_claimed_running_with_no_skipped_or_duplicate_transition(
) {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: an ownership contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let story = format!("FORGE-CLAIM-010-CHAIN-{tag}");

    // The board's own trigger queues the work: the chain starts at Ready, like production.
    let item = harness
        .seed_ready_story(&story)
        .await
        .expect("the Ready trigger must queue exactly one item");
    assert_eq!(item_state(pool, &item).await, "Ready");

    // ── NO SKIP: a run cannot open without a claim. ───────────────────────────
    let skipped = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs");
    assert_eq!(
        skipped, None,
        "{HARNESS}: `Ready → Running` is refused — a run with no claim is work nobody owns"
    );
    assert_eq!(item_state(pool, &item).await, "Ready");
    assert_eq!(run_count(pool, &story).await, 0);

    // ── STEP ONE: Ready -> Claimed, exactly once. ─────────────────────────────
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable by one worker");
    assert_eq!(claimed.state, "Claimed");
    assert_eq!(claimed.claimed_by.as_deref(), Some(OWNER));
    let attempts: i32 =
        sqlx::query_scalar("select attempts from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .expect("the attempt count is readable");
    assert_eq!(attempts, 1, "{HARNESS}: the claim counts its attempt");

    // No duplicate claim: the row has left Ready, so the CAS finds nothing.
    let reclaimed = harness
        .engine()
        .claim_specific_agent_work(&item, "forge-second-owner")
        .await
        .expect("the production claim runs");
    assert!(
        reclaimed.is_none(),
        "{HARNESS}: a claimed item cannot be claimed again — ownership is exclusive"
    );
    assert_eq!(item_state(pool, &item).await, "Claimed");

    // ── STEP TWO: Claimed -> Running, exactly once, with the run. ────────────
    let begun = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");
    assert!(
        !begun.story_run_id.is_empty(),
        "{HARNESS}: beginning execution opens the durable Story Run"
    );
    assert_eq!(item_state(pool, &item).await, "Running");
    assert_eq!(run_count(pool, &story).await, 1);

    // No duplicate begin: the row has left Claimed, so no second run opens.
    let rebegun = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs");
    assert_eq!(
        rebegun, None,
        "{HARNESS}: a running item cannot begin again — one claim opens one run"
    );
    assert_eq!(
        run_count(pool, &story).await,
        1,
        "{HARNESS}: the refused begin wrote no second run row"
    );

    // No late claim over a running item either.
    let late = harness
        .engine()
        .claim_specific_agent_work(&item, "forge-late-owner")
        .await
        .expect("the production claim runs");
    assert!(
        late.is_none(),
        "{HARNESS}: a running item cannot be claimed — the owner is the claim that opened the run"
    );

    // The chain settles through the same guard: terminate the proof cleanly.
    let settled = harness
        .engine()
        .finish_agent_work_run(
            &item,
            AgentWorkOutcome::Cancelled,
            Some("proof complete"),
            None,
        )
        .await
        .expect("the production settle runs")
        .expect("a Running claim must settle");
    assert_eq!(settled.item_state, "Cancelled");
    assert_eq!(item_state(pool, &item).await, "Cancelled");

    harness
        .cleanup_story(&story)
        .await
        .expect("the harness owns the rows it made and puts them back");
    let leftovers: i64 = sqlx::query_scalar("select count(*) from storyboard_story where id = $1")
        .bind(&story)
        .fetch_one(pool)
        .await
        .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

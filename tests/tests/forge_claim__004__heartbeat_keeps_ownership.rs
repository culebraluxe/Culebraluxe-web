//! FORGE.CLAIM-004 — heartbeat keeps ownership (TST-FORGE-CLAIM-004).
//!
//! CONTRACT. A live worker holds its claim by heartbeating: `heartbeat_agent_work`
//! (`db/src/forge_engine.rs:346`) refreshes `updated_at` on a `Claimed`/`Running` item, and `stale_agent_work`
//! decides staleness on `updated_at` alone — so a heartbeated claim is invisible to the recovery sweep and keeps its
//! owner, while a claim whose owner went silent becomes recoverable. Without this, a run longer than the stale
//! window would be requeued while still running and the next tick would launch a second engine over the same story.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__004__heartbeat_keeps_ownership -- --ignored

use db::DbTarget;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER: &str = "forge-heartbeat-owner";
/// Older than the sweep window below, so the claim starts out recoverable and only the heartbeat rescues it.
const STALE_AGE_MINUTES: i64 = 120;
const SWEEP_MINUTES: i64 = 60;

async fn backdate(pool: &sqlx::PgPool, item_id: &str, minutes: i64) {
    sqlx::query("update agent_work_item set updated_at = now() - ($2::text || ' minutes')::interval where id = $1::uuid")
        .bind(item_id)
        .bind(minutes.to_string())
        .execute(pool)
        .await
        .expect("age the proof claim in the database's own time");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_004__heartbeat_keeps_ownership() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: a heartbeat contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let story = format!("FORGE-CLAIM-004-HB-{tag}");

    // The board's own trigger queues the item: the proof starts from the queue production claims from.
    let item = harness
        .seed_ready_story(&story)
        .await
        .expect("the Ready trigger must queue exactly one item");
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");

    // Age the claim past the sweep window: without a heartbeat this claim is recoverable.
    backdate(pool, &item, STALE_AGE_MINUTES).await;
    let stale: Vec<String> = harness
        .control()
        .stale_agent_work(SWEEP_MINUTES)
        .await
        .expect("the stale window is answerable")
        .into_iter()
        .filter(|row| row.story_id == story)
        .map(|row| row.id)
        .collect();
    assert_eq!(
        stale,
        vec![item.clone()],
        "{HARNESS}: the silent claim must be recoverable before its heartbeat"
    );

    // ── THE CONTRACT: the heartbeat keeps the claim. ─────────────────────────
    assert!(
        harness
            .engine()
            .heartbeat_agent_work(&item, OWNER, std::time::Duration::from_secs(300))
            .await
            .expect("the production heartbeat runs"),
        "{HARNESS}: a live `Claimed` claim must be heartbeatable"
    );
    let (state, claimed_by): (String, Option<String>) =
        sqlx::query_as("select state, claimed_by from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .expect("read the committed claim back");
    assert_eq!(state, "Claimed", "{HARNESS}: the heartbeat moves no state");
    assert_eq!(
        claimed_by.as_deref(),
        Some(OWNER),
        "{HARNESS}: the heartbeat keeps the owner — it does not clear or steal the claim"
    );

    // The sweep no longer sees it: freshness is the database's `updated_at` against the window.
    let after: Vec<String> = harness
        .control()
        .stale_agent_work(SWEEP_MINUTES)
        .await
        .expect("the stale window is answerable")
        .into_iter()
        .filter(|row| row.story_id == story)
        .map(|row| row.id)
        .collect();
    assert!(
        after.is_empty(),
        "{HARNESS}: a heartbeated claim is ALIVE; the sweep must not see it"
    );

    // Ownership survives into execution: the run still opens for the heartbeat's owner.
    let begun = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs")
        .expect("a heartbeated claim still owns its run");
    assert!(
        !begun.story_run_id.is_empty(),
        "{HARNESS}: the kept claim opens its run"
    );
    assert!(
        harness
            .engine()
            .heartbeat_agent_work(&item, OWNER, std::time::Duration::from_secs(300))
            .await
            .expect("the production heartbeat runs"),
        "{HARNESS}: a `Running` claim stays heartbeatable while its run is in flight"
    );

    // ── NEGATIVE: a heartbeat touches only a live claim. ─────────────────────
    let missing = harness
        .engine()
        .heartbeat_agent_work(
            "00000000-0000-0000-0000-000000000000",
            OWNER,
            std::time::Duration::from_secs(300),
        )
        .await
        .expect("the production heartbeat runs");
    assert!(
        !missing,
        "{HARNESS}: heartbeating a row that is not a live claim must answer false, not claim it"
    );

    // Settle the claim, then prove the heartbeat lets go of the dead.
    sqlx::query("update storyboard_story set status = 'Complete' where id = $1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("the board confirms the work");
    harness
        .engine()
        .finish_agent_work_run(&item, db::AgentWorkOutcome::Done, None, None)
        .await
        .expect("the production settle runs")
        .expect("Done over a Complete board must settle");
    assert!(
        !harness
            .engine()
            .heartbeat_agent_work(&item, OWNER, std::time::Duration::from_secs(300))
            .await
            .expect("the production heartbeat runs"),
        "{HARNESS}: a settled (`Done`) claim is no longer heartbeatable — the worker must stop, not revive it"
    );

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

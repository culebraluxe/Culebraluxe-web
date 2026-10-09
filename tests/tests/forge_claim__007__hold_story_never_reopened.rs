//! FORGE.CLAIM-007 — Hold story never reopened (TST-FORGE-CLAIM-007).
//!
//! CONTRACT. `Hold` already needs a human: the settlement pair (`forge_settlement_pair`, migration 263) moves the
//! board to `Hold` (never back from it), and the stale recovery (`forge_requeue_stale_work`, migration 266) ends a
//! stale claim over a held story as `Error` — "the board is not reopened". The dispatch reconcile restates only
//! `In Progress` stories and queues only `Ready` ones (migration 265), so a held story is invisible to both sweeps.
//! A sweep that reopened a held story would hand a human-gated story back to the unattended queue.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__007__hold_story_never_reopened -- --ignored

use db::DbTarget;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER: &str = "forge-hold-owner";

async fn story_status(pool: &sqlx::PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the committed story back")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_007__hold_story_never_reopened() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: a hold contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let held_story = format!("FORGE-CLAIM-007-HELD-{tag}");

    let item = harness
        .seed_ready_story(&held_story)
        .await
        .expect("the Ready trigger must queue exactly one item");
    sqlx::query("update storyboard_story set status = 'In Progress' where id = $1")
        .bind(&held_story)
        .execute(pool)
        .await
        .expect("the board marks the story running");
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .begin_claim(&item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");

    // A human takes ownership: the board moves to Hold under a running claim.
    harness
        .engine()
        .mark_story_human_hold(&held_story, "proof hold: a human owns this story")
        .await
        .expect("the production hold runs");
    assert_eq!(story_status(pool, &held_story).await, "Hold");

    // ── THE CONTRACT: recovery ends the claim WITHOUT reopening the story. ──
    harness
        .control()
        .requeue_stale_work(&item, &held_story)
        .await
        .expect("recovery must settle a stale claim over held work, not error");
    let (state, error_text): (String, Option<String>) =
        sqlx::query_as("select state, error_text from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .expect("read the settled claim back");
    assert_eq!(
        state, "Error",
        "{HARNESS}: a stale claim over a story a human holds settles Error, never Ready"
    );
    assert_eq!(
        error_text.as_deref(),
        Some("stale claim on a story a human holds")
    );
    assert_eq!(
        story_status(pool, &held_story).await,
        "Hold",
        "{HARNESS}: the human gate is not reopened by a bookkeeping sweep"
    );

    // The dispatch reconcile leaves the held pair alone: no restate (Hold is not In Progress), no requeue (Hold
    // is not Ready), no clear (the item is terminal, not Ready/Paused).
    let before: (String, String) =
        sqlx::query_as("select state, updated_at::text from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .expect("read the held claim back");
    harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("the production reconcile runs");
    let after: (String, String) =
        sqlx::query_as("select state, updated_at::text from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .expect("read the held claim back");
    assert_eq!(
        after.0, "Error",
        "{HARNESS}: reconcile must not move a held claim"
    );
    assert_eq!(
        after.1, before.1,
        "{HARNESS}: reconcile must not touch a held claim at all — not even its `updated_at`"
    );
    assert_eq!(story_status(pool, &held_story).await, "Hold");
    let ready_items: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&held_story)
    .fetch_one(pool)
    .await
    .expect("the queue is readable");
    assert_eq!(
        ready_items, 0,
        "{HARNESS}: reconcile must not queue a retry for a story a human holds"
    );

    // ── NEGATIVE: a second recovery is a no-op, and the story is still Hold. ──
    harness
        .control()
        .requeue_stale_work(&item, &held_story)
        .await
        .expect("recovering an already-settled claim is a no-op, not an error");
    assert_eq!(story_status(pool, &held_story).await, "Hold");

    harness
        .cleanup_story(&held_story)
        .await
        .expect("the harness owns the rows it made and puts them back");
    let leftovers: i64 = sqlx::query_scalar("select count(*) from storyboard_story where id = $1")
        .bind(&held_story)
        .fetch_one(pool)
        .await
        .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

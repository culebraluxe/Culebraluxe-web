//! FORGE.CLAIM-006 — Complete story never resurrected (TST-FORGE-CLAIM-006).
//!
//! CONTRACT. `Complete` is the truth and no failure demotes it: the settlement pair
//! (`forge_settlement_pair`, migration 263) accepts `Done` only over a `Complete` (or `Hold`) board, and the stale
//! recovery (`forge_requeue_stale_work`, migration 266) settles a stale claim over landed work `Done` instead of
//! requeueing it. Once a story is `Complete` with its claim `Done`, no sweep — recovery or reconcile — may move
//! either row back into the queue. A resurrected Complete story would rerun landed work.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__006__complete_story_never_resurrected -- --ignored

use db::{AgentWorkOutcome, DbTarget};
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER: &str = "forge-complete-owner";

async fn story_status(pool: &sqlx::PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the committed story back")
}

async fn item_state(pool: &sqlx::PgPool, item_id: &str) -> (String, String) {
    sqlx::query_as("select state, updated_at::text from agent_work_item where id = $1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("read the committed claim back")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_006__complete_story_never_resurrected() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: a settlement contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let landed_story = format!("FORGE-CLAIM-006-LANDED-{tag}");
    let refused_story = format!("FORGE-CLAIM-006-REFUSED-{tag}");

    // ── THE CONTRACT: Done over a Complete board settles, and stays settled. ──
    let item = harness
        .seed_ready_story(&landed_story)
        .await
        .expect("the Ready trigger must queue exactly one item");
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
    sqlx::query("update storyboard_story set status = 'Complete', completion = 100 where id = $1")
        .bind(&landed_story)
        .execute(pool)
        .await
        .expect("the board confirms the work landed");
    let settlement = harness
        .settle_claim_writing(&item, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs");
    assert_eq!(
        settlement.item_state, "Done",
        "{HARNESS}: the claim settles Done"
    );
    assert_eq!(
        settlement.story_status, None,
        "{HARNESS}: the board is already Complete — the settle writes no board move"
    );
    assert_eq!(story_status(pool, &landed_story).await, "Complete");

    // Exactly-once: a second settle is a no-op, not a second verdict.
    // Exactly-once: a second settle is a no-op, not a second verdict. Read as the WRITE it is not — the typed
    // answer (migration 278) says which fact it is instead of collapsing them into `None`.
    let second = harness
        .settle_claim(&item, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs");
    assert!(
        !second.wrote(),
        "{HARNESS}: a settled claim settles exactly once — the guard makes a second settle a no-op (got {})",
        second.name()
    );

    // The recovery sweep cannot resurrect it: the guard's `state in (...)` admits no terminal row.
    let before = item_state(pool, &item).await;
    harness
        .control()
        .requeue_stale_work(&item, &landed_story)
        .await
        .expect("recovering a settled claim is a no-op, not an error");
    let after = item_state(pool, &item).await;
    assert_eq!(
        after.0, "Done",
        "{HARNESS}: a terminal claim is never requeued"
    );
    assert_eq!(
        after.1, before.1,
        "{HARNESS}: a no-op recovery commits NOTHING — the terminal row is byte-for-byte what it was"
    );
    assert_eq!(story_status(pool, &landed_story).await, "Complete");

    // The dispatch reconcile cannot resurrect it either: no new Ready item, board unmoved.
    harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("the production reconcile runs");
    let ready_items: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&landed_story)
    .fetch_one(pool)
    .await
    .expect("the queue is readable");
    assert_eq!(
        ready_items, 0,
        "{HARNESS}: reconcile must not queue a retry for landed work"
    );
    assert_eq!(story_status(pool, &landed_story).await, "Complete");

    // ── NEGATIVE: Done is ACCEPTED only because the board confirms it. ───────
    // Over a board that still expects a run, the same Done is REFUSED — the item records Error and the story is
    // held. This is the guard that makes the positive case meaningful: without it every Done would settle.
    let refused_item = harness
        .seed_ready_story(&refused_story)
        .await
        .expect("the Ready trigger must queue exactly one item");
    sqlx::query("update storyboard_story set status = 'In Progress' where id = $1")
        .bind(&refused_story)
        .execute(pool)
        .await
        .expect("the board still expects a run");
    harness
        .engine()
        .claim_specific_agent_work(&refused_item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .begin_claim(&refused_item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");
    let refused = harness
        .settle_claim_writing(&refused_item, AgentWorkOutcome::Done, None)
        .await
        .expect("the production settle runs");
    assert_eq!(
        refused.item_state, "Error",
        "{HARNESS}: Done without the board's confirmation is REFUSED — the item records Error"
    );
    assert_eq!(refused.story_status.as_deref(), Some("Hold"));
    assert!(
        refused
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("Done refused"),
        "{HARNESS}: the refusal names itself: {:?}",
        refused.reason
    );
    assert_eq!(story_status(pool, &refused_story).await, "Hold");

    for story in [&landed_story, &refused_story] {
        harness
            .cleanup_story(story)
            .await
            .expect("the harness owns the rows it made and puts them back");
    }
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id = any($1::text[])")
            .bind(vec![landed_story, refused_story])
            .fetch_one(pool)
            .await
            .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

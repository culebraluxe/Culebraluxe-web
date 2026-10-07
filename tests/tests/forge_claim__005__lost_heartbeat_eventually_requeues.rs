//! FORGE.CLAIM-005 — lost heartbeat eventually requeues (TST-FORGE-CLAIM-005).
//!
//! CONTRACT. A claim whose owner stopped heartbeating is **eventually** recoverable: "eventually" is the worker's
//! stale sweep, and the sweep's input is `stale_agent_work` (the `updated_at` window, `db/src/forge_control.rs:61`)
//! with its output `requeue_stale_work` (migration 266 `forge_requeue_stale_work`), which returns a claim the board
//! still expects to the queue (`Ready`/`Ready`) with the claim cleared. The twin that kept heartbeating is untouched.
//! A lost heartbeat therefore costs the story a retry, never its turn — and never a second engine over live work.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__005__lost_heartbeat_eventually_requeues -- --ignored

use db::DbTarget;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const SILENT_OWNER: &str = "forge-silent-owner";
const LIVE_OWNER: &str = "forge-live-owner";
const STALE_AGE_MINUTES: i64 = 120;
const SWEEP_MINUTES: i64 = 60;

async fn backdate(pool: &sqlx::PgPool, item_id: &str) {
    sqlx::query("update agent_work_item set updated_at = now() - ($2::text || ' minutes')::interval where id = $1::uuid")
        .bind(item_id)
        .bind(STALE_AGE_MINUTES.to_string())
        .execute(pool)
        .await
        .expect("age the proof claim in the database's own time");
}

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
async fn forge_claim_005__lost_heartbeat_eventually_requeues() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: a requeue contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let silent_story = format!("FORGE-CLAIM-005-SILENT-{tag}");
    let live_story = format!("FORGE-CLAIM-005-LIVE-{tag}");

    // Two identical claims, both running, both gone silent past the window.
    for (story, owner) in [(&silent_story, SILENT_OWNER), (&live_story, LIVE_OWNER)] {
        let item = harness
            .seed_ready_story(story)
            .await
            .expect("the Ready trigger must queue exactly one item");
        sqlx::query("update storyboard_story set status = 'In Progress' where id = $1")
            .bind(story)
            .execute(pool)
            .await
            .expect("the board marks the story running");
        harness
            .engine()
            .claim_specific_agent_work(&item, owner)
            .await
            .expect("the production claim runs")
            .expect("a Ready item must be claimable");
        harness
            .engine()
            .begin_agent_work_run(&item)
            .await
            .expect("the production begin runs")
            .expect("a Claimed item must open its run");
        backdate(pool, &item).await;
    }
    let silent_item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1")
            .bind(&silent_story)
            .fetch_one(pool)
            .await
            .expect("the silent proof item is readable");
    let live_item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1")
            .bind(&live_story)
            .fetch_one(pool)
            .await
            .expect("the live proof item is readable");

    // Only the live twin heartbeats — the difference under test is exactly one heartbeat.
    assert!(
        harness
            .engine()
            .heartbeat_agent_work(&live_item)
            .await
            .expect("the production heartbeat runs"),
        "{HARNESS}: the live twin's heartbeat must hold"
    );
    let live_kept: (String, String) =
        sqlx::query_as("select state, updated_at::text from agent_work_item where id = $1::uuid")
            .bind(&live_item)
            .fetch_one(pool)
            .await
            .expect("read the heartbeated claim back");

    // ── "EVENTUALLY": the sweep finds the silent claim and not the live one. ──
    let discovered: Vec<String> = harness
        .control()
        .stale_agent_work(SWEEP_MINUTES)
        .await
        .expect("the stale window is answerable")
        .into_iter()
        .filter(|row| row.story_id == silent_story || row.story_id == live_story)
        .map(|row| row.story_id)
        .collect();
    assert!(
        discovered.contains(&silent_story),
        "{HARNESS}: a claim silent past the window must be discovered as stale"
    );
    assert!(
        !discovered.contains(&live_story),
        "{HARNESS}: the heartbeated twin is ALIVE; the sweep must not see it"
    );

    // ── THE REQUEUE: the lost claim goes back to the queue, cleared. ─────────
    harness
        .control()
        .requeue_stale_work(&silent_item, &silent_story)
        .await
        .expect("a stale claim the board still expects must be recoverable");
    let (state, claimed_by, claimed_at, started_at, finished_at): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select state, claimed_by, claimed_at::text, started_at::text, finished_at::text
           from agent_work_item where id = $1::uuid",
    )
    .bind(&silent_item)
    .fetch_one(pool)
    .await
    .expect("read the requeued claim back");
    assert_eq!(
        state, "Ready",
        "{HARNESS}: the lost claim returns to the queue — a retry, not a verdict"
    );
    assert_eq!(claimed_by, None, "{HARNESS}: a requeued row names nobody");
    assert_eq!(claimed_at, None);
    assert_eq!(started_at, None);
    assert_eq!(finished_at, None);
    assert_eq!(
        story_status(pool, &silent_story).await,
        "Ready",
        "{HARNESS}: the board moves with the item, or nothing dispatches the retry"
    );

    // The requeue re-timestamps, so the retry is not immediately stale again.
    let rediscovered: Vec<String> = harness
        .control()
        .stale_agent_work(SWEEP_MINUTES)
        .await
        .expect("the stale window is answerable")
        .into_iter()
        .filter(|row| row.story_id == silent_story || row.story_id == live_story)
        .map(|row| row.story_id)
        .collect();
    assert!(
        rediscovered.is_empty(),
        "{HARNESS}: after recovery neither proof story is stale — recovery removed them from the sweep's own \
         input, not merely from this test's expectations: {rediscovered:?}"
    );

    // ── NEGATIVE: the live twin survived byte-for-byte. ──────────────────────
    let live_after: (String, String) =
        sqlx::query_as("select state, updated_at::text from agent_work_item where id = $1::uuid")
            .bind(&live_item)
            .fetch_one(pool)
            .await
            .expect("read the live twin back");
    assert_eq!(
        live_after.0, "Running",
        "{HARNESS}: the heartbeating claim must NOT be requeued — otherwise the next tick starts a second engine \
         over a story that is still running"
    );
    assert_eq!(
        live_after.1, live_kept.1,
        "{HARNESS}: the sweep must not touch a live peer at all — not even its `updated_at`"
    );
    assert_eq!(live_after.0, live_kept.0);
    assert_eq!(story_status(pool, &live_story).await, "In Progress");

    for story in [&silent_story, &live_story] {
        harness
            .cleanup_story(story)
            .await
            .expect("the harness owns the rows it made and puts them back");
    }
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id = any($1::text[])")
            .bind(vec![silent_story, live_story])
            .fetch_one(pool)
            .await
            .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

//! FORGE.CLAIM-008 — maximum attempts (TST-FORGE-CLAIM-008).
//!
//! CONTRACT. A cleared claim is not a free retry: every claim increments `attempts`
//! (`forge_claim_specific_agent_work`, migration 262), and `forge_finish_agent_work_run` (migration 263) settles
//! the pair as `Error` once `attempts >= max_attempts` (default 3) instead of clearing it back to `Ready` — so a
//! broken engine stops spinning one story through the queue. The first two cleared runs rule nothing (their Story
//! Runs close with `result_status` NULL); the capped run rules `Failed` and holds the board.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__008__maximum_attempts -- --ignored

use db::{AgentWorkOutcome, DbTarget};
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER: &str = "forge-attempts-owner";

async fn attempts_of(pool: &sqlx::PgPool, item_id: &str) -> i32 {
    sqlx::query_scalar("select attempts from agent_work_item where id = $1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the attempt count is readable")
}

async fn story_status(pool: &sqlx::PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the committed story back")
}

/// One engine-fault cycle through the production path: claim, begin, abandon.
async fn abandon_cycle(
    harness: &ForgeHarness,
    _story: &str,
    item: &str,
) -> Option<db::AgentWorkSettlement> {
    harness
        .engine()
        .claim_specific_agent_work(item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .engine()
        .begin_agent_work_run(item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");
    harness
        .engine()
        .finish_agent_work_run(
            item,
            AgentWorkOutcome::Abandoned,
            Some("engine fault: host went away"),
        )
        .await
        .expect("the production settle runs")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_008__maximum_attempts() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: an attempts contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let capped_story = format!("FORGE-CLAIM-008-CAPPED-{tag}");
    let fresh_story = format!("FORGE-CLAIM-008-FRESH-{tag}");

    // ── THE CAP: the third cleared run settles Error, not Ready. ─────────────
    let item = harness
        .seed_ready_story(&capped_story)
        .await
        .expect("the Ready trigger must queue exactly one item");
    sqlx::query("update storyboard_story set status = 'In Progress' where id = $1")
        .bind(&capped_story)
        .execute(pool)
        .await
        .expect("the board marks the story running");

    let first = abandon_cycle(&harness, &capped_story, &item)
        .await
        .expect("the first engine fault must settle the claim");
    assert_eq!(
        first.item_state, "Ready",
        "{HARNESS}: fault 1 of 3 clears the claim back to the queue"
    );
    assert_eq!(first.story_status.as_deref(), Some("Ready"));
    assert_eq!(attempts_of(pool, &item).await, 1);

    let second = abandon_cycle(&harness, &capped_story, &item)
        .await
        .expect("the second engine fault must settle the claim");
    assert_eq!(
        second.item_state, "Ready",
        "{HARNESS}: fault 2 of 3 clears the claim back to the queue"
    );
    assert_eq!(attempts_of(pool, &item).await, 2);

    let third = abandon_cycle(&harness, &capped_story, &item)
        .await
        .expect("the third engine fault must settle the claim");
    assert_eq!(
        third.item_state, "Error",
        "{HARNESS}: fault 3 of 3 exhausts the budget — a cleared claim is not a free retry"
    );
    assert_eq!(third.story_status.as_deref(), Some("Hold"));
    assert_eq!(attempts_of(pool, &item).await, 3);
    assert_eq!(story_status(pool, &capped_story).await, "Hold");

    // The cleared runs ruled nothing; the capped run ruled Failed. One story, three runs, in order.
    let rulings: Vec<Option<String>> = sqlx::query_scalar(
        "select result_status from storyboard_story_run where story_id = $1 order by started_at",
    )
    .bind(&capped_story)
    .fetch_all(pool)
    .await
    .expect("the run ledger is readable");
    assert_eq!(
        rulings,
        vec![None, None, Some("Failed".to_string())],
        "{HARNESS}: cleared claims rule nothing; the capped run rules Failed: {rulings:?}"
    );

    // ── NEGATIVE: the cap is about the COUNT, not the outcome. ───────────────
    // A first engine fault on a fresh story clears it — the same outcome the capped story was capped for.
    let fresh_item = harness
        .seed_ready_story(&fresh_story)
        .await
        .expect("the Ready trigger must queue exactly one item");
    sqlx::query("update storyboard_story set status = 'In Progress' where id = $1")
        .bind(&fresh_story)
        .execute(pool)
        .await
        .expect("the board marks the story running");
    let fresh = abandon_cycle(&harness, &fresh_story, &fresh_item)
        .await
        .expect("a first engine fault must settle the claim");
    assert_eq!(
        fresh.item_state, "Ready",
        "{HARNESS}: fault 1 of 3 on a fresh story clears it — the budget counts attempts, not faults"
    );
    assert_eq!(story_status(pool, &fresh_story).await, "Ready");

    for story in [&capped_story, &fresh_story] {
        harness
            .cleanup_story(story)
            .await
            .expect("the harness owns the rows it made and puts them back");
    }
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id = any($1::text[])")
            .bind(vec![capped_story, fresh_story])
            .fetch_one(pool)
            .await
            .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

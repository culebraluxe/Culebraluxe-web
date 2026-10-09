//! FORGE.CLAIM-009 — Abandoned engine fault is not story failure (TST-FORGE-CLAIM-009).
//!
//! CONTRACT. `Abandoned` is the engine's own fault — a database session the server took away, a launch that never
//! started, a child that died with no verdict — so nothing about the story was decided and the claim is **cleared
//! back into the queue** (`Ready`/`Ready`) rather than held against it (`forge_settlement_pair`, migration 263).
//! The cleared run rules nothing: its Story Run closes with `result_status` NULL, because an engine fault is not a
//! story verdict. A story must not lose its turn to a broken engine.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__009__abandoned_engine_fault_is_not_story_failure -- --ignored

use db::{AgentWorkOutcome, DbTarget};
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER: &str = "forge-abandoned-owner";

async fn story_status(pool: &sqlx::PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the committed story back")
}

async fn run_ruling(pool: &sqlx::PgPool, story_id: &str) -> (Option<String>, bool) {
    sqlx::query_as(
        "select result_status, ended_at is not null
           from storyboard_story_run where story_id = $1 order by started_at desc limit 1",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("the run ledger is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_009__abandoned_engine_fault_is_not_story_failure() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: an abandon contract must never run against PROD"
    );

    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let abandoned_story = format!("FORGE-CLAIM-009-ABANDONED-{tag}");
    let error_story = format!("FORGE-CLAIM-009-ERROR-{tag}");
    let landed_story = format!("FORGE-CLAIM-009-LANDED-{tag}");

    for story in [&abandoned_story, &error_story, &landed_story] {
        harness
            .seed_ready_story(story)
            .await
            .expect("the Ready trigger must queue exactly one item");
        sqlx::query("update storyboard_story set status = 'In Progress' where id = $1")
            .bind(story)
            .execute(pool)
            .await
            .expect("the board marks the story running");
    }
    let abandoned_item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1")
            .bind(&abandoned_story)
            .fetch_one(pool)
            .await
            .expect("the proof item is readable");
    let error_item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1")
            .bind(&error_story)
            .fetch_one(pool)
            .await
            .expect("the proof item is readable");
    let landed_item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1")
            .bind(&landed_story)
            .fetch_one(pool)
            .await
            .expect("the proof item is readable");

    // ── THE CONTRACT: an engine fault clears the claim; the story keeps its turn. ──
    harness
        .engine()
        .claim_specific_agent_work(&abandoned_item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .engine()
        .begin_agent_work_run(&abandoned_item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");
    let settlement = harness
        .engine()
        .finish_agent_work_run(
            &abandoned_item,
            AgentWorkOutcome::Abandoned,
            Some("engine fault: host went away"),
            None,
        )
        .await
        .expect("the production settle runs")
        .expect("an engine fault must settle the claim");
    assert_eq!(
        settlement.item_state, "Ready",
        "{HARNESS}: an Abandoned claim goes back to the queue"
    );
    assert_eq!(
        settlement.story_status.as_deref(),
        Some("Ready"),
        "{HARNESS}: the board goes back with it — the story keeps its turn"
    );
    assert_eq!(
        settlement.reason, None,
        "{HARNESS}: a cleared claim carries no failure reason: {:?}",
        settlement.reason
    );
    let (state, claimed_by): (String, Option<String>) =
        sqlx::query_as("select state, claimed_by from agent_work_item where id = $1::uuid")
            .bind(&abandoned_item)
            .fetch_one(pool)
            .await
            .expect("read the cleared claim back");
    assert_eq!(state, "Ready");
    assert_eq!(
        claimed_by, None,
        "{HARNESS}: a cleared row names nobody — the dead engine holds nothing"
    );
    assert_eq!(story_status(pool, &abandoned_story).await, "Ready");
    let (ruling, ended) = run_ruling(pool, &abandoned_story).await;
    assert_eq!(
        ruling, None,
        "{HARNESS}: a cleared claim rules nothing — an engine fault is not a story verdict"
    );
    assert!(
        ended,
        "{HARNESS}: the cleared run still closes, so it cannot be mistaken for a run in flight"
    );

    // ── NEGATIVE: the SAME lifecycle with a real Error IS a story failure. ──
    // This is the control that proves Abandoned is special: everything identical except the outcome.
    harness
        .engine()
        .claim_specific_agent_work(&error_item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .engine()
        .begin_agent_work_run(&error_item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");
    let failed = harness
        .engine()
        .finish_agent_work_run(
            &error_item,
            AgentWorkOutcome::Error,
            Some("smith exited 101"),
            None,
        )
        .await
        .expect("the production settle runs")
        .expect("an Error must settle the claim");
    assert_eq!(failed.item_state, "Error");
    assert_eq!(failed.story_status.as_deref(), Some("Hold"));
    assert_eq!(story_status(pool, &error_story).await, "Hold");
    let (error_ruling, _) = run_ruling(pool, &error_story).await;
    assert_eq!(
        error_ruling.as_deref(),
        Some("Failed"),
        "{HARNESS}: a real Error rules Failed — the verdict Abandoned must never write"
    );

    // ── NEGATIVE: an engine fault over LANDED work disturbs nothing. ─────────
    sqlx::query("update storyboard_story set status = 'Complete', completion = 100 where id = $1")
        .bind(&landed_story)
        .execute(pool)
        .await
        .expect("the board confirms the work landed");
    harness
        .engine()
        .claim_specific_agent_work(&landed_item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .engine()
        .begin_agent_work_run(&landed_item)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must open its run");
    let landed = harness
        .engine()
        .finish_agent_work_run(
            &landed_item,
            AgentWorkOutcome::Abandoned,
            Some("engine fault over landed work"),
            None,
        )
        .await
        .expect("the production settle runs")
        .expect("an engine fault over landed work must settle the claim");
    assert_eq!(
        landed.item_state, "Cancelled",
        "{HARNESS}: over a board that no longer expects a run the fault cancels the item"
    );
    assert_eq!(
        landed.story_status, None,
        "{HARNESS}: ...and leaves the landed board alone: Complete stays Complete"
    );
    assert_eq!(story_status(pool, &landed_story).await, "Complete");

    for story in [&abandoned_story, &error_story, &landed_story] {
        harness
            .cleanup_story(story)
            .await
            .expect("the harness owns the rows it made and puts them back");
    }
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id = any($1::text[])")
            .bind(vec![abandoned_story, error_story, landed_story])
            .fetch_one(pool)
            .await
            .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

//! FORGE.STORY_RUN — goal snapshot (TST-FORGE-STORY-RUN-011).
//!
//! Contract: the run holds a COPY of the story's goal, taken in the statement that opens the run
//! (`goal_snapshot`, `db/migrations/264_forge_agent_work_begin.sql:46-57`), and that copy belongs to exactly one
//! story. Migration 025 states it in as many words: the authoritative spec lives on `storyboard_story` and is
//! snapshotted into `storyboard_story_run` when execution begins — `BeginAgentWorkRun` takes no snapshot argument
//! because a caller-supplied copy would be a second writer (`db/src/forge_engine.rs:88-99`).
//!
//! What this file proves:
//!
//!   1. **the copy is stored** — after `begin`, `run.goal_snapshot` equals the goal its own story held at that
//!      moment, byte for byte;
//!   2. **it belongs to its own story** — two concurrent runs of two stories with different goals each hold their
//!      OWN goal; neither run borrows the other's;
//!   3. **the copy is frozen** — rewriting the story's goal after the run opened does not move the snapshot: the
//!      run keeps what execution started under;
//!   4. **negative / fault — blank is absence** — a whitespace-only goal stores NULL, never `''`, and a goal the
//!      story never set is not invented by the writer.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! raw SQL here is fixture setup and read-back only. No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__011__goal_snapshot -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-011-";
const OWNER: &str = "forge-story-run-011-owner";
/// The goal story A holds when its run opens.
const GOAL_A: &str = "ship the goal snapshot proof for story A";
/// The goal story B holds when its run opens — deliberately different, so a borrowed copy cannot pass.
const GOAL_B: &str = "an entirely different goal, owned by story B";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
async fn connect_dev() -> ForgeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ForgeHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; TestDatabase refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Insert a story carrying `goal` and return the exactly-one work item its dispatch trigger queued. The story
/// row is the ONLY source of the snapshot, so the fixture writes the goal on it.
async fn seed_with_goal(harness: &ForgeHarness, story_id: &str, goal: Option<&str>) -> String {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes, goal)
         values ($1, 'PROOF', 'Goal snapshot proof', 'High', 'Ready', '', $2)",
    )
    .bind(story_id)
    .bind(goal)
    .execute(harness.pool())
    .await
    .expect("the goal story is insertable");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(harness.pool())
        .await
        .expect("the board's Ready trigger queued exactly one work item")
}

/// Claim the item and open its run through the production seam, returning the run id.
async fn begin(harness: &ForgeHarness, item: &str) -> String {
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
        .expect("the claim opens the run")
        .story_run_id
}

/// The run's `goal_snapshot`, read on the pool the DAO wrote to.
async fn goal_snapshot(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar("select goal_snapshot from storyboard_story_run where id=$1::uuid")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("the run's goal_snapshot is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-011).
async fn forge_story_run_011__goal_snapshot() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let story_a = format!("{PROOF_PREFIX}A-{ns}");
    let story_b = format!("{PROOF_PREFIX}B-{ns}");
    let story_blank = format!("{PROOF_PREFIX}blank-{ns}");

    // 1. TWO STORIES, TWO GOALS, TWO RUNS.
    let item_a = seed_with_goal(&harness, &story_a, Some(GOAL_A)).await;
    let run_a = begin(&harness, &item_a).await;
    let item_b = seed_with_goal(&harness, &story_b, Some(GOAL_B)).await;
    let run_b = begin(&harness, &item_b).await;
    let snapshot_a = goal_snapshot(&pool, &run_a).await;
    let snapshot_b = goal_snapshot(&pool, &run_b).await;

    // 2. FROZEN AT OPEN: story A's goal is rewritten after its run opened. The snapshot must not move.
    sqlx::query("update storyboard_story set goal='MUTATED GOAL' where id=$1")
        .bind(&story_a)
        .execute(&pool)
        .await
        .expect("the story's goal can be edited");
    let snapshot_a_after_edit = goal_snapshot(&pool, &run_a).await;

    // 3. NEGATIVE / FAULT — BLANK IS ABSENCE. A whitespace goal stores NULL; a goal never set stays unset.
    let item_blank = seed_with_goal(&harness, &story_blank, Some("   ")).await;
    let run_blank = begin(&harness, &item_blank).await;
    let snapshot_blank = goal_snapshot(&pool, &run_blank).await;
    let item_unset = seed_with_goal(&harness, &format!("{PROOF_PREFIX}unset-{ns}"), None).await;
    let run_unset = begin(&harness, &item_unset).await;
    let snapshot_unset = goal_snapshot(&pool, &run_unset).await;

    // 4. CLEANUP / ROLLBACK: the proof stories are deleted; their items and runs go with them.
    for story in [
        &story_a,
        &story_b,
        &story_blank,
        &format!("{PROOF_PREFIX}unset-{ns}"),
    ] {
        harness
            .cleanup_story(story)
            .await
            .expect("reap the proof story");
    }
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let stories_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    let runs_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();

    // ---------------------------------------------------------------------------------------------------------
    // 5. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert_eq!(
        snapshot_a.as_deref(),
        Some(GOAL_A),
        "{HARNESS}: the run holds the goal its own story had when execution began"
    );
    assert_eq!(
        snapshot_b.as_deref(),
        Some(GOAL_B),
        "{HARNESS}: a second run holds ITS story's goal — no run borrows another story's copy"
    );
    assert_ne!(
        snapshot_a, snapshot_b,
        "{HARNESS}: two stories with different goals cannot produce two identical snapshots"
    );
    assert_eq!(
        snapshot_a_after_edit.as_deref(),
        Some(GOAL_A),
        "{HARNESS}: the goal snapshot is FROZEN at open — editing the story afterwards never moves it"
    );

    // 6. NEGATIVE / FAULT CASES.
    assert_eq!(
        snapshot_blank, None,
        "{HARNESS}: a whitespace-only goal is the ABSENCE of a goal — NULL, never an empty string"
    );
    assert_eq!(
        snapshot_unset, None,
        "{HARNESS}: a goal the story never set is not invented by the writer"
    );

    // 7. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

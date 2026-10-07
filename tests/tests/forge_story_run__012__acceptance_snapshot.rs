//! FORGE.STORY_RUN — acceptance snapshot (TST-FORGE-STORY-RUN-012).
//!
//! Contract: the run holds a COPY of the story's acceptance criteria, taken in the statement that opens the run
//! (`acceptance_criteria_snapshot`, `db/migrations/264_forge_agent_work_begin.sql:46-57`), and that copy belongs
//! to exactly one story. Migration 025 states it in as many words: the authoritative spec lives on
//! `storyboard_story` and is snapshotted into `storyboard_story_run` when execution begins — `BeginAgentWorkRun`
//! takes no snapshot argument because a caller-supplied copy would be a second writer
//! (`db/src/forge_engine.rs:88-99`).
//!
//! What this file proves:
//!
//!   1. **the copy is stored, whole** — after `begin`, `run.acceptance_criteria_snapshot` equals the acceptance
//!      criteria its own story held at that moment, byte for byte, multi-line text and all (a truncated or
//!      first-line-only copy fails on the length);
//!   2. **it belongs to its own story** — two concurrent runs of two stories with different criteria each hold
//!      THEIR OWN criteria; neither run borrows the other's;
//!   3. **the copy is frozen** — rewriting the story's acceptance criteria after the run opened does not move the
//!      snapshot: the run keeps what execution started under, so a later edit can never retro-fit a run's proof;
//!   4. **negative / fault — blank is absence** — whitespace-only criteria store NULL, never `''`, and criteria
//!      the story never set are not invented by the writer.
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
//!     --test forge_story_run__012__acceptance_snapshot -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-012-";
const OWNER: &str = "forge-story-run-012-owner";
/// Story A's acceptance criteria — multi-line on purpose, so a first-line-only copy cannot pass.
const ACCEPTANCE_A: &str =
    "1. the acceptance snapshot is stored whole\n2. it never drifts after open\n3. a blank \
                            field stays NULL";
/// Story B's acceptance criteria — deliberately different, so a borrowed copy cannot pass.
const ACCEPTANCE_B: &str = "- an entirely different acceptance list, owned by story B";

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

/// Insert a story carrying `acceptance_criteria` and return the exactly-one work item its dispatch trigger
/// queued. The story row is the ONLY source of the snapshot, so the fixture writes the criteria on it.
async fn seed_with_acceptance(
    harness: &ForgeHarness,
    story_id: &str,
    criteria: Option<&str>,
) -> String {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes, acceptance_criteria)
         values ($1, 'PROOF', 'Acceptance snapshot proof', 'High', 'Ready', '', $2)",
    )
    .bind(story_id)
    .bind(criteria)
    .execute(harness.pool())
    .await
    .expect("the acceptance story is insertable");
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

/// The run's `acceptance_criteria_snapshot`, read on the pool the DAO wrote to.
async fn acceptance_snapshot(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar(
        "select acceptance_criteria_snapshot from storyboard_story_run where id=$1::uuid",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("the run's acceptance_criteria_snapshot is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-012).
async fn forge_story_run_012__acceptance_snapshot() {
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
    let story_unset = format!("{PROOF_PREFIX}unset-{ns}");

    // 1. TWO STORIES, TWO ACCEPTANCE LISTS, TWO RUNS.
    let item_a = seed_with_acceptance(&harness, &story_a, Some(ACCEPTANCE_A)).await;
    let run_a = begin(&harness, &item_a).await;
    let item_b = seed_with_acceptance(&harness, &story_b, Some(ACCEPTANCE_B)).await;
    let run_b = begin(&harness, &item_b).await;
    let snapshot_a = acceptance_snapshot(&pool, &run_a).await;
    let snapshot_b = acceptance_snapshot(&pool, &run_b).await;

    // 2. FROZEN AT OPEN: story A's acceptance criteria are rewritten after its run opened. The snapshot must not
    //    move — a later edit can never retro-fit the proof of a run that already happened.
    sqlx::query("update storyboard_story set acceptance_criteria='MUTATED ACCEPTANCE' where id=$1")
        .bind(&story_a)
        .execute(&pool)
        .await
        .expect("the story's acceptance criteria can be edited");
    let snapshot_a_after_edit = acceptance_snapshot(&pool, &run_a).await;

    // 3. NEGATIVE / FAULT — BLANK IS ABSENCE. Whitespace criteria store NULL; criteria never set stay unset.
    let item_blank = seed_with_acceptance(&harness, &story_blank, Some("   ")).await;
    let run_blank = begin(&harness, &item_blank).await;
    let snapshot_blank = acceptance_snapshot(&pool, &run_blank).await;
    let item_unset = seed_with_acceptance(&harness, &story_unset, None).await;
    let run_unset = begin(&harness, &item_unset).await;
    let snapshot_unset = acceptance_snapshot(&pool, &run_unset).await;

    // 4. CLEANUP / ROLLBACK: the proof stories are deleted; their items and runs go with them.
    for story in [&story_a, &story_b, &story_blank, &story_unset] {
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
        Some(ACCEPTANCE_A),
        "{HARNESS}: the run holds the acceptance criteria its own story had when execution began, whole and \
         multi-line"
    );
    assert_eq!(
        snapshot_a.as_ref().map(String::len),
        Some(ACCEPTANCE_A.len()),
        "{HARNESS}: the snapshot is the full text — not a first line, not a truncated copy"
    );
    assert_eq!(
        snapshot_b.as_deref(),
        Some(ACCEPTANCE_B),
        "{HARNESS}: a second run holds ITS story's criteria — no run borrows another story's copy"
    );
    assert_ne!(
        snapshot_a, snapshot_b,
        "{HARNESS}: two stories with different criteria cannot produce two identical snapshots"
    );
    assert_eq!(
        snapshot_a_after_edit.as_deref(),
        Some(ACCEPTANCE_A),
        "{HARNESS}: the acceptance snapshot is FROZEN at open — editing the story afterwards never moves it"
    );

    // 6. NEGATIVE / FAULT CASES.
    assert_eq!(
        snapshot_blank, None,
        "{HARNESS}: whitespace-only acceptance criteria are the ABSENCE of a contract — NULL, never an empty string"
    );
    assert_eq!(
        snapshot_unset, None,
        "{HARNESS}: acceptance criteria the story never set are not invented by the writer"
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

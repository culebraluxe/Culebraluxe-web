//! FORGE.STORY_RUN — base SHA snapshot (TST-FORGE-STORY-RUN-010).
//!
//! Contract: a Story Run records the base commit it was provisioned from, ONCE. `base_commit_hash` is deliberately
//! absent from the insert that opens the run (`db/migrations/264_forge_agent_work_begin.sql:16` — the worktree does
//! not exist yet), and the one writer is `ForgeEngineDao::stamp_run_base_commit`
//! (`db/src/forge_engine.rs:429-445`):
//!
//! * **first stamp wins** — the update runs `where id=$1 and base_commit_hash is null`, so the first base a run was
//!   provisioned from is the base it ran on and a second stamp is a NO-OP that reports `false`, never an
//!   overwrite;
//! * **blank is not a base** — a whitespace-only stamp returns `false` and writes nothing;
//! * **nothing is invented** — stamping a run that does not exist matches no row and returns `false`.
//!
//! What this file proves:
//!
//!   1. a freshly opened run has no base yet (the worktree is provisioned afterwards);
//!   2. the first stamp stores the trimmed SHA on the run row;
//!   3. a second, different stamp is refused and leaves the first base in place;
//!   4. **negative / fault** — blank and unknown-run stamps change nothing and report `false`.
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
//!     --test forge_story_run__010__base_sha_snapshot -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-010-";
const OWNER: &str = "forge-story-run-010-owner";
/// The base the run is provisioned from — padded on purpose, so the trim is part of the proof.
const FIRST_BASE: &str = "  0f1e2d3c4b5a6978  ";
const FIRST_BASE_TRIMMED: &str = "0f1e2d3c4b5a6978";
/// The base a second stamp tries to write over the first.
const SECOND_BASE: &str = "8899aabbccddeeff";
/// A run id nobody opened.
const MISSING_RUN: &str = "00000000-0000-4000-8000-000000000043";

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

/// Put one disposable story on the board at `Ready` and return the exactly-one work item its dispatch trigger
/// queued — the same queue production dispatches from.
async fn seed(harness: &ForgeHarness, story_id: &str) -> String {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match harness.seed_ready_story(story_id).await {
            Ok(item) => return item,
            Err(error) => {
                let message = error.to_string();
                let landed: Option<String> = sqlx::query_scalar(
                    "select id::text from agent_work_item where story_id=$1 and state='Ready'",
                )
                .bind(story_id)
                .fetch_optional(harness.pool())
                .await
                .ok()
                .flatten();
                if let Some(item) = landed {
                    return item;
                }
                eprintln!("proof: seed attempt {attempt} failed: {message}");
                last = Some(message);
                tokio::time::sleep(std::time::Duration::from_millis(250 * attempt)).await;
            }
        }
    }
    panic!(
        "the board's Ready trigger must queue exactly one work item for {story_id}: {}",
        last.unwrap_or_default()
    );
}

/// The run's `base_commit_hash`, read on the pool the DAO wrote to.
async fn base(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar("select base_commit_hash from storyboard_story_run where id=$1::uuid")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("the run's base_commit_hash is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-010).
async fn forge_story_run_010__base_sha_snapshot() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine = harness.engine().clone();
    let ns = harness.database().namespace().to_string();
    let story = format!("{PROOF_PREFIX}base-{ns}");

    // 1. THE RUN OPENS WITHOUT A BASE — the worktree does not exist yet.
    let item = seed(&harness, &story).await;
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    let run_id = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs")
        .expect("the claim opens the run")
        .story_run_id;
    let base_at_open = base(&pool, &run_id).await;

    // 2. THE FIRST STAMP, padded on purpose: the stored base is trimmed.
    let first_stamped = engine
        .stamp_run_base_commit(&run_id, FIRST_BASE)
        .await
        .expect("the production stamp runs");
    let base_after_first = base(&pool, &run_id).await;

    // 3. THE SECOND STAMP IS A NO-OP — the first base a run was provisioned from is the base it ran on.
    let second_stamped = engine
        .stamp_run_base_commit(&run_id, SECOND_BASE)
        .await
        .expect("the production stamp runs");
    let base_after_second = base(&pool, &run_id).await;

    // 4. NEGATIVE / FAULT — BLANK IS NOT A BASE.
    let blank_stamped = engine
        .stamp_run_base_commit(&run_id, "   ")
        .await
        .expect("the production stamp runs");
    let base_after_blank = base(&pool, &run_id).await;

    // 5. NEGATIVE / FAULT — A RUN NOBODY OPENED IS NEVER INVENTED.
    let missing_preexisting: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where id=$1::uuid")
            .bind(MISSING_RUN)
            .fetch_one(&pool)
            .await
            .expect("the control read is runnable");
    let missing_stamped = engine
        .stamp_run_base_commit(MISSING_RUN, SECOND_BASE)
        .await
        .expect("the production stamp runs");
    let missing_after: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where id=$1::uuid")
            .bind(MISSING_RUN)
            .fetch_one(&pool)
            .await
            .expect("the control read is runnable");

    // 6. CLEANUP / ROLLBACK: the proof story is deleted; its item and run go with it.
    harness
        .cleanup_story(&story)
        .await
        .expect("reap the proof story");
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
    // 7. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert_eq!(
        base_at_open, None,
        "{HARNESS}: a freshly opened run carries no base yet — the worktree is provisioned afterwards"
    );
    assert!(
        first_stamped,
        "{HARNESS}: the first base stamp lands on the run"
    );
    assert_eq!(
        base_after_first.as_deref(),
        Some(FIRST_BASE_TRIMMED),
        "{HARNESS}: the stored base is the trimmed SHA the run was provisioned from"
    );
    assert!(
        !second_stamped,
        "{HARNESS}: a second base stamp is a NO-OP — the first base is the base the run ran on"
    );
    assert_eq!(
        base_after_second.as_deref(),
        Some(FIRST_BASE_TRIMMED),
        "{HARNESS}: the refused stamp left the first base exactly as it was"
    );

    // 8. NEGATIVE / FAULT CASES.
    assert!(
        !blank_stamped,
        "{HARNESS}: a whitespace-only base is not a base — the stamp reports false"
    );
    assert_eq!(
        base_after_blank.as_deref(),
        Some(FIRST_BASE_TRIMMED),
        "{HARNESS}: the blank stamp wrote nothing"
    );
    assert_eq!(
        missing_preexisting, 0,
        "{HARNESS}: control — the id this fault case names belongs to no run"
    );
    assert!(
        !missing_stamped,
        "{HARNESS}: stamping a run that does not exist reports false — nothing matches, nothing is invented"
    );
    assert_eq!(
        missing_after, 0,
        "{HARNESS}: the stamp never fabricates a Story Run row"
    );

    // 9. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

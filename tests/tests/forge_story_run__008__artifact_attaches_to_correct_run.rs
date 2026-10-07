//! FORGE.STORY_RUN — artifact attaches to correct run (TST-FORGE-STORY-RUN-008).
//!
//! Contract: a `forge_tool_artifact` row hangs off the Story Run its execution belongs to. The one write is
//! `ForgeEngineDao::record_tool_artifact` (`db/src/forge_engine.rs:577-595`) driving `forge_record_tool_artifact`
//! (`db/migrations/267_forge_tool_artifact_write.sql`), and "correct run" means the run OF THE STORY THE ARTIFACT
//! IS FILED UNDER — an artifact that lands under some other story's run is evidence attached to the wrong
//! execution, which is the whole defect this contract names.
//!
//! What this file proves:
//!
//!   1. **the positive** — an artifact recorded through the production seam for story A with A's own run comes back
//!      and persists with `story_id = A` and `story_run_id = A's run`, and it is found when reading that run's
//!      artifacts;
//!   2. **fault — an unknown run is refused** — a run id that does not exist is rejected by the foreign key: an
//!      artifact can never be attached to a run nobody opened;
//!   3. **fault — an unknown story is refused** — likewise on the story side;
//!   4. **subject refusal — another story's run** — recording story A's artifact under story B's run must be
//!      refused, so a mismatched `(story, run)` pair can never be written. If this production boundary accepts that
//!      pair, the contract is violated by an invalid case and this test fails.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim, the run open and the artifact write are the
//! production `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable
//! DEV/Neon target; raw SQL here is fixture setup and read-back only. No production code is changed to make this
//! pass — a faithful test that exposes the gap is the result.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__008__artifact_attaches_to_correct_run -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbTarget, ForgeEngineDao, NewToolArtifact};
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-008-";
const OWNER: &str = "forge-story-run-008-owner";

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

/// The artifact shape this file writes: one reading of one execution.
fn artifact(story_id: &str, run_id: Option<String>) -> NewToolArtifact {
    NewToolArtifact {
        story_id: story_id.to_string(),
        story_run_id: run_id,
        tool: "opencode".into(),
        kind: "qa-assay-evidence".into(),
        verdict: Some("PASS".into()),
        summary: Some("the assay's own reading".into()),
        detail: None,
        sha: None,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-008).
async fn forge_story_run_008__artifact_attaches_to_correct_run() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine: ForgeEngineDao = harness.engine().clone();
    let ns = harness.database().namespace().to_string();
    let story_a = format!("{PROOF_PREFIX}A-{ns}");
    let story_b = format!("{PROOF_PREFIX}B-{ns}");

    // 1. TWO STORIES, TWO RUNS — an artifact has to pick the right one of them.
    let item_a = seed(&harness, &story_a).await;
    let run_a = begin(&harness, &item_a).await;
    let item_b = seed(&harness, &story_b).await;
    let run_b = begin(&harness, &item_b).await;
    assert_ne!(
        run_a, run_b,
        "{HARNESS}: control — the two proof stories own two different runs"
    );

    // 2. THE POSITIVE — story A's artifact, written through the production seam with A's own run.
    let written = engine
        .record_tool_artifact(&artifact(&story_a, Some(run_a.clone())))
        .await
        .expect("the production artifact write runs");
    let persisted: (Option<String>, Option<String>) = sqlx::query_as(
        "select story_id, story_run_id::text from forge_tool_artifact where id=$1::uuid",
    )
    .bind(&written.id)
    .fetch_one(&pool)
    .await
    .expect("the artifact row is readable");
    let under_a: i64 =
        sqlx::query_scalar("select count(*) from forge_tool_artifact where story_run_id=$1::uuid")
            .bind(&run_a)
            .fetch_one(&pool)
            .await
            .expect("artifacts under run A are countable");
    let under_b: i64 =
        sqlx::query_scalar("select count(*) from forge_tool_artifact where story_run_id=$1::uuid")
            .bind(&run_b)
            .fetch_one(&pool)
            .await
            .expect("artifacts under run B are countable");

    // 3. FAULT — AN UNKNOWN RUN IS REFUSED: no run, no attachment, and nothing invented in between.
    const MISSING_RUN: &str = "00000000-0000-4000-8000-000000000042";
    let preexisting: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where id=$1::uuid")
            .bind(MISSING_RUN)
            .fetch_one(&pool)
            .await
            .expect("the control read is runnable");
    assert_eq!(
        preexisting, 0,
        "{HARNESS}: control — the id this fault case names belongs to no run"
    );
    let unknown_run_refused = engine
        .record_tool_artifact(&artifact(&story_a, Some(MISSING_RUN.to_string())))
        .await
        .is_err();

    // 4. FAULT — AN UNKNOWN STORY IS REFUSED.
    let unknown_story_refused = engine
        .record_tool_artifact(&artifact(
            &format!("{PROOF_PREFIX}missing-{ns}"),
            Some(run_a.clone()),
        ))
        .await
        .is_err();

    // 5. THE SUBJECT REFUSAL — story A's artifact handed story B's run. The pair must be refused: it is the
    //    definition of an artifact attached to the WRONG run. Whatever this returns is captured and asserted after
    //    cleanup, because the refusal (or its absence) IS the contract under test.
    let cross_pair = engine
        .record_tool_artifact(&artifact(&story_a, Some(run_b.clone())))
        .await;
    let cross_refused = cross_pair.is_err();
    let cross_story: Option<String> = sqlx::query_scalar(
        "select story_id from forge_tool_artifact where story_run_id=$1::uuid order by created_at desc limit 1",
    )
    .bind(&run_b)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();
    let a_artifacts: i64 =
        sqlx::query_scalar("select count(*) from forge_tool_artifact where story_id=$1")
            .bind(&story_a)
            .fetch_one(&pool)
            .await
            .expect("story A's artifacts are countable");

    // 6. CLEANUP / ROLLBACK: both proof stories are deleted; their items, runs and artifacts cascade with them.
    for story in [&story_a, &story_b] {
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
    let artifacts_left: i64 =
        sqlx::query_scalar("select count(*) from forge_tool_artifact where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();

    // ---------------------------------------------------------------------------------------------------------
    // 7. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert_eq!(
        written.story_id, story_a,
        "{HARNESS}: the artifact is filed under its own story"
    );
    assert_eq!(
        written.story_run_id.as_deref(),
        Some(run_a.as_str()),
        "{HARNESS}: the artifact attaches to THE RUN of the story it is filed under"
    );
    assert_eq!(
        persisted,
        (Some(story_a.clone()), Some(run_a.clone())),
        "{HARNESS}: the committed row carries the same story/run pair the seam returned"
    );
    assert_eq!(
        under_a, 1,
        "{HARNESS}: the artifact is found when reading its own run"
    );
    assert_eq!(
        under_b, 0,
        "{HARNESS}: story A's artifact never appears under story B's run"
    );

    // 8. NEGATIVE / FAULT CASES.
    assert!(
        unknown_run_refused,
        "{HARNESS}: an artifact naming a run that does not exist must be refused — it can never attach to a run \
         nobody opened"
    );
    assert!(
        unknown_story_refused,
        "{HARNESS}: an artifact naming a story that does not exist must be refused"
    );
    assert!(
        cross_refused,
        "{HARNESS}: story A's artifact handed story B's run must be REFUSED — an artifact may only attach to the \
         run of its own story, and a mismatched pair is an artifact on the wrong execution"
    );
    assert_ne!(
        cross_story.as_deref(),
        Some(story_a.as_str()),
        "{HARNESS}: story A's artifact must never be found under story B's run"
    );
    assert_eq!(
        a_artifacts, 1,
        "{HARNESS}: exactly one artifact is ever filed under story A"
    );

    // 9. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        artifacts_left, 0,
        "{HARNESS}: the proof must leave no artifact behind"
    );
}

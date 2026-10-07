//! FORGE.STORY_RUN — actual execution environment recorded (TST-FORGE-STORY-RUN-003).
//!
//! Contract: the Story Run records the environment it ACTUALLY executed on. The value is taken from the database
//! this process is connected to — `run_execution_environment(self.db.declared_target())`, bound by
//! `ForgeEngineDao::begin_agent_work_run` (`db/src/forge_engine.rs:128-135`, `299-315`) into
//! `forge_begin_agent_work_run` (`db/migrations/264_forge_agent_work_begin.sql`) — and written to
//! `storyboard_story_run.execution_environment` in the insert that opens the run (migration 030). The DAO method
//! takes no environment argument, so no caller can declare a target for the run it is opening.
//!
//! What this file proves:
//!
//!   1. a run opened through the production seam on a DEV database records `DEV` — the actual target, not the
//!      intent the work item carries and not anything a caller passed;
//!   2. the intent (`agent_work_item.execution_environment`) survives beside the fact, unreconciled, so a mismatch
//!      stays visible;
//!   3. **negative / refusal** — a word outside the schema's four (`DEV` / `PROD` / `TEST` / `LOCAL`) is refused by
//!      the column's CHECK, so a run can never be labelled with an environment the schema does not know;
//!   4. **negative / refusal** — `guard_target(DbTarget::Prod)` refuses PRODUCTION before any socket is opened, so
//!      the harness this proof runs on can never record a production run.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target.
//! Raw SQL here is fixture setup and read-back only. No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__003__actual_execution_environment_recorded -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::{guard_target, ForgeHarness, HarnessDbError};

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-003-";
const OWNER: &str = "forge-story-run-003-owner";

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
                // A story that DID land is not retried into a duplicate: the item is read back instead.
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

/// The proof-row census, taken after cleanup: every row this file created must be gone.
async fn leftover(pool: &PgPool, scope: &str) -> (i64, i64, i64) {
    let stories: i64 = sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
        .bind(scope)
        .fetch_one(pool)
        .await
        .expect("stories are countable");
    let items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(scope)
            .fetch_one(pool)
            .await
            .expect("work items are countable");
    let runs: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(scope)
            .fetch_one(pool)
            .await
            .expect("runs are countable");
    (stories, items, runs)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-003).
async fn forge_story_run_003__actual_execution_environment_recorded() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let story = format!("{PROOF_PREFIX}environment-{ns}");

    // 1. THE INTENT IS DECLARED ON THE ITEM — deliberately as PROD, so that if the run ever read the intent
    //    instead of the connection, this proof would say so instead of quietly agreeing.
    let item = seed(&harness, &story).await;
    sqlx::query("update agent_work_item set execution_environment='PROD' where id=$1::uuid")
        .bind(&item)
        .execute(&pool)
        .await
        .expect("`PROD` is one of the four words migration 030 allows");

    // 2. THE CLAIM, THEN THE RUN OPEN through the production seam.
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("the proof item is claimable");
    let began = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .expect("the production begin runs")
        .expect("the claim opens the run");
    assert!(
        !began.story_run_id.is_empty(),
        "{HARNESS}: control — beginning execution opens a durable Story Run"
    );

    // 3. READ THE COMMITTED TRUTH OFF THE ROWS (on the pool, not from the DAO's return value), then the fault
    //    case, then CLEANUP — so a failing contract still leaves DEV as it was found.
    let run_environment: Option<String> = sqlx::query_scalar(
        "select execution_environment from storyboard_story_run where id=$1::uuid",
    )
    .bind(&began.story_run_id)
    .fetch_one(&pool)
    .await
    .expect("the run's execution_environment is readable");
    let run_story: Option<String> =
        sqlx::query_scalar("select story_id from storyboard_story_run where id=$1::uuid")
            .bind(&began.story_run_id)
            .fetch_one(&pool)
            .await
            .expect("the run row is readable");
    let run_count: i64 = sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id=$1 and execution_environment is not null",
    )
    .bind(&story)
    .fetch_one(&pool)
    .await
    .expect("runs are countable");
    let item_intent: Option<String> =
        sqlx::query_scalar("select execution_environment from agent_work_item where id=$1::uuid")
            .bind(&item)
            .fetch_one(&pool)
            .await
            .expect("the item's intent is readable");

    // 4. NEGATIVE / FAULT — THE COLUMN ONLY HOLDS ITS FOUR WORDS: a run can never be labelled with an
    //    environment the schema does not know, so the recorded value cannot be forged after the fact.
    let check_refused = sqlx::query(
        "update storyboard_story_run set execution_environment='MARS' where id=$1::uuid",
    )
    .bind(&began.story_run_id)
    .execute(&pool)
    .await
    .is_err();

    // 5. CLEANUP / ROLLBACK: the proof story is deleted; its item and run go with it.
    harness
        .cleanup_story(&story)
        .await
        .expect("reap the proof story");
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let (stories, items, runs) = leftover(&pool, &scope).await;

    // ---------------------------------------------------------------------------------------------------------
    // 6. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert_eq!(
        run_environment.as_deref(),
        Some("DEV"),
        "{HARNESS}: the Story Run records the ACTUAL target of the database it executed on"
    );
    assert_eq!(
        run_story.as_deref(),
        Some(story.as_str()),
        "{HARNESS}: the recorded environment belongs to this story's own run"
    );
    assert_eq!(
        run_count, 1,
        "{HARNESS}: exactly one run opened for the proof story, and it carries the environment"
    );
    assert_eq!(
        item_intent.as_deref(),
        Some("PROD"),
        "{HARNESS}: the enqueuer's intent survives beside the fact; only the run writes the actual one"
    );
    assert_ne!(
        run_environment, item_intent,
        "{HARNESS}: the intent declared on the item must never be read as the run's actual environment"
    );

    // 7. NEGATIVE / REFUSALS.
    assert!(
        check_refused,
        "{HARNESS}: a word outside the schema's four must be refused by the column's CHECK"
    );
    match guard_target(DbTarget::Prod) {
        Err(HarnessDbError::ProductionRefused(_)) => {}
        other => panic!(
            "{HARNESS}: a PRODUCTION target must be refused by the harness before any socket, got {other:?}"
        ),
    }
    guard_target(DbTarget::Dev)
        .expect("{HARNESS}: the disposable DEV target is the one the harness opens");

    // 8. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        items, 0,
        "{HARNESS}: the proof must leave no work item behind"
    );
    assert_eq!(
        runs, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

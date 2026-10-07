//! FORGE.ENVELOPE — execution environment (TST-FORGE-ENVELOPE-009).
//!
//! Contract: a run records the environment it ACTUALLY executed on, taken from the database the process is
//! connected to — never from the intent somebody declared on the way in, and never from a caller's argument.
//!
//!   1. **the run open** — `ForgeEngineDao::begin_agent_work_run` binds
//!      `run_execution_environment(self.db.declared_target())` (`db/src/forge_engine.rs:130-135`, `299-315`),
//!      the one fact only the calling process knows, into `forge_begin_agent_work_run`
//!      (`db/migrations/264_forge_agent_work_begin.sql`), which writes it to
//!      `storyboard_story_run.execution_environment` in the insert that opens the run. The DAO method takes no
//!      environment parameter, so no caller can declare a target for the run it is opening.
//!   2. **two columns, one writer each** — `agent_work_item.execution_environment` is the INTENDED target the
//!      enqueuer declared; `storyboard_story_run.execution_environment` is the ACTUAL target of the run
//!      (migration 030). The intent is preserved beside the fact and is never read as it: a mismatch between
//!      them must be visible on the board, not reconciled away.
//!   3. **the refusal** — the harness never opens a PRODUCTION socket: `guard_target`
//!      (`tests/src/database.rs`) refuses `DbTarget::Prod` before any connection is attempted, which is the
//!      guard every `TestDatabase` constructor is built on.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target.
//! Raw SQL here is fixture setup and read-back only.
//!
//! Boundary: L3 Composition, harness `ForgeHarness`.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_envelope__009__execution_environment -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::{guard_target, ForgeHarness, HarnessDbError};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner the proof claim is taken under.
const OWNER: &str = "forge-envelope-009-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-009-";

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
    // Retry the seed the way `connect_dev` retries the handshake: this DEV branch is shared with several
    // contract tests and the engine at once, so one statement can fail under contention before anything about
    // the queue has actually gone wrong. A story that DID land is not retried into a duplicate — the item is
    // read back instead, which is the same row the helper would have returned.
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

/// Read one fact back off the durable rows. Fixture read-back, not a second implementation of the seam.
async fn stored_intended(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select execution_environment from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's intended target is readable")
}

/// The `execution_environment` the Story Run recorded — read back from the committed run row.
async fn run_environment(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar("select execution_environment from storyboard_story_run where id=$1::uuid")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("the Story Run's own execution_environment is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-009).
async fn forge_envelope_009__execution_environment() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let story = format!("{PROOF_PREFIX}environment-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE INTENT IS DECLARED ON THE ITEM — deliberately as PROD, so that if the run ever read the intent
    //    instead of the connection, this proof would say so instead of quietly agreeing.
    // -----------------------------------------------------------------------------------------------------------
    let item = seed(&harness, &story).await;
    sqlx::query("update agent_work_item set execution_environment='PROD' where id=$1::uuid")
        .bind(&item)
        .execute(&pool)
        .await
        .expect("`PROD` is one of the four words migration 030 allows");
    assert_eq!(
        stored_intended(&pool, &item).await.as_deref(),
        Some("PROD"),
        "{HARNESS}: control — the durable envelope carries the enqueuer's intended target"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CLAIM, THEN THE RUN OPEN. The run's environment must be the target of the database this process is
    //    actually on — DEV — and not the PROD the row declares as intent.
    // -----------------------------------------------------------------------------------------------------------
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .unwrap()
        .expect("the proof item is claimable");
    assert!(
        !claimed.id.is_empty(),
        "{HARNESS}: control — the claim handed over a row"
    );
    let began = harness
        .engine()
        .begin_agent_work_run(&item)
        .await
        .unwrap()
        .expect("the claim opens a run");
    assert!(
        !began.story_run_id.is_empty(),
        "{HARNESS}: control — the run was opened"
    );
    assert_eq!(
        run_environment(&pool, &began.story_run_id).await.as_deref(),
        Some("DEV"),
        "{HARNESS}: the run records the ACTUAL target of the database it executed on"
    );
    assert_ne!(
        run_environment(&pool, &began.story_run_id).await.as_deref(),
        Some("PROD"),
        "{HARNESS}: the intended target on the item must never be read as the run's actual environment"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. TWO COLUMNS, ONE WRITER EACH. The intent survives beside the fact — undeleted, un-overwritten, and not
    //    reconciled into agreement with the run.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        stored_intended(&pool, &item).await.as_deref(),
        Some("PROD"),
        "{HARNESS}: the item's intended target is preserved exactly as declared; only the run writes the actual one"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — THE HARNESS NEVER OPENS PRODUCTION. `guard_target` is the guard every
    //    `TestDatabase` constructor is built on, and it refuses PROD before any socket is opened.
    // -----------------------------------------------------------------------------------------------------------
    match guard_target(DbTarget::Prod) {
        Err(HarnessDbError::ProductionRefused(_)) => {}
        other => panic!(
            "{HARNESS}: a PRODUCTION target must be refused by the harness before any socket, got {other:?}"
        ),
    }
    guard_target(DbTarget::Dev)
        .expect("{HARNESS}: the disposable DEV target is the one the harness opens");

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — THE COLUMN ONLY HOLDS ITS FOUR WORDS, so a run can never be labelled with an
    //    environment the schema does not know.
    // -----------------------------------------------------------------------------------------------------------
    let refused =
        sqlx::query("update agent_work_item set execution_environment='MARS' where id=$1::uuid")
            .bind(&item)
            .execute(&pool)
            .await;
    assert!(
        refused.is_err(),
        "{HARNESS}: a word outside the four must be refused by the column's CHECK"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / ROLLBACK. This run's proof story is deleted; its items and run go with it.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&story)
        .await
        .expect("reap the proof story");
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let leftover_stories: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_stories, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    let leftover_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_items, 0,
        "{HARNESS}: the proof must leave no work item behind"
    );
    let leftover_runs: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_runs, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

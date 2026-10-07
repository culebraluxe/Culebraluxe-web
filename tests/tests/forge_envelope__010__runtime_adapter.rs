//! FORGE.ENVELOPE — runtime adapter (TST-FORGE-ENVELOPE-010).
//!
//! Contract: the `runtime_adapter` the durable dispatch envelope carries reaches the process that executes the
//! run. The envelope column is `agent_work_item.runtime_adapter` (migration 028: "adapter id selected for this
//! attempt"), and the claim is the one seam where that envelope crosses from the durable row into the executing
//! process: `forge_claim_specific_agent_work` returns `w.*` (`db/migrations/262_forge_agent_work_claim.sql`), so
//! every column is right there in the routine's answer.
//!
//! What crosses is decided by `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246-261`),
//! whose SELECT list — and therefore `db::ForgeAgentWorkRow`, the type the executing process receives — names
//! `id, story_id, state, claimed_by, role, kind, work_type, execution_policy, model_policy, stop_after,
//! launch_intent`. This test observes exactly that: the payload the production claim hands across, read from the
//! derived `Debug` of the production row type, which names the fields that cross and nothing else. Two control
//! assertions prove the observation is sound and that absence is preserved before the subject assertion runs, so
//! a failure here is the envelope dropping a field, not a fixture that never landed and not an observation that
//! cannot see anything. The `agent_runtime` the opened run records is reported alongside it as corroboration.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! `TestDatabase` refuses PRODUCTION before any socket is opened. Raw SQL here is fixture setup and read-back
//! only. No production code is changed to make this pass — a faithful test that exposes the gap is the result.
//!
//! Boundary: L3 Composition, harness `ForgeHarness`.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_envelope__010__runtime_adapter -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner every proof claim is taken under.
const OWNER: &str = "forge-envelope-010-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-010-";
/// The adapter the fixture puts on the durable envelope.
const ADAPTER: &str = "opencode-harness";

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

/// Read the adapter back off the durable row. Fixture read-back, not a second claim.
async fn stored_adapter(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select runtime_adapter from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored runtime_adapter is readable")
}

/// The `agent_runtime` the Story Run recorded — read back from the committed run row.
async fn run_agent_runtime(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar("select agent_runtime from storyboard_story_run where id=$1::uuid")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("the Story Run's own agent_runtime is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-010).
async fn forge_envelope_010__runtime_adapter() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let adapter_story = format!("{PROOF_PREFIX}adapter-{ns}");
    let bare_story = format!("{PROOF_PREFIX}bare-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. CONTROL — THE FIXTURE LANDED. The adapter is durable on the envelope row the claim reads from, so a
    //    failure below cannot be explained by a fixture that never applied.
    // -----------------------------------------------------------------------------------------------------------
    let adapter_item = seed(&harness, &adapter_story).await;
    sqlx::query("update agent_work_item set runtime_adapter=$2 where id=$1::uuid")
        .bind(&adapter_item)
        .bind(ADAPTER)
        .execute(&pool)
        .await
        .expect("the runtime-adapter column accepts the fixture adapter");
    assert_eq!(
        stored_adapter(&pool, &adapter_item).await.as_deref(),
        Some(ADAPTER),
        "{HARNESS}: control — the durable envelope carries the runtime adapter"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CLAIM, AND THE RUN IT OPENS. The claim is the composition boundary the envelope has to cross; the
    //    run open is the execution it has to configure.
    // -----------------------------------------------------------------------------------------------------------
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&adapter_item, OWNER)
        .await
        .unwrap()
        .expect("the adapter proof item is claimable");
    let payload = format!("{claimed:?}");
    let began = harness
        .engine()
        .begin_agent_work_run(&adapter_item)
        .await
        .unwrap()
        .expect("the claim opens a run");

    // CONTROL — THE OBSERVATION WORKS. The payload the production claim hands across names the envelope fields
    // production does carry, so the subject assertion below is reading a real payload and not an empty one.
    assert!(
        payload.contains("execution_policy"),
        "{HARNESS}: control — the claim payload names the execution policy field"
    );
    assert!(
        payload.contains("launch_intent"),
        "{HARNESS}: control — the claim payload names the launch_intent field"
    );
    assert!(
        !began.story_run_id.is_empty(),
        "{HARNESS}: control — the claim opened the Story Run the executing process hangs its work off"
    );
    assert_eq!(
        stored_adapter(&pool, &adapter_item).await.as_deref(),
        Some(ADAPTER),
        "{HARNESS}: control — the claim does not consume the adapter from the row; it stays durable"
    );

    // NEGATIVE — ABSENCE IS PRESERVED. An item that declares no adapter must not have one invented for it by
    // the claim or by the run it opens: a run never records an adapter nobody selected.
    let bare_item = seed(&harness, &bare_story).await;
    let bare_claim = harness
        .engine()
        .claim_specific_agent_work(&bare_item, OWNER)
        .await
        .unwrap()
        .expect("the bare proof item is claimable");
    let bare_payload = format!("{bare_claim:?}");
    let bare_began = harness
        .engine()
        .begin_agent_work_run(&bare_item)
        .await
        .unwrap()
        .expect("the bare claim opens a run");
    assert!(
        !bare_payload.contains(ADAPTER),
        "{HARNESS}: a claim must never invent an adapter for an item that declares none"
    );
    assert_eq!(
        run_agent_runtime(&pool, &bare_began.story_run_id).await,
        None,
        "{HARNESS}: a run whose envelope named no adapter records none — absence is preserved"
    );

    // The run-side observation is read back BEFORE cleanup, because the proof story is about to take its runs
    // with it. It is reported alongside the subject assertion below as the same fact on the run's side.
    let observed_run_runtime = run_agent_runtime(&pool, &began.story_run_id).await;

    // -----------------------------------------------------------------------------------------------------------
    // 3. CLEANUP / ROLLBACK, run BEFORE the subject assertion so a failing contract still leaves DEV as it was
    //    found. This run's proof stories are deleted; their items and runs go with them.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&adapter_story)
        .await
        .expect("reap the adapter proof story");
    harness
        .cleanup_story(&bare_story)
        .await
        .expect("reap the bare proof story");
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

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE CONTRACT — THE ENVELOPE MUST HAND THE ADAPTER TO THE PROCESS THAT EXECUTES. The database routine
    //    returns `w.*` (migration 262), so the value is in the claim's own answer; what the executing process
    //    receives is `db::ForgeAgentWorkRow`, and it must carry this field like it carries the rest. The
    //    `agent_runtime` the opened run recorded is reported alongside, as the same fact on the run's side.
    //
    //    A FAILURE HERE IS THE FINDING, NOT A BROKEN TEST: `runtime_adapter` is selected by nothing and written
    //    by nothing (only stale recovery ever nulls it, `db/migrations/266_forge_stale_recovery.sql`), so the
    //    adapter the board names never reaches the lane it was meant to describe.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        payload.contains("runtime_adapter"),
        "{HARNESS}: the durable dispatch envelope must carry runtime_adapter across the claim into the process \
         that executes the run. The payload the production claim handed across was: {payload}. The durable row \
         said runtime_adapter={ADAPTER:?} and the Story Run this claim opened recorded agent_runtime={observed_run_runtime:?}."
    );
}

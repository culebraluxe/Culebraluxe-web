//! FORGE.ENVELOPE — special instructions (TST-FORGE-ENVELOPE-006).
//!
//! Contract: the `special_instructions` the durable dispatch envelope carries reach the task text the child
//! executes. The envelope column is `agent_work_item.special_instructions` (migration 028: "optional additive
//! instructions for a run"), and the only place instructions ever become something an executing agent reads is
//! `forge::engine::packet::build_task_text` (`forge/src/engine/packet.rs:104-110`), which writes them under the
//! sentinel `Special instructions (additive, do not replace the architect brief):`.
//!
//! So the chain has two halves, and this test holds both:
//!
//!   1. **the crossing** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) is the
//!      seam where the durable envelope leaves the row and enters the process that executes the story. The claim
//!      routine returns `w.*` (`db/migrations/262_forge_agent_work_claim.sql`), so the value is in its answer;
//!      what the process receives is `db::ForgeAgentWorkRow`, whose SELECT list is what decides the crossing.
//!   2. **the consumption** — the packet builder embeds the instructions verbatim, once, and writes no line at
//!      all for an absent fact. This half is asserted as a control: it passes, which is what makes a failure of
//!      the first half a finding about the crossing rather than a broken builder.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! `TestDatabase` refuses PRODUCTION before any socket is opened. Raw SQL here is fixture setup and read-back
//! only. No production code is changed to make this pass.
//!
//! Boundary: L3 Composition, harness `ForgeHarness`.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_envelope__006__special_instructions -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use forge::engine::packet::{build_task_text, StoryPacket};
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner the proof claim is taken under.
const OWNER: &str = "forge-envelope-006-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-006-";
/// The instructions the fixture puts on the durable envelope.
const INSTRUCTION: &str = "ALPHA-INSTRUCTION: land the fix before the assay runs";
/// The exact sentinel `build_task_text` writes before the instructions (`forge/src/engine/packet.rs:107`).
const SENTINEL: &str = "Special instructions (additive, do not replace the architect brief):";

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

/// Read the instructions back off the durable row. Fixture read-back, not a second claim.
async fn stored_instructions(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select special_instructions from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored special instructions are readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-006).
async fn forge_envelope_006__special_instructions() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let story = format!("{PROOF_PREFIX}instructions-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. CONTROL — THE FIXTURE LANDED. The instructions are durable on the envelope row the claim reads from,
    //    so a failure below cannot be explained by a fixture that never applied.
    // -----------------------------------------------------------------------------------------------------------
    let item = seed(&harness, &story).await;
    sqlx::query("update agent_work_item set special_instructions=$2 where id=$1::uuid")
        .bind(&item)
        .bind(INSTRUCTION)
        .execute(&pool)
        .await
        .expect("the special-instructions column accepts the fixture value");
    assert_eq!(
        stored_instructions(&pool, &item).await.as_deref(),
        Some(INSTRUCTION),
        "{HARNESS}: control — the durable envelope carries the special instructions"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CLAIM, AND THE RUN IT OPENS. The claim is the composition boundary the envelope has to cross; the
    //    run open is the execution it has to configure.
    // -----------------------------------------------------------------------------------------------------------
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&item, OWNER)
        .await
        .unwrap()
        .expect("the proof item is claimable");
    let payload = format!("{claimed:?}");
    let began = harness
        .engine()
        .begin_agent_work_run(&item)
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
        payload.contains("stop_after"),
        "{HARNESS}: control — the claim payload names the stop_after field"
    );
    assert!(
        !began.story_run_id.is_empty(),
        "{HARNESS}: control — the claim opened the Story Run the executing process hangs its work off"
    );
    assert_eq!(
        stored_instructions(&pool, &item).await.as_deref(),
        Some(INSTRUCTION),
        "{HARNESS}: control — the claim does not consume the instructions from the row; they stay durable"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CONTROL — THE BUILDER HONOURS THE FIELD. When a packet carries instructions, the task text embeds them
    //    verbatim under the sentinel, exactly once. This half of the chain works, which is what localises any
    //    failure above to the crossing rather than to the consumption.
    // -----------------------------------------------------------------------------------------------------------
    let carried = StoryPacket {
        id: story.clone(),
        title: "Forge envelope proof".to_string(),
        special_instructions: Some(INSTRUCTION.to_string()),
        ..Default::default()
    };
    let text = build_task_text("smith", "task-a", &carried, None);
    assert!(
        text.contains(&format!("{SENTINEL} {INSTRUCTION}")),
        "{HARNESS}: control — the task text embeds the packet's instructions verbatim under the sentinel"
    );
    assert_eq!(
        text.matches(SENTINEL).count(),
        1,
        "{HARNESS}: control — the special-instructions block appears exactly once in the task text"
    );

    // NEGATIVE — ABSENCE IS PRESERVED. An absent or empty instruction set fabricates no line, so a child is
    // never handed an instruction nobody wrote, and one packet's instructions never appear in another's text.
    let absent = StoryPacket {
        special_instructions: None,
        ..Default::default()
    };
    let absent_text = build_task_text("smith", "task-b", &absent, None);
    assert!(
        !absent_text.contains("Special instructions"),
        "{HARNESS}: the builder must not fabricate an instruction line for an absent fact"
    );
    let empty = StoryPacket {
        special_instructions: Some(String::new()),
        ..Default::default()
    };
    let empty_text = build_task_text("smith", "task-c", &empty, None);
    assert!(
        !empty_text.contains("Special instructions"),
        "{HARNESS}: an empty instruction set is absence, not an instruction"
    );
    assert!(
        !absent_text.contains(INSTRUCTION),
        "{HARNESS}: one packet's instructions must never appear in another packet's task text"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / ROLLBACK, run BEFORE the subject assertion so a failing contract still leaves DEV as it was
    //    found. This run's proof story is deleted; its items and run go with it.
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

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE CONTRACT — THE ENVELOPE MUST HAND THE INSTRUCTIONS TO THE PROCESS THAT EXECUTES. The database
    //    routine returns `w.*` (migration 262), so the value is in the claim's own answer; what the executing
    //    process receives is `db::ForgeAgentWorkRow`, and it must carry this field like it carries the rest.
    //
    //    A FAILURE HERE IS THE FINDING, NOT A BROKEN TEST: the instructions reach a run only through an
    //    environment packet nobody populates from the row (docs/agent/OLD-ENGINE-CONTRACT-RESTORATION.md), so
    //    the column the board writes never reaches the lane it was meant to instruct.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        payload.contains("special_instructions"),
        "{HARNESS}: the durable dispatch envelope must carry special_instructions across the claim into the \
         process that executes the run; the payload the production claim handed across was: {payload}"
    );
}

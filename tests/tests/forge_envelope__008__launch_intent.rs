//! FORGE.ENVELOPE — launch intent (TST-FORGE-ENVELOPE-008).
//!
//! Contract: the `launch_intent` the durable dispatch envelope carries is the operator's cap for THIS dispatch,
//! and it is read off the row at the moment execution begins — so a launcher cannot substitute its own.
//!
//!   1. **the claim** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) hands the
//!      claimed row, `launch_intent` included, to the process that executes the story.
//!   2. **the run open** — `forge_begin_agent_work_run` returns `w.launch_intent` from the same statement that
//!      moves the item `Claimed → Running` (`db/migrations/264_forge_agent_work_begin.sql`), and
//!      `BeginAgentWorkRun` carries it back to the engine (`db/src/forge_engine.rs:85`) as the Lead's bench
//!      intent. Both reads are one fact; a disagreement between them would mean the run was opened under
//!      something other than the row.
//!
//! The column is CHECK-constrained to `SOLO` / `SMITH` / `SPLIT` / `HOLD` (`agent_work_item_launch_intent_check`)
//! and is nullable — migration 167's meaning for NULL is "the Lead decides", so absence is preserved rather than
//! defaulted into a cap nobody set.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! `TestDatabase` refuses PRODUCTION before any socket is opened. Raw SQL here is fixture setup and read-back
//! only.
//!
//! Boundary: L3 Composition, harness `ForgeHarness`.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_envelope__008__launch_intent -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner every proof claim is taken under.
const OWNER: &str = "forge-envelope-008-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-008-";

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

/// Read the cap back off the durable row. Fixture read-back, not a second claim.
async fn stored_intent(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select launch_intent from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored launch_intent is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-008).
async fn forge_envelope_008__launch_intent() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let solo_story = format!("{PROOF_PREFIX}solo-{ns}");
    let hold_story = format!("{PROOF_PREFIX}hold-{ns}");
    let open_story = format!("{PROOF_PREFIX}open-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLAIM HANDS THE CAP TO THE PROCESS THAT EXECUTES.
    // -----------------------------------------------------------------------------------------------------------
    let solo_item = seed(&harness, &solo_story).await;
    sqlx::query("update agent_work_item set launch_intent='SOLO' where id=$1::uuid")
        .bind(&solo_item)
        .execute(&pool)
        .await
        .expect("`SOLO` is one of the four words migration 167 allows");
    assert_eq!(
        stored_intent(&pool, &solo_item).await.as_deref(),
        Some("SOLO"),
        "{HARNESS}: control — the durable envelope carries the cap"
    );
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&solo_item, OWNER)
        .await
        .unwrap()
        .expect("the SOLO proof item is claimable");
    assert_eq!(
        claimed.launch_intent.as_deref(),
        Some("SOLO"),
        "{HARNESS}: the claim hands the executing process the row's launch intent"
    );
    let payload = format!("{claimed:?}");
    assert!(
        payload.contains("launch_intent:"),
        "{HARNESS}: the claim payload names the launch_intent field (payload: {payload})"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RUN OPEN READS IT AGAIN, FROM THE ROW. `begin_agent_work_run` takes no intent argument, so the cap
    //    the run starts under can only be the one the durable envelope carries.
    // -----------------------------------------------------------------------------------------------------------
    let began = harness
        .engine()
        .begin_agent_work_run(&solo_item)
        .await
        .unwrap()
        .expect("the SOLO claim opens a run");
    assert_eq!(
        began.launch_intent.as_deref(),
        Some("SOLO"),
        "{HARNESS}: the statement that opens the run returns the row's launch intent"
    );
    assert_eq!(
        claimed.launch_intent, began.launch_intent,
        "{HARNESS}: both reads are one fact — the claim and the run open may not disagree"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — NO SUBSTITUTION ACROSS STORIES. A second story capped `HOLD` is read as `HOLD`, never as
    //    the cap the previous claim returned.
    // -----------------------------------------------------------------------------------------------------------
    let hold_item = seed(&harness, &hold_story).await;
    sqlx::query("update agent_work_item set launch_intent='HOLD' where id=$1::uuid")
        .bind(&hold_item)
        .execute(&pool)
        .await
        .expect("`HOLD` is one of the four words migration 167 allows");
    let hold_claim = harness
        .engine()
        .claim_specific_agent_work(&hold_item, OWNER)
        .await
        .unwrap()
        .expect("the HOLD proof item is claimable");
    assert_eq!(
        hold_claim.launch_intent.as_deref(),
        Some("HOLD"),
        "{HARNESS}: each claim reads its own row, never a neighbour's cap"
    );
    let hold_began = harness
        .engine()
        .begin_agent_work_run(&hold_item)
        .await
        .unwrap()
        .expect("the HOLD claim opens a run");
    assert_eq!(
        hold_began.launch_intent.as_deref(),
        Some("HOLD"),
        "{HARNESS}: the run opens under its own row's cap"
    );
    assert_ne!(
        hold_began.launch_intent, began.launch_intent,
        "{HARNESS}: one story's cap must never be read as another story's"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — ABSENCE MEANS THE LEAD DECIDES. A NULL cap is preserved as `None` across both reads, not
    //    defaulted into a cap nobody set.
    // -----------------------------------------------------------------------------------------------------------
    let open_item = seed(&harness, &open_story).await;
    assert_eq!(
        stored_intent(&pool, &open_item).await,
        None,
        "{HARNESS}: a dispatch queued with no cap carries none"
    );
    let open_claim = harness
        .engine()
        .claim_specific_agent_work(&open_item, OWNER)
        .await
        .unwrap()
        .expect("the open proof item is claimable");
    assert_eq!(
        open_claim.launch_intent, None,
        "{HARNESS}: absence is preserved across the claim, never backfilled into a cap"
    );
    let open_began = harness
        .engine()
        .begin_agent_work_run(&open_item)
        .await
        .unwrap()
        .expect("the open claim opens a run");
    assert_eq!(
        open_began.launch_intent, None,
        "{HARNESS}: absence is preserved across the run open — the Lead still decides"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — THE COLUMN ONLY HOLDS ITS FOUR WORDS, so a fifth can never cap a dispatch.
    // -----------------------------------------------------------------------------------------------------------
    let refused =
        sqlx::query("update agent_work_item set launch_intent='SUPERVISOR' where id=$1::uuid")
            .bind(&solo_item)
            .execute(&pool)
            .await;
    assert!(
        refused.is_err(),
        "{HARNESS}: a word outside the four must be refused by the column's CHECK"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / ROLLBACK. Each proof story is deleted; items and runs go with it.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&solo_story)
        .await
        .expect("reap the SOLO proof story");
    harness
        .cleanup_story(&hold_story)
        .await
        .expect("reap the HOLD proof story");
    harness
        .cleanup_story(&open_story)
        .await
        .expect("reap the open proof story");
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

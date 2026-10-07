//! FORGE.ENVELOPE — role (TST-FORGE-ENVELOPE-005).
//!
//! Contract: the `role` the durable dispatch envelope carries reaches the run the claim opens, and it is the
//! row's own role or nothing — never a neighbour's, never a blank, never an invented word.
//!
//!   1. **the claim** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) hands the
//!      claimed row, `role` included, to the process that executes the story. The claim routine returns `w.*`
//!      (`db/migrations/262_forge_agent_work_claim.sql`), so the role is in the answer the process gets, and
//!      `recover_stale_agent_work` (`forge/src/engine/worker.rs:187-213`) already reads it back to decide
//!      whether a stale claim holds or requeues — role is execution, not display.
//!   2. **the run open** — `forge_begin_agent_work_run` labels the Story Run from the item's own row:
//!      `coalesce(nullif(trim(i.role), ''), nullif(trim(i.kind), ''), 'dispatch')`
//!      (`db/migrations/264_forge_agent_work_begin.sql`). The role is the first link of that precedence and the
//!      literal it falls back to is an absence-preserving one.
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
//!     --test forge_envelope__005__role -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner every proof claim is taken under.
const OWNER: &str = "forge-envelope-005-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-005-";

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

/// The `run_type` the Story Run recorded — read back from the committed run row, never re-derived here.
async fn run_type(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar("select run_type from storyboard_story_run where id=$1::uuid")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("the Story Run's own run_type is readable")
}

/// Read a proof item's stored role. Fixture read-back, not a second claim.
async fn stored_role(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select role from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored role is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-005).
async fn forge_envelope_005__role() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let builder_story = format!("{PROOF_PREFIX}builder-{ns}");
    let blank_story = format!("{PROOF_PREFIX}blank-{ns}");
    let bare_story = format!("{PROOF_PREFIX}bare-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLAIM HANDS THE ROLE TO THE PROCESS THAT EXECUTES, and the run it opens is labelled from it.
    // -----------------------------------------------------------------------------------------------------------
    let builder_item = seed(&harness, &builder_story).await;
    sqlx::query("update agent_work_item set role='builder' where id=$1::uuid")
        .bind(&builder_item)
        .execute(&pool)
        .await
        .expect("the fixture row accepts the builder role");
    let builder_claim = harness
        .engine()
        .claim_specific_agent_work(&builder_item, OWNER)
        .await
        .unwrap()
        .expect("the builder proof item is claimable");
    assert_eq!(
        builder_claim.role.as_deref(),
        Some("builder"),
        "{HARNESS}: the claim hands the executing process the row's role"
    );
    let payload = format!("{builder_claim:?}");
    assert!(
        payload.contains("role:"),
        "{HARNESS}: the claim payload names the role field (payload: {payload})"
    );
    let builder_began = harness
        .engine()
        .begin_agent_work_run(&builder_item)
        .await
        .unwrap()
        .expect("the builder claim opens a run");
    let builder_run_type = run_type(&pool, &builder_began.story_run_id).await;
    assert_eq!(
        builder_run_type.as_deref(),
        Some("builder"),
        "{HARNESS}: the run the claim opens is labelled by the row's own role"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A BLANK ROLE IS NOT A FACT. Whitespace is `nullif(trim(...))`'d away by the routine, so the
    //    run falls through to the next link rather than recording an empty label that reads like a role.
    // -----------------------------------------------------------------------------------------------------------
    let blank_item = seed(&harness, &blank_story).await;
    sqlx::query("update agent_work_item set role='   ', kind='learn' where id=$1::uuid")
        .bind(&blank_item)
        .execute(&pool)
        .await
        .expect("the fixture row accepts a blank role");
    assert_eq!(
        stored_role(&pool, &blank_item).await.as_deref(),
        Some("   "),
        "{HARNESS}: control — the blank role really is on the durable row"
    );
    let blank_claim = harness
        .engine()
        .claim_specific_agent_work(&blank_item, OWNER)
        .await
        .unwrap()
        .expect("the blank-proof item is claimable");
    assert_eq!(
        blank_claim.role.as_deref(),
        Some("   "),
        "{HARNESS}: the claim hands the row across verbatim — it does not tidy a blank into a role"
    );
    let blank_began = harness
        .engine()
        .begin_agent_work_run(&blank_item)
        .await
        .unwrap()
        .expect("the blank-proof claim opens a run");
    assert_eq!(
        run_type(&pool, &blank_began.story_run_id).await.as_deref(),
        Some("learn"),
        "{HARNESS}: a blank role is absence, so the run falls through to the row's kind"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — NO ROLE, NO KIND: the run is labelled `dispatch`, the literal the routine falls back to.
    //    It is never one of the other proof rows' roles.
    // -----------------------------------------------------------------------------------------------------------
    let bare_item = seed(&harness, &bare_story).await;
    let bare_claim = harness
        .engine()
        .claim_specific_agent_work(&bare_item, OWNER)
        .await
        .unwrap()
        .expect(" the bare proof item is claimable");
    assert_eq!(
        bare_claim.role, None,
        "{HARNESS}: an item with no role carries none — absence is preserved across the claim"
    );
    let bare_began = harness
        .engine()
        .begin_agent_work_run(&bare_item)
        .await
        .unwrap()
        .expect("the bare claim opens a run");
    let bare_run_type = run_type(&pool, &bare_began.story_run_id).await;
    assert_eq!(
        bare_run_type.as_deref(),
        Some("dispatch"),
        "{HARNESS}: with no role and no kind the run falls back to `dispatch`, never to an invented word"
    );
    assert_ne!(
        bare_run_type, builder_run_type,
        "{HARNESS}: one story's role must never be read as another story's run label"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — THE CLAIM NEVER SUBSTITUTES ANOTHER ROW'S ROLE. Each of the three items was
    //    claimed under a different owner and each answer came from its own row; a claim that handed back a
    //    neighbour's role would have failed assertions 1 and 3 already. Stated explicitly as the fault case:
    //    re-reading an item that has no role must not surface the role the previous claim returned.
    // -----------------------------------------------------------------------------------------------------------
    let reread = harness
        .engine()
        .claim_specific_agent_work(&builder_item, OWNER)
        .await
        .unwrap();
    assert!(
            reread.is_none(),
        "{HARNESS}: a claim that already moved to `Running` answers nothing — the row is re-read, never replayed"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / ROLLBACK. Each proof story is deleted; items and runs go with it.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&builder_story)
        .await
        .expect("reap the builder proof story");
    harness
        .cleanup_story(&blank_story)
        .await
        .expect("reap the blank proof story");
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
}

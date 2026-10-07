//! FORGE.ENVELOPE — kind (TST-FORGE-ENVELOPE-004).
//!
//! Contract: the `kind` the durable dispatch envelope carries reaches the run the claim opens, and it is never
//! invented on the way.
//!
//!   1. **the claim** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) hands the
//!      claimed row, `kind` included, to the process that executes the story. The claim routine returns `w.*`
//!      (`db/migrations/262_forge_agent_work_claim.sql`), so the batch word is in the answer the process gets.
//!   2. **the run open** — `forge_begin_agent_work_run` labels the Story Run from the item's own row:
//!      `coalesce(nullif(trim(i.role), ''), nullif(trim(i.kind), ''), 'dispatch')`
//!      (`db/migrations/264_forge_agent_work_begin.sql`). The run is named by the envelope, in that precedence,
//!      and the last alternative is an absence-preserving literal rather than a word somebody made up.
//!
//! The `kind` column is CHECK-constrained to the six batch words
//! (`agent_work_item_kind_check`: `qa` / `fix` / `feature` / `crm` / `judgment` / `learn`), so a seventh is
//! refused by the row rather than reaching a run.
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
//!     --test forge_envelope__004__kind -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner every proof claim is taken under.
const OWNER: &str = "forge-envelope-004-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-004-";

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

/// Delete one proof story. Items and runs cascade with it.
async fn reap(harness: &ForgeHarness, story_id: &str) {
    harness
        .cleanup_story(story_id)
        .await
        .expect("reap the proof story");
}

/// Assert that every proof row in this namespace is gone — run only once ALL proof stories are reaped.
async fn assert_no_leftovers(pool: &PgPool, ns: &str) {
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let leftover_stories: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_stories, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    let leftover_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_items, 0,
        "{HARNESS}: the proof must leave no work item behind"
    );
    let leftover_runs: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        leftover_runs, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-004).
async fn forge_envelope_004__kind() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let kind_story = format!("{PROOF_PREFIX}batch-{ns}");
    let role_story = format!("{PROOF_PREFIX}role-{ns}");
    let bare_story = format!("{PROOF_PREFIX}bare-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLAIM HANDS THE BATCH WORD TO THE PROCESS THAT EXECUTES, and the run it opens is labelled from it.
    //    `role` is deliberately left NULL here so the second link of the precedence — `kind` — is the one that
    //    names the run.
    // -----------------------------------------------------------------------------------------------------------
    let kind_item = seed(&harness, &kind_story).await;
    sqlx::query("update agent_work_item set kind='fix' where id=$1::uuid")
        .bind(&kind_item)
        .execute(&pool)
        .await
        .expect("`fix` is one of the six words migration 179 allows");
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&kind_item, OWNER)
        .await
        .unwrap()
        .expect("the proof item is claimable");
    assert_eq!(
        claimed.kind.as_deref(),
        Some("fix"),
        "{HARNESS}: the claim hands the executing process the row's kind"
    );
    let payload = format!("{claimed:?}");
    assert!(
        payload.contains("kind:"),
        "{HARNESS}: the claim payload names the kind field (payload: {payload})"
    );
    let began = harness
        .engine()
        .begin_agent_work_run(&kind_item)
        .await
        .unwrap()
        .expect("the claim opens a run");
    assert_eq!(
        run_type(&pool, &began.story_run_id).await.as_deref(),
        Some("fix"),
        "{HARNESS}: with no role set, the run the claim opens is labelled by the row's kind"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — ROLE PRECEDES KIND, and neither may invent the other's answer. A role on the row wins, so a
    //    run is never labelled with the batch word of work it was actually handed to a role to do.
    // -----------------------------------------------------------------------------------------------------------
    let role_item = seed(&harness, &role_story).await;
    sqlx::query("update agent_work_item set role='builder', kind='qa' where id=$1::uuid")
        .bind(&role_item)
        .execute(&pool)
        .await
        .expect("the fixture row accepts a role and a batch word together");
    let role_claim = harness
        .engine()
        .claim_specific_agent_work(&role_item, OWNER)
        .await
        .unwrap()
        .expect("the role proof item is claimable");
    assert_eq!(
        role_claim.kind.as_deref(),
        Some("qa"),
        "{HARNESS}: the claim hands the executing process the row's own kind, not another item's"
    );
    let role_began = harness
        .engine()
        .begin_agent_work_run(&role_item)
        .await
        .unwrap()
        .expect("the role claim opens a run");
    assert_eq!(
        run_type(&pool, &role_began.story_run_id).await.as_deref(),
        Some("builder"),
        "{HARNESS}: role precedes kind — the run is labelled by the role the row carries"
    );
    assert_ne!(
        run_type(&pool, &role_began.story_run_id).await.as_deref(),
        Some("fix"),
        "{HARNESS}: one story's kind must never leak into another story's run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — ABSENCE IS PRESERVED. With neither role nor kind the run is labelled `dispatch`, the
    //    literal the routine falls back to. It is not a made-up word, and it is not a neighbour's kind.
    // -----------------------------------------------------------------------------------------------------------
    let bare_item = seed(&harness, &bare_story).await;
    let bare_claim = harness
        .engine()
        .claim_specific_agent_work(&bare_item, OWNER)
        .await
        .unwrap()
        .expect("the bare proof item is claimable");
    assert_eq!(
        bare_claim.kind, None,
        "{HARNESS}: an item with no batch word carries none — absence is preserved across the claim"
    );
    let bare_began = harness
        .engine()
        .begin_agent_work_run(&bare_item)
        .await
        .unwrap()
        .expect("the bare claim opens a run");
    assert_eq!(
        run_type(&pool, &bare_began.story_run_id).await.as_deref(),
        Some("dispatch"),
        "{HARNESS}: with no role and no kind the run falls back to `dispatch`, never to an invented word"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — THE COLUMN ONLY HOLDS ITS SIX WORDS, so a seventh can never reach a run.
    // -----------------------------------------------------------------------------------------------------------
    let refused = sqlx::query("update agent_work_item set kind='deploy' where id=$1::uuid")
        .bind(&kind_item)
        .execute(&pool)
        .await;
    assert!(
        refused.is_err(),
        "{HARNESS}: a word outside the six must be refused by the column's CHECK"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / ROLLBACK. Each proof story is deleted; items and runs go with it.
    // -----------------------------------------------------------------------------------------------------------
    reap(&harness, &kind_story).await;
    reap(&harness, &role_story).await;
    reap(&harness, &bare_story).await;
    assert_no_leftovers(&pool, &ns).await;
}

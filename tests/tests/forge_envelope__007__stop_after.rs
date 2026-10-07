//! FORGE.ENVELOPE — stop after (TST-FORGE-ENVELOPE-007).
//!
//! Contract: the `stop_after` the durable dispatch envelope carries caps THIS dispatch, the cap is decided by the
//! row, and a word the parser does not know is refused rather than quietly widened to the full chain.
//!
//!   1. **the claim** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) hands the
//!      claimed row, `stop_after` included, to the process that executes the story; `WorkerDispatch` carries it
//!      to the child as `--stop-after` (`forge/src/engine/worker.rs:31`, `486-487`), because the child is a
//!      different process and an envelope the child is not told is decoration.
//!   2. **the parse and the resolution** — `parse_forge_stop_after` (`forge/src/engine/executor/dispatch.rs:15`)
//!      recognises exactly `scout` / `architect` / `lead` (migration 167's three words) and answers `None` for
//!      anything else, and `resolve_forge_stop_target` (`dispatch.rs:36`) turns a target into the node set that
//!      ends the run — `None` meaning the full chain, which is what a NULL `stop_after` on the row means too.
//!
//! The column is CHECK-constrained to the three words (`agent_work_item_stop_after_check`), so a fourth is
//! refused by the row before it could ever reach a child.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim is the production `ForgeEngineDao` method
//! driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target; `TestDatabase` refuses
//! PRODUCTION before any socket is opened. Raw SQL here is fixture setup and read-back only.
//!
//! Boundary: L3 Composition, harness `ForgeHarness`.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_envelope__007__stop_after -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use forge::engine::executor::{parse_forge_stop_after, resolve_forge_stop_target, ForgeStopTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner every proof claim is taken under.
const OWNER: &str = "forge-envelope-007-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-007-";

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
async fn stored_cap(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select stop_after from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored stop_after is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-007).
async fn forge_envelope_007__stop_after() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let capped_story = format!("{PROOF_PREFIX}capped-{ns}");
    let full_story = format!("{PROOF_PREFIX}full-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLAIM HANDS THE CAP TO THE PROCESS THAT EXECUTES. The row's cap is the one the child is given.
    // -----------------------------------------------------------------------------------------------------------
    let capped_item = seed(&harness, &capped_story).await;
    sqlx::query("update agent_work_item set stop_after='architect' where id=$1::uuid")
        .bind(&capped_item)
        .execute(&pool)
        .await
        .expect("`architect` is one of the three words migration 167 allows");
    assert_eq!(
        stored_cap(&pool, &capped_item).await.as_deref(),
        Some("architect"),
        "{HARNESS}: control — the durable envelope carries the cap"
    );
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&capped_item, OWNER)
        .await
        .unwrap()
        .expect("the capped proof item is claimable");
    assert_eq!(
        claimed.stop_after.as_deref(),
        Some("architect"),
        "{HARNESS}: the claim hands the executing process the row's stop_after"
    );
    let payload = format!("{claimed:?}");
    assert!(
        payload.contains("stop_after:"),
        "{HARNESS}: the claim payload names the stop_after field (payload: {payload})"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CAP BECOMES A RUN-ENDING NODE SET. Every word the row may hold parses, and each resolves to the
    //    nodes that end the run — an empty set would stop nothing and silently widen the dispatch.
    // -----------------------------------------------------------------------------------------------------------
    for word in ["scout", "architect", "lead"] {
        let target = parse_forge_stop_after(word)
            .unwrap_or_else(|| panic!("{HARNESS}: {word} is one of migration 167's three words"));
        let nodes = resolve_forge_stop_target(Some(&target))
            .unwrap_or_else(|| panic!("{HARNESS}: {word} must resolve to a cap, never to no cap"));
        assert!(
            !nodes.is_empty(),
            "{HARNESS}: {word} must resolve to at least one node that ends the run"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — A WORD THIS DOES NOT KNOW IS REFUSED. It is never read as the full chain: the
    //    whole defect this rail replaced was a cap parsed to nothing and handed to the run as `None`.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        parse_forge_stop_after("deploy").is_none(),
        "{HARNESS}: a word outside the three must be refused, never widened to the full chain"
    );
    for junk in ["", "   ", "SCOUT LEAD", "architekt", "null"] {
        assert!(
            parse_forge_stop_after(junk).is_none(),
            "{HARNESS}: {junk:?} is not a cap and must be refused"
        );
    }
    assert!(
        resolve_forge_stop_target(Some(&ForgeStopTarget::Role("deploy"))).is_none(),
        "{HARNESS}: an unknown role target must resolve to nothing rather than to a silent full run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — ABSENCE MEANS THE FULL CHAIN. A NULL cap on the row is the absence of a cap, preserved as
    //    `None` through the claim and resolved as the full chain downstream — not an invented stopping point.
    // -----------------------------------------------------------------------------------------------------------
    let full_item = seed(&harness, &full_story).await;
    assert_eq!(
        stored_cap(&pool, &full_item).await,
        None,
        "{HARNESS}: a dispatch queued with no cap carries none"
    );
    let full_claim = harness
        .engine()
        .claim_specific_agent_work(&full_item, OWNER)
        .await
        .unwrap()
        .expect("the full-chain proof item is claimable");
    assert_eq!(
        full_claim.stop_after, None,
        "{HARNESS}: absence is preserved across the claim, never backfilled into a cap"
    );
    assert!(
        resolve_forge_stop_target(None).is_none(),
        "{HARNESS}: a NULL cap resolves to the full chain, which is the meaning of having no cap"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — THE COLUMN ONLY HOLDS ITS THREE WORDS, so a fourth can never reach a child.
    // -----------------------------------------------------------------------------------------------------------
    let refused = sqlx::query("update agent_work_item set stop_after='deploy' where id=$1::uuid")
        .bind(&capped_item)
        .execute(&pool)
        .await;
    assert!(
        refused.is_err(),
        "{HARNESS}: a word outside the three must be refused by the column's CHECK"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / ROLLBACK. Each proof story is deleted; items and runs go with it.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&capped_story)
        .await
        .expect("reap the capped proof story");
    harness
        .cleanup_story(&full_story)
        .await
        .expect("reap the full-chain proof story");
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

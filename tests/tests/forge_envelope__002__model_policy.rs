//! FORGE.ENVELOPE — model policy (TST-FORGE-ENVELOPE-002).
//!
//! Contract: the `model_policy` the durable dispatch envelope carries decides which model the lane bills, and
//! that decision is made by the ROW at two points, neither of which a caller can override with argv:
//!
//!   1. **the claim** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) hands the
//!      claimed row, `model_policy` included, to the process that executes the story.
//!   2. **the run open** — `ForgeEngineDao::begin_agent_work_run` (`db/src/forge_engine.rs:299`) returns it from
//!      the same statement that moves the item, so the policy the run starts under is read off the row at the
//!      moment execution begins.
//!
//! And at construction the harness honours it: `OpenCodeHarness::from_env_for_policy`
//! (`forge/src/engine/opencode.rs:391`) resolves the model from the row's policy — with an explicit attended
//! `OPENCODE_MODEL` winning over every policy, because that is a deliberate configuration and a policy is not.
//! The vocabulary is exactly two words (`FORGE_MODEL_POLICIES`, migration 179), an unknown or absent policy
//! reads as `cheap` rather than throwing (`as_model_policy`), and every policy names a priceable model
//! (`model_for_policy`), which is what keeps the cost lens honest.
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
//!     --test forge_envelope__002__model_policy -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use forge::engine::opencode::{
    as_model_policy, model_for_policy, resolve_model_for_policy, OpenCodeHarness,
    FORGE_MODEL_POLICIES, MODEL_FOR_CHEAP, MODEL_FOR_JUDGMENT,
};
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner both proof claims are taken under.
const OWNER: &str = "forge-envelope-002-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-002-";

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

/// Read the model policy back off the durable row. Fixture read-back, not a second claim.
async fn stored_policy(pool: &PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select model_policy from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored model policy is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-002).
async fn forge_envelope_002__model_policy() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let judged_story = format!("{PROOF_PREFIX}judgment-{ns}");
    let default_story = format!("{PROOF_PREFIX}default-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLAIM HANDS THE POLICY TO THE PROCESS THAT EXECUTES.
    // -----------------------------------------------------------------------------------------------------------
    let judged_item = seed(&harness, &judged_story).await;
    sqlx::query("update agent_work_item set model_policy='judgment' where id=$1::uuid")
        .bind(&judged_item)
        .execute(&pool)
        .await
        .expect("`judgment` is one of the two words migration 179 allows");
    assert_eq!(
        stored_policy(&pool, &judged_item).await.as_deref(),
        Some("judgment"),
        "{HARNESS}: the fixture row carries the policy the Cockpit queues work under"
    );
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&judged_item, OWNER)
        .await
        .unwrap()
        .expect("the judged item is claimable");
    assert_eq!(
        claimed.model_policy.as_deref(),
        Some("judgment"),
        "{HARNESS}: the claim hands the executing process the row's model policy"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RUN OPEN READS IT AGAIN, FROM THE ROW. No policy argument exists to substitute.
    // -----------------------------------------------------------------------------------------------------------
    let began = harness
        .engine()
        .begin_agent_work_run(&judged_item)
        .await
        .unwrap()
        .expect("the judged claim opens a run");
    assert_eq!(
        began.model_policy.as_deref(),
        Some("judgment"),
        "{HARNESS}: the statement that opens the run returns the row's model policy"
    );
    assert_eq!(
        claimed.model_policy, began.model_policy,
        "{HARNESS}: both reads are one fact — the claim and the run open may not disagree"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE HARNESS IS BUILT FROM THE ROW. The policy the dispatch carried is the model the lane bills, unless
    //    an explicit attended `OPENCODE_MODEL` says otherwise — that precedence is production's own, so the
    //    assertion follows it rather than re-declaring it.
    // -----------------------------------------------------------------------------------------------------------
    let judged_harness = OpenCodeHarness::from_env_for_policy(claimed.model_policy.as_deref())
        .expect("the row's policy builds a lane harness");
    let cheap_harness = OpenCodeHarness::from_env_for_policy(Some("cheap"))
        .expect("the cheap policy builds a lane harness");
    let absent_harness = OpenCodeHarness::from_env_for_policy(None)
        .expect("an absent policy builds a lane harness — NULL reads as cheap");
    assert!(
        !judged_harness.model.is_empty(),
        "{HARNESS}: a policy always resolves to a model the price table can price"
    );
    let explicit = std::env::var("OPENCODE_MODEL")
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if explicit {
        // An explicit attended configuration wins over every policy (documented at `from_env_for_policy`).
        assert_eq!(
            judged_harness.model, cheap_harness.model,
            "{HARNESS}: an explicit OPENCODE_MODEL outranks the row's policy, so no policy changes the model"
        );
    } else {
        // THE ROW DECIDES. This is the rail that stopped the policy the Cockpit showed from being decoration.
        assert_eq!(
            judged_harness.model,
            resolve_model_for_policy(Some("judgment")).expect("`judgment` names a model"),
            "{HARNESS}: the judged lane bills the model the row's policy names"
        );
        assert_eq!(
            cheap_harness.model,
            resolve_model_for_policy(Some("cheap")).expect("`cheap` names a model"),
            "{HARNESS}: the cheap lane bills the model the row's policy names"
        );
        assert_eq!(
            absent_harness.model,
            resolve_model_for_policy(None).expect("an absent policy names the default model"),
            "{HARNESS}: a NULL policy reads as cheap rather than inventing a third model"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE VOCABULARY. Exactly two policies, every one priceable, an unknown word reading as the cheaper
    //    default instead of opening a door to a model nobody priced.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        FORGE_MODEL_POLICIES,
        ["cheap", "judgment"],
        "{HARNESS}: the dispatch envelope's model vocabulary is exactly two words"
    );
    assert_eq!(as_model_policy(Some("judgment")), "judgment");
    assert_eq!(as_model_policy(Some("  judgment  ")), "judgment");
    assert_eq!(as_model_policy(Some("cheap")), "cheap");
    assert_eq!(
        as_model_policy(None),
        "cheap",
        "{HARNESS}: NULL reads as cheap"
    );
    assert_eq!(
        as_model_policy(Some("premium")),
        "cheap",
        "{HARNESS}: an unknown policy must read as the default, never be admitted as a third"
    );
    assert_eq!(model_for_policy(Some("judgment")), MODEL_FOR_JUDGMENT);
    assert_eq!(model_for_policy(None), MODEL_FOR_CHEAP);
    assert!(
        !MODEL_FOR_CHEAP.is_empty() && !MODEL_FOR_JUDGMENT.is_empty(),
        "{HARNESS}: every policy names a model the price table can price"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — THE COLUMN ONLY HOLDS ITS TWO WORDS, AND NULL IS LEGAL (absence means cheap).
    // -----------------------------------------------------------------------------------------------------------
    let refused =
        sqlx::query("update agent_work_item set model_policy='premium' where id=$1::uuid")
            .bind(&judged_item)
            .execute(&pool)
            .await;
    assert!(
        refused.is_err(),
        "{HARNESS}: a policy outside the two words must be refused by the column's CHECK"
    );

    let default_item = seed(&harness, &default_story).await;
    assert_eq!(
        stored_policy(&pool, &default_item).await,
        None,
        "{HARNESS}: an item queued before migration 179 carries no policy at all"
    );
    let default_claim = harness
        .engine()
        .claim_specific_agent_work(&default_item, OWNER)
        .await
        .unwrap()
        .expect("the default item is claimable");
    assert_eq!(
        default_claim.model_policy, None,
        "{HARNESS}: absence is preserved across the claim, never backfilled into a fact"
    );
    let default_began = harness
        .engine()
        .begin_agent_work_run(&default_item)
        .await
        .unwrap()
        .expect("the default claim opens a run");
    assert_eq!(
        default_began.model_policy, None,
        "{HARNESS}: absence is preserved across the run open, and reads as cheap downstream"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / ROLLBACK. This run's proof stories are deleted; items and runs go with them.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&judged_story)
        .await
        .expect("reap the judged proof story");
    harness
        .cleanup_story(&default_story)
        .await
        .expect("reap the default proof story");
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

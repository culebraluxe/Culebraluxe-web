//! FORGE.ENVELOPE — execution policy (TST-FORGE-ENVELOPE-001).
//!
//! Contract: the `execution_policy` the durable dispatch envelope carries is read TWICE on the way to execution,
//! and both reads are the row's own — never the launcher's:
//!
//!   1. **the claim** — `ForgeEngineDao::claim_specific_agent_work` (`db/src/forge_engine.rs:246`) hands the
//!      claimed row, `execution_policy` included, to the process that is about to execute the story.
//!   2. **the run open** — `ForgeEngineDao::begin_agent_work_run` (`db/src/forge_engine.rs:299`) returns it
//!      again, from the same statement that moves the item `Claimed → Running`. The routine has no policy
//!      parameter (`db/migrations/264_forge_agent_work_begin.sql`), so a caller has nowhere to put its own.
//!
//! And the rail: `forge::engine::agent_work::execution_policy_allows_unattended`
//! (`forge/src/engine/agent_work.rs:16`) admits only `Unattended OK`. The column is CHECK-constrained to four
//! words (`agent_work_item_execution_policy_check`), and the value the CHECK refuses is refused rather than
//! admitted — a rail that opens on a word it does not know is not a rail.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! `TestDatabase` refuses PRODUCTION before any socket is opened (`tests/src/database.rs`), and the harness
//! asserts the target is DEV. Raw SQL here is fixture setup and read-back only.
//!
//! Boundary: L3 Composition, harness `ForgeHarness`.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_envelope__001__execution_policy -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use forge::engine::agent_work::execution_policy_allows_unattended;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner both proof claims are taken under.
const OWNER: &str = "forge-envelope-001-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs never share a row.
const PROOF_PREFIX: &str = "TST-FORGE-ENVELOPE-001-";
/// The policy the unattended poller may claim (migration 029), as the row carries it by default.
const UNATTENDED: &str = "Unattended OK";
/// The policy that says a human must be present — the value whose whole purpose is to refuse.
const HUMAN_GATED: &str = "Human Gate";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: several contract tests plus the engine can be opening pools against
/// the same DEV branch at once, so a single handshake can time out before any statement runs. The retry changes
/// nothing about which database is targeted — `TestDatabase` still refuses PRODUCTION before any socket.
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

/// Read the execution policy back off the durable row. Fixture read-back, not a second claim.
async fn stored_policy(pool: &PgPool, item_id: &str) -> String {
    sqlx::query_scalar("select execution_policy from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item's stored execution policy is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-ENVELOPE-001).
async fn forge_envelope_001__execution_policy() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let gated_story = format!("{PROOF_PREFIX}gated-{ns}");
    let free_story = format!("{PROOF_PREFIX}free-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CLAIM HANDS THE POLICY TO THE PROCESS THAT EXECUTES. The row's default is `Unattended OK`
    //    (migration 029), and the claim carries it forward verbatim.
    // -----------------------------------------------------------------------------------------------------------
    let free_item = seed(&harness, &free_story).await;
    assert_eq!(
        stored_policy(&pool, &free_item).await,
        UNATTENDED,
        "{HARNESS}: the fixture row carries the default policy the board queues work under"
    );
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&free_item, OWNER)
        .await
        .unwrap()
        .expect("the free item is claimable");
    assert_eq!(
        claimed.execution_policy, UNATTENDED,
        "{HARNESS}: the claim hands the executing process the row's execution policy"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RUN OPEN READS IT AGAIN, FROM THE ROW. `begin_agent_work_run` has no policy parameter, so the
    //    value that gates the run can only be the one the durable envelope carries.
    // -----------------------------------------------------------------------------------------------------------
    let began = harness
        .engine()
        .begin_agent_work_run(&free_item)
        .await
        .unwrap()
        .expect("the free claim opens a run");
    assert_eq!(
        began.execution_policy, UNATTENDED,
        "{HARNESS}: the statement that opens the run returns the row's execution policy"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE RAIL. The value the rail was written for must refuse, and an unrecognised word must refuse too —
    //    a policy nobody defined is not admitted.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        execution_policy_allows_unattended(UNATTENDED),
        "{HARNESS}: `Unattended OK` is the one policy that may run with nobody watching"
    );
    assert!(
        execution_policy_allows_unattended("  Unattended OK  "),
        "{HARNESS}: the rail reads the column's own padding, not a differently-spelled policy"
    );
    for policy in ["Daytime Only", HUMAN_GATED, "Manual Only"] {
        assert!(
            !execution_policy_allows_unattended(policy),
            "{HARNESS}: {policy} must never execute unattended"
        );
    }
    for unknown in ["", "unattended ok", "UNATTENDED OK", "anything", "null"] {
        assert!(
            !execution_policy_allows_unattended(unknown),
            "{HARNESS}: an unknown policy word must be refused, not admitted ({unknown:?})"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — THE COLUMN ONLY HOLDS ITS FOUR WORDS. A policy outside the engine's vocabulary is
    //    rejected by the row itself, so no path can queue work under a policy the rail has never heard of.
    // -----------------------------------------------------------------------------------------------------------
    let gated_item = seed(&harness, &gated_story).await;
    let refused =
        sqlx::query("update agent_work_item set execution_policy='Whenever' where id=$1::uuid")
            .bind(&gated_item)
            .execute(&pool)
            .await;
    assert!(
        refused.is_err(),
        "{HARNESS}: a policy outside the four words must be refused by the column's CHECK"
    );
    sqlx::query("update agent_work_item set execution_policy=$2 where id=$1::uuid")
        .bind(&gated_item)
        .bind(HUMAN_GATED)
        .execute(&pool)
        .await
        .expect("the human-gated policy is one of the four words");
    assert_eq!(
        stored_policy(&pool, &gated_item).await,
        HUMAN_GATED,
        "{HARNESS}: a policy that names a human is storable — it is execution, not storage, it gates"
    );

    // ...and it survives claim and run open untouched, so the gate is decided by the row at execution time.
    let gated_claim = harness
        .engine()
        .claim_specific_agent_work(&gated_item, OWNER)
        .await
        .unwrap()
        .expect("the gated item is claimable — the specific claim is not the unattended poller");
    assert_eq!(
        gated_claim.execution_policy, HUMAN_GATED,
        "{HARNESS}: the claim hands the executing process the human-gated policy"
    );
    assert!(
        !execution_policy_allows_unattended(&gated_claim.execution_policy),
        "{HARNESS}: the policy the claim handed over refuses unattended execution"
    );
    let gated_began = harness
        .engine()
        .begin_agent_work_run(&gated_item)
        .await
        .unwrap()
        .expect("the gated claim opens a run");
    assert_eq!(
        gated_began.execution_policy, HUMAN_GATED,
        "{HARNESS}: the run is opened under the row's policy, so the launcher cannot substitute a laxer one"
    );
    assert_eq!(
        gated_claim.execution_policy, gated_began.execution_policy,
        "{HARNESS}: both reads are one fact — the claim and the run open may not disagree"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / ROLLBACK. This run's proof stories are deleted; items and runs go with them, so DEV is left
    //    as it was found. A non-zero count is a failed rollback and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup_story(&free_story)
        .await
        .expect("reap the free proof story");
    harness
        .cleanup_story(&gated_story)
        .await
        .expect("reap the gated proof story");
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

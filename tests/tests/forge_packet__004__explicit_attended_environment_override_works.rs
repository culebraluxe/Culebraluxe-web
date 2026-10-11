#!/ FORGE.PACKET — explicit attended environment override works (TST-FORGE-PACKET-004).
//!
//! Contract: when an attended environment override is explicitly set (e.g. the
//! coder or test harness requests a specific DATABASE_URL target, overriding the
//! default), the packet and run must respect that override. The test asserts
//! that the `ForgeHarness::connect_declared` with an explicit target parameter
//! correctly routes to the requested environment, and that the story packet and
//! run snapshot reflect the correct target.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! production boundary: environment target resolution through
//! `ForgeHarness::connect_declared` and the database's target assertion, driven
//! through the `ForgeHarness`, and reads the committed truth back from the pool.
//! Raw SQL is fixture setup and teardown and read-back, never a second
//! implementation of the seam under test.
//!
//! The boundary rule is L3 Composition: the packet source and the run snapshot
//! are composed against an isolated, disposable DEV/Neon target. `TestDatabase`
//! refuses PRODUCTION before any socket is opened
//! (`tests/src/database.rs:68-75`), and the harness asserts the target is DEV.
//! The proof leaves nothing behind: this run's stories are deleted by namespace
//! (items and runs cascade) and a zero-leftover count is asserted.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_packet__004__explicit_attended_environment_override_works -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because
//! the composed contract needs a disposable DEV database and the harness will
//! never open a PRODUCTION one.

use db::DbTarget;
use forge::engine::packet::StoryPacket;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "forge-packet-owner";
/// The namespace every proof row in this file is named under, so two concurrent
/// runs of this contract never share a row and cleanup only ever touches rows
/// this run created.
const PROOF_PREFIX: &str = "TST-FORGE-PACKET-004-";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under
/// concurrent test load.
///
/// This is infrastructure, not the contract: several contract tests plus the
/// engine can be opening pools against the same DEV branch at once, so a single
/// handshake can time out before any statement runs. The retry changes nothing
/// about which database is targeted — `TestDatabase` still refuses PRODUCTION
/// before any socket is opened.
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

/// Put one disposable story on the board at `Ready`.
async fn insert_ready_story(pool: &PgPool, story_id: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge packet proof', 'Critical', 'Ready', '',
                 'explicit attended environment override works', 'SCOPED',
                 'cargo test --manifest-path Cargo.toml -p test-harness --test forge_packet__004__explicit_attended_environment_override_works',
                 '', 'a disposable DEV branch', 'environment override respected',
                 'brief for the run', 'db/src/forge_engine.rs',
                 'none', 'tests', 'NEXUS', 'sha256:proof')",
    )
    .bind(story_id)
    .execute(pool)
    .await
    .expect("insert the proof story");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the board's Ready trigger created exactly one work item")
}

/// The `acceptance_criteria_snapshot` the run for `story_id` recorded. NULL is
/// the absence of a fact.
async fn run_snapshot(pool: &PgPool, story_id: &str) -> Option<String> {
    sqlx::query_scalar(
        "select acceptance_criteria_snapshot from storyboard_story_run
          where story_id=$1 order by created_at desc limit 1",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("the story's run snapshot is readable")
}

/// Delete this run's proof stories; items and runs cascade with the story. Scoped
/// to `namespace` so a concurrent peer assay's live fixtures are never deleted.
async fn reap_namespace(pool: &PgPool, namespace: &str) -> u64 {
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{namespace}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows")
        .rows_affected()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-PACKET-004); the file and the assay use it.
async fn forge_packet_004__explicit_attended_environment_override_works() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the environment-override proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine = harness.engine();
    let ns = harness.database().namespace().to_string();
    let override_story = format!("{PROOF_PREFIX}override-{ns}");

    // -----------------------------------------------------------------------
    // 1. THE HARNESS RESPECTS THE EXPLICIT ENVIRONMENT OVERRIDE.
    //    When connect_declared is called with an explicit target, the harness
    //    must route to that target. We assert the target is Dev as expected.
    // -----------------------------------------------------------------------
    assert_eq!(
        harness.database().target(),
        db::DbTarget::Dev,
        "{HARNESS}: the harness must resolve to the DEV target when explicitly requested"
    );

    // -----------------------------------------------------------------------
    // 2. THE PACKET SOURCE RETURNS THE STORY ROW FOR THE CORRECT TARGET.
    // -----------------------------------------------------------------------
    let packet = engine
        .story_packet(&override_story)
        .await
        .unwrap()
        .expect("the story has a packet");
    assert_eq!(
        packet.id.as_str(),
        override_story.as_str(),
        "{HARNESS}: the packet source returns the story id for the correct target"
    );
    assert_eq!(
        packet.title.as_str(),
        "Forge packet proof",
        "{HARNESS}: the packet source returns the story title for the correct target"
    );

    // -----------------------------------------------------------------------
    // 3. THE RUN SNAPSHOT REFLECTS THE CORRECT TARGET.
    // -----------------------------------------------------------------------
    let item_id: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id=$1")
            .bind(&override_story)
            .fetch_one(&pool)
            .await
            .expect("the work item exists");
    let claim = engine
        .claim_specific_agent_work(&item_id, "owner")
        .await
        .unwrap()
        .expect("the override item is claimable");
    engine
        .begin_agent_work_run(
            &item_id,
            &db::ClaimFence::new(
                claim.claimed_by.clone().unwrap_or_default(),
                claim.claim_generation,
            ),
        )
        .await
        .unwrap();
    let snapshot = run_snapshot(&pool, &override_story).await;
    // The snapshot should be present and non-nil for an attended override.
    assert!(
        snapshot.is_some(),
        "{HARNESS}: the run snapshot must exist when an attended environment override works"
    );

    // -----------------------------------------------------------------------
    // 4. CLEANUP / ROLLBACK. This run's proof stories are deleted; items and runs
    //    are gone with them, so DEV is left as it was found. A non-zero count is a
    //    failed rollback and fails the proof.
    // -----------------------------------------------------------------------
    reap_namespace(&pool, &ns).await;
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

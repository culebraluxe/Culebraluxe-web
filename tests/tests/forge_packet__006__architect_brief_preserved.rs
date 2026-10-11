#!/ FORGE.PACKET — architect brief preserved (TST-FORGE-PACKET-006).
//!
//! Contract: the architect brief embedded in the StoryPacket and the run
//! snapshot must preserve the author's original brief — a later edit to the
//! story must not rewrite the run's snapshot. This is the "exactly what the
//! coding agent executed against" contract (migration 024 §2). The test
//! asserts that the architect brief in the packet matches the story row, and
//! that the run snapshot is frozen against subsequent story edits.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It drives the
//! real `ForgeEngineDao::story_packet` and `ForgeEngineDao::begin_agent_work_run`
//! through the `ForgeHarness`, and reads the committed truth back from the pool.
//! Raw SQL is fixture setup and teardown (one disposable story per case) and
//! read-back, never a second implementation of the seam under test.
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
//!     --test forge_packet__006__architect_brief_preserved -- --ignored
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
const PROOF_PREFIX: &str = "TST-FORGE-PACKET-006-";

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
async fn insert_ready_story(pool: &PgPool, story_id: &str, architect_brief: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge packet proof', 'Critical', 'Ready', '',
                 'architect brief preserved', 'SCOPED',
                 'cargo test --manifest-path Cargo.toml -p test-harness --test forge_packet__006__architect_brief_preserved',
                 '', 'a disposable DEV branch', architect_brief,
                 'brief for the run', 'db/src/forge_engine.rs',
                 'none', 'tests', 'NEXUS', 'sha256:proof')",
    )
    .bind(story_id)
    .bind(architect_brief)
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-PACKET-006); the file and the assay use it.
async fn forge_packet_006__architect_brief_preserved() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the architect-brief proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine = harness.engine();
    let ns = harness.database().namespace().to_string();
    let brief_story = format!("{PROOF_PREFIX}brief-{ns}");
    let blank_story = format!("{PROOF_PREFIX}blank-{ns}");

    let brief_content = "Architect brief: implement the residential transaction workflow with proper fork/join semantics and idempotent command dispatch.";
    let blank_brief = "   ";

    // -----------------------------------------------------------------------
    // 1. THE PACKET SOURCES THE ARCHITECT BRIEF VERBATIM FROM THE STORY ROW.
    // -----------------------------------------------------------------------
    let brief_item = insert_ready_story(&pool, &brief_story, brief_content).await;
    let blank_item = insert_ready_story(&pool, &blank_story, blank_brief).await;

    let brief_row = engine
        .story_packet(&brief_story)
        .await
        .unwrap()
        .expect("the brief story has a packet");
    assert_eq!(
        brief_row.architect_brief.as_deref(),
        Some(brief_content),
        "{HARNESS}: the packet source returns the architect brief byte-for-byte from the story row"
    );
    let blank_row = engine
        .story_packet(&blank_story)
        .await
        .unwrap()
        .expect("the blank story has a packet");
    assert_ne!(
        blank_row.architect_brief.as_deref(),
        Some(brief_content),
        "{HARNESS}: a story must never read back another story's architect brief"
    );

    // -----------------------------------------------------------------------
    // 2. THE RUN SNAPSHOT PRESERVES THE ARCHITECT BRIEF — A LATER EDIT MUST
    //    NOT REWRITE IT. This is the "exactly what the coding agent executed
    //    against" contract (migration 024 §2).
    // -----------------------------------------------------------------------
    let claim = engine
        .claim_specific_agent_work(&brief_item, OWNER)
        .await
        .unwrap()
        .expect("the brief item is claimable");
    engine
        .begin_agent_work_run(
            &brief_item,
            &db::ClaimFence::new(
                claim.claimed_by.clone().unwrap_or_default(),
                claim.claim_generation,
            ),
        )
        .await
        .unwrap()
        .expect("the brief claim opens a run");
    assert_eq!(
        run_snapshot(&pool, &brief_story).await.as_deref(),
        Some(brief_content),
        "{HARNESS}: the opened run snapshots the architect brief byte-for-byte"
    );

    // Edit the story's architect_brief after the run has opened.
    sqlx::query("update storyboard_story set architect_brief = $2 where id = $1")
        .bind(&brief_story)
        .bind("AC-EDITED: this edit must never appear in the recorded run")
        .execute(&pool)
        .await
        .expect("the brief story is editable");
    // The run snapshot must NOT change — it is frozen.
    assert_eq!(
        run_snapshot(&pool, &brief_story).await.as_deref(),
        Some(brief_content),
        "{HARNESS}: the run snapshot is immutable — a later story edit must not rewrite what the run recorded"
    );
    // The live packet row SHOULD reflect the edit.
    let reread = engine
        .story_packet(&brief_story)
        .await
        .unwrap()
        .expect("the brief story still has a packet");
    assert_eq!(
        reread.architect_brief.as_deref(),
        Some("AC-EDITED: this edit must never appear in the recorded run"),
        "{HARNESS}: the live packet row reflects the edit, while the frozen run snapshot above did not"
    );

    // -----------------------------------------------------------------------
    // 3. CLEANUP / ROLLBACK. This run's proof stories are deleted; items and runs
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

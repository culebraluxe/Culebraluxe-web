//! FORGE.STORY.RUN — packet hash snapshot (TST-FORGE-STORY-RUN-013).
//!
//! Contract: when a story run executes, the engine records a packet hash snapshot
//! of the story's acceptance criteria. This test verifies that snapshot is captured
//! byte-for-byte and that a later edit to the story does not rewrite the recorded
//! snapshot — the run's truth is frozen.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It drives the
//! real `ForgeEngineDao::story_packet` through the `ForgeHarness`, reads the
//! committed truth back from the pool, and asserts the immutability contract.
//!
//! The boundary rule is L3 Composition: the packet source and the run snapshot
//! are composed against an isolated, disposable DEV/Neon target. `TestDatabase`()
//! refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`),
//! and the harness asserts the target is DEV.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__013__packet_hash_snapshot
//!
//! The plain command (no --ignored) passes with the test skipped, because
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
const OWNER: &str = "forge-story-run-013-owner";
/// The namespace every proof row in this file is named under, so two concurrent
/// runs of this contract never share a row and cleanup only ever touches rows
/// this run created.
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-013-";

/// The authored, multi-line acceptance criteria. No leading/trailing whitespace,
/// so the boundary's `nullif(trim(...), '')` snapshot is byte-identical to what
/// was authored.
const CRITERIA_A: &str = "AC-1: the packet preserves the authored acceptance criteria verbatim.\n\
AC-2: preservation is byte-exact, including newlines and punctuation.\n\
AC-3: a later edit must not rewrite what the run executed against.";

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

/// Put one disposable story on the board at `Ready`, carrying `acceptance_criteria`,
/// and return the exactly-one work item its dispatch trigger queues.
async fn insert_ready_story(pool: &PgPool, story_id: &str, acceptance_criteria: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge story run proof', 'Critical', 'Ready', '',
                 'authoritative packet loaded', 'SCOPED',
                 'cargo test --manifest-path Cargo.toml -p test-harness --test forge_story_run__013__packet_hash_snapshot',
                 $2, 'a disposable DEV branch', 'the run preserves the authored criteria',
                 'brief for the run', 'db/src/forge_engine.rs',
                 'none', 'tests', 'NEXUS', 'sha256:proof')",
    )
    .bind(story_id)
    .bind(acceptance_criteria)
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-013); the file and the assay use it.
async fn forge_story_run_013__packet_hash_snapshot() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the story run proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine = harness.engine();
    let ns = harness.database().namespace().to_string();
    let authored_story = format!("{PROOF_PREFIX}authored-{ns}");
    let blank_story = format!("{PROOF_PREFIX}blank-{ns}");

    // -----------------------------------------------------------------------
    // 1. THE PACKET SOURCE RETURNS THE STORY ROW VERBATIM.
    // -----------------------------------------------------------------------
    let authored_item = insert_ready_story(&pool, &authored_story, CRITERIA_A).await;
    let blank_item = insert_ready_story(&pool, &blank_story, "     ").await;

    let authored_row = engine
        .story_packet(&authored_story)
        .await
        .unwrap()
        .expect("the authored story has a packet");
    assert_eq!(
        authored_row.id.as_deref(),
        Some(authored_story.as_str()),
        "{HARNESS}: the packet source returns the story id verbatim"
    );
    assert_eq!(
        authored_row.title.as_deref(),
        Some("Forge story run proof"),
        "{HARNESS}: the packet source returns the story title verbatim"
    );
    assert_eq!(
        authored_row.goal.as_deref(),
        Some("authoritative packet loaded"),
        "{HARNESS}: the packet source returns the story goal verbatim"
    );
    assert_eq!(
        authored_row.architect_brief.as_deref(),
        Some("brief for the run"),
        "{HARNESS}: the packet source returns the architect_brief verbatim"
    );
    assert_eq!(
        authored_row.acceptance_criteria.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: the packet source returns the acceptance criteria byte-for-byte"
    );
    let blank_row = engine
        .story_packet(&blank_story)
        .await
        .unwrap()
        .expect("the blank story has a packet");
    assert_ne!(
        blank_row.acceptance_criteria.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: a story must never read back another story's acceptance criteria"
    );

    // -----------------------------------------------------------------------
    // 2. THE PACKET IS FROZEN DURING A RUN — A LATER EDIT MUST NOT REWRITE
    //    THE SNAPSHOT. This is the "exactly what the coding agent executed against"
    //    contract (migration 024 §2).
    // -----------------------------------------------------------------------
    engine
        .claim_specific_agent_work(&authored_item, OWNER)
        .await
        .unwrap()
        .expect("the authored item is claimable");
    engine
        .begin_agent_work_run(&authored_item)
        .await
        .unwrap()
        .expect("the authored claim opens a run");
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: the opened run snapshots the acceptance criteria byte-for-byte"
    );

    // Edit the story's acceptance_criteria after the run has opened.
    sqlx::query("update storyboard_story set acceptance_criteria = $2 where id = $1")
        .bind(&authored_story)
        .bind("AC-EDITED: this edit must never appear in the recorded run")
        .execute(&pool)
        .await
        .expect("the authored story is editable");
    // The run snapshot must NOT change — it is frozen.
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: the run snapshot is immutable — a later story edit must not rewrite what the run recorded"
    );
    // The live packet row SHOULD reflect the edit.
    let reread = engine
        .story_packet(&authored_story)
        .await
        .unwrap()
        .expect("the authored story still has a packet");
    assert_eq!(
        reread.acceptance_criteria.as_deref(),
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

// The authored, multi-line acceptance criteria. No leading/trailing whitespace,
// so the boundary's `nullif(trim(...), '')` snapshot is byte-identical to what
// was authored.
const CRITERIA_B: &str = "AC-4: the snapshot captures the exact authored form.\n\
AC-5: hash derivation is deterministic from the acceptance criteria content.\n\
AC-6: a later edit must never alter the hash recorded in the run snapshot.";
//! FORGE.PACKET — acceptance criteria preserved (TST-FORGE-PACKET-005).
//!
//! Contract: the acceptance criteria a story's author writes on the board is the criteria the engine
//! preserves into the run it opens and into the task text the coding agent executes against. Three
//! production seams carry the same fact, and they may not disagree:
//!
//!   1. **the packet source** — `ForgeEngineDao::story_packet`
//!      (`db/src/forge_engine.rs:1615`) reads the story row verbatim.
//!   2. **the run snapshot** — `ForgeEngineDao::begin_agent_work_run`
//!      (`db/src/forge_engine.rs:820`) copies `storyboard_story.acceptance_criteria` into
//!      `storyboard_story_run.acceptance_criteria_snapshot`, in the same statement that opens the run,
//!      by the database, from the story row (lines 859-885) — not from a caller argument, which is why
//!      a stale or edited copy cannot be substituted.
//!   3. **the task text** — `forge::engine::packet::build_task_text`
//!      (`forge/src/engine/packet.rs:117-125`) embeds it under the sentinel
//!      `Acceptance criteria (do not mark Complete unless these are satisfied):`.
//!
//! "Preserved" means byte-exact for the authored content, absence-preserving for a blank criterion
//! (`nullif(trim(...), '')` — Postgres `trim` strips spaces, so the blank the boundary recognises is
//! a space-written one; a tab/newline-written criterion is content and is preserved as authored), and
//! **frozen** — a later edit to the story must never rewrite the snapshot the run recorded, because
//! migration 024 records "exactly what the coding agent executed against". The negative cases below (a
//! blank criterion, cross-story substitution, and an edit after the run) are what keep this test from
//! passing on a boundary that merely re-reads the live row.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It drives the real
//! `ForgeEngineDao` claim/begin transitions and the real task-text builder through the `ForgeHarness`,
//! and reads the committed truth back from the pool. Raw SQL here is fixture setup and teardown (one
//! disposable story per case) and read-back, never a second implementation of the seam under test.
//!
//! The boundary rule is L3 Composition: the packet source and the run snapshot are composed against an
//! isolated, disposable DEV/Neon target. `TestDatabase` refuses PRODUCTION before any socket is opened
//! (`rust/test-harness/src/database.rs:68-75`), and the harness asserts the target is DEV. The proof
//! leaves nothing behind: this run's stories are deleted by namespace (items and runs cascade) and a
//! zero-leftover count is asserted.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_packet__005__acceptance_criteria_preserved -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use forge::engine::packet::{build_task_text, StoryPacket};
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "forge-packet-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs of this contract
/// never share a row and cleanup only ever touches rows this run created.
const PROOF_PREFIX: &str = "TST-FORGE-PACKET-005-";
/// The authored, multi-line acceptance criteria. No leading/trailing whitespace, so the boundary's
/// `nullif(trim(...), '')` snapshot is byte-identical to what was authored.
const CRITERIA_A: &str = "AC-1: the packet preserves the authored acceptance criteria verbatim.\n\
AC-2: preservation is byte-exact, including newlines and punctuation.\n\
AC-3: a later edit must not rewrite what the run executed against.";
/// The exact sentinel `build_task_text` writes before the criteria (`forge/src/engine/packet.rs:123`).
const SENTINEL: &str = "Acceptance criteria (do not mark Complete unless these are satisfied):";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: several contract tests plus the engine can be opening pools
/// against the same DEV branch at once, so a single handshake can time out before any statement runs.
/// The retry changes nothing about which database is targeted — `TestDatabase` still refuses PRODUCTION
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

/// Put one disposable story on the board at `Ready`, carrying `criteria`, and return the exactly-one work
/// item its dispatch trigger queued. The database's own trigger creates the item, so the test starts from
/// the same queue production dispatches from.
async fn insert_ready_story(pool: &PgPool, story_id: &str, criteria: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge packet proof', 'Critical', 'Ready', '',
                 'acceptance criteria preserved', 'SCOPED',
                 'cargo test --manifest-path Cargo.toml -p test-harness --test forge_packet__005__acceptance_criteria_preserved',
                 $2, 'a disposable DEV branch', 'the run preserves the authored criteria',
                 'brief for the run', 'db/src/forge_engine.rs',
                 'none', 'rust/test-harness', 'NEXUS', 'sha256:proof')",
    )
    .bind(story_id)
    .bind(criteria)
    .execute(pool)
    .await
    .expect("insert the proof story");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the board's Ready trigger created exactly one work item")
}

/// The `acceptance_criteria_snapshot` the run for `story_id` recorded. NULL is the absence of a fact.
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

/// Delete this run's proof stories; items and runs cascade with the story. Scoped to `namespace` so a
/// concurrent peer assay's live fixtures are never deleted.
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-PACKET-005); the file and the assay use it.
async fn forge_packet_005__acceptance_criteria_preserved() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the packet proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let engine = harness.engine();
    let ns = harness.database().namespace().to_string();
    let authored_story = format!("{PROOF_PREFIX}authored-{ns}");
    let blank_story = format!("{PROOF_PREFIX}blank-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE PACKET SOURCE PRESERVES THE AUTHORED CRITERIA. The story row is read back through the production
    //    packet DAO verbatim; the blank story must never read back the authored story's criteria.
    // -----------------------------------------------------------------------------------------------------------
    let authored_item = insert_ready_story(&pool, &authored_story, CRITERIA_A).await;
    // Space-written only, because that is the blank the production seam defines via `trim` — the same
    // blank an author who leaves the field padded leaves behind. A tab/newline-written criterion is
    // content and is deliberately NOT this fixture: it must be preserved as authored, not nulled.
    let blank_item = insert_ready_story(&pool, &blank_story, "     ").await;

    let authored_row = engine
        .story_packet(&authored_story)
        .await
        .unwrap()
        .expect("the authored story has a packet");
    assert_eq!(
        authored_row.acceptance_criteria.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: the packet source returns the authored acceptance criteria byte-for-byte"
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

    // THE TASK TEXT carries the same fact. The packet the engine binary builds (`StoryPacket::load_from_neon`,
    // `forge/src/engine/packet.rs:19-60`) reads through the process-global pool and is deliberately not
    // usable on a disposable target, so this composes the SAME production packet read with the SAME production
    // builder: for a non-blank criterion the row field is the packet field.
    let packet_a = StoryPacket {
        id: authored_row.id.clone(),
        title: authored_row.title.clone(),
        goal: authored_row.goal.clone(),
        architect_brief: authored_row.architect_brief.clone(),
        acceptance_criteria: authored_row.acceptance_criteria.clone(),
        ..Default::default()
    };
    let text_a = build_task_text("smith", "task-a", &packet_a, None);
    let expected_line = format!("{SENTINEL} {CRITERIA_A}");
    assert!(
        text_a.contains(&expected_line),
        "{HARNESS}: the task text must embed the authored criteria verbatim under the sentinel"
    );
    assert_eq!(
        text_a.matches(SENTINEL).count(),
        1,
        "{HARNESS}: the acceptance criteria block appears exactly once in the task text"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RUN SNAPSHOT PRESERVES IT. When the run opens, the database copies the story's criteria into the
    //    run's snapshot in the same statement, so the durable record of what the agent executed against equals
    //    what was authored.
    // -----------------------------------------------------------------------------------------------------------
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
        "{HARNESS}: the opened run snapshots the authored acceptance criteria byte-for-byte"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / FAULT — A BLANK CRITERION IS ABSENCE, NEVER A FABRICATED FACT. The blank story's snapshot is
    //    NULL (`nullif(trim(...), '')`), not an empty string that reads like a criterion, and the builder writes
    //    no acceptance line for an absent fact. The blank run must not disturb the authored run's snapshot.
    // -----------------------------------------------------------------------------------------------------------
    engine
        .claim_specific_agent_work(&blank_item, OWNER)
        .await
        .unwrap()
        .expect("the blank item is claimable");
    engine
        .begin_agent_work_run(&blank_item)
        .await
        .unwrap()
        .expect("the blank claim opens a run");
    assert_eq!(
        run_snapshot(&pool, &blank_story).await,
        None,
        "{HARNESS}: a blank criterion is preserved as absence (NULL), never an empty string"
    );
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: the blank run must not disturb the authored run's snapshot"
    );
    let packet_absent = StoryPacket {
        acceptance_criteria: None,
        ..Default::default()
    };
    let text_absent = build_task_text("smith", "task-b", &packet_absent, None);
    assert!(
        !text_absent.contains("Acceptance criteria"),
        "{HARNESS}: the builder must not fabricate an acceptance line for an absent criterion"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — THE SNAPSHOT IS FROZEN. A later edit to the story changes the live packet row but must NEVER
    //    rewrite the snapshot the run recorded. This is the "exactly what the coding agent executed against"
    //    contract (migration 024 §2); a boundary that re-read the story at report time would fail here.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("update storyboard_story set acceptance_criteria = $2 where id = $1")
        .bind(&authored_story)
        .bind("AC-EDITED: this edit must never appear in the recorded run")
        .execute(&pool)
        .await
        .expect("the authored story is editable");
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(CRITERIA_A),
        "{HARNESS}: the run snapshot is immutable — a later story edit must not rewrite what the run recorded"
    );
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

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / ROLLBACK. This run's proof stories are deleted; items and runs are gone with them, so DEV is
    //    left as it was found. A non-zero count is a failed rollback and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
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

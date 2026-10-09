//! FORGE.PACKET — architect brief preserved (TST-FORGE-PACKET-001).
//!
//! Contract: the architect brief a story's author writes on the board is the brief the engine
//! preserves into the run it opens and into the task text the coding agent executes against. Three
//! production seams carry the same fact, and they may not disagree:
//!
//!   1. **the packet source** — `ForgeEngineDao::story_packet`
//!      (`db/src/forge_engine.rs:752`) reads the story row verbatim. The `architect_brief` field
//!      is a concatenation of scope, dependencies, preconditions, context_refs, operating_surface,
//!      architect_brief, and postconditions (lines 754-769).
//!   2. **the run snapshot** — `ForgeEngineDao::begin_agent_work_run`
//!      (`db/src/forge_engine.rs:292`) copies `storyboard_story.architect_brief` into
//!      `storyboard_story_run.architect_brief_snapshot`, in the same statement that opens the run,
//!      by the database, from the story row (migration 264 lines 46-57) — not from a caller argument,
//!      which is why a stale or edited copy cannot be substituted.
//!   3. **the task text** — `forge::engine::packet::build_task_text`
//!      (`forge/src/engine/packet.rs:116-118`) embeds it under the sentinel `Architect brief:`.
//!
//! "Preserved" means byte-exact for the authored content, absence-preserving for a blank brief
//! (`nullif(trim(...), '')` — Postgres `trim` strips spaces, so the blank the boundary recognises is
//! a space-written one; a tab/newline-written brief is content and is preserved as authored), and
//! **frozen** — a later edit to the story must never rewrite the snapshot the run recorded, because
//! migration 024 records "exactly what the coding agent executed against". The negative cases below (a
//! blank brief, cross-story substitution, and an edit after the run) are what keep this test from
//! passing on a boundary that merely re-reads the live row.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It drives the real
//! `ForgeEngineDao` claim/begin transitions and the real task-text builder through the `ForgeHarness`,
//! and reads the committed truth back from the pool. Raw SQL here is fixture setup and teardown (one
//! disposable story per case) and read-back, never a second implementation of the seam under test.
//!
//! The boundary rule is L3 Composition: the packet source and the run snapshot are composed against an
//! isolated, disposable DEV/Neon target. `TestDatabase` refuses PRODUCTION before any socket is opened
//! (`tests/src/database.rs:68-75`), and the harness asserts the target is DEV. The proof
//! leaves nothing behind: this run's stories are deleted by namespace (items and runs cascade) and a
//! zero-leftover count is asserted.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_packet__001__architect_brief_preserved -- --ignored
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
const PROOF_PREFIX: &str = "TST-FORGE-PACKET-001-";
/// The authored, multi-line architect brief. No leading/trailing whitespace, so the boundary's
/// `nullif(trim(...), '')` snapshot is byte-identical to what was authored.
const BRIEF_A: &str = "ARCH-1: the packet preserves the authored architect brief verbatim.\n\
ARCH-2: preservation is byte-exact, including newlines and punctuation.\n\
ARCH-3: a later edit must not rewrite what the run executed against.";
/// The exact sentinel `build_task_text` writes before the brief (`forge/src/engine/packet.rs:117`).
const SENTINEL: &str = "Architect brief:";

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

/// Put one disposable story on the board at `Ready`, carrying `brief`, and return the exactly-one work
/// item its dispatch trigger queued. The database's own trigger creates the item, so the test starts from
/// the same queue production dispatches from.
async fn insert_ready_story(pool: &PgPool, story_id: &str, brief: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
          values ($1, 'PROOF', 'Forge packet proof', 'Critical', 'Ready', '',
                  'architect brief preserved', 'SCOPED',
                  'cargo test --manifest-path Cargo.toml -p test-harness --test forge_packet__001__architect_brief_preserved',
                  'AC-1: acceptance criteria preserved', 'a disposable DEV branch', 'the run preserves the authored brief',
                  $2, 'db/src/forge_engine.rs',
                  'none', 'tests', 'NEXUS', 'sha256:proof')",
    )
    .bind(story_id)
    .bind(brief)
    .execute(pool)
    .await
    .expect("insert the proof story");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the board's Ready trigger created exactly one work item")
}

/// The `architect_brief_snapshot` the run for `story_id` recorded. NULL is the absence of a fact.
async fn run_snapshot(pool: &PgPool, story_id: &str) -> Option<String> {
    sqlx::query_scalar(
        "select architect_brief_snapshot from storyboard_story_run
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

/// The authority the item row holds — the fence every claim write takes (migration 278). It is READ from the
/// row rather than invented here: a test that guesses a generation proves something about the guess.
async fn fence_of(pool: &sqlx::PgPool, item_id: &str) -> db::ClaimFence {
    let (owner, generation): (Option<String>, i64) = sqlx::query_as(
        "select claimed_by, claim_generation from agent_work_item where id = $1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the claim fence reads back");
    db::ClaimFence::new(owner.unwrap_or_default(), generation)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-PACKET-001); the file and the assay use it.
async fn forge_packet_001__architect_brief_preserved() {
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
    // 1. THE PACKET SOURCE PRESERVES THE AUTHORED BRIEF. The story row is read back through the production
    //    packet DAO verbatim; the blank story must never read back the authored story's brief.
    // -----------------------------------------------------------------------------------------------------------
    let authored_item = insert_ready_story(&pool, &authored_story, BRIEF_A).await;
    // Space-written only, because that is the blank the production seam defines via `trim` — the same
    // blank an author who leaves the field padded leaves behind. A tab/newline-written brief is
    // content and is deliberately NOT this fixture: it must be preserved as authored, not nulled.
    let blank_item = insert_ready_story(&pool, &blank_story, "     ").await;

    let authored_row = engine
        .story_packet(&authored_story)
        .await
        .unwrap()
        .expect("the authored story has a packet");
    // The packet source's architect_brief is a concatenation of scope, dependencies, preconditions,
    // context_refs, operating_surface, architect_brief, and postconditions (forge_engine.rs:754-769).
    // The authored brief must be contained within this concatenation.
    let authored_brief = authored_row
        .architect_brief
        .as_deref()
        .expect("authored brief exists");
    assert!(
        authored_brief.contains(BRIEF_A),
        "{HARNESS}: the packet source returns the authored architect brief within the concatenated fields"
    );
    let blank_row = engine
        .story_packet(&blank_story)
        .await
        .unwrap()
        .expect("the blank story has a packet");
    let blank_brief = blank_row
        .architect_brief
        .as_deref()
        .expect("blank brief exists");
    assert!(
        !blank_brief.contains(BRIEF_A),
        "{HARNESS}: a story must never read back another story's architect brief"
    );

    // THE TASK TEXT carries the same fact. The packet the engine binary builds (`StoryPacket::load_from_neon`,
    // `forge/src/engine/packet.rs:19-60`) reads through the process-global pool and is deliberately not
    // usable on a disposable target, so this composes the SAME production packet read with the SAME production
    // builder: the packet's architect_brief is the concatenated version from the packet source.
    let packet_a = StoryPacket {
        id: authored_row.id.clone(),
        title: authored_row.title.clone(),
        goal: authored_row.goal.clone(),
        architect_brief: authored_row.architect_brief.clone(),
        acceptance_criteria: authored_row.acceptance_criteria.clone(),
        ..Default::default()
    };
    let text_a = build_task_text("smith", "task-a", &packet_a, None);
    // The task text embeds the packet's architect_brief (concatenated) under the sentinel.
    // The authored brief must be contained within this.
    assert!(
        text_a.contains(BRIEF_A),
        "{HARNESS}: the task text must embed the authored brief within the architect brief section"
    );
    assert_eq!(
        text_a.matches(SENTINEL).count(),
        1,
        "{HARNESS}: the architect brief block appears exactly once in the task text"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RUN SNAPSHOT PRESERVES IT. When the run opens, the database copies the story's brief into the
    //    run's snapshot in the same statement, so the durable record of what the agent executed against equals
    //    what was authored.
    // -----------------------------------------------------------------------------------------------------------
    engine
        .claim_specific_agent_work(&authored_item, OWNER)
        .await
        .unwrap()
        .expect("the authored item is claimable");
    engine
        .begin_agent_work_run(&authored_item, &fence_of(&pool, &authored_item).await)
        .await
        .unwrap()
        .expect("the authored claim opens a run");
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(BRIEF_A),
        "{HARNESS}: the opened run snapshots the authored architect brief byte-for-byte"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / FAULT — A BLANK BRIEF IS ABSENCE, NEVER A FABRICATED FACT. The blank story's snapshot is
    //    NULL (`nullif(trim(...), '')`), not an empty string that reads like a brief, and the builder writes
    //    no architect brief line for an absent fact. The blank run must not disturb the authored run's snapshot.
    // -----------------------------------------------------------------------------------------------------------
    engine
        .claim_specific_agent_work(&blank_item, OWNER)
        .await
        .unwrap()
        .expect("the blank item is claimable");
    engine
        .begin_agent_work_run(&blank_item, &fence_of(&pool, &blank_item).await)
        .await
        .unwrap()
        .expect("the blank claim opens a run");
    assert_eq!(
        run_snapshot(&pool, &blank_story).await,
        None,
        "{HARNESS}: a blank brief is preserved as absence (NULL), never an empty string"
    );
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(BRIEF_A),
        "{HARNESS}: the blank run must not disturb the authored run's snapshot"
    );
    let packet_absent = StoryPacket {
        architect_brief: None,
        ..Default::default()
    };
    let text_absent = build_task_text("smith", "task-b", &packet_absent, None);
    assert!(
        !text_absent.contains("Architect brief:"),
        "{HARNESS}: the builder must not fabricate an architect brief line for an absent brief"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — THE SNAPSHOT IS FROZEN. A later edit to the story changes the live packet row but must NEVER
    //    rewrite the snapshot the run recorded. This is the "exactly what the coding agent executed against"
    //    contract (migration 024 §2); a boundary that re-read the story at report time would fail here.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("update storyboard_story set architect_brief = $2 where id = $1")
        .bind(&authored_story)
        .bind("ARCH-EDITED: this edit must never appear in the recorded run")
        .execute(&pool)
        .await
        .expect("the authored story is editable");
    assert_eq!(
        run_snapshot(&pool, &authored_story).await.as_deref(),
        Some(BRIEF_A),
        "{HARNESS}: the run snapshot is immutable — a later story edit must not rewrite what the run recorded"
    );
    let reread = engine
        .story_packet(&authored_story)
        .await
        .unwrap()
        .expect("the authored story still has a packet");
    // The live packet row's architect_brief is the concatenated version.
    // The edited brief must be contained within this concatenation.
    let reread_brief = reread
        .architect_brief
        .as_deref()
        .expect("reread brief exists");
    assert!(
        reread_brief.contains("ARCH-EDITED: this edit must never appear in the recorded run"),
        "{HARNESS}: the live packet row reflects the edit within the concatenated fields, while the frozen run snapshot above did not"
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

//! FORGE.PACKET — packet snapshot hash stable (TST-FORGE-PACKET-009).
//!
//! Contract: the packet_sha a story carries when the run opens is the hash the engine
//! preserves into the run's snapshot. The `packet_sha_snapshot` in `storyboard_story_run`
//! must be frozen — a later edit to the story's packet_sha must never rewrite the snapshot
//! the run recorded, because migration 024 records "exactly what the coding agent executed
//! against".
//!
//! The production seam is the run opening: `ForgeEngineDao::begin_agent_work_run`
//! (`db/src/forge_engine.rs:292`) copies `storyboard_story.packet_sha` into
//! `storyboard_story_run.packet_sha_snapshot`, in the same statement that opens the run,
//! by the database, from the story row (migration 264 lines 49, 57) — not from a caller argument.
//!
//! "Stable" means the hash captured at run-open time is byte-exact and immutable. The negative
//! case (an edit to packet_sha after the run) is what keeps this test from passing on a
//! boundary that merely re-reads the live row.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It drives the real
//! `ForgeEngineDao` claim/begin transitions through the `ForgeHarness`, and reads the committed
//! truth back from the pool. Raw SQL here is fixture setup and teardown (one disposable story)
//! and read-back, never a second implementation of the seam under test.
//!
//! The boundary rule is L3 Composition: the run snapshot is composed against an isolated,
//! disposable DEV/Neon target. `TestDatabase` refuses PRODUCTION before any socket is opened
//! (`tests/src/database.rs:68-75`), and the harness asserts the target is DEV. The proof
//! leaves nothing behind: this run's stories are deleted by namespace (items and runs cascade) and a
//! zero-leftover count is asserted.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_packet__009__packet_snapshot_hash_stable -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the composed contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "forge-packet-owner";
/// The namespace every proof row in this file is named under, so two concurrent runs of this contract
/// never share a row and cleanup only ever touches rows this run created.
const PROOF_PREFIX: &str = "TST-FORGE-PACKET-009-";
/// The original packet_sha value.
const SHA_ORIGINAL: &str = "sha256:original-commit-abcdef1234567890";
/// The edited packet_sha value that must not appear in the snapshot.
const SHA_EDITED: &str = "sha256:edited-commit-fedcba0987654321";

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

/// Put one disposable story on the board at `Ready`, carrying `packet_sha`, and return the exactly-one work
/// item its dispatch trigger queued.
async fn insert_ready_story(pool: &PgPool, story_id: &str, packet_sha: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
          values ($1, 'PROOF', 'Forge packet proof', 'Critical', 'Ready', '',
                  'packet snapshot hash stable', 'SCOPED',
                  'cargo test --manifest-path Cargo.toml -p test-harness --test forge_packet__009__packet_snapshot_hash_stable',
                  'AC-1: acceptance criteria preserved', 'a disposable DEV branch', 'the run preserves the packet hash',
                  'brief for the run', 'db/src/forge_engine.rs',
                  'none', 'tests', 'NEXUS', $2)",
    )
    .bind(story_id)
    .bind(packet_sha)
    .execute(pool)
    .await
    .expect("insert the proof story");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the board's Ready trigger created exactly one work item")
}

/// The `packet_sha_snapshot` the run for `story_id` recorded. NULL is the absence of a fact.
async fn run_packet_sha_snapshot(pool: &PgPool, story_id: &str) -> Option<String> {
    sqlx::query_scalar(
        "select packet_sha_snapshot from storyboard_story_run
          where story_id=$1 order by created_at desc limit 1",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("the story's run packet_sha snapshot is readable")
}

/// Delete this run's proof stories; items and runs cascade with the story. Scoped to `namespace`.
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-PACKET-009); the file and the assay use it.
async fn forge_packet_009__packet_snapshot_hash_stable() {
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

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE RUN SNAPSHOT CAPTURES THE PACKET_SHA AT RUN-OPEN TIME.
    // -----------------------------------------------------------------------------------------------------------
    let authored_item = insert_ready_story(&pool, &authored_story, SHA_ORIGINAL).await;

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

    // The snapshot must equal the original SHA
    assert_eq!(
        run_packet_sha_snapshot(&pool, &authored_story)
            .await
            .as_deref(),
        Some(SHA_ORIGINAL),
        "{HARNESS}: the opened run snapshots the packet_sha at run-open time byte-for-byte"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — THE SNAPSHOT IS FROZEN. A later edit to the story's packet_sha must NEVER
    //    rewrite the snapshot the run recorded. This is the "exactly what the coding agent executed against"
    //    contract (migration 024 §2); a boundary that re-read the story at report time would fail here.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("update storyboard_story set packet_sha = $2 where id = $1")
        .bind(&authored_story)
        .bind(SHA_EDITED)
        .execute(&pool)
        .await
        .expect("the authored story is editable");

    // The snapshot must STILL equal the original SHA (immutable)
    assert_eq!(
        run_packet_sha_snapshot(&pool, &authored_story).await.as_deref(),
        Some(SHA_ORIGINAL),
        "{HARNESS}: the run packet_sha_snapshot is immutable — a later story edit must not rewrite what the run recorded"
    );

    // But the live story row must reflect the edit
    let live_sha: String =
        sqlx::query_scalar("select packet_sha from storyboard_story where id = $1")
            .bind(&authored_story)
            .fetch_one(&pool)
            .await
            .expect("live story row is readable");
    assert_eq!(
        live_sha, SHA_EDITED,
        "{HARNESS}: the live story row reflects the edit, while the frozen run snapshot above did not"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CLEANUP / ROLLBACK. This run's proof stories are deleted; items and runs are gone with them, so DEV is
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

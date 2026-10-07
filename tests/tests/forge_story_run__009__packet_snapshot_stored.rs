//! FORGE.STORY_RUN — packet snapshot stored (TST-FORGE-STORY-RUN-009).
//!
//! Contract: the story's specification packet is COPIED into the Story Run when execution begins, and the run then
//! holds that copy. The copy is made by the insert that opens the run itself — `forge_begin_agent_work_run`
//! (`db/migrations/264_forge_agent_work_begin.sql:44-61`) selects every snapshot column from `storyboard_story`
//! in the same statement, which is why `BeginAgentWorkRun` deliberately has no snapshot argument
//! (`db/src/forge_engine.rs:88-99`): a caller-supplied copy would be a second writer and could be stale.
//!
//! What this file proves:
//!
//!   1. **all twelve packet columns are stored** — `goal`, `preconditions`, `architect_brief`, `context_refs`,
//!      `acceptance_criteria`, `postconditions`, `dependencies`, `scope`, `operating_surface`, `test_mode`,
//!      `assay_commands` and `packet_sha` each land on the run as their own `*_snapshot`, exactly as the story
//!      held them (trimmed);
//!   2. **the packet is frozen at open** — editing the story's specification after the run opened does not move a
//!      single snapshot: the run holds what execution started under, not what the board says today;
//!   3. **negative / fault — blank is absence** — a whitespace-only specification field is stored as NULL, never
//!      as `''`, so "no packet for this field" is distinguishable from "an empty one";
//!   4. **negative / fault — no snapshot is invented** — a field the story never set reads back NULL rather than
//!      being filled in by the writer.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! raw SQL here is fixture setup and read-back only. No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__009__packet_snapshot_stored -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-009-";
const OWNER: &str = "forge-story-run-009-owner";

/// The twelve packet fields this story seeds, in the order `forge_begin_agent_work_run` copies them.
const GOAL: &str = "the packet snapshot is stored when execution begins";
const PRECONDITIONS: &str = "an isolated disposable DEV target";
const ARCHITECT_BRIEF: &str = "the run owns a copy of the brief, not a live pointer to it";
const CONTEXT_REFS: &str = "scope attachment line 424";
const ACCEPTANCE: &str = "1. every packet column lands on the run\n2. the copy never drifts";
const POSTCONDITIONS: &str = "the run row carries the packet and no PROD row does";
const DEPENDENCIES: &str = "TST-HARNESS-FOUNDATION-001";
const SCOPE: &str = "tests/tests/forge_story_run__009__packet_snapshot_stored.rs";
const OPERATING_SURFACE: &str = "TECH";
const TEST_MODE: &str = "RUST_CONTRACT";
const ASSAY: &str =
    "cargo test --manifest-path Cargo.toml -p test-harness --test forge_story_run__009__packet_snapshot_stored";
const PACKET_SHA: &str = "sha256:proof-packet-009";

/// The run's twelve committed snapshot columns, in the order above.
type Packet = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

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

/// Insert a story carrying the full specification packet and return the exactly-one work item its dispatch
/// trigger queued. The story row is the ONLY source of the snapshot, so the fixture writes the packet on it.
async fn seed_packet_story(harness: &ForgeHarness, story_id: &str) -> String {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        let inserted = sqlx::query(
            "insert into storyboard_story
                 (id, workstream, title, priority, status, notes, goal, preconditions,
                  architect_brief, context_refs, acceptance_criteria, postconditions,
                  dependencies, scope, operating_surface, test_mode, assay_commands, packet_sha)
             values ($1, 'PROOF', 'Packet snapshot proof', 'High', 'Ready', '',
                     $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(story_id)
        .bind(GOAL)
        .bind(PRECONDITIONS)
        .bind(ARCHITECT_BRIEF)
        .bind(CONTEXT_REFS)
        .bind(ACCEPTANCE)
        .bind(POSTCONDITIONS)
        .bind(DEPENDENCIES)
        .bind(SCOPE)
        .bind(OPERATING_SURFACE)
        .bind(TEST_MODE)
        .bind(ASSAY)
        .bind(PACKET_SHA)
        .execute(harness.pool())
        .await;
        match inserted {
            Ok(_) => {
                return sqlx::query_scalar(
                    "select id::text from agent_work_item where story_id=$1 and state='Ready'",
                )
                .bind(story_id)
                .fetch_one(harness.pool())
                .await
                .expect("the board's Ready trigger queued exactly one work item");
            }
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

/// Insert a story whose specification is deliberately blank in one field, for the absence-of-a-fact case.
async fn seed_blank_story(harness: &ForgeHarness, story_id: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, dependencies, packet_sha)
         values ($1, 'PROOF', 'Blank packet proof', 'High', 'Ready', '', $2, '   ', null)",
    )
    .bind(story_id)
    .bind(GOAL)
    .execute(harness.pool())
    .await
    .expect("the blank packet story is insertable");
    sqlx::query_scalar("select id::text from agent_work_item where story_id=$1 and state='Ready'")
        .bind(story_id)
        .fetch_one(harness.pool())
        .await
        .expect("the board's Ready trigger queued exactly one work item")
}

/// Claim the item and open its run through the production seam, returning the run id.
async fn begin(harness: &ForgeHarness, item: &str) -> String {
    harness
        .engine()
        .claim_specific_agent_work(item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    harness
        .engine()
        .begin_agent_work_run(item)
        .await
        .expect("the production begin runs")
        .expect("the claim opens the run")
        .story_run_id
}

/// The run's twelve committed snapshot columns, read on the pool the DAO wrote to.
async fn packet(pool: &PgPool, run_id: &str) -> Packet {
    sqlx::query_as(
        "select goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
                context_refs_snapshot, acceptance_criteria_snapshot, postconditions_snapshot,
                dependencies_snapshot, scope_snapshot, operating_surface_snapshot,
                test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot
           from storyboard_story_run where id=$1::uuid",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("the run's packet snapshot is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-009).
async fn forge_story_run_009__packet_snapshot_stored() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();
    let story = format!("{PROOF_PREFIX}packet-{ns}");
    let story_blank = format!("{PROOF_PREFIX}blank-{ns}");

    // 1. THE PACKET IS COPIED WHEN EXECUTION BEGINS.
    let item = seed_packet_story(&harness, &story).await;
    let run_id = begin(&harness, &item).await;
    let stored = packet(&pool, &run_id).await;

    // 2. FROZEN AT OPEN: the story's specification is rewritten after the run opened. The run must not move.
    sqlx::query(
        "update storyboard_story
            set goal='MUTATED GOAL', acceptance_criteria='MUTATED ACCEPTANCE',
                packet_sha='sha256:mutated', assay_commands='mutated command', scope='mutated scope'
          where id=$1",
    )
    .bind(&story)
    .execute(&pool)
    .await
    .expect("the story's specification can be edited");
    let stored_after_edit = packet(&pool, &run_id).await;

    // 3. NEGATIVE / FAULT — BLANK IS ABSENCE, AND NOTHING IS INVENTED. The second story carries a whitespace
    //    `dependencies` and no `packet_sha`; its run must hold NULL for both, not `''` and not a filler.
    let item_blank = seed_blank_story(&harness, &story_blank).await;
    let run_blank = begin(&harness, &item_blank).await;
    let blank = packet(&pool, &run_blank).await;

    // 4. CLEANUP / ROLLBACK: the proof stories are deleted; their items and runs go with them.
    for story in [&story, &story_blank] {
        harness
            .cleanup_story(story)
            .await
            .expect("reap the proof story");
    }
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let stories_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    let runs_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();

    // ---------------------------------------------------------------------------------------------------------
    // 5. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert_eq!(
        stored,
        (
            Some(GOAL.to_string()),
            Some(PRECONDITIONS.to_string()),
            Some(ARCHITECT_BRIEF.to_string()),
            Some(CONTEXT_REFS.to_string()),
            Some(ACCEPTANCE.to_string()),
            Some(POSTCONDITIONS.to_string()),
            Some(DEPENDENCIES.to_string()),
            Some(SCOPE.to_string()),
            Some(OPERATING_SURFACE.to_string()),
            Some(TEST_MODE.to_string()),
            Some(ASSAY.to_string()),
            Some(PACKET_SHA.to_string()),
        ),
        "{HARNESS}: all twelve packet columns are stored on the run exactly as the story held them"
    );
    assert_eq!(
        stored_after_edit, stored,
        "{HARNESS}: the packet is FROZEN at open — editing the story afterwards moves not one snapshot column"
    );

    // 6. NEGATIVE / FAULT CASES.
    assert_eq!(
        blank.0,
        Some(GOAL.to_string()),
        "{HARNESS}: control — the blank story still carries a goal, so the NULLs below are about the blank fields"
    );
    assert_eq!(
        blank.6, None,
        "{HARNESS}: a whitespace-only field is the ABSENCE of a fact — NULL, never an empty string"
    );
    assert_eq!(
        blank.11, None,
        "{HARNESS}: a packet_sha the story never set is not invented by the writer"
    );
    assert_eq!(
        blank.4, None,
        "{HARNESS}: acceptance criteria the story never set stay unset on the run"
    );

    // 7. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}

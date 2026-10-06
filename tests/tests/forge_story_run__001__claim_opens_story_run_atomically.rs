//! FORGE.STORY_RUN — claim opens Story Run atomically (TST-FORGE-STORY-RUN-001).
//!
//! Contract: the `Claimed → Running` seam that a claimant drives before its first role turn —
//! `ForgeEngineDao::begin_agent_work_run` over the database's `forge_begin_agent_work_run` — opens
//! the Story Run and moves the item Running in ONE transaction. There is no interleaving in which a
//! story is Running with no run row, or a run row exists for a story that never entered Running. The
//! refusal half is proven too: beginning a story that was never claimed (or an item that is not
//! Claimed) creates no run and returns `None`.
//!
//! Level: L2 Persistence — an isolated, disposable DEV/Neon target only; `TestDatabase` refuses PROD.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__001__claim_opens_story_run_atomically -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbTarget, ForgeControlDao, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::TestDatabase;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-001-";
const OWNER: &str = "forge-run-owner";

async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Insert a Ready story; the board's Ready trigger opens exactly one work item.
async fn insert_ready_story(pool: &PgPool, story_id: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge story-run proof', 'Critical', 'Ready', '',
                 'claim opens a run atomically', 'SCOPED',
                 'cargo test --manifest-path Cargo.toml -p test-harness --test forge_story_run__001__claim_opens_story_run_atomically',
                 'a run opens exactly once, for the live Claimed claim', 'a disposable DEV branch',
                 'the claim is settled or requeued', 'brief for the run', 'db/src/forge_engine.rs',
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

/// Delete this run's proof stories; items and runs cascade with the story.
async fn reap_namespace(pool: &PgPool, namespace: &str) -> u64 {
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{namespace}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows")
        .rows_affected()
}

/// The committed durable facts a begin writes, read back: `(state, story_run_id)`.
async fn item_row(pool: &PgPool, item_id: &str) -> (String, Option<String>) {
    sqlx::query_as("select state, story_run_id::text from agent_work_item where id=$1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the work item is readable")
}

/// Every Story Run opened for a story: `(id, started_at is not null, ended_at is null)`.
async fn runs_for(pool: &PgPool, story_id: &str) -> Vec<(String, bool, bool)> {
    sqlx::query_as(
        "select id::text, started_at is not null, ended_at is null
           from storyboard_story_run where story_id=$1 order by created_at",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("the story's runs are readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-001); the file and the assay use it.
async fn forge_story_run_001__claim_opens_story_run_atomically() {
    let test_db = connect_dev().await;
    assert_eq!(
        test_db.target(),
        DbTarget::Dev,
        "{HARNESS}: the run proof runs only on an isolated DEV target"
    );
    let pool = test_db.database().pool().clone();
    let engine = ForgeEngineDao::new(test_db.database().clone());
    let _ = ForgeControlDao::new(test_db.database().clone());

    let ns = test_db.namespace().to_string();
    let story_id = format!("{PROOF_PREFIX}owned-{ns}");

    let item = insert_ready_story(&pool, &story_id).await;

    // 1. Refusal first: a begin before any claim asserts nothing into the run table.
    let refused = engine
        .begin_agent_work_run(&item)
        .await
        .expect("begin is answerable");
    assert!(refused.is_none(), "an unclaimed item opens no run");
    assert!(runs_for(&pool, &story_id).await.is_empty());

    // 2. The claim names an owner and moves Claimed; no run yet.
    engine
        .claim_specific_agent_work(&item, OWNER)
        .await
        .unwrap()
        .expect("the owner claims");
    let (state, run_id) = item_row(&pool, &item).await;
    assert_eq!(state, "Claimed");
    assert!(run_id.is_none(), "claiming opens no run");
    assert!(runs_for(&pool, &story_id).await.is_empty());

    // 3. The begin opens exactly one run and flips the item Running in the same transaction.
    let begun = engine
        .begin_agent_work_run(&item)
        .await
        .unwrap()
        .expect("the live claim opens the run");
    assert!(!begun.story_run_id.is_empty());
    let (state, run_id) = item_row(&pool, &item).await;
    assert_eq!(state, "Running", "the begin moves the item Running");
    assert_eq!(run_id.as_deref(), Some(begun.story_run_id.as_str()));

    let runs = runs_for(&pool, &story_id).await;
    assert_eq!(runs.len(), 1, "exactly one run for the story");
    assert_eq!(
        runs[0].0, begun.story_run_id,
        "the run the item carries is the run that opened"
    );

    // 4. A second begin refuses — no duplicate run, no silent re-open.
    let again = engine.begin_agent_work_run(&item).await.unwrap();
    assert!(again.is_none(), "a settled begin cannot be replayed");
    assert_eq!(runs_for(&pool, &story_id).await.len(), 1);

    // Cleanup: this run's proof stories and their cascading items/runs leave no trace.
    let reaped = reap_namespace(&pool, &ns).await;
    assert!(reaped >= 1);
}

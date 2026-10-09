//! FORGE.STORY_RUN — agent_work_item.story_run_id populated (TST-FORGE-STORY-RUN-002).
//!
//! Contract: the one writer of `agent_work_item.story_run_id` is the begin seam
//! (`ForgeEngineDao::begin_agent_work_run`, the database's `forge_begin_agent_work_run`), and after
//! it, `story_run_id` is set on exactly one work item — the one whose claim owns the run. Read back,
//! the item row and the run row are the same story: no other item carries this run id, and the item
//! that carries it is `Running`.
//!
//! Level: L2 Persistence — an isolated, disposable DEV/Neon target only; `TestDatabase` refuses PROD.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__002__agent_work_item_story_run_id_populated -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbTarget, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::TestDatabase;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-002-";
const OWNER: &str = "forge-run-owner-2";

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

async fn insert_ready_story(pool: &PgPool, story_id: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge story-run proof', 'Critical', 'Ready', '',
                 'agent_work_item carries the run id', 'SCOPED',
                 'cargo test --manifest-path Cargo.toml -p test-harness --test forge_story_run__002__agent_work_item_story_run_id_populated',
                 'one item, one run, one id', 'a disposable DEV branch',
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

async fn reap_namespace(pool: &PgPool, namespace: &str) -> u64 {
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{namespace}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows")
        .rows_affected()
}

/// Every work item bound to one Story Run.
async fn holders_of(pool: &PgPool, run_id: &str) -> Vec<String> {
    sqlx::query_scalar(
        "select id::text from agent_work_item where story_run_id=$1::uuid order by id",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .expect("the run's holders are readable")
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-002); the file and the assay use it.
async fn forge_story_run_002__agent_work_item_story_run_id_populated() {
    let test_db = connect_dev().await;
    assert_eq!(
        test_db.target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = test_db.database().pool().clone();
    let engine = ForgeEngineDao::new(test_db.database().clone());

    let ns = test_db.namespace().to_string();
    let story_id = format!("{PROOF_PREFIX}owned-{ns}");
    let item = insert_ready_story(&pool, &story_id).await;

    engine
        .claim_specific_agent_work(&item, OWNER)
        .await
        .unwrap()
        .expect("the owner claims");

    let begun = engine
        .begin_agent_work_run(&item, &fence_of(&pool, &item).await)
        .await
        .unwrap()
        .expect("the live claim opens the run");

    // 1. Exactly one item carries this run id — the one whose claim owns it.
    let holders = holders_of(&pool, &begun.story_run_id).await;
    assert_eq!(
        holders,
        vec![item.clone()],
        "{HARNESS}: the run id is stamped on exactly the owner item"
    );

    // 2. The row that carries it is Running.
    let state: String = sqlx::query_scalar("select state from agent_work_item where id=$1::uuid")
        .bind(&item)
        .fetch_one(&pool)
        .await
        .expect("the item row is readable");
    assert_eq!(
        state, "Running",
        "the run id never lands on a non-Running item"
    );

    // 3. The item's story_run_id and the run's story_id agree.
    let holder_story: String =
        sqlx::query_scalar("select story_id from storyboard_story_run where id=$1::uuid")
            .bind(&begun.story_run_id)
            .fetch_one(&pool)
            .await
            .expect("the run row is readable");
    assert_eq!(
        holder_story, story_id,
        "the item's run belongs to the same story"
    );

    // 4. Cleanup.
    let reaped = reap_namespace(&pool, &ns).await;
    assert!(reaped >= 1);
}

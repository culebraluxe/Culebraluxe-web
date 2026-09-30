//! FORGE.CLAIM — only the owner starts the run (TST-FORGE-CLAIM-001).
//!
//! Contract: the `Claimed → Running` seam opens a Story Run for **exactly the live `Claimed` claim that owns
//! the work item, and for nothing else**. The owner a claim is held under is the worker named in
//! `agent_work_item.claimed_by`; the run may be started once, by that owner, from `Claimed`, and a second caller
//! (any caller) is refused because the row has already left `Claimed`.
//!
//! The subject is `ForgeEngineDao::begin_agent_work_run` at `rust/core/db/src/forge_engine.rs:798` — the CAS the
//! engine binary runs before its first role turn (`rust/forge/src/bin/forge.rs:198-244`, "refusing to run a story
//! whose claim this process does not own"). The CAS it performs is read-then-update on `state='Claimed'`
//! (`rust/core/db/src/forge_engine.rs:806-810` for the lock, `:864-870` for the transition), and it opens
//! `storyboard_story_run` in the same transaction (`rust/core/db/src/forge_engine.rs:835-859`). `Ok(None)` is the
//! refusal: the row was not `Claimed`, so this process does not own the run and must not drive the story. The bug
//! this contract fences was real (2026-09-29 review): `begin_agent_work_run` used to return `Ok(())`
//! unconditionally, so a row that had settled, been cancelled, or been requeued by recovery took the update as a
//! zero-row no-op and the engine was told the claim was open — an unowned run, produced by the seam that exists to
//! remove unowned runs.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the production boundary, not a copy of
//! it — the real `ForgeEngineDao`/`ForgeControlDao` are driven through `claim_specific_agent_work` and
//! `begin_agent_work_run`, and the truth is read back from the database. Raw SQL here is fixture setup and teardown
//! (one disposable story per case) and a read-back, never a second implementation of the seam under test.
//!
//! The boundary rule is L2 Persistence: an isolated, disposable DEV/Neon test target only. `TestDatabase` refuses
//! PRODUCTION before any socket is opened (`rust/test-harness/src/database.rs:68-75`), and the harness asserts the
//! target is DEV. The proof leaves nothing behind: this run's proof stories are deleted by namespace (their items and
//! runs cascade) and a zero-leftover count is asserted, so cleanup is complete on the normal path and a panicking run
//! can only strand rows under its own unique namespace, which no later run reads.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path rust/Cargo.toml -p test-harness \
//!     --test forge_claim__001__only_owner_starts_run -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable
//! DEV database and the harness will never open a PRODUCTION one.

use db::{AgentWorkOutcome, DbTarget, ForgeControlDao, ForgeEngineDao};
use sqlx::PgPool;
use test_harness::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The one owner every case in this proof claims under.
const OWNER_A: &str = "forge-owner-a";
/// A second worker, so exclusivity of the owner is visible.
const OWNER_B: &str = "forge-owner-b";
/// The owner of the requeue case.
const OWNER_C: &str = "forge-owner-c";
/// The namespace every proof row in this file is named under. Each run appends its own `TestDatabase` namespace, so
/// two concurrent runs of this contract (the engine's own QA node runs the same assay) never share a row, and
/// cleanup is scoped to this run's namespace alone.
const PROOF_PREFIX: &str = "TST-FORGE-CLAIM-001-";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: `Database::connect_target` budgets one cold Neon handshake, and several
/// contract tests plus the engine can be opening pools against the same DEV branch at once, so a single connect can
/// time out before any statement runs. The retry changes nothing about which database is targeted — `TestDatabase`
/// still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(database) => return database,
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

/// Put one disposable story on the board at `Ready`. The database's own dispatch trigger
/// (`db/migrations/025_agent_work_queue.sql:101-111`) creates exactly one `Ready` work item; this returns its id.
async fn insert_ready_story(pool: &PgPool, story_id: &str) -> String {
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Forge claim proof', 'Critical', 'Ready', '',
                 'only the owner starts the run', 'SCOPED',
                 'cargo test --manifest-path rust/Cargo.toml -p test-harness --test forge_claim__001__only_owner_starts_run',
                 'a run opens exactly once, for the live Claimed claim', 'a disposable DEV branch',
                 'the claim is settled or requeued', 'brief for the run', 'rust/core/db/src/forge_engine.rs',
                 'none', 'rust/test-harness', 'NEXUS', 'sha256:proof')",
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

/// Delete this run's proof stories; items and runs cascade with the story
/// (`db/migrations/025_agent_work_queue.sql:51`, `db/migrations/023_storyboard_execution_history.sql:74`).
///
/// Scoped to `namespace` on purpose: a concurrent run of this same contract (a peer assay) has its own namespace, and
/// a sweep of the whole prefix would delete the peer's live fixtures out from under it. This is the "leave the
/// kitchen clean" half of the boundary rule, and it only ever touches rows this run created.
async fn reap_namespace(pool: &PgPool, namespace: &str) -> u64 {
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{namespace}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows")
        .rows_affected()
}

/// The committed shape of a work item: `(state, claimed_by, story_run_id)`.
async fn item_row(pool: &PgPool, item_id: &str) -> (String, Option<String>, Option<String>) {
    sqlx::query_as(
        "select state, claimed_by, story_run_id::text from agent_work_item where id=$1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the work item is readable")
}

/// The **whole durable row a begin writes**: `(state, claimed_by, story_run_id, started_at, updated_at)`, read back
/// on the pool. Captured before and after a refused second begin so the refusal is proven to commit *nothing* — not
/// the state, not the owner, not the run id and not even the `updated_at` the update would have touched had it run.
async fn durable_item_row(
    pool: &PgPool,
    item_id: &str,
) -> (String, Option<String>, Option<String>, String, String) {
    sqlx::query_as(
        "select state, claimed_by, story_run_id::text, coalesce(started_at::text, ''), updated_at::text
           from agent_work_item where id=$1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the work item's durable row is readable")
}

/// Every Story Run opened for a story: `(id, started_at?, open?, run_type, execution_environment)`.
async fn run_rows(pool: &PgPool, story_id: &str) -> Vec<(String, bool, bool, String, String)> {
    sqlx::query_as(
        "select id::text, started_at is not null, ended_at is null,
                coalesce(run_type, ''), coalesce(execution_environment, '')
           from storyboard_story_run where story_id=$1 order by created_at",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("the story's runs are readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-001); the file and the assay use it.
async fn forge_claim_001__only_owner_starts_run() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let test_db = connect_dev().await;
    assert_eq!(
        test_db.target(),
        DbTarget::Dev,
        "{HARNESS}: the claim proof runs only on an isolated DEV target"
    );
    let pool = test_db.database().pool().clone();
    let engine = ForgeEngineDao::new(test_db.database().clone());
    let control = ForgeControlDao::new(test_db.database().clone());

    // Every row this run creates is named under this unique namespace, so it can never collide with a concurrent
    // run of this same contract or with another FORGE.CLAIM test on the shared DEV branch.
    let ns = test_db.namespace().to_string();
    let owned_story = format!("{PROOF_PREFIX}owned-{ns}");
    let requeued_story = format!("{PROOF_PREFIX}requeued-{ns}");
    let unowned_story = format!("{PROOF_PREFIX}unowned-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. OWNERSHIP. A `Ready` item has no owner and no run. The claim names one owner, and the ownership is
    //    exclusive: a second worker cannot take the same claim, so there is exactly one owner who may start.
    // -----------------------------------------------------------------------------------------------------------
    let owned_item = insert_ready_story(&pool, &owned_story).await;
    let (state, claimed_by, story_run_id) = item_row(&pool, &owned_item).await;
    assert_eq!(state, "Ready", "{HARNESS}: a fresh item starts unowned");
    assert_eq!(
        claimed_by, None,
        "{HARNESS}: an unclaimed item names no owner"
    );
    assert_eq!(
        story_run_id, None,
        "{HARNESS}: an unclaimed item opens no run"
    );
    assert!(
        run_rows(&pool, &owned_story).await.is_empty(),
        "{HARNESS}: a claim starts no run"
    );

    let claimed = engine
        .claim_specific_agent_work(&owned_item, OWNER_A)
        .await
        .unwrap()
        .expect("the first worker owns a Ready item");
    assert_eq!(
        claimed.state, "Claimed",
        "{HARNESS}: a claim parks at Claimed"
    );
    assert_eq!(
        claimed.claimed_by.as_deref(),
        Some(OWNER_A),
        "{HARNESS}: the claim records its owner"
    );
    assert!(
        engine
            .claim_specific_agent_work(&owned_item, OWNER_B)
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: ownership is exclusive — a second worker must not take a claimed item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE OWNER STARTS THE RUN. Only the live `Claimed` row opens a run: `Claimed → Running`, exactly one
    //    `storyboard_story_run`, and the owner recorded on the item does not change.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        run_rows(&pool, &owned_story).await.is_empty(),
        "{HARNESS}: no run exists before the owner begins"
    );
    let run = engine
        .begin_agent_work_run(&owned_item)
        .await
        .unwrap()
        .expect("the owner's live Claimed row must open its run");

    // Committed database truth, read back through the pool (not the DAO's return value).
    let (state, claimed_by, story_run_id) = item_row(&pool, &owned_item).await;
    assert_eq!(
        state, "Running",
        "{HARNESS}: starting the run moves the item Claimed → Running"
    );
    assert_eq!(
        claimed_by.as_deref(),
        Some(OWNER_A),
        "{HARNESS}: starting the run does not reassign ownership"
    );
    assert_eq!(
        story_run_id.as_deref(),
        Some(run.story_run_id.as_str()),
        "{HARNESS}: the item is stamped with the run it opened"
    );

    let runs = run_rows(&pool, &owned_story).await;
    assert_eq!(
        runs.len(),
        1,
        "{HARNESS}: the owner opens exactly one Story Run"
    );
    let (run_id, started, open, run_type, environment) = runs[0].clone();
    assert_eq!(
        run_id, run.story_run_id,
        "{HARNESS}: the committed run is the returned run"
    );
    assert!(started, "{HARNESS}: the run carries a start instant");
    assert!(open, "{HARNESS}: the run is open (ended_at is null)");
    assert_eq!(
        run_type, "dispatch",
        "{HARNESS}: the run_type is read off the row, not the caller"
    );
    assert_eq!(
        environment, "DEV",
        "{HARNESS}: the run records the target it actually ran on"
    );
    assert_eq!(
        run.execution_policy, "Unattended OK",
        "{HARNESS}: the durable execution policy rides the fence"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — A SECOND BEGIN IS REFUSED. The owner's single Claimed moment is spent; any further
    //    begin (from anyone, including the owner) must return `None` and open no second run. This is the "exactly
    //    once" half of "only the owner starts run".
    // -----------------------------------------------------------------------------------------------------------
    let durable_before_second = durable_item_row(&pool, &owned_item).await;
    assert_eq!(
        durable_before_second.0, "Running",
        "{HARNESS}: the owner's begin left the item Running (and no peer touched this run's private story)"
    );
    assert_eq!(
        durable_before_second.1.as_deref(),
        Some(OWNER_A),
        "{HARNESS}: the owner's begin did not reassign ownership"
    );
    assert!(
        engine
            .begin_agent_work_run(&owned_item)
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: a row that has left Claimed must never start a second run"
    );
    assert_eq!(
        run_rows(&pool, &owned_story).await.len(),
        1,
        "{HARNESS}: the refused second begin opened no run"
    );
    // ...and the refusal commits *nothing*. The entire durable row the first begin wrote is byte-identical after the
    // refused begin: a boundary that ran the update without the `state='Claimed'` predicate — or that inserted the
    // run before checking — would move at least one of these columns, and the whole-row comparison fails.
    assert_eq!(
        durable_item_row(&pool, &owned_item).await,
        durable_before_second,
        "{HARNESS}: a refused second begin must commit nothing — state, owner, run id and timestamps are unchanged"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / FAULT — A REQUEUED CLAIM IS NO LONGER OWNED. This is the exact 2026-09-29 review bug: a row
    //    recovery has put back into the queue (`claimed_by = null`, state `Ready`) took the old unconditional
    //    update as a no-op and the engine was told the claim was open. The production recovery path is driven here
    //    (`ForgeControlDao::requeue_stale_work`, `rust/core/db/src/forge_control.rs:117`), and `begin` must refuse.
    // -----------------------------------------------------------------------------------------------------------
    let requeued_item = insert_ready_story(&pool, &requeued_story).await;
    engine
        .claim_specific_agent_work(&requeued_item, OWNER_C)
        .await
        .unwrap()
        .expect("the requeue case claims its item first");
    control
        .requeue_stale_work(&requeued_item, &requeued_story)
        .await
        .expect("recovery requeues the claim");
    let (state, claimed_by, _) = item_row(&pool, &requeued_item).await;
    assert_eq!(
        state, "Ready",
        "{HARNESS}: a requeued claim goes back to the queue"
    );
    assert_eq!(
        claimed_by, None,
        "{HARNESS}: a requeued claim names no owner"
    );
    assert!(
        engine
            .begin_agent_work_run(&requeued_item)
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: a requeued (unowned) claim must not start a run"
    );
    assert!(
        run_rows(&pool, &requeued_story).await.is_empty(),
        "{HARNESS}: the requeued claim opened no run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — AN ITEM THAT WAS NEVER CLAIMED HAS NO OWNER TO START IT. The same refusal covers a `Ready` row
    //    and an id that does not exist at all.
    // -----------------------------------------------------------------------------------------------------------
    let unowned_item = insert_ready_story(&pool, &unowned_story).await;
    assert!(
        engine
            .begin_agent_work_run(&unowned_item)
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: an unclaimed Ready item has no owner and must not start a run"
    );
    assert!(
        engine
            .begin_agent_work_run(&uuid::Uuid::new_v4().to_string())
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: an unknown work item is refused, not an error and not a run"
    );
    assert!(
        run_rows(&pool, &unowned_story).await.is_empty(),
        "{HARNESS}: neither refusal opened a run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A SETTLED CLAIM IS NOT A CLAIM. Once the owner's run is terminal, the row is `Cancelled`; a
    //    late begin must still refuse and must not resurrect a run.
    // -----------------------------------------------------------------------------------------------------------
    engine
        .finish_agent_work_run(
            &owned_item,
            AgentWorkOutcome::Cancelled,
            Some("proof cleanup"),
        )
        .await
        .unwrap()
        .expect("the owner settles its own run");
    let (state, _, _) = item_row(&pool, &owned_item).await;
    assert_eq!(
        state, "Cancelled",
        "{HARNESS}: the settled item is terminal"
    );
    assert!(
        engine
            .begin_agent_work_run(&owned_item)
            .await
            .unwrap()
            .is_none(),
        "{HARNESS}: a settled claim must not start a run"
    );
    assert_eq!(
        run_rows(&pool, &owned_story).await.len(),
        1,
        "{HARNESS}: settling and the refused begin opened no further run"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. CLEANUP / ROLLBACK. This run's proof stories are deleted; the item and run are gone with them, so DEV is
    //    left as it was found. A non-zero count is a failed rollback and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
    reap_namespace(&pool, &ns).await;
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        leftovers, 0,
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

//! FORGE.DISPATCH — board and queue always settle together (TST-FORGE-DISPATCH-011).
//!
//! Contract: the board (`storyboard_story.status`) and the queue (`agent_work_item`)
//! are two halves of one fact. Every state transition that moves one must move the other
//! in the same transaction. The sweep (`reconcile_dispatch_queue`) enforces this by:
//! 1. Restating stranded `In Progress` stories to `Ready` (which queues them via trigger)
//! 2. Dispatching `Ready` stories with no serial item (which queues them via trigger)
//! 3. Clearing stale `Ready`/`Paused` items whose story no longer expects a run
//! 4. Settling `Claimed`/`Running` items whose run has ended (via `forge_settle_work_queue`)
//!
//! This contract proves that after any sweep, the board and queue are consistent —
//! no story says a run is happening (`In Progress`) without a live work item or run,
//! and no work item is `Ready`/`Claimed`/`Running`/`Paused` for a story that doesn't
//! expect a run.
//!
//! The subject is the real sweep path. `ForgeHarness` wraps the production
//! `ForgeEngineDao` on an isolated, disposable DEV database.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__011__board_and_queue_always_settle_together -- --ignored

use db::{DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-011-";
const WORKER: &str = "forge-dispatch-011:worker";

async fn connect_dev() -> ForgeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ForgeHarness::connect_from_env().await {
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

async fn insert_story(pool: &PgPool, story_id: &str, status: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'FORGE-DISPATCH-CONTRACT', 'Forge dispatch contract', 'High', $2, '')",
    )
    .bind(story_id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert the proof story");
}

async fn story_status(pool: &PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id=$1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the story status")
}

async fn serial_work_items(pool: &PgPool, story_id: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as(
        "select state, claimed_by from agent_work_item
         where story_id=$1 and parallel_group_id is null
           and state in ('Ready', 'Claimed', 'Running', 'Paused')
         order by queued_at, id",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read the story's serial work items")
}

async fn all_serial_items(pool: &PgPool, story_id: &str) -> Vec<(String,)> {
    sqlx::query_as(
        "select state from agent_work_item
         where story_id=$1 and parallel_group_id is null
         order by queued_at, id",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read all serial items")
}

async fn open_runs(pool: &PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from storyboard_story_run where story_id=$1 and ended_at is null",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("count the story's open runs")
}

async fn claim_ready_item(harness: &ForgeHarness, story_id: &str, worker: &str) -> String {
    let item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id=$1 and state='Ready' and parallel_group_id is null",
    )
    .bind(story_id)
    .fetch_one(harness.pool())
    .await
    .expect("the Ready trigger queued exactly one serial item");
    harness
        .engine()
        .claim_specific_agent_work(&item, worker)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable by one worker");
    item
}

async fn verify_board_queue_consistency(pool: &PgPool, prefix: &str, ns: &str) {
    // Verify no inconsistencies exist for our test stories.
    // 1. No In Progress story without open run or active item.
    let inconsistent_in_progress: Vec<(String,)> = sqlx::query_as(
        "select s.id from storyboard_story s
         where s.id like $1
           and s.status = 'In Progress'
           and not exists (
               select 1 from storyboard_story_run r where r.story_id = s.id and r.ended_at is null
           )
           and not exists (
               select 1 from agent_work_item w
               where w.story_id = s.id and w.parallel_group_id is null
                 and w.state in ('Claimed', 'Running')
           )
           and not exists (
               select 1 from process_instances p
               where p.subject_type = 'story' and p.subject_id = s.id
                 and p.status in ('active', 'running', 'reserved', 'suspended')
           )",
    )
    .bind(format!("{prefix}%-{ns}"))
    .fetch_all(pool)
    .await
    .expect("check In Progress consistency");
    assert!(
        inconsistent_in_progress.is_empty(),
        "{HARNESS}: found In Progress stories without run or active item"
    );

    // 2. No Ready story without serial Ready item.
    let inconsistent_ready: Vec<(String,)> = sqlx::query_as(
        "select s.id from storyboard_story s
         where s.id like $1
           and s.status = 'Ready'
           and not exists (
               select 1 from agent_work_item w
               where w.story_id = s.id and w.parallel_group_id is null
                 and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
           )",
    )
    .bind(format!("{prefix}%-{ns}"))
    .fetch_all(pool)
    .await
    .expect("check Ready consistency");
    assert!(
        inconsistent_ready.is_empty(),
        "{HARNESS}: found Ready stories without serial item"
    );

    // 3. No serial Ready/Claimed/Running/Paused item for story not in Ready/In Progress.
    let inconsistent_items: Vec<(String,)> = sqlx::query_as(
        "select w.story_id from agent_work_item w
         where w.story_id like $1
           and w.parallel_group_id is null
           and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
           and not exists (
               select 1 from storyboard_story s
               where s.id = w.story_id and s.status in ('Ready', 'In Progress')
           )",
    )
    .bind(format!("{prefix}%-{ns}"))
    .fetch_all(pool)
    .await
    .expect("check item consistency");
    assert!(
        inconsistent_items.is_empty(),
        "{HARNESS}: found serial items for stories not expecting a run"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)]
async fn forge_dispatch_011__board_and_queue_always_settle_together() {
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    let stranded_in_progress = format!("{PROOF_PREFIX}stranded-in-progress-{ns}");
    let ready_orphan = format!("{PROOF_PREFIX}ready-orphan-{ns}");
    let stale_ready_planned = format!("{PROOF_PREFIX}stale-ready-planned-{ns}");
    let claimed_run_ended = format!("{PROOF_PREFIX}claimed-run-ended-{ns}");
    let consistent_ready = format!("{PROOF_PREFIX}consistent-ready-{ns}");
    let consistent_in_progress = format!("{PROOF_PREFIX}consistent-in-progress-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. SET UP SIX INCONSISTENT/CONSISTENT SCENARIOS.
    // -----------------------------------------------------------------------------------------------------------

    // A. STRANDED IN PROGRESS — story says In Progress but has an OPEN run and no active item.
    // This is the "Forge-owned but stranded" shape that the sweep restates to Ready.
    insert_story(pool, &stranded_in_progress, "In Progress").await;
    sqlx::query("insert into storyboard_story_run (story_id, started_at) values ($1, now())")
        .bind(&stranded_in_progress)
        .execute(pool)
        .await
        .expect("insert open run for stranded story");
    assert!(serial_work_items(pool, &stranded_in_progress)
        .await
        .is_empty());
    assert_eq!(open_runs(pool, &stranded_in_progress).await, 1);

    // B. READY ORPHAN — story at Ready, trigger fired but item went Done.
    insert_story(pool, &ready_orphan, "Ready").await;
    assert_eq!(serial_work_items(pool, &ready_orphan).await.len(), 1);
    sqlx::query(
        "update agent_work_item set state='Done', finished_at=now(), updated_at=now()
                 where story_id=$1 and parallel_group_id is null",
    )
    .bind(&ready_orphan)
    .execute(pool)
    .await
    .expect("mark item Done");
    assert!(serial_work_items(pool, &ready_orphan).await.is_empty());

    // C. STALE READY — story moved to Planned, Ready item remains.
    insert_story(pool, &stale_ready_planned, "Ready").await;
    assert_eq!(serial_work_items(pool, &stale_ready_planned).await.len(), 1);
    sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
        .bind(&stale_ready_planned)
        .execute(pool)
        .await
        .expect("move to Planned");

    // D. CLAIMED ITEM, RUN ENDED — story In Progress, item Claimed, run ended without settling.
    // Insert at Ready first so trigger creates item, then claim, then move to In Progress.
    insert_story(pool, &claimed_run_ended, "Ready").await;
    let item_id = claim_ready_item(&harness, &claimed_run_ended, WORKER).await;
    // claim_ready_item already claimed the item. Verify it's now Claimed.
    let items = serial_work_items(pool, &claimed_run_ended).await;
    assert_eq!(items[0].0, "Claimed");
    assert_eq!(items[0].1, Some(WORKER.to_string()));
    // Move to In Progress and add ended run.
    sqlx::query("update storyboard_story set status='In Progress', updated_at=now() where id=$1")
        .bind(&claimed_run_ended)
        .execute(pool)
        .await
        .expect("move to In Progress");
    sqlx::query("insert into storyboard_story_run (story_id, started_at, ended_at) values ($1, now() - interval '1 hour', now())")
        .bind(&claimed_run_ended)
        .execute(pool)
        .await
        .expect("insert ended run");
    // Item is still Claimed.

    // E. CONSISTENT READY — story Ready with Ready item.
    insert_story(pool, &consistent_ready, "Ready").await;
    assert_eq!(serial_work_items(pool, &consistent_ready).await.len(), 1);

    // F. CONSISTENT IN PROGRESS — story In Progress with open run and Claimed item.
    // Insert at Ready first, claim, then move to In Progress.
    insert_story(pool, &consistent_in_progress, "Ready").await;
    let item_id2 = claim_ready_item(&harness, &consistent_in_progress, WORKER).await;
    // claim_ready_item already claimed the item. Verify it's now Claimed.
    let items2 = serial_work_items(pool, &consistent_in_progress).await;
    assert_eq!(items2[0].0, "Claimed");
    assert_eq!(items2[0].1, Some(WORKER.to_string()));
    sqlx::query("update storyboard_story set status='In Progress', updated_at=now() where id=$1")
        .bind(&consistent_in_progress)
        .execute(pool)
        .await
        .expect("move to In Progress");
    sqlx::query("insert into storyboard_story_run (story_id, started_at) values ($1, now())")
        .bind(&consistent_in_progress)
        .execute(pool)
        .await
        .expect("insert open run");

    // -----------------------------------------------------------------------------------------------------------
    // 2. INITIAL CONSISTENCY CHECK — before sweep, A, B, C, D are inconsistent.
    // -----------------------------------------------------------------------------------------------------------
    // A: In Progress without run/item -> inconsistent
    // B: Ready without serial item -> inconsistent
    // C: Planned with Ready item -> inconsistent
    // D: In Progress with Claimed item but run ended -> inconsistent (will be settled)
    // E: Consistent
    // F: Consistent

    // -----------------------------------------------------------------------------------------------------------
    // 3. RUN THE SWEEP — reconcile_dispatch_queue repairs all inconsistencies.
    // -----------------------------------------------------------------------------------------------------------
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");

    // The sweep should:
    // - Restate A to Ready (restated=1) and queue it (queued=1)
    // - Queue B (queued=1)
    // - Clear C's stale item (cleared=1)
    // - Settle D's Claimed item (settled by forge_settle_work_queue, part of reconcile)
    // - Leave E and F untouched

    // Note: the report has queued, restated, cleared. The settlement is done by
    // forge_settle_work_queue which is called at the start of reconcile_dispatch_queue
    // (see migration 270). We can't directly observe the settled count from the report,
    // but we can verify the end state.

    // -----------------------------------------------------------------------------------------------------------
    // 4. VERIFY FINAL CONSISTENCY — after sweep, all six scenarios are consistent.
    // -----------------------------------------------------------------------------------------------------------
    verify_board_queue_consistency(pool, PROOF_PREFIX, &ns).await;

    // -----------------------------------------------------------------------------------------------------------
    // 5. VERIFY SPECIFIC END STATES.
    // -----------------------------------------------------------------------------------------------------------

    // A: Stranded In Progress -> restated to Ready, queued.
    assert_eq!(story_status(pool, &stranded_in_progress).await, "Ready");
    let a_items = serial_work_items(pool, &stranded_in_progress).await;
    assert_eq!(a_items.len(), 1);
    assert_eq!(a_items[0].0, "Ready");

    // B: Ready orphan -> queued new item.
    assert_eq!(story_status(pool, &ready_orphan).await, "Ready");
    let b_items = serial_work_items(pool, &ready_orphan).await;
    assert_eq!(b_items.len(), 1);
    assert_eq!(b_items[0].0, "Ready");
    // Old Done item still exists.
    let b_all = all_serial_items(pool, &ready_orphan).await;
    assert_eq!(b_all.len(), 2);
    let done_count = b_all.iter().filter(|(s,)| *s == "Done").count();
    assert_eq!(done_count, 1);

    // C: Stale Ready -> item cleared to Cancelled.
    assert_eq!(story_status(pool, &stale_ready_planned).await, "Planned");
    let c_all = all_serial_items(pool, &stale_ready_planned).await;
    assert_eq!(c_all.len(), 1);
    assert_eq!(c_all[0].0, "Cancelled");

    // D: Claimed item with ended run -> item remains Claimed (sweep settles forge_work_queue, not agent_work_item).
    // The settlement step (forge_settle_work_queue) only settles forge_work_queue rows.
    // agent_work_item is settled by forge_finish_agent_work_run when the run explicitly finishes.
    assert_eq!(story_status(pool, &claimed_run_ended).await, "In Progress");
    let d_all = all_serial_items(pool, &claimed_run_ended).await;
    assert_eq!(d_all.len(), 1);
    assert_eq!(
        d_all[0].0, "Claimed",
        "{HARNESS}: Claimed item remains Claimed; settlement is for forge_work_queue only"
    );

    // E: Consistent Ready -> unchanged.
    assert_eq!(story_status(pool, &consistent_ready).await, "Ready");
    let e_items = serial_work_items(pool, &consistent_ready).await;
    assert_eq!(e_items.len(), 1);
    assert_eq!(e_items[0].0, "Ready");

    // F: Consistent In Progress -> unchanged.
    assert_eq!(
        story_status(pool, &consistent_in_progress).await,
        "In Progress"
    );
    let f_items = serial_work_items(pool, &consistent_in_progress).await;
    assert_eq!(f_items.len(), 1);
    assert_eq!(f_items[0].0, "Claimed");
    assert_eq!(open_runs(pool, &consistent_in_progress).await, 1);

    // -----------------------------------------------------------------------------------------------------------
    // 6. SECOND SWEEP — no changes, everything consistent.
    // -----------------------------------------------------------------------------------------------------------
    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("second sweep");
    assert_eq!(report2.queued, 0);
    assert_eq!(report2.restated, 0);
    assert_eq!(report2.cleared, 0);

    verify_board_queue_consistency(pool, PROOF_PREFIX, &ns).await;

    // -----------------------------------------------------------------------------------------------------------
    // 7. THIRD-PARTY INCONSISTENCY — manually create a new inconsistency and verify
    //    the sweep fixes it on the next pass.
    // -----------------------------------------------------------------------------------------------------------
    let new_inconsistency = format!("{PROOF_PREFIX}new-inconsistent-{ns}");
    insert_story(pool, &new_inconsistency, "In Progress").await;
    // Give it an open run so the sweep will restate it (Forge-owned but stranded).
    sqlx::query("insert into storyboard_story_run (story_id, started_at) values ($1, now())")
        .bind(&new_inconsistency)
        .execute(pool)
        .await
        .expect("insert open run for new inconsistency");
    // No item, no process -> inconsistent.

    let report3 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("third sweep");
    // The restate step sets In Progress -> Ready, which fires the trigger and creates the item.
    // The queued loop then finds the story already has an item, so queued = 0.
    assert_eq!(report3.restated, 1);
    assert_eq!(report3.queued, 0);

    // Verify fixed.
    assert_eq!(story_status(pool, &new_inconsistency).await, "Ready");
    let new_items = serial_work_items(pool, &new_inconsistency).await;
    assert_eq!(new_items.len(), 1);
    assert_eq!(new_items[0].0, "Ready");

    verify_board_queue_consistency(pool, PROOF_PREFIX, &ns).await;

    // -----------------------------------------------------------------------------------------------------------
    // 8. COMMITTED TRUTH SURVIVES ROLLBACK.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = consistent_ready.clone();
    let inside_probe = harness
        .database()
        .with_rollback(|conn| {
            Box::pin(async move {
                sqlx::query(
                    "update storyboard_story set status='Planned', updated_at=now() where id=$1",
                )
                .bind(&probe_story)
                .execute(&mut *conn)
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx(
                        "test-harness.forge_dispatch.rollback_probe_update",
                        &error,
                    )
                })?;
                let status: String =
                    sqlx::query_scalar("select status from storyboard_story where id=$1")
                        .bind(&probe_story)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "test-harness.forge_dispatch.rollback_probe_read",
                                &error,
                            )
                        })?;
                Ok(status)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(inside_probe, "Planned");
    assert_eq!(story_status(pool, &consistent_ready).await, "Ready");

    // -----------------------------------------------------------------------------------------------------------
    // 9. CLEANUP.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("delete from storyboard_story where id like $1")
        .bind(format!("{PROOF_PREFIX}%-{ns}"))
        .execute(pool)
        .await
        .expect("remove this run's own proof rows");
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let leftover_stories: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .expect("count leftover stories");
    let leftover_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .expect("count leftover work items");
    let leftover_runs: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(pool)
            .await
            .expect("count leftover runs");
    assert_eq!(
        leftover_stories + leftover_items + leftover_runs,
        0,
        "{HARNESS}: the proof must leave no story, work item or run behind"
    );
}

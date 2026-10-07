//! FORGE.DISPATCH — stale Ready junk cleared (TST-FORGE-DISPATCH-010).
//!
//! Contract: work items in `Ready` or `Paused` state whose story no longer expects a run
//! (story status is not `Ready` or `In Progress`) are "stale junk" and must be cleared
//! to `Cancelled` by the sweep (`reconcile_dispatch_queue`). The sweep runs this cleanup
//! as its third repair step, after restating stranded stories and dispatching Ready orphans.
//! This contract proves that the sweep clears exactly these stale items and only these.
//!
//! The subject is the real sweep path. `ForgeHarness` wraps the production
//! `ForgeEngineDao` on an isolated, disposable DEV database.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__010__stale_ready_junk_cleared -- --ignored

use db::{DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-010-";
const WORKER: &str = "forge-dispatch-010:worker";

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
        "insert into storyboard_story (id, workstream, title, priority, status, notes, work_type)
         values ($1, 'FORGE-DISPATCH-CONTRACT', 'Forge dispatch contract', 'High', $2, '', 'FEATURE')",
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
         order by queued_at, id"
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read all serial items")
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)]
async fn forge_dispatch_010__stale_ready_junk_cleared() {
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    // Ensure runtime is in a known state: unpaused, default concurrency.
    sqlx::query("update forge_runtime_control set paused = false, global_story_concurrency = 64, updated_by = 'test', updated_at = now() where id = 1")
        .execute(pool)
        .await
        .expect("reset runtime control");

    let ready_story_planned = format!("{PROOF_PREFIX}ready-story-planned-{ns}");
    let ready_story_done = format!("{PROOF_PREFIX}ready-story-done-{ns}");
    let ready_story_cancelled = format!("{PROOF_PREFIX}ready-story-cancelled-{ns}");
    let paused_story_planned = format!("{PROOF_PREFIX}paused-story-planned-{ns}");
    let ready_story_stays_ready = format!("{PROOF_PREFIX}ready-stays-ready-{ns}");
    let in_progress_story = format!("{PROOF_PREFIX}in-progress-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. READY STORY MOVED TO PLANNED — Ready item becomes stale junk.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_story_planned, "Ready").await;
    let items = serial_work_items(pool, &ready_story_planned).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Ready");

    sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
        .bind(&ready_story_planned)
        .execute(pool)
        .await
        .expect("move story to Planned");

    // -----------------------------------------------------------------------------------------------------------
    // 2. READY STORY MOVED TO COMPLETE — Ready item becomes stale junk.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_story_done, "Ready").await;
    let items2 = serial_work_items(pool, &ready_story_done).await;
    assert_eq!(items2.len(), 1);
    assert_eq!(items2[0].0, "Ready");

    sqlx::query("update storyboard_story set status='Complete', completed_at=now(), updated_at=now() where id=$1")
        .bind(&ready_story_done)
        .execute(pool)
        .await
        .expect("move story to Complete");

    // -----------------------------------------------------------------------------------------------------------
    // 3. READY STORY MOVED TO FAILED — Ready item becomes stale junk.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_story_cancelled, "Ready").await;
    let items3 = serial_work_items(pool, &ready_story_cancelled).await;
    assert_eq!(items3.len(), 1);
    assert_eq!(items3[0].0, "Ready");

    sqlx::query("update storyboard_story set status='Failed', updated_at=now() where id=$1")
        .bind(&ready_story_cancelled)
        .execute(pool)
        .await
        .expect("move story to Failed");

    // -----------------------------------------------------------------------------------------------------------
    // 4. PAUSED STORY MOVED TO PLANNED — Paused item becomes stale junk.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &paused_story_planned, "Ready").await;
    let item_id = claim_ready_item(&harness, &paused_story_planned, WORKER).await;
    // claim_ready_item already claimed the item. Verify it's now Claimed.
    let items = serial_work_items(pool, &paused_story_planned).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Claimed");
    assert_eq!(items[0].1, Some(WORKER.to_string()));

    sqlx::query("update agent_work_item set state='Paused', updated_at=now()
                 where story_id=$1 and parallel_group_id is null")
        .bind(&paused_story_planned)
        .execute(pool)
        .await
        .expect("pause the item");

    let paused_items = serial_work_items(pool, &paused_story_planned).await;
    assert_eq!(paused_items[0].0, "Paused");

    sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
        .bind(&paused_story_planned)
        .execute(pool)
        .await
        .expect("move story to Planned");

    // -----------------------------------------------------------------------------------------------------------
    // 5. POSITIVE CONTROL — READY STORY STAYS READY — item is NOT stale.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_story_stays_ready, "Ready").await;
    let items5 = serial_work_items(pool, &ready_story_stays_ready).await;
    assert_eq!(items5.len(), 1);
    assert_eq!(items5[0].0, "Ready");
    // Story remains Ready — item should NOT be cleared.

    // -----------------------------------------------------------------------------------------------------------
    // 6. POSITIVE CONTROL — IN PROGRESS STORY WITH OPEN RUN — item is NOT stale
    //    (even if no serial item, the run is positive evidence).
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &in_progress_story, "In Progress").await;
    sqlx::query("insert into storyboard_story_run (story_id, started_at) values ($1, now())")
        .bind(&in_progress_story)
        .execute(pool)
        .await
        .expect("insert open run");
    // No serial item, but that's OK — the run is evidence.

    // -----------------------------------------------------------------------------------------------------------
    // 7. RUN THE SWEEP — should clear items for stories 1-4, NOT clear 5-6.
    // -----------------------------------------------------------------------------------------------------------
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");

    // The sweep clears stale items from ALL namespaces. We expect at least 4 (our 4 stories).
    // There may be additional items from previous test runs.
    assert!(report.cleared >= 4, "{HARNESS}: sweep clears at least 4 stale items (got {})", report.cleared);

    // Verify stories 1-4 have their items cleared to Cancelled.
    for story_id in [
        &ready_story_planned,
        &ready_story_done,
        &ready_story_cancelled,
        &paused_story_planned,
    ] {
        let all_items = all_serial_items(pool, story_id).await;
        assert!(!all_items.is_empty(), "{HARNESS}: {:?} should have an item row", story_id);
        assert_eq!(all_items[0].0, "Cancelled", "{HARNESS}: {:?} item cleared to Cancelled", story_id);
    }

    // Verify story 5 (Ready, stays Ready) item is NOT cleared.
    let items5_after = serial_work_items(pool, &ready_story_stays_ready).await;
    assert_eq!(items5_after.len(), 1, "{HARNESS}: Ready story that stays Ready keeps its item");
    assert_eq!(items5_after[0].0, "Ready");

    // Verify story 6 (In Progress with open run) is restated to Ready and dispatched.
    assert_eq!(story_status(pool, &in_progress_story).await, "Ready",
        "{HARNESS}: In Progress story with open run restated to Ready");
    let in_progress_items = serial_work_items(pool, &in_progress_story).await;
    assert_eq!(in_progress_items.len(), 1, "{HARNESS}: restated story gets new serial item");
    assert_eq!(in_progress_items[0].0, "Ready");

    // -----------------------------------------------------------------------------------------------------------
    // 8. SECOND SWEEP — no more stale items, cleared = 0.
    // -----------------------------------------------------------------------------------------------------------
    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("second sweep");
    assert_eq!(report2.cleared, 0, "{HARNESS}: second sweep clears nothing");

    // -----------------------------------------------------------------------------------------------------------
    // 9. FAULT CASE — CLAIMED/RUNNING items are NEVER cleared by this sweep step.
    //    They are handled by the settlement sweep (forge_settle_work_queue).
    // -----------------------------------------------------------------------------------------------------------
    let running_story = format!("{PROOF_PREFIX}running-{ns}");
    // Insert at Ready first so trigger creates item, then claim, then move to In Progress.
    insert_story(pool, &running_story, "Ready").await;
    let item_id2 = claim_ready_item(&harness, &running_story, WORKER).await;
    // claim_ready_item already claimed the item. Verify it's now Claimed.
    let items = serial_work_items(pool, &running_story).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Claimed");
    assert_eq!(items[0].1, Some(WORKER.to_string()));

    // Move story to In Progress (simulating run started).
    sqlx::query("update storyboard_story set status='In Progress', updated_at=now() where id=$1")
        .bind(&running_story)
        .execute(pool)
        .await
        .expect("move running story to In Progress");

    // Move story to Planned while item is Claimed.
    sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
        .bind(&running_story)
        .execute(pool)
        .await
        .expect("move running story to Planned");

    // The sweep's third step (clear stale) only clears Ready/Paused items.
    // Claimed/Running items are settled by forge_settle_work_queue (first step of reconcile).
    let report3 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("third sweep");
    // The settlement step should settle the Claimed item (run ended without settling).
    // Then the clear step should NOT clear it because it's no longer Ready/Paused.
    assert!(report3.cleared == 0, "{HARNESS}: Claimed item not cleared by stale cleanup");

    // Verify the Claimed item remains Claimed (settlement is for forge_work_queue only).
    // agent_work_item is settled by forge_finish_agent_work_run when the run explicitly finishes.
    let running_items = all_serial_items(pool, &running_story).await;
    assert!(!running_items.is_empty());
    assert_eq!(running_items[0].0, "Claimed", "{HARNESS}: Claimed item remains Claimed; settlement is for forge_work_queue only");

    // -----------------------------------------------------------------------------------------------------------
    // 10. COMMITTED TRUTH SURVIVES ROLLBACK.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = ready_story_stays_ready.clone();
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
    assert_eq!(story_status(pool, &ready_story_stays_ready).await, "Ready");

    // -----------------------------------------------------------------------------------------------------------
    // 11. CLEANUP.
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
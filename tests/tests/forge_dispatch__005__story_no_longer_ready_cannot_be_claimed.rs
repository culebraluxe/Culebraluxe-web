//! FORGE.DISPATCH — a story no longer Ready cannot be claimed (TST-FORGE-DISPATCH-005).
//!
//! Contract: the claim door (`forge_claim_specific_agent_work`, migration 262/275) refuses to claim
//! a work item when its story is no longer in a state that expects a run (`Ready` or `In Progress`).
//! If a story's status has been changed to `Planned`, `Done`, `Cancelled`, or any other non-run state,
//! the claim must fail — the item is cleared by the sweep, not claimed by a worker. This proves the
//! guard that prevents a worker from claiming work the board has already withdrawn.
//!
//! The subject is the real claim path. `ForgeHarness` wraps the production `ForgeEngineDao` on an
//! isolated, disposable DEV database, and the production claim function is used directly.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__005__story_no_longer_ready_cannot_be_claimed -- --ignored

use db::{DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-005-";
const LIVE_WORKER: &str = "forge-dispatch-005:live";

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
async fn forge_dispatch_005__story_no_longer_ready_cannot_be_claimed() {
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the claim contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    let story_planned = format!("{PROOF_PREFIX}story-planned-{ns}");
    let story_done = format!("{PROOF_PREFIX}story-done-{ns}");
    let story_ready_claimable = format!("{PROOF_PREFIX}story-ready-claimable-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. STORY AT PLANNED — a Ready item exists (from trigger), but story moved to Planned.
    //    The sweep clears Ready items for stories that no longer expect a run.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &story_planned, "Ready").await;
    // Get the Ready item (don't claim it).
    let item_id: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id=$1 and state='Ready' and parallel_group_id is null",
    )
    .bind(&story_planned)
    .fetch_one(pool)
    .await
    .expect("the Ready trigger queued exactly one serial item");
    // Now move story to Planned (simulating board withdrawal).
    sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
        .bind(&story_planned)
        .execute(pool)
        .await
        .expect("move story to Planned");

    assert_eq!(story_status(pool, &story_planned).await, "Planned");
    // The item is still Ready (not yet claimed).
    let items = serial_work_items(pool, &story_planned).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Ready");

    // Run sweep - should clear the Ready item for Planned story.
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    assert!(report.cleared >= 1, "{HARNESS}: sweep clears Ready item for Planned story (cleared={})", report.cleared);

    // Debug: check item state directly using a fresh query
    let debug_items: Vec<(String,)> = sqlx::query_as(
        "select state from agent_work_item where story_id=$1 and parallel_group_id is null"
    )
    .bind(&story_planned)
    .fetch_all(pool)
    .await
    .expect("read all serial items for debug");
    eprintln!("Item states after sweep: {:?}", debug_items);

    // After sweep, item should be Cancelled.
    let items_after_sweep = serial_work_items(pool, &story_planned).await;
    assert!(items_after_sweep.is_empty(), "{HARNESS}: Ready item cleared by sweep for Planned story");
    let all_items: Vec<(String,)> = sqlx::query_as(
        "select state from agent_work_item where story_id=$1 and parallel_group_id is null"
    )
    .bind(&story_planned)
    .fetch_all(pool)
    .await
    .expect("read all serial items");
    assert_eq!(all_items[0].0, "Cancelled", "{HARNESS}: item cleared to Cancelled");

    // Now try to claim the Cancelled item - should fail.
    let claim_result = harness.engine().claim_specific_agent_work(&item_id, "another-worker").await;
    assert!(claim_result.is_ok(), "claim call succeeds");
    assert!(claim_result.unwrap().is_none(), "{HARNESS}: claim refused for Cancelled item");

    // -----------------------------------------------------------------------------------------------------------
    // 2. STORY AT COMPLETE — story completed, item still Ready. Sweep clears it.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &story_done, "Ready").await;
    let item_id2: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id=$1 and state='Ready' and parallel_group_id is null",
    )
    .bind(&story_done)
    .fetch_one(pool)
    .await
    .expect("the Ready trigger queued exactly one serial item");
    sqlx::query("update storyboard_story set status='Complete', completed_at=now(), updated_at=now() where id=$1")
        .bind(&story_done)
        .execute(pool)
        .await
        .expect("move story to Complete");

    assert_eq!(story_status(pool, &story_done).await, "Complete");

    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("second sweep");
    assert!(report2.cleared >= 1, "{HARNESS}: sweep clears Ready item for Complete story");

    let items_after2 = serial_work_items(pool, &story_done).await;
    assert!(items_after2.is_empty(), "{HARNESS}: Ready item cleared by sweep for Complete story");

    let claim_result2 = harness.engine().claim_specific_agent_work(&item_id2, "another-worker").await;
    assert!(claim_result2.is_ok());
    assert!(claim_result2.unwrap().is_none(), "{HARNESS}: claim refused for Cancelled item");

    // -----------------------------------------------------------------------------------------------------------
    // 3. POSITIVE CONTROL — story at Ready, claim succeeds.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &story_ready_claimable, "Ready").await;
    let item_id4 = claim_ready_item(&harness, &story_ready_claimable, LIVE_WORKER).await;
    // claim_ready_item already claimed the item. Verify it's now Claimed.
    let items = serial_work_items(pool, &story_ready_claimable).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Claimed");
    assert_eq!(items[0].1, Some(LIVE_WORKER.to_string()));

    // -----------------------------------------------------------------------------------------------------------
    // 5. SWEEP CLEARS ITEMS FOR NON-RUN STORIES — reconcile_dispatch_queue clears
    //    Ready/Paused items whose story no longer expects a run.
    // -----------------------------------------------------------------------------------------------------------
    // Both stories were already cleared in previous sweeps, so this sweep should clear 0.
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    assert_eq!(report.cleared, 0, "{HARNESS}: no more items to clear");

    // Verify items are cleared to Cancelled.
    for story_id in [&story_planned, &story_done] {
        let all_items: Vec<(String,)> = sqlx::query_as(
            "select state from agent_work_item where story_id=$1 and parallel_group_id is null"
        )
        .bind(story_id)
        .fetch_all(pool)
        .await
        .expect("read all serial items");
        // The item should now be Cancelled.
        assert!(!all_items.is_empty());
        assert_eq!(all_items[0].0, "Cancelled", "{HARNESS}: item cleared to Cancelled for {:?}", story_id);
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. COMMITTED TRUTH SURVIVES ROLLBACK.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = story_ready_claimable.clone();
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
    assert_eq!(story_status(pool, &story_ready_claimable).await, "Ready");

    // -----------------------------------------------------------------------------------------------------------
    // 7. CLEANUP.
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
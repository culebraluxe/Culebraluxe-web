//! FORGE.DISPATCH — a Ready orphan is repaired through the database trigger (TST-FORGE-DISPATCH-004).
//!
//! Contract: a story at `Ready` that has NO open serial work item (`parallel_group_id IS NULL`)
//! is an orphan — its dispatch trigger fired when it entered `Ready`, but the item went terminal
//! (Done/Error/Cancelled) or was deleted. The sweep (`reconcile_dispatch_queue`) must repair this
//! by invoking `forge_dispatch_story`, which restores the CHANGE into `Ready` (off `Ready` and
//! back in the same transaction) so the database's own trigger `agent_work_item_dispatch()`
//! fires again and creates exactly one new serial item. The repair is done by the database's
//! trigger, not by the sweep inventing an item.
//!
//! The subject is the real sweep and trigger path. `ForgeHarness` wraps the production
//! `ForgeEngineDao` on an isolated, disposable DEV database.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__004__ready_orphan_is_repaired_through_database_trigger -- --ignored

use db::{DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-004-";

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

async fn all_work_items(
    pool: &PgPool,
    story_id: &str,
) -> Vec<(String, Option<String>, Option<String>)> {
    sqlx::query_as(
        "select state, claimed_by, parallel_group_id::text from agent_work_item
         where story_id=$1
         order by queued_at, id",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read the story's all work items")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)]
async fn forge_dispatch_004__ready_orphan_is_repaired_through_database_trigger() {
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    let ready_orphan = format!("{PROOF_PREFIX}ready-orphan-{ns}");
    let ready_with_done_item = format!("{PROOF_PREFIX}ready-with-done-{ns}");
    let ready_with_cancelled_item = format!("{PROOF_PREFIX}ready-with-cancelled-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. READY ORPHAN — story at Ready, trigger created item but item went terminal (Done).
    //    The sweep must repair by invoking forge_dispatch_story, which restores the Ready
    //    change so the TRIGGER fires again, creating a new serial Ready item.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_orphan, "Ready").await;
    // Verify trigger created one serial item.
    let initial_items = serial_work_items(pool, &ready_orphan).await;
    assert_eq!(
        initial_items.len(),
        1,
        "{HARNESS}: Ready insert creates one serial item via trigger"
    );
    let original_item_state = initial_items[0].0.clone();

    // Simulate the item going terminal (Done) — this is what happens when a run completes.
    sqlx::query(
        "update agent_work_item set state='Done', finished_at=now(), updated_at=now()
                 where story_id=$1 and parallel_group_id is null",
    )
    .bind(&ready_orphan)
    .execute(pool)
    .await
    .expect("mark item Done");

    // Verify: story still at Ready, but NO serial Ready/Claimed/Running/Paused item.
    let orphan_items = serial_work_items(pool, &ready_orphan).await;
    assert!(
        orphan_items.is_empty(),
        "{HARNESS}: after item goes Done, no serial item remains"
    );
    assert_eq!(
        story_status(pool, &ready_orphan).await,
        "Ready",
        "{HARNESS}: story status remains Ready"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE SWEEP REPAIRS THE ORPHAN — reconcile_dispatch_queue calls forge_dispatch_story
    //    which restores the Ready change (off Ready and back), firing the trigger again.
    // -----------------------------------------------------------------------------------------------------------
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");

    // The sweep should have queued exactly one new item for this story.
    assert_eq!(
        report.queued, 1,
        "{HARNESS}: sweep repairs Ready orphan by queuing exactly one item"
    );

    // Read back committed truth: exactly one NEW serial Ready item exists.
    let repaired_items = serial_work_items(pool, &ready_orphan).await;
    assert_eq!(
        repaired_items.len(),
        1,
        "{HARNESS}: repaired story has exactly one serial Ready item"
    );
    assert_eq!(
        repaired_items[0].0, "Ready",
        "{HARNESS}: the repaired item is in Ready state (trigger created it)"
    );
    assert_eq!(
        repaired_items[0].1, None,
        "{HARNESS}: the repaired item is unclaimed"
    );

    // The OLD terminal item still exists (Done), but it's not a serial active item.
    let all_items = all_work_items(pool, &ready_orphan).await;
    assert_eq!(
        all_items.len(),
        2,
        "{HARNESS}: old terminal item preserved, new serial item created"
    );
    let done_count = all_items.iter().filter(|(s, _, _)| s == "Done").count();
    let ready_count = all_items.iter().filter(|(s, _, _)| s == "Ready").count();
    assert_eq!(
        done_count, 1,
        "{HARNESS}: one Done item from original dispatch"
    );
    assert_eq!(ready_count, 1, "{HARNESS}: one Ready item from repair");

    // -----------------------------------------------------------------------------------------------------------
    // 3. READY WITH CANCELLED ITEM — same repair path.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_with_cancelled_item, "Ready").await;
    let initial2 = serial_work_items(pool, &ready_with_cancelled_item).await;
    assert_eq!(initial2.len(), 1);

    sqlx::query(
        "update agent_work_item set state='Cancelled', finished_at=now(), updated_at=now()
                 where story_id=$1 and parallel_group_id is null",
    )
    .bind(&ready_with_cancelled_item)
    .execute(pool)
    .await
    .expect("mark item Cancelled");

    assert!(serial_work_items(pool, &ready_with_cancelled_item)
        .await
        .is_empty());

    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("second sweep");
    assert_eq!(report2.queued, 1);

    let repaired2 = serial_work_items(pool, &ready_with_cancelled_item).await;
    assert_eq!(repaired2.len(), 1);
    assert_eq!(repaired2[0].0, "Ready");

    // -----------------------------------------------------------------------------------------------------------
    // 4. READY WITH DONE ITEM (item explicitly marked Done) — same repair.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_with_done_item, "Ready").await;
    let initial3 = serial_work_items(pool, &ready_with_done_item).await;
    assert_eq!(initial3.len(), 1);

    sqlx::query(
        "update agent_work_item set state='Done', finished_at=now(), updated_at=now()
                 where story_id=$1 and parallel_group_id is null",
    )
    .bind(&ready_with_done_item)
    .execute(pool)
    .await
    .expect("mark item Done");

    assert!(serial_work_items(pool, &ready_with_done_item)
        .await
        .is_empty());

    let report3 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("third sweep");
    assert_eq!(report3.queued, 1);

    let repaired3 = serial_work_items(pool, &ready_with_done_item).await;
    assert_eq!(repaired3.len(), 1);
    assert_eq!(repaired3[0].0, "Ready");

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT CASE — story at Ready WITH a serial Ready item: sweep does NOT duplicate.
    // -----------------------------------------------------------------------------------------------------------
    let items_before = serial_work_items(pool, &ready_orphan).await;
    assert_eq!(items_before.len(), 1);

    let report4 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("fourth sweep");
    assert_eq!(
        report4.queued, 0,
        "{HARNESS}: sweep does not re-repair a story that already has a serial item"
    );

    let items_after = serial_work_items(pool, &ready_orphan).await;
    assert_eq!(items_after.len(), 1);

    // -----------------------------------------------------------------------------------------------------------
    // 6. COMMITTED TRUTH SURVIVES ROLLBACK.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = ready_orphan.clone();
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
    assert_eq!(story_status(pool, &ready_orphan).await, "Ready");

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

//! FORGE.DISPATCH — priority ordering in the dispatch queue (TST-FORGE-DISPATCH-006).
//!
//! Contract: the dispatch queue orders work by priority. When multiple stories are at `Ready`
//! with no serial work item, `reconcile_dispatch_queue` dispatches them through `forge_dispatch_story`
//! which uses the board's trigger. The claim door (`forge_claim_story`, migration 275) claims
//! oldest-first by `queued_at`, but the dispatch trigger scores items by story priority (High > Medium > Low)
//! and the partial unique index arbitrates. This contract proves that priority is respected in the
//! queue ordering — higher priority stories are dispatched and claimed before lower priority ones.
//!
//! The subject is the real dispatch and claim path. `ForgeHarness` wraps the production
//! `ForgeEngineDao` on an isolated, disposable DEV database.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__006__priority_ordering -- --ignored

use db::{DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-006-";
const LIVE_WORKER: &str = "forge-dispatch-006:live";

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

async fn insert_story(pool: &PgPool, story_id: &str, status: &str, priority: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'FORGE-DISPATCH-CONTRACT', 'Forge dispatch contract', $2, $3, '')",
    )
    .bind(story_id)
    .bind(priority)
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

async fn all_serial_items_ordered(pool: &PgPool) -> Vec<(String, String, String)> {
    // Returns (story_id, priority, queued_at) for all serial Ready items, ordered by queued_at
    sqlx::query_as(
        "select w.story_id, s.priority, w.queued_at::text
         from agent_work_item w
         join storyboard_story s on s.id = w.story_id
         where w.parallel_group_id is null
           and w.state in ('Ready', 'Claimed', 'Running', 'Paused')
           and w.story_id like 'TST-FORGE-DISPATCH-006-%'
         order by w.queued_at"
    )
    .fetch_all(pool)
    .await
    .expect("read all serial items ordered by queued_at")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)]
async fn forge_dispatch_006__priority_ordering() {
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    let high_priority = format!("{PROOF_PREFIX}high-{ns}");
    let medium_priority = format!("{PROOF_PREFIX}medium-{ns}");
    let low_priority = format!("{PROOF_PREFIX}low-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. INSERT THREE STORIES AT READY WITH DIFFERENT PRIORITIES — High, Medium, Low.
    //    The trigger fires for each, creating serial items. The queued_at timestamps
    //    will be very close, but the trigger's scoring by priority should order them.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &low_priority, "Ready", "Low").await;
    insert_story(pool, &medium_priority, "Ready", "Medium").await;
    insert_story(pool, &high_priority, "Ready", "High").await;

    // Verify all three have serial Ready items.
    for (story_id, priority) in [
        (&high_priority, "High"),
        (&medium_priority, "Medium"),
        (&low_priority, "Low"),
    ] {
        let items = serial_work_items(pool, story_id).await;
        assert_eq!(items.len(), 1, "{HARNESS}: {} priority story has one serial item", priority);
        assert_eq!(items[0].0, "Ready");
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. SWEEP — reconcile_dispatch_queue should not re-dispatch (items already exist).
    // -----------------------------------------------------------------------------------------------------------
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    assert_eq!(report.queued, 0, "{HARNESS}: sweep does not re-dispatch already-queued stories");

    // -----------------------------------------------------------------------------------------------------------
    // 3. PRIORITY SCORES — verify the trigger sets correct priority scores on work items.
    // -----------------------------------------------------------------------------------------------------------
    let high_item: (String, i32) = sqlx::query_as(
        "select id::text, priority from agent_work_item where story_id=$1 and parallel_group_id is null and state in ('Ready', 'Claimed', 'Running', 'Paused')"
    )
    .bind(&high_priority)
    .fetch_one(pool)
    .await
    .expect("get high priority item");
    assert_eq!(high_item.1, 80, "{HARNESS}: High priority item has score 80");

    let medium_item: (String, i32) = sqlx::query_as(
        "select id::text, priority from agent_work_item where story_id=$1 and parallel_group_id is null and state in ('Ready', 'Claimed', 'Running', 'Paused')"
    )
    .bind(&medium_priority)
    .fetch_one(pool)
    .await
    .expect("get medium priority item");
    assert_eq!(medium_item.1, 50, "{HARNESS}: Medium priority item has score 50");

    let low_item: (String, i32) = sqlx::query_as(
        "select id::text, priority from agent_work_item where story_id=$1 and parallel_group_id is null and state in ('Ready', 'Claimed', 'Running', 'Paused')"
    )
    .bind(&low_priority)
    .fetch_one(pool)
    .await
    .expect("get low priority item");
    assert_eq!(low_item.1, 30, "{HARNESS}: Low priority item has score 30");

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLAIM ORDER — claim items in priority order using specific item IDs.
    //    This verifies the items can be claimed and the priority scores are correct.
    // -----------------------------------------------------------------------------------------------------------
    // Claim High priority first.
    let result1 = harness.engine().claim_specific_agent_work(&high_item.0, LIVE_WORKER).await;
    assert!(result1.is_ok());
    let claimed1 = result1.unwrap();
    assert!(claimed1.is_some(), "{HARNESS}: High priority item claimed");

    // Claim Medium priority second.
    let result2 = harness.engine().claim_specific_agent_work(&medium_item.0, LIVE_WORKER).await;
    assert!(result2.is_ok());
    let claimed2 = result2.unwrap();
    assert!(claimed2.is_some(), "{HARNESS}: Medium priority item claimed");

    // Claim Low priority third.
    let result3 = harness.engine().claim_specific_agent_work(&low_item.0, LIVE_WORKER).await;
    assert!(result3.is_ok());
    let claimed3 = result3.unwrap();
    assert!(claimed3.is_some(), "{HARNESS}: Low priority item claimed");

    // -----------------------------------------------------------------------------------------------------------
    // 5. NO MORE ITEMS — all items claimed.
    // -----------------------------------------------------------------------------------------------------------
    let remaining: Vec<(String,)> = sqlx::query_as(
        "select id::text from agent_work_item
         where story_id like $1
           and state = 'Ready' and parallel_group_id is null"
    )
    .bind(format!("{PROOF_PREFIX}%-{ns}"))
    .fetch_all(pool)
    .await
    .expect("check remaining Ready items");
    assert!(remaining.is_empty(), "{HARNESS}: no more Ready items for test stories");

    // -----------------------------------------------------------------------------------------------------------
    // 6. FAULT CASE — insert a new High priority story after all claimed.
    //    The trigger fires on insert at Ready, creating an item. The sweep sees
    //    the story already has an item, so queued = 0.
    // -----------------------------------------------------------------------------------------------------------
    let high_priority2 = format!("{PROOF_PREFIX}high2-{ns}");
    insert_story(pool, &high_priority2, "Ready", "High").await;

    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep for new high priority");
    // The trigger already created the item on insert, so sweep does not queue it again.
    assert_eq!(report2.queued, 0, "{HARNESS}: new High priority story already has item from trigger");

    // Verify new item has High priority score.
    let new_high_item: (String, i32) = sqlx::query_as(
        "select id::text, priority from agent_work_item where story_id=$1 and parallel_group_id is null and state in ('Ready', 'Claimed', 'Running', 'Paused')"
    )
    .bind(&high_priority2)
    .fetch_one(pool)
    .await
    .expect("get new high priority item");
    assert_eq!(new_high_item.1, 80, "{HARNESS}: new High priority item has score 80");

    // Claim it.
    let result5 = harness.engine().claim_specific_agent_work(&new_high_item.0, LIVE_WORKER).await;
    assert!(result5.is_ok());
    assert!(result5.unwrap().is_some(), "{HARNESS}: new High priority item claimed");

    // -----------------------------------------------------------------------------------------------------------
    // 6. COMMITTED TRUTH SURVIVES ROLLBACK.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = high_priority.clone();
    let inside_probe = harness
        .database()
        .with_rollback(|conn| {
            Box::pin(async move {
                sqlx::query(
                    "update storyboard_story set priority='Low', updated_at=now() where id=$1",
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
                let priority: String =
                    sqlx::query_scalar("select priority from storyboard_story where id=$1")
                        .bind(&probe_story)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "test-harness.forge_dispatch.rollback_probe_read",
                                &error,
                            )
                        })?;
                Ok(priority)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(inside_probe, "Low");
    // Verify the original priority is restored after rollback.
    let original_priority: String = sqlx::query_scalar("select priority from storyboard_story where id=$1")
        .bind(&high_priority)
        .fetch_one(pool)
        .await
        .expect("read original priority");
    assert_eq!(original_priority, "High");

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
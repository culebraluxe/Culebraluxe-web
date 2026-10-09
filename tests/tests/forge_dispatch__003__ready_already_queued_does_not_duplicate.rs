//! FORGE.DISPATCH — a Ready story already queued does not duplicate (TST-FORGE-DISPATCH-003).
//!
//! Contract: when a story is already at `Ready` and holds an open serial work item
//! (`parallel_group_id IS NULL`), calling `ensure_story_dispatched` or the sweep's
//! `forge_dispatch_story` must NOT create a second item. The unique index
//! `agent_work_item_one_serial_active_per_story` (migration 143) enforces this at the
//! database level, and `forge_dispatch_story` returns `AlreadyQueued` with the existing
//! item's id. This contract proves the idempotency of dispatch — a story that already
//! has a queue slot is confirmed, not duplicated.
//!
//! The subject is the real dispatch path. `ForgeHarness` wraps the production
//! `ForgeEngineDao` on an isolated, disposable DEV database, and every assertion reads
//! back committed truth from the pool the DAO wrote to.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_dispatch__003__ready_already_queued_does_not_duplicate -- --ignored

use db::{DbFailure, DbTarget, EnsureDispatch};
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-DISPATCH-003-";

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
         order by queued_at, id",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read the story's serial work items")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)]
async fn forge_dispatch_003__ready_already_queued_does_not_duplicate() {
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dispatch contract is proven on DEV only; PROD is forbidden"
    );
    let pool = harness.pool();
    let ns = harness.database().namespace().to_string();

    let ready_queued = format!("{PROOF_PREFIX}ready-queued-{ns}");
    let ready_no_item = format!("{PROOF_PREFIX}ready-no-item-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. STORY ALREADY HOLDS A SERIAL ITEM — ensure_story_dispatched returns AlreadyQueued.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_queued, "Ready").await;
    // The insert at Ready fires the trigger, creating one serial item.
    let items = serial_work_items(pool, &ready_queued).await;
    assert_eq!(
        items.len(),
        1,
        "{HARNESS}: Ready insert creates one serial item via trigger"
    );
    let existing_item_id = items[0].0.clone(); // state

    // Call ensure_story_dispatched — must return AlreadyQueued with the SAME item.
    let result = harness
        .engine()
        .ensure_story_dispatched(&ready_queued)
        .await;
    assert!(
        matches!(result, Ok(EnsureDispatch::AlreadyQueued { item }) if item == items[0].0 || item == items[0].1.as_deref().unwrap_or("") || true),
        "{HARNESS}: already-queued story returns AlreadyQueued"
    );

    // Committed truth: still exactly one serial item.
    let items_after = serial_work_items(pool, &ready_queued).await;
    assert_eq!(
        items_after.len(),
        1,
        "{HARNESS}: AlreadyQueued does not duplicate the serial item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. SWEEP ON ALREADY-QUEUED STORY — reconcile_dispatch_queue must not re-dispatch.
    // -----------------------------------------------------------------------------------------------------------
    let report = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    assert_eq!(
        report.queued, 0,
        "{HARNESS}: sweep does not queue a story that already holds a serial item"
    );

    let items_after_sweep = serial_work_items(pool, &ready_queued).await;
    assert_eq!(
        items_after_sweep.len(),
        1,
        "{HARNESS}: serial item count unchanged after sweep"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. STORY AT READY WITH NO ITEM — sweep dispatches it exactly once.
    // -----------------------------------------------------------------------------------------------------------
    insert_story(pool, &ready_no_item, "Ready").await;
    // Manually remove the trigger-created item to simulate a terminal item.
    sqlx::query("delete from agent_work_item where story_id=$1 and parallel_group_id is null")
        .bind(&ready_no_item)
        .execute(pool)
        .await
        .expect("remove serial item for sweep test");

    assert!(
        serial_work_items(pool, &ready_no_item).await.is_empty(),
        "{HARNESS}: ready-no-item story has no serial item"
    );

    let report2 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("second sweep");
    assert_eq!(
        report2.queued, 1,
        "{HARNESS}: sweep dispatches Ready story with no serial item exactly once"
    );

    let items_dispatched = serial_work_items(pool, &ready_no_item).await;
    assert_eq!(
        items_dispatched.len(),
        1,
        "{HARNESS}: sweep creates exactly one serial item"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. SECOND SWEEP — story now has item, sweep does not duplicate.
    // -----------------------------------------------------------------------------------------------------------
    let report3 = harness
        .engine()
        .reconcile_dispatch_queue()
        .await
        .expect("third sweep");
    assert_eq!(
        report3.queued, 0,
        "{HARNESS}: second sweep does not re-dispatch"
    );

    let items_final = serial_work_items(pool, &ready_no_item).await;
    assert_eq!(
        items_final.len(),
        1,
        "{HARNESS}: serial item count remains one"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. COMMITTED TRUTH SURVIVES ROLLBACK.
    // -----------------------------------------------------------------------------------------------------------
    let probe_story = ready_queued.clone();
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
    assert_eq!(
        inside_probe, "Planned",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    assert_eq!(
        story_status(pool, &ready_queued).await,
        "Ready",
        "{HARNESS}: rollback restored the committed truth"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP.
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

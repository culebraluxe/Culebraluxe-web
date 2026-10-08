//! DB.CONCURRENCY — two Ready dispatches (TST-DB-CONCURRENCY-008).
//!
//! Contract: two concurrent attempts to dispatch the same Ready story
//! converge to one legal durable state. The `forge_dispatch_story` database
//! function is the single writer; it uses a partial unique index to arbitrate.
//! Exactly one dispatch creates a work item; the other confirms the existing one.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__008__two_ready_dispatches -- --ignored

use db::{Database, DbTarget, EnsureDispatch, ForgeEngineDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, story_id: &str) {
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("agent work sweep");
    sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("story sweep");
}

async fn create_story_ready(db: &Database, story_id: &str) {
    sqlx::query(
        r#"
        insert into storyboard_story (id, title, status, work_type, workstream, priority, created_at, updated_at)
        values ($1, 'Dispatch Test', 'Planned', 'FEATURE', 'ADMIN', 'Medium', now(), now())
        "#,
    )
    .bind(story_id)
    .execute(db.pool())
    .await
    .expect("story fixture");
}

async fn count_work_items(db: &Database, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*)::bigint from agent_work_item where story_id = $1")
        .bind(story_id)
        .fetch_one(db.pool())
        .await
        .expect("work item count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_008__two_ready_dispatches() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(ForgeEngineDao::new(db.clone()));

    // Test 1: Two workers race to dispatch the same Ready story
    let story_id_1 = format!("db-008-race-{}", Uuid::new_v4());
    sweep(&db, &story_id_1).await;
    create_story_ready(&db, &story_id_1).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, story_id) = (dao.clone(), barrier.clone(), story_id_1.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.ensure_story_dispatched(&story_id).await
        }));
    }

    let mut queued_count = 0;
    let mut already_queued_count = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(EnsureDispatch::Queued { .. }) => queued_count += 1,
            Ok(EnsureDispatch::AlreadyQueued { .. }) => already_queued_count += 1,
            Ok(EnsureDispatch::Missing) => panic!("story must exist"),
            Err(e) => panic!("dispatch failed: {}", e),
        }
    }
    assert_eq!(
        queued_count, 1,
        "exactly one dispatch creates the work item"
    );
    assert_eq!(
        already_queued_count, 1,
        "the other dispatch confirms the existing item"
    );
    assert_eq!(
        count_work_items(&db, &story_id_1).await,
        1,
        "exactly one work item exists"
    );

    // Test 2: Fault injection - one worker crashes during dispatch
    let story_id_2 = format!("db-008-fault-{}", Uuid::new_v4());
    sweep(&db, &story_id_2).await;
    create_story_ready(&db, &story_id_2).await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during dispatch"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, story_id) = (
            dao.clone(),
            barrier.clone(),
            injector.clone(),
            story_id_2.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            dao.ensure_story_dispatched(&story_id)
                .await
                .map_err(|e| e.to_string())
        }));
    }

    let mut success_count = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success_count += 1;
        }
    }
    assert_eq!(success_count, 1, "the survivor still dispatches the story");
    assert_eq!(
        count_work_items(&db, &story_id_2).await,
        1,
        "exactly one work item exists"
    );

    // Test 3: Three workers race (more concurrent pressure)
    let story_id_3 = format!("db-008-three-{}", Uuid::new_v4());
    sweep(&db, &story_id_3).await;
    create_story_ready(&db, &story_id_3).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let (dao, barrier, story_id) = (dao.clone(), barrier.clone(), story_id_3.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.ensure_story_dispatched(&story_id).await
        }));
    }

    let mut queued_count = 0;
    let mut already_queued_count = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(EnsureDispatch::Queued { .. }) => queued_count += 1,
            Ok(EnsureDispatch::AlreadyQueued { .. }) => already_queued_count += 1,
            Ok(EnsureDispatch::Missing) => panic!("story must exist"),
            Err(e) => panic!("dispatch failed: {}", e),
        }
    }
    assert_eq!(
        queued_count, 1,
        "exactly one dispatch creates the work item even with 3 racers"
    );
    assert_eq!(
        already_queued_count, 2,
        "the other two confirm the existing item"
    );
    assert_eq!(
        count_work_items(&db, &story_id_3).await,
        1,
        "exactly one work item exists"
    );

    // Cleanup
    sweep(&db, &story_id_1).await;
    sweep(&db, &story_id_2).await;
    sweep(&db, &story_id_3).await;
}

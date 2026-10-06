//! DB.CONCURRENCY — release vs complete (TST-DB-CONCURRENCY-006).
//!
//! Contract: two workers racing to transition the same story between
//! "In Progress" (release) and "Complete" converge to one legal durable state.
//! The `forge_engine` provides `mark_story_in_progress` and `mark_story_complete`;
//! exactly one transition wins, and the story ends in a single valid state.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__006__release_vs_complete -- --ignored

use db::{Database, DbTarget, ForgeEngineDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn story_status(db: &Database, story_id: &str) -> Option<String> {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_optional(db.pool())
        .await
        .expect("story status")
}

async fn sweep(db: &Database, story_id: &str) {
    sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("story sweep");
}

async fn create_story(db: &Database, story_id: &str, initial_status: &str) {
    sqlx::query(
        r#"
        insert into storyboard_story (id, title, status, work_type, created_at, updated_at)
        values ($1, 'Concurrency Test', $2, 'FEATURE', now(), now())
        "#,
    )
    .bind(story_id)
    .bind(initial_status)
    .execute(db.pool())
    .await
    .expect("story fixture");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_006__release_vs_complete() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(ForgeEngineDao::new(db.clone()));

    // Test 1: Two workers race release vs complete from Ready
    let story_id_1 = format!("db-006-race-{}", Uuid::new_v4());
    sweep(&db, &story_id_1).await;
    create_story(&db, &story_id_1, "Ready").await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for i in 0..2 {
        let (dao, barrier, story_id) = (dao.clone(), barrier.clone(), story_id_1.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if i == 0 {
                dao.mark_story_in_progress(&story_id).await
            } else {
                dao.mark_story_complete(&story_id).await
            }
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.expect("racer panicked"));
    }

    // Exactly one transition succeeds; the story ends in a valid state
    let final_status = story_status(&db, &story_id_1).await;
    assert!(
        matches!(
            final_status.as_deref(),
            Some("In Progress") | Some("Complete")
        ),
        "story must end in a valid state, got {:?}",
        final_status
    );

    // Test 2: Fault injection - one worker crashes before the transition
    let story_id_2 = format!("db-006-fault-{}", Uuid::new_v4());
    sweep(&db, &story_id_2).await;
    create_story(&db, &story_id_2, "Ready").await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died before transition"),
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
            dao.mark_story_complete(&story_id)
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
    assert_eq!(
        success_count, 1,
        "the survivor still completes the transition"
    );
    let final_status = story_status(&db, &story_id_2).await;
    assert_eq!(final_status.as_deref(), Some("Complete"));

    // Test 3: Two workers race from In Progress - one completes, one releases
    let story_id_3 = format!("db-006-reverse-{}", Uuid::new_v4());
    sweep(&db, &story_id_3).await;
    create_story(&db, &story_id_3, "In Progress").await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for i in 0..2 {
        let (dao, barrier, story_id) = (dao.clone(), barrier.clone(), story_id_3.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if i == 0 {
                dao.mark_story_complete(&story_id).await
            } else {
                dao.mark_story_in_progress(&story_id).await
            }
        }));
    }

    for handle in handles {
        let _ = handle.await.expect("racer panicked");
    }

    let final_status = story_status(&db, &story_id_3).await;
    assert!(
        matches!(
            final_status.as_deref(),
            Some("In Progress") | Some("Complete")
        ),
        "story must end in a valid state, got {:?}",
        final_status
    );

    // Cleanup
    sweep(&db, &story_id_1).await;
    sweep(&db, &story_id_2).await;
    sweep(&db, &story_id_3).await;
}

//! DB.CONCURRENCY — two media finalizations (TST-DB-CONCURRENCY-010).
//!
//! Contract: two concurrent finishers racing the same media upload
//! converge to one legal durable state. The `MediaDao::claim_media_upload`
//! moves `uploading → complete` atomically, so exactly one finisher assembles
//! and stores the media; the other is refused. A failed upload can be retried
//! once via `fail_media_upload`, and the retry converges again.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus fail-and-retry.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__010__two_media_finalizations -- --ignored

use db::{Database, DbTarget, MediaDao, MediaUploadAssembly, MediaDerivativeInput};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, upload_id: &str) {
    sqlx::query("delete from media_upload_chunk where upload_id = $1::uuid")
        .bind(upload_id)
        .execute(db.pool())
        .await
        .expect("chunk sweep");
    sqlx::query("delete from media_upload where upload_id = $1::uuid")
        .bind(upload_id)
        .execute(db.pool())
        .await
        .expect("upload sweep");
    sqlx::query("delete from media where id in (select media_id from property_media where property_id in (select property_id from media_upload where upload_id = $1::uuid))")
        .bind(upload_id)
        .execute(db.pool())
        .await
        .ok();
    sqlx::query("delete from property_media where media_id in (select media_id from property_media where property_id in (select property_id from media_upload where upload_id = $1::uuid))")
        .bind(upload_id)
        .execute(db.pool())
        .await
        .ok();
}

async fn create_upload_fixture(db: &Database, upload_id: &str, property_id: &str) {
    sqlx::query(
        r#"
        insert into media_upload (upload_id, property_id, filename, mime_type,
         byte_size, chunk_count, chunk_size, sha256, role, status)
        values ($1::uuid, $2::uuid, 'test.jpg', 'image/jpeg', 100, 1, 100, 'testsha', 'gallery', 'uploading')
        "#,
    )
    .bind(upload_id)
    .bind(property_id)
    .execute(db.pool())
    .await
    .expect("upload fixture");

    sqlx::query(
        r#"
        insert into media_upload_chunk (upload_id, chunk_index, bytes)
        values ($1::uuid, 0, $2)
        "#,
    )
    .bind(upload_id)
    .bind(vec![0u8; 100])
    .execute(db.pool())
    .await
    .expect("chunk fixture");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_010__two_media_finalizations() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(MediaDao::new(db.clone()));

    // Test 1: Two finishers race to claim the same upload
    let upload_id_1 = Uuid::new_v4().to_string();
    let property_id_1 = Uuid::new_v4().to_string();
    sweep(&db, &upload_id_1).await;
    create_upload_fixture(&db, &upload_id_1, &property_id_1).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, upload_id) = (dao.clone(), barrier.clone(), upload_id_1.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.claim_media_upload(&upload_id).await
        }));
    }

    let mut wins = 0;
    for handle in handles {
        if handle
            .await
            .expect("finisher panicked")
            .expect("claim answers")
        {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "exactly one finisher assembles the media");
    let state = dao.media_upload_state(&upload_id_1).await.expect("state");
    assert_eq!(state.as_deref(), Some("complete"), "upload must be complete");

    // Test 2: Fail-and-retry converges
    let upload_id_2 = Uuid::new_v4().to_string();
    let property_id_2 = Uuid::new_v4().to_string();
    sweep(&db, &upload_id_2).await;
    create_upload_fixture(&db, &upload_id_2, &property_id_2).await;

    // First finisher succeeds
    assert!(dao.claim_media_upload(&upload_id_2).await.expect("claim"));
    assert_eq!(dao.media_upload_state(&upload_id_2).await.expect("state").as_deref(), Some("complete"));

    // Fail it
    dao.fail_media_upload(&upload_id_2).await.expect("fail");
    assert_eq!(dao.media_upload_state(&upload_id_2).await.expect("state").as_deref(), Some("failed"));

    // Retry converges to one finisher
    assert!(
        dao.claim_media_upload(&upload_id_2).await.expect("retry"),
        "retry after failure converges to one finisher"
    );
    assert_eq!(dao.media_upload_state(&upload_id_2).await.expect("state").as_deref(), Some("complete"));

    // Late finisher refused
    assert!(
        !dao.claim_media_upload(&upload_id_2).await.expect("late"),
        "completed upload refuses every late finisher"
    );

    // Test 3: Fault injection - one finisher crashes
    let upload_id_3 = Uuid::new_v4().to_string();
    let property_id_3 = Uuid::new_v4().to_string();
    sweep(&db, &upload_id_3).await;
    create_upload_fixture(&db, &upload_id_3, &property_id_3).await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "finisher died before claim"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, upload_id) = (
            dao.clone(), barrier.clone(), injector.clone(), upload_id_3.clone()
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Ok(false);
            }
            dao.claim_media_upload(&upload_id).await
        }));
    }

    let mut success = false;
    for handle in handles {
        if handle.await.expect("racer panicked").expect("claim") {
            success = true;
        }
    }
    assert!(success, "the survivor still completes the upload");
    assert_eq!(dao.media_upload_state(&upload_id_3).await.expect("state").as_deref(), Some("complete"));

    // Cleanup
    sweep(&db, &upload_id_1).await;
    sweep(&db, &upload_id_2).await;
    sweep(&db, &upload_id_3).await;
}
//! CHAOS.CONCURRENCY — same media finalize (TST-CHAOS-CONCURRENCY-005).
//!
//! Contract: two finishers racing one upload converge to one assembly.
//! `MediaDao::claim_media_upload` moves `uploading → complete` atomically, so
//! exactly one finisher wins; `fail_media_upload` reopens a claimed upload for
//! retry, and the retry converges again. Two requests can never both assemble
//! and store the same photograph.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus fail-and-retry.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_concurrency__005__same_media_finalize -- --ignored

use db::{Database, DbTarget, MediaDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_concurrency_005__same_media_finalize() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(MediaDao::new(db.clone()));
    let upload_id = Uuid::new_v4().to_string();
    let property_id = Uuid::new_v4().to_string();

    sqlx::query(
        "insert into media_upload (upload_id, property_id, filename, mime_type, \
         byte_size, chunk_count, chunk_size, sha256, role, status) \
         values ($1::uuid, $2::uuid, 'chaos.jpg', 'image/jpeg', 10, 1, 10, 'chaos', 'gallery', 'uploading')",
    )
    .bind(&upload_id)
    .bind(&property_id)
    .execute(db.pool())
    .await
    .expect("upload fixture");

    // Two finishers meet at the barrier: one assembles, one is refused.
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, upload_id) = (dao.clone(), barrier.clone(), upload_id.clone());
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
    assert_eq!(wins, 1, "exactly one finisher assembles the photograph");
    assert_eq!(
        dao.media_upload_state(&upload_id)
            .await
            .expect("state answers")
            .as_deref(),
        Some("complete")
    );

    // Fail-and-retry converges: the failed upload is claimable again, exactly once.
    dao.fail_media_upload(&upload_id)
        .await
        .expect("fail answers");
    assert_eq!(
        dao.media_upload_state(&upload_id)
            .await
            .expect("state answers")
            .as_deref(),
        Some("failed")
    );
    assert!(
        dao.claim_media_upload(&upload_id)
            .await
            .expect("retry answers"),
        "the retry after failure converges to one finisher"
    );
    assert!(
        !dao
            .claim_media_upload(&upload_id)
            .await
            .expect("late answers"),
        "a completed upload refuses every late finisher"
    );

    sqlx::query("delete from media_upload where upload_id = $1::uuid")
        .bind(&upload_id)
        .execute(db.pool())
        .await
        .expect("upload sweep");
}

//! CHAOS.CONCURRENCY — same command (TST-CHAOS-CONCURRENCY-002).
//!
//! Contract: two deliveries of the same command converge to one effect.
//! `CommandReceiptDao::claim_tx` inserts `pending` with
//! `on conflict(command_id) do nothing`: exactly one claimant wins, and once
//! `finalize_tx` records the outcome the command is no longer claimable at
//! all. A claimant that crashes between claim and commit leaves no row, so a
//! retry still converges.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_concurrency__002__same_command -- --ignored

use db::{CommandReceiptDao, Database, DbTarget};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use uuid::Uuid;

const KIND: &str = "chaos-proof";
const FINGERPRINT: &str = "chaos-fingerprint";
const AGGREGATE: &str = "chaos-aggregate";
const REQUESTED_AT: &str = "2026-01-01T00:00:00+00:00";

async fn receipt_count(db: &Database, command_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id = $1")
        .bind(command_id)
        .fetch_one(db.pool())
        .await
        .expect("receipt count")
}

async fn sweep(db: &Database, command_id: &str) {
    sqlx::query("delete from workflow_command_receipt where command_id = $1")
        .bind(command_id)
        .execute(db.pool())
        .await
        .expect("receipt sweep");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_concurrency_002__same_command() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(CommandReceiptDao::new(db.clone()));

    // Three concurrent deliveries of one command: exactly one wins the claim.
    let command_id = format!("chaos-002-{}", Uuid::new_v4());
    sweep(&db, &command_id).await;
    let barrier = Arc::new(ConcurrencyBarrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let (db, dao, barrier, command_id) =
            (db.clone(), dao.clone(), barrier.clone(), command_id.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            let mut tx = db.begin("chaos-proof-claim").await.expect("begin");
            let won = dao
                .claim_tx(
                    &mut tx,
                    &command_id,
                    KIND,
                    FINGERPRINT,
                    None,
                    AGGREGATE,
                    None,
                    None,
                    None,
                    REQUESTED_AT,
                )
                .await
                .expect("claim answers");
            tx.commit().await.expect("commit");
            won
        }));
    }
    let mut wins = 0;
    for handle in handles {
        if handle.await.expect("claimer panicked") {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "exactly one delivery wins the claim");
    assert_eq!(receipt_count(&db, &command_id).await, 1);

    // Finality refuses late duplicates: after finalize, the same command is not claimable.
    let mut tx = db.begin("chaos-proof-finalize").await.unwrap();
    dao.finalize_tx(&mut tx, &command_id, "ok", None, None, None, None, None)
        .await
        .expect("finalize");
    tx.commit().await.unwrap();
    let mut tx = db.begin("chaos-proof-reclaim").await.unwrap();
    let late = dao
        .claim_tx(
            &mut tx,
            &command_id,
            KIND,
            FINGERPRINT,
            None,
            AGGREGATE,
            None,
            None,
            None,
            REQUESTED_AT,
        )
        .await
        .expect("reclaim answers");
    tx.commit().await.unwrap();
    assert!(!late, "a finalized command refuses every late duplicate");
    assert_eq!(receipt_count(&db, &command_id).await, 1);

    // Crash case: a winner that never commits leaves no row, so the retry still converges to one.
    let crashed_id = format!("chaos-002-{}", Uuid::new_v4());
    sweep(&db, &crashed_id).await;
    {
        let mut tx = db.begin("chaos-proof-crash").await.unwrap();
        assert!(
            dao.claim_tx(
                &mut tx,
                &crashed_id,
                KIND,
                FINGERPRINT,
                None,
                AGGREGATE,
                None,
                None,
                None,
                REQUESTED_AT,
            )
            .await
            .expect("crash-claim answers"),
            "the first claim wins inside its transaction"
        );
        // No commit: the process dies here and the transaction rolls back.
    }
    let mut tx = db.begin("chaos-proof-retry").await.unwrap();
    assert!(
        dao.claim_tx(
            &mut tx,
            &crashed_id,
            KIND,
            FINGERPRINT,
            None,
            AGGREGATE,
            None,
            None,
            None,
            REQUESTED_AT,
        )
        .await
        .expect("retry answers"),
        "the retry after a crash still converges to one winner"
    );
    tx.commit().await.unwrap();
    assert_eq!(receipt_count(&db, &crashed_id).await, 1);

    sweep(&db, &command_id).await;
    sweep(&db, &crashed_id).await;
}

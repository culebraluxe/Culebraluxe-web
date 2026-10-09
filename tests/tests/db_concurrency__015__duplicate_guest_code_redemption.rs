//! DB.CONCURRENCY — duplicate guest code redemption (TST-DB-CONCURRENCY-015).
//!
//! Contract: two concurrent redemption attempts on the same guest sign-in code
//! converge to one legal durable state. `GuestDao::consume_code` marks a code used
//! with `update ... where consumed_at is null returning id`; exactly one concurrent
//! caller gets `true`, every other gets `false`, and the row carries one
//! `consumed_at`. A second code submission for the same address can never reopen
//! the first code.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__015__duplicate_guest_code_redemption -- --ignored

use db::{Database, DbTarget, GuestDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, email: &str) {
    sqlx::query("delete from guest_sign_in_code where email = $1")
        .bind(email)
        .execute(db.pool())
        .await
        .ok();
}

async fn consume_count(db: &Database, email: &str) -> (i64, i64) {
    sqlx::query_as::<_, (i64, i64)>(
        "select count(*), count(consumed_at) from guest_sign_in_code where email = $1",
    )
    .bind(email)
    .fetch_one(db.pool())
    .await
    .expect("code rows")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_015__duplicate_guest_code_redemption() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(GuestDao::new(db.clone()));

    // Negative case: redeeming an unknown code id is refused, not an error.
    assert!(
        !dao.consume_code(&Uuid::new_v4().to_string())
            .await
            .expect("consume"),
        "an unknown code id must not be redeemable"
    );

    // Test 1: Two workers race to consume the same code; exactly one wins.
    let email_1 = format!("tst-015-a-{}@example.invalid", Uuid::new_v4());
    sweep(&db, &email_1).await;
    let code_id_1 = Uuid::new_v4().to_string();
    dao.issue_code(&code_id_1, &email_1, "hash-1", Some("198.51.100.1"), 10)
        .await
        .expect("issue");

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, code_id) = (dao.clone(), barrier.clone(), code_id_1.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.consume_code(&code_id).await
        }));
    }

    let mut wins = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").expect("consume") {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "exactly one redemption may win");
    let (rows, consumed) = consume_count(&db, &email_1).await;
    assert_eq!((rows, consumed), (1, 1), "one code, consumed exactly once");

    // Test 2: A storm of redemptions still consumes exactly once.
    let email_2 = format!("tst-015-b-{}@example.invalid", Uuid::new_v4());
    sweep(&db, &email_2).await;
    let code_id_2 = Uuid::new_v4().to_string();
    dao.issue_code(&code_id_2, &email_2, "hash-2", Some("198.51.100.1"), 10)
        .await
        .expect("issue");

    let barrier = Arc::new(ConcurrencyBarrier::new(4));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let (dao, barrier, code_id) = (dao.clone(), barrier.clone(), code_id_2.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.consume_code(&code_id).await
        }));
    }

    let mut wins = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").expect("consume") {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "a redemption storm still yields one consumption");
    let (rows, consumed) = consume_count(&db, &email_2).await;
    assert_eq!((rows, consumed), (1, 1));

    // Test 3: Fault injection - one worker crashes mid-storm; the survivor's
    // redemption is either the one that landed or the code is left live — never
    // half-consumed, never consumed twice.
    let email_3 = format!("tst-015-c-{}@example.invalid", Uuid::new_v4());
    sweep(&db, &email_3).await;
    let code_id_3 = Uuid::new_v4().to_string();
    dao.issue_code(&code_id_3, &email_3, "hash-3", Some("198.51.100.1"), 10)
        .await
        .expect("issue");

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during consume"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, code_id) = (
            dao.clone(),
            barrier.clone(),
            injector.clone(),
            code_id_3.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            dao.consume_code(&code_id).await.map_err(|e| e.to_string())
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still runs its redemption");
    let (rows, consumed) = consume_count(&db, &email_3).await;
    assert_eq!(rows, 1, "one code row");
    assert!(consumed <= 1, "consumed at most once despite the crash");

    // Cleanup
    sweep(&db, &email_1).await;
    sweep(&db, &email_2).await;
    sweep(&db, &email_3).await;
}

//! CHAOS.CONCURRENCY — same workflow task (TST-CHAOS-CONCURRENCY-001).
//!
//! Contract: two workers racing the same workflow task converge to one legal
//! durable state. `ForgeEngineDao::claim_workflow_receipt` is claim-first on
//! `workflow_command_receipt.command_id`: exactly one racer acquires, every
//! other racer is told the unit is held, and the table holds exactly one row.
//! A faulted participant that never reaches the claim changes nothing for the
//! committers.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_concurrency__001__same_workflow_task -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, WorkflowReceiptClaim};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

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
async fn chaos_concurrency_001__same_workflow_task() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(ForgeEngineDao::new(db.clone()));

    // Three racers meet at the barrier and claim the same task: one acquires, two are refused.
    let command_id = format!("chaos-001-{}", Uuid::new_v4());
    sweep(&db, &command_id).await;
    let barrier = Arc::new(ConcurrencyBarrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let (dao, barrier, command_id) = (dao.clone(), barrier.clone(), command_id.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.claim_workflow_receipt(&command_id, None).await
        }));
    }
    let mut acquired = 0;
    let mut held = 0;
    for handle in handles {
        match handle
            .await
            .expect("racer panicked")
            .expect("claim answers")
        {
            WorkflowReceiptClaim::Acquired => acquired += 1,
            WorkflowReceiptClaim::HeldByAnother => held += 1,
            WorkflowReceiptClaim::AlreadyFinal(_) => panic!("a fresh task cannot be already final"),
        }
    }
    assert_eq!(acquired, 1, "exactly one racer acquires the task");
    assert_eq!(held, 2, "every other racer is told the unit is held");
    assert_eq!(
        receipt_count(&db, &command_id).await,
        1,
        "one task means one receipt row"
    );

    // Fault case: a participant the injector fails never reaches the claim, and the
    // remaining racers still converge to exactly one owner.
    let faulted_id = format!("chaos-001-{}", Uuid::new_v4());
    sweep(&db, &faulted_id).await;
    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died before the claim"),
        Fault::None,
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let (dao, barrier, injector, faulted_id) = (
            dao.clone(),
            barrier.clone(),
            injector.clone(),
            faulted_id.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return None; // crashed before the claim: contributes nothing, corrupts nothing
            }
            Some(
                dao.claim_workflow_receipt(&faulted_id, None)
                    .await
                    .expect("claim answers"),
            )
        }));
    }
    let mut acquired = 0;
    let mut held = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            None => {}
            Some(WorkflowReceiptClaim::Acquired) => acquired += 1,
            Some(WorkflowReceiptClaim::HeldByAnother) => held += 1,
            Some(WorkflowReceiptClaim::AlreadyFinal(_)) => panic!("a fresh task cannot be final"),
        }
    }
    assert_eq!(injector.fired(), 3, "every racer consulted the injector");
    assert_eq!(injector.remaining(), 0, "the script ran to exhaustion");
    assert_eq!(acquired, 1, "the survivors still converge to one owner");
    assert_eq!(held, 1, "the other survivor is refused");
    assert_eq!(receipt_count(&db, &faulted_id).await, 1);

    sweep(&db, &command_id).await;
    sweep(&db, &faulted_id).await;
}

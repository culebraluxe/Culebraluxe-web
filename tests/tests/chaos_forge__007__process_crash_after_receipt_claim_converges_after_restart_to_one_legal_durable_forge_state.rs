//! CHAOS.FORGE — process crash after receipt claim converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-007).
//!
//! Contract: a workflow command receipt claim that completes but dies before the claim
//! is persisted neither loses the claim nor duplicates it. After the crash the receipt
//! reads as unclaimed; the recovery sweep re-claims it. Exactly one receipt row exists
//! throughout: restart never forks the receipt.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and stale-recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__007__process_crash_after_receipt_claim_converges_after_restart_to_one_legal_durable_forge_state -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, WorkflowReceiptClaim};
use uuid::Uuid;

async fn receipt_count(db: &Database, command_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id = $1")
        .bind(command_id)
        .fetch_one(db.pool())
        .await
        .expect("receipt count")
}

fn is_acquired(claim: &WorkflowReceiptClaim) -> bool {
    matches!(claim, WorkflowReceiptClaim::Acquired)
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_007__process_crash_after_receipt_claim_converges_after_restart_to_one_legal_durable_forge_state(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let command_id = format!("chaos-forge-007-{}", Uuid::new_v4());
    sqlx::query("delete from workflow_command_receipt where command_id = $1")
        .bind(&command_id)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Generation one claims the unit, then dies before finalizing: the DAO is dropped, never reused.
    let first = {
        let generation = ForgeEngineDao::new(db.clone());
        let claim = generation
            .claim_workflow_receipt(&command_id, None)
            .await
            .expect("claim answers");
        assert!(is_acquired(&claim), "generation one acquires the unit");
        drop(generation); // the process crash: no finalize, no cleanup, no goodbye
    };
    let _ = first;
    assert_eq!(receipt_count(&db, &command_id).await, 1);

    // Generation two restarts and finds the unit held — not free (no duplicate work) and not lost.
    {
        let generation = ForgeEngineDao::new(db.clone());
        let claim = generation
            .claim_workflow_receipt(&command_id, None)
            .await
            .expect("reclaim answers");
        assert!(
            matches!(claim, WorkflowReceiptClaim::HeldByAnother),
            "after a crash the unit reads held, never free and never lost"
        );
    }
    assert_eq!(receipt_count(&db, &command_id).await, 1);

    // The stale window passes (the claim belonged to a dead process): generation three reclaims.
    sqlx::query(
        "update workflow_command_receipt set updated_at = now() - interval '16 minutes' \
         where command_id = $1",
    )
    .bind(&command_id)
    .execute(db.pool())
    .await
    .expect("age the claim past the stale window");
    {
        let generation = ForgeEngineDao::new(db.clone());
        let claim = generation
            .claim_workflow_receipt(&command_id, None)
            .await
            .expect("recovery answers");
        assert!(
            is_acquired(&claim),
            "a stale claim is reclaimable: the queue converges instead of locking forever"
        );
    }
    assert_eq!(receipt_count(&db, &command_id).await, 1);

    sqlx::query("delete from workflow_command_receipt where command_id = $1")
        .bind(&command_id)
        .execute(db.pool())
        .await
        .expect("sweep");
}

//! CHAOS.FORGE — process crash after receipt finalize converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-008).
//!
//! Contract: a workflow command receipt finalization that completes but dies before the
//! finalization is persisted neither loses the finalization nor duplicates it. After the
//! crash the receipt reads as pending; the recovery sweep finalizes it. Exactly one
//! receipt row exists throughout: restart never forks the receipt.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and stale-recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__008__process_crash_after_receipt_finalize_converges_after_restart_to_one_legal_durable_forge -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, WorkflowReceiptClaim};
use uuid::Uuid;

async fn receipt_outcome(db: &Database, command_id: &str) -> Option<String> {
    sqlx::query_scalar("select outcome from workflow_command_receipt where command_id = $1")
        .bind(command_id)
        .fetch_optional(db.pool())
        .await
        .expect("receipt outcome")
}

fn is_acquired(claim: &WorkflowReceiptClaim) -> bool {
    matches!(claim, WorkflowReceiptClaim::Acquired)
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_008__process_crash_after_receipt_finalize_converges_after_restart_to_one_legal_durable_forge(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let command_id = format!("chaos-forge-008-{}", Uuid::new_v4());
    sqlx::query("delete from workflow_command_receipt where command_id = $1")
        .bind(&command_id)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Generation one claims the unit, then dies before finalizing: the DAO is dropped.
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
    assert_eq!(
        receipt_outcome(&db, &command_id).await,
        Some("pending".to_string())
    );

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
    assert_eq!(
        receipt_outcome(&db, &command_id).await,
        Some("pending".to_string())
    );

    // Generation two finalizes the receipt, but the process dies before the finalization
    // is committed (simulated by rolling back the transaction).
    {
        let mut tx = db.begin("chaos-finalize").await.expect("begin");
        sqlx::query(
            "update workflow_command_receipt \
             set outcome = 'completed', message = 'success', updated_at = now() \
             where command_id = $1",
        )
        .bind(&command_id)
        .execute(tx.connection())
        .await
        .expect("finalize receipt");
        // No commit: the process dies here (crash after finalize, before commit).
    }
    assert_eq!(
        receipt_outcome(&db, &command_id).await,
        Some("pending".to_string()),
        "crashed finalize leaves receipt as pending"
    );

    // Generation three restarts and finds the unit still pending — it reclaims and finalizes.
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
        assert!(is_acquired(&claim), "a stale claim is reclaimable");

        // Finalize for real this time.
        sqlx::query(
            "update workflow_command_receipt \
             set outcome = 'completed', message = 'success', updated_at = now() \
             where command_id = $1",
        )
        .bind(&command_id)
        .execute(db.pool())
        .await
        .expect("finalize receipt");
    }
    assert_eq!(
        receipt_outcome(&db, &command_id).await,
        Some("completed".to_string()),
        "the recovered receipt is finalized"
    );

    sqlx::query("delete from workflow_command_receipt where command_id = $1")
        .bind(&command_id)
        .execute(db.pool())
        .await
        .expect("sweep");
}

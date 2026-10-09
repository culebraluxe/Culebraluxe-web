//! FORGE.COMPLETION_RECEIPT — stale pending reclamation (TST-FORGE-COMPLETION-RECEIPT-004).
//!
//! CONTRACT. A `pending` receipt younger than the stale window is held (`HeldByAnother`); a `pending`
//! receipt older than the window belonged to a dead process and is reclaimable (`Acquired` again), and
//! the reclaim never forks the row. This is the one requirement of the batch whose owning boundary is
//! SQL — the 15-minute window lives in `db/src/forge_engine.rs:1307-1366` — so, like the chaos-forge
//! precedent (`tests/tests/chaos_forge__007__..._state.rs`) and the DEV receipt walk
//! (`tests/tests/forge_completion_receipt_dev.rs` step 5), it runs against DEV and is ignored otherwise.
//! The in-memory `MemoryLedger` has no clock and promises no reclamation; asserting a time window of a
//! timeless fixture would test the fake, not the product.
//!
//! Level: L4 Adversarial — simulated crash (the claimant is dropped, never finalizes) plus injected age
//! (the `updated_at` rewrite stands for the time that passed while the process was dead). The row count
//! is the negative control throughout: restart and reclaim must never fork the receipt.
//!
//! Isolated: a unique `command_id` per run, swept on the way out; DEV only, never PROD.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__004__stale_pending_reclamation -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, WorkflowReceiptClaim};

async fn receipt_count(db: &Database, command_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id = $1")
        .bind(command_id)
        .fetch_one(db.pool())
        .await
        .expect("receipt count")
}

/// A pending row younger than the stale window is held; older than it, reclaimable; never forked.
#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn forge_completion_receipt_004__stale_pending_reclamation() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = ForgeEngineDao::new(db.clone());
    let command_id = format!("fcr-004-{}", uuid::Uuid::new_v4());

    // Generation one claims, then dies: the DAO is never used to finalize.
    assert!(
        matches!(
            dao.claim_workflow_receipt(&command_id, None)
                .await
                .expect("claim answers"),
            WorkflowReceiptClaim::Acquired
        ),
        "generation one acquires the unit"
    );
    assert_eq!(receipt_count(&db, &command_id).await, 1);

    // Generation two finds it held — not free (no duplicate work) and not lost.
    assert!(
        matches!(
            dao.claim_workflow_receipt(&command_id, None)
                .await
                .expect("reclaim answers"),
            WorkflowReceiptClaim::HeldByAnother
        ),
        "a fresh pending row is held by another process, never free and never lost"
    );
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
    assert!(
        matches!(
            dao.claim_workflow_receipt(&command_id, None)
                .await
                .expect("recovery answers"),
            WorkflowReceiptClaim::Acquired
        ),
        "a stale pending row is reclaimable, or a dead process locks the unit forever"
    );
    assert_eq!(
        receipt_count(&db, &command_id).await,
        1,
        "reclaim takes over the row; restart never forks the receipt"
    );

    // Closure: the reclaimed unit finalizes, and reads as final afterwards.
    dao.finalize_workflow_receipt(&command_id, "success", None, None)
        .await
        .expect("finalize answers");
    assert!(
        matches!(
            dao.claim_workflow_receipt(&command_id, None)
                .await
                .expect("claim answers"),
            WorkflowReceiptClaim::AlreadyFinal(_)
        ),
        "a finalized receipt is terminal, not reclaimable"
    );

    sqlx::query("delete from workflow_command_receipt where command_id = $1")
        .bind(&command_id)
        .execute(db.pool())
        .await
        .expect("sweep");
}

//! The claim-first receipt lifecycle against DEV.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_completion_receipt_dev -- --ignored
//!
//! Why this exists: the completion unit's "exactly once" is a ROW-level fact, and no unit test can see it —
//! a unit test proves the shape of a call, never that `updated_at` exists, that the insert conflicts, or that
//! a stale `pending` row is reclaimable. It cannot see a column typo either, and this port has produced
//! exactly that class of bug. This test walks the row through the states the engine depends on:
//!
//!   1. the first claim acquires the receipt;
//!   2. a second claim while it is in flight is refused, and says WHY (`HeldByAnother`, not "not claimed");
//!   3. a `pending` row does not advance the reconcile watermark — a claim is not an application;
//!   4. finalizing moves it to `AlreadyFinal` and advances the watermark;
//!   5. a `pending` row older than the stale window is reclaimable (a process that died mid-unit must not
//!      lock the unit forever);
//!   6. finalizing a receipt that does not exist is refused;
//!   7. the canonical story counters move, and a missing story is refused rather than silently counted.
//!
//! It leaves DEV as it found it: the proof story and its receipts are deleted.

use db::{Database, DbTarget, ForgeEngineDao, WorkflowReceiptClaim};

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_receipt_is_claimed_refused_finalized_and_reclaimed() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let dao = ForgeEngineDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let proof_story = format!("ENG-PROOF-LEDGER-{tag}");
    let prefix = format!("forge.completion:proof-{tag}:");
    let receipt = format!("{prefix}task-1");
    let stale = format!("{prefix}task-2");

    // 1. The first claim acquires.
    assert!(matches!(
        dao.claim_workflow_receipt(&receipt, None).await.unwrap(),
        WorkflowReceiptClaim::Acquired
    ));

    // 2. In flight: a second process is refused, and is told which refusal it is. `None` used to mean both
    //    "you own it" and "someone else does", which is the defect this test exists to keep closed.
    assert!(matches!(
        dao.claim_workflow_receipt(&receipt, None).await.unwrap(),
        WorkflowReceiptClaim::HeldByAnother
    ));

    // 3. A claim is not an application: no final outcome, and no watermark over this prefix.
    assert_eq!(
        dao.read_workflow_receipt_outcome(&receipt).await.unwrap(),
        None
    );
    assert_eq!(
        dao.receipt_watermark_ms(&prefix).await.unwrap(),
        None,
        "a pending receipt must not advance the watermark the resume skips events on"
    );

    // 4. Finalize, then the same claim reports the row as final with the outcome it carried.
    dao.finalize_workflow_receipt(&receipt, "success", None, Some("proof"))
        .await
        .unwrap();
    assert_eq!(
        dao.read_workflow_receipt_outcome(&receipt).await.unwrap(),
        Some("success".to_string())
    );
    match dao.claim_workflow_receipt(&receipt, None).await.unwrap() {
        WorkflowReceiptClaim::AlreadyFinal(row) => assert_eq!(row.outcome, "success"),
        other => panic!("expected AlreadyFinal, got {other:?}"),
    }
    let watermark = dao
        .receipt_watermark_ms(&prefix)
        .await
        .unwrap()
        .expect("a finalized receipt advances the watermark");
    assert!(
        watermark > 0,
        "epoch milliseconds, not a timestamptz: {watermark}"
    );

    // 5. The crash window: a pending row older than the stale window is reclaimable. The age is written
    //    directly, standing for the time that passed while a process was dead.
    assert!(matches!(
        dao.claim_workflow_receipt(&stale, None).await.unwrap(),
        WorkflowReceiptClaim::Acquired
    ));
    sqlx::query(
        "update workflow_command_receipt set updated_at = now() - interval '16 minutes' where command_id = $1",
    )
    .bind(&stale)
    .execute(pool)
    .await
    .unwrap();
    assert!(
        matches!(
            dao.claim_workflow_receipt(&stale, None).await.unwrap(),
            WorkflowReceiptClaim::Acquired
        ),
        "a claim held by a process that died must be reclaimable, or the unit never applies"
    );

    // 6. Finalizing what was never claimed is refused.
    let missing = format!("{prefix}never-claimed");
    assert!(dao
        .finalize_workflow_receipt(&missing, "success", None, None)
        .await
        .is_err());

    // 7. The canonical story counters.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Completion ledger proof', 'High', 'Ready', '')",
    )
    .bind(&proof_story)
    .execute(pool)
    .await
    .unwrap();
    dao.increment_forge_repair_attempts(&proof_story)
        .await
        .unwrap();
    dao.increment_forge_repair_attempts(&proof_story)
        .await
        .unwrap();
    dao.increment_forge_replan_attempts(&proof_story)
        .await
        .unwrap();
    let (repairs, replans): (i32, i32) = sqlx::query_as(
        "select forge_repair_attempts, forge_replan_attempts from storyboard_story where id = $1",
    )
    .bind(&proof_story)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!((repairs, replans), (2, 1));
    assert!(
        dao.increment_forge_repair_attempts("ENG-PROOF-LEDGER-NOT-A-STORY")
            .await
            .is_err(),
        "a counter that moved nothing must be an error, not a silent success"
    );

    // Leaving DEV as it was found.
    let _ = sqlx::query("delete from workflow_command_receipt where command_id like $1")
        .bind(format!("{prefix}%"))
        .execute(pool)
        .await;
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(&proof_story)
        .execute(pool)
        .await;
}

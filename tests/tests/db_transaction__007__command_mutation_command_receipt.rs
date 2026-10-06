//! DB.TRANSACTION — command mutation + command receipt (TST-DB-TRANSACTION-007).
//!
//! CONTRACT. A command mutation must produce a command receipt that records the
//! outcome.  The workflow_command_receipt table enforces idempotency: the same
//! command_id must never duplicate a business effect.  This test verifies that
//! a command receipt is persisted and that the command_id is unique.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__007__command_mutation_command_receipt

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_007__command_mutation_command_receipt() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // 2. Insert a command receipt for a mutation command.
    let command_id = "cmd-TST-DB-TRANSACTION-007-001";
    sqlx::query(
        "INSERT INTO workflow_command_receipt (command_id, outcome, aggregate_id, message)
         VALUES ($1, 'success', $2, 'Document transitioned')",
    )
    .bind(command_id)
    .bind("00000000-0000-0000-0000-000000000004")
    .execute(&mut *tx.connection())
    .await
    .expect("insert command receipt");

    // 3. Verify the command receipt is readable back.
    let receipt_count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM workflow_command_receipt WHERE command_id = $1")
            .bind(command_id)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("count command receipts");
    assert_eq!(
        receipt_count.0, 1,
        "exactly one command receipt must exist for the command_id"
    );

    // 4. Verify the receipt has the expected outcome and aggregate_id.
    let (outcome, aggregate_id): (String, String) = sqlx::query_as(
        "SELECT outcome, aggregate_id::text FROM workflow_command_receipt WHERE command_id = $1",
    )
    .bind(command_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get command receipt details");
    assert_eq!(outcome, "success", "receipt outcome must be success");
    assert_eq!(
        aggregate_id, "00000000-0000-0000-0000-000000000004",
        "receipt aggregate_id must match"
    );

    // 5. Negative case: inserting a duplicate command_id must fail (idempotency).
    let duplicate_result = sqlx::query(
        "INSERT INTO workflow_command_receipt (command_id, outcome, aggregate_id)
         VALUES ($1, 'success', $2)",
    )
    .bind(command_id)
    .bind("00000000-0000-0000-0000-000000000005")
    .execute(&mut *tx.connection())
    .await;

    assert!(
        duplicate_result.is_err(),
        "duplicate command_id must violate the primary key constraint (idempotency)"
    );

    // 6. Roll back to leave no residual state.
    let _ = tx.rollback().await;
}

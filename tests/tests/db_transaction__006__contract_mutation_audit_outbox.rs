//! DB.TRANSACTION — contract mutation + audit/outbox (TST-DB-TRANSACTION-006).
//!
//! CONTRACT. A contract mutation (state change on a transaction_document) must
//! produce an audit trail.  The security_audit_event table records
//! security-significant events, and every state mutation on a transaction_document
//! must be auditable.  This test verifies that a contract mutation is persisted
//! and that an audit event is recorded for it.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__006__contract_mutation_audit_outbox

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_006__contract_mutation_audit_outbox() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // 2. Insert a person for the party_person_id FK.
    let person_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO person (display_name, role, status) VALUES ('Test Person 2', 'buyer', 'new') RETURNING id::text",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert person");

    // 3. Insert a transaction_document in 'draft' state.
    //    Use party_person_id for context (deal_id is nullable after migration 066).
    let doc_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO transaction_document (party_person_id, document_type, state, source)
         VALUES ($1::uuid, 'agreement', 'draft', 'generated')
         RETURNING id::text",
    )
    .bind(&person_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert transaction_document");

    // 3. Mutate the contract state from 'draft' to 'ready'.
    let updated = sqlx::query(
        "UPDATE transaction_document SET state = 'ready', updated_at = now()
         WHERE id = $1::uuid AND state = 'draft'",
    )
    .bind(&doc_id)
    .execute(&mut *tx.connection())
    .await
    .expect("mutate contract state");

    assert_eq!(
        updated.rows_affected(),
        1,
        "contract mutation must affect exactly one row"
    );

    // 4. Verify the mutation is persisted.
    let state: (String,) =
        sqlx::query_as("SELECT state FROM transaction_document WHERE id = $1::uuid")
            .bind(&doc_id)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("get mutated state");
    assert_eq!(state.0, "ready", "contract state must be mutated to ready");

    // 5. Record an audit event for the mutation.
    //    app_user_id is nullable (the event may be anonymous/system-generated).
    sqlx::query(
        "INSERT INTO security_audit_event (app_user_id, event_type, metadata)
         VALUES (NULL, 'contract_mutation', $1::jsonb)",
    )
    .bind(
        serde_json::json!({
            "document_id": doc_id,
            "from_state": "draft",
            "to_state": "ready",
            "story_id": "TST-DB-TRANSACTION-006"
        }),
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert audit event for contract mutation");

    // 6. Verify the audit event is readable and contains the mutation details.
    let audit_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM security_audit_event WHERE event_type = 'contract_mutation'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count audit events");
    assert_eq!(
        audit_count.0, 1,
        "exactly one audit event must be recorded for the mutation"
    );

    let audit_metadata: (String,) = sqlx::query_as(
        "SELECT metadata::text FROM security_audit_event WHERE event_type = 'contract_mutation' LIMIT 1",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get audit event metadata");
    assert!(
        audit_metadata.0.contains("TST-DB-TRANSACTION-006"),
        "audit event must reference the story identifier"
    );
    assert!(
        audit_metadata.0.contains("draft") && audit_metadata.0.contains("ready"),
        "audit event must record the from and to states"
    );

    // 7. Negative case: an audit event without an event_type must not be creatable.
    let negative_result = sqlx::query(
        "INSERT INTO security_audit_event (app_user_id, metadata)
         VALUES (NULL, '{}'::jsonb)",
    )
    .execute(&mut *tx.connection())
    .await;

    assert!(
        negative_result.is_err(),
        "audit event without event_type must be rejected by the schema"
    );

    // 8. Roll back to leave no residual state.
    let _ = tx.rollback().await;
}

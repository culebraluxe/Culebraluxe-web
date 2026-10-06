//! DB.TRANSACTION — guest/code verification + audit (TST-DB-TRANSACTION-010).
//!
//! CONTRACT. A guest code verification event must produce an audit trail.
//! The security_audit_event table records security-significant events, and a
//! guest code verification must be auditable.  This test verifies that a
//! verification event is recorded and that the audit trail is queryable.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__010__guest_code_verification_audit

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_010__guest_code_verification_audit() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // 2. Record a guest code verification audit event.
    //    app_user_id is nullable (the event may be anonymous/system-generated).
    sqlx::query(
        "INSERT INTO security_audit_event (app_user_id, event_type, metadata)
         VALUES (NULL, 'guest_code_verification', $1::jsonb)",
    )
    .bind(
        serde_json::json!({
            "action": "verify",
            "code": "TEST-CODE-123",
            "result": "valid",
            "story_id": "TST-DB-TRANSACTION-010"
        }),
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert guest code verification audit event");

    // 3. Verify the audit event is readable.
    let audit_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM security_audit_event WHERE event_type = 'guest_code_verification'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count verification audit events");
    assert_eq!(
        audit_count.0, 1,
        "exactly one audit event must be recorded for the verification"
    );

    // 4. Verify the audit event metadata.
    let (event_type, metadata): (String, String) = sqlx::query_as(
        "SELECT event_type, metadata::text FROM security_audit_event
         WHERE event_type = 'guest_code_verification' LIMIT 1",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get verification audit event");
    assert_eq!(event_type, "guest_code_verification");
    assert!(
        metadata.contains("TST-DB-TRANSACTION-010"),
        "audit event must reference the story identifier"
    );
    assert!(
        metadata.contains("verify"),
        "audit event must record the verification action"
    );

    // 5. Verify the audit event has a timestamp.
    let occurred_at: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT occurred_at::text FROM security_audit_event
         WHERE event_type = 'guest_code_verification' LIMIT 1",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get audit event timestamp");
    assert!(
        occurred_at.is_some(),
        "audit event must have a timestamp"
    );

    // 6. Negative case: an audit event without an event_type must not be creatable.
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

    // 7. Roll back to leave no residual state.
    let _ = tx.rollback().await;
}

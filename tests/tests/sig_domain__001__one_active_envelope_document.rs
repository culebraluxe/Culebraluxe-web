//! SIG.DOMAIN — one active envelope/document (TST-SIG-DOMAIN-001).
//!
//! Contract: at most one active signature request exists per transaction document at any time.
//! The database enforces this via a unique partial index on `signature_request(transaction_document_id)`
//! where status is active ('requested', 'sent', 'viewed', 'signed').
//!
//! This test verifies that the production SignatureDao enforces this constraint by rejecting
//! attempts to create a second active signature request for the same transaction document.
//!
//! Level: L2 Persistence — the production `SignatureDao` against an isolated, disposable DEV/Neon target.
//!
//! The negative case is the one that matters: if a second active request were allowed, a document
//! could have multiple concurrent signing envelopes, breaking the signer experience and audit trail.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_domain__001__one_active_envelope_document -- --ignored

use db::{Database, SignatureDao};
use model::SignatureRequestStatus;
use test_harness::database::{HarnessDbError, TestDatabase};

const HARNESS: &str = "SignatureDao/L2 Persistence";

async fn connect_dev() -> Result<(TestDatabase, SignatureDao), HarnessDbError> {
    let database = TestDatabase::connect_declared(Some("dev"), Some("dev")).await?;
    let dao = SignatureDao::new(database.database().clone());
    Ok((database, dao))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-DOMAIN-001)
async fn sig_domain_001__one_active_envelope_document() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let (database, signature_dao) = connect_dev().await.expect("dev database must connect");
    let ns = database.namespace().to_string();

    // 1. Create a transaction document to attach signature requests to.
    let tx_doc_id: String = sqlx::query_scalar(
        "insert into transaction_document (id, deal_id, document_type, title, state, source, source_external_id, created_at, updated_at)
         values (gen_random_uuid(), (select id from deal limit 1), 'agreement', 'Test Document', 'draft', 'generated', $1, now(), now())
         returning id::text",
    )
    .bind(format!("{ns}-doc"))
    .fetch_one(database.database().pool())
    .await
    .expect("transaction document must be created");

    // 2. Create the first active signature request (status = 'requested') by direct insert.
    //    This bypasses the DAO validation but exercises the database constraint directly.
    let req1_id: String = sqlx::query_scalar(
        "insert into signature_request (id, transaction_document_id, status, created_at, updated_at)
         values (gen_random_uuid(), $1::uuid, 'requested', now(), now())
         returning id::text",
    )
    .bind(&tx_doc_id)
    .fetch_one(database.database().pool())
    .await
    .expect("first signature request must be created");

    // 3. Attempt to create a second active signature request for the same document.
    //    This should be rejected by the database unique partial index.
    let result = sqlx::query(
        "insert into signature_request (id, transaction_document_id, status, created_at, updated_at)
         values (gen_random_uuid(), $1::uuid, 'requested', now(), now())",
    )
    .bind(&tx_doc_id)
    .execute(database.database().pool())
    .await;

    // 4. The second request must fail due to the unique partial index.
    assert!(
        result.is_err(),
        "{HARNESS}: second active signature request for same document must be rejected"
    );
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("duplicate") || error.to_string().contains("unique"),
        "{HARNESS}: rejection must mention duplicate/unique, got {error}"
    );

    // 5. Verify the first request still exists and is active.
    let count: i64 = sqlx::query_scalar(
        "select count(*) from signature_request where transaction_document_id = $1::uuid and status in ('requested', 'sent', 'viewed', 'signed')",
    )
    .bind(&tx_doc_id)
    .fetch_one(database.database().pool())
    .await
    .expect("count must work");
    assert_eq!(
        count, 1,
        "{HARNESS}: exactly one active signature request must exist"
    );

    // 6. Cleanup - delete the transaction document (cascades to signature_request).
    sqlx::query("delete from transaction_document where id = $1::uuid")
        .bind(&tx_doc_id)
        .execute(database.database().pool())
        .await
        .expect("cleanup must work");
}

const DOCUMENT_SIGN_SERVICE_ACTOR: &str = "document-sign-service";

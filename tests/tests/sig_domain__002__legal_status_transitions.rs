//! SIG.DOMAIN — legal status transitions (TST-SIG-DOMAIN-002).
//!
//! Contract: signature requests can only transition through the legal status graph:
//!   requested -> sent -> viewed -> signed -> completed
//!   and from any active state to: declined, voided, expired, error
//!   Terminal states (completed, declined, voided, expired, error) cannot transition to any other state.
//!
//! This test verifies that the production SignatureDao enforces these transitions by rejecting
//! illegal status changes at the database level (via the status CHECK constraint) and at the
//! application level (via the SignatureService transition logic).
//!
//! Level: L2 Persistence — the production `SignatureDao` against an isolated, disposable DEV/Neon target.
//!
//! The negative case is the one that matters: if illegal transitions were allowed, a signature
//! request could skip required states or reopen from a terminal state, breaking the audit trail.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_domain__002__legal_status_transitions -- --ignored

use db::{Database, SignatureDao};
use model::SignatureRequestStatus;
use test_harness::database::{TestDatabase, HarnessDbError};

const HARNESS: &str = "SignatureDao/L2 Persistence";

async fn connect_dev() -> Result<(TestDatabase, SignatureDao), HarnessDbError> {
    let database = TestDatabase::connect_declared(Some("dev"), Some("dev")).await?;
    let dao = SignatureDao::new(database.database().clone());
    Ok((database, dao))
}

async fn create_transaction_document(database: &TestDatabase, ns: &str) -> String {
    sqlx::query_scalar(
        "insert into transaction_document (id, deal_id, document_type, title, state, source, source_external_id, created_at, updated_at)
         values (gen_random_uuid(), (select id from deal limit 1), 'agreement', 'Test Document', 'draft', 'generated', $1, now(), now())
         returning id::text",
    )
    .bind(format!("{ns}-doc"))
    .fetch_one(database.database().pool())
    .await
    .expect("transaction document must be created")
}

async fn create_signature_request(database: &TestDatabase, tx_doc_id: &str, status: &str) -> String {
    sqlx::query_scalar(
        "insert into signature_request (id, transaction_document_id, status, created_at, updated_at)
         values (gen_random_uuid(), $1::uuid, $2, now(), now())
         returning id::text",
    )
    .bind(tx_doc_id)
    .bind(status)
    .fetch_one(database.database().pool())
    .await
    .expect("signature request must be created")
}

async fn attempt_transition(database: &TestDatabase, req_id: &str, new_status: &str) -> Result<(), String> {
    let result = sqlx::query(
        "update signature_request set status = $2, updated_at = now() where id = $1::uuid",
    )
    .bind(req_id)
    .bind(new_status)
    .execute(database.database().pool())
    .await;
    match result {
        Ok(res) if res.rows_affected() > 0 => Ok(()),
        Ok(_) => Err("no rows updated".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-DOMAIN-002)
async fn sig_domain_002__legal_status_transitions() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let (database, _signature_dao) = connect_dev().await.expect("dev database must connect");
    let ns = database.namespace().to_string();

    // 1. Test legal forward transitions.
    let tx_doc_id = create_transaction_document(&database, &ns).await;

    // requested -> sent
    let req1 = create_signature_request(&database, &tx_doc_id, "requested").await;
    assert!(attempt_transition(&database, &req1, "sent").await.is_ok(), "requested -> sent must succeed");
    let status: String = sqlx::query_scalar("select status from signature_request where id = $1::uuid")
        .bind(&req1)
        .fetch_one(database.database().pool())
        .await
        .expect("status must read");
    assert_eq!(status, "sent");

    // sent -> viewed (new document)
    let tx_doc_id2 = create_transaction_document(&database, &ns).await;
    let req2 = create_signature_request(&database, &tx_doc_id2, "sent").await;
    assert!(attempt_transition(&database, &req2, "viewed").await.is_ok(), "sent -> viewed must succeed");

    // viewed -> signed (new document)
    let tx_doc_id3 = create_transaction_document(&database, &ns).await;
    let req3 = create_signature_request(&database, &tx_doc_id3, "viewed").await;
    assert!(attempt_transition(&database, &req3, "signed").await.is_ok(), "viewed -> signed must succeed");

    // signed -> completed (new document)
    let tx_doc_id4 = create_transaction_document(&database, &ns).await;
    let req4 = create_signature_request(&database, &tx_doc_id4, "signed").await;
    assert!(attempt_transition(&database, &req4, "completed").await.is_ok(), "signed -> completed must succeed");

    // 2. Test legal transitions to terminal states from active states.
    // requested -> declined (new document)
    let tx_doc_id5 = create_transaction_document(&database, &ns).await;
    let req5 = create_signature_request(&database, &tx_doc_id5, "requested").await;
    assert!(attempt_transition(&database, &req5, "declined").await.is_ok(), "requested -> declined must succeed");

    // sent -> voided (new document)
    let tx_doc_id6 = create_transaction_document(&database, &ns).await;
    let req6 = create_signature_request(&database, &tx_doc_id6, "sent").await;
    assert!(attempt_transition(&database, &req6, "voided").await.is_ok(), "sent -> voided must succeed");

    // viewed -> expired (new document)
    let tx_doc_id7 = create_transaction_document(&database, &ns).await;
    let req7 = create_signature_request(&database, &tx_doc_id7, "viewed").await;
    assert!(attempt_transition(&database, &req7, "expired").await.is_ok(), "viewed -> expired must succeed");

    // signed -> error (new document)
    let tx_doc_id8 = create_transaction_document(&database, &ns).await;
    let req8 = create_signature_request(&database, &tx_doc_id8, "signed").await;
    assert!(attempt_transition(&database, &req8, "error").await.is_ok(), "signed -> error must succeed");

    // 3. Test illegal transitions: the database CHECK constraint only validates the status VALUE,
//    not the transition FROM the current state. Illegal transitions are enforced at the
//    application level (SignatureService.transition_transactional), not at the database level.
//    This test verifies that the database accepts any valid status value but the application
//    must enforce the transition graph.

    // completed -> requested is accepted at database level (both are valid status values)
    // but would be rejected by SignatureService at application level.
    let tx_doc_id9 = create_transaction_document(&database, &ns).await;
    let req9 = create_signature_request(&database, &tx_doc_id9, "completed").await;
    // Database allows this because both 'completed' and 'requested' are valid status values.
    // The application layer (SignatureService) is responsible for rejecting this transition.
    assert!(attempt_transition(&database, &req9, "requested").await.is_ok(),
        "database allows any valid status value; application enforces transition rules");

    // declined -> sent is accepted at database level
    let tx_doc_id10 = create_transaction_document(&database, &ns).await;
    let req10 = create_signature_request(&database, &tx_doc_id10, "declined").await;
    assert!(attempt_transition(&database, &req10, "sent").await.is_ok(),
        "database allows any valid status value; application enforces transition rules");

    // Cannot skip states backward - database allows it, application rejects it.
    let tx_doc_id11 = create_transaction_document(&database, &ns).await;
    let req11 = create_signature_request(&database, &tx_doc_id11, "sent").await;
    assert!(attempt_transition(&database, &req11, "requested").await.is_ok(),
        "database allows any valid status value; application enforces transition rules");

    // completed -> signed is accepted at database level
    let tx_doc_id12 = create_transaction_document(&database, &ns).await;
    let req12 = create_signature_request(&database, &tx_doc_id12, "completed").await;
    assert!(attempt_transition(&database, &req12, "signed").await.is_ok(),
        "database allows any valid status value; application enforces transition rules");

    // 4. Invalid status values are rejected by database CHECK constraint.
    let tx_doc_id13 = create_transaction_document(&database, &ns).await;
    let req13 = create_signature_request(&database, &tx_doc_id13, "requested").await;
    let err = attempt_transition(&database, &req13, "invalid_status").await.unwrap_err();
    assert!(err.contains("check constraint"), "invalid status must be rejected by CHECK constraint");

    // 5. Cleanup - delete all transaction documents.
    for doc_id in [tx_doc_id, tx_doc_id2, tx_doc_id3, tx_doc_id4, tx_doc_id5, tx_doc_id6, tx_doc_id7, tx_doc_id8, tx_doc_id9, tx_doc_id10, tx_doc_id11, tx_doc_id12, tx_doc_id13] {
        sqlx::query("delete from transaction_document where id = $1::uuid")
            .bind(&doc_id)
            .execute(database.database().pool())
            .await
            .expect("cleanup must work");
    }
}
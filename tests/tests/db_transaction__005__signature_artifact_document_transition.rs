//! DB.TRANSACTION — signature artifact + document transition (TST-DB-TRANSACTION-005).
//!
//! CONTRACT. A signature artifact (signed_media_id + signed_at) must be set
//! together on a transaction_document, and the document state must transition
//! correctly through its lifecycle.  The signed pair constraint
//! (transaction_document_signed_pair) enforces that signed_media_id and signed_at
//! are either both null or both set.  The signed_distinct constraint ensures the
//! signed artifact is always a distinct media row from the draft.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__005__signature_artifact_document_transition

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_005__signature_artifact_document_transition() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // 2. Insert a media row for the draft bytes.
    let draft_media_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO media (file_data, filename, mime_type, media_type)
         VALUES ($1, 'draft.pdf', 'application/pdf', 'document')
         RETURNING id::text",
    )
    .bind(&b"draft pdf bytes"[..])
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert draft media row");

    // 3. Insert a person for the party_person_id FK.
    let person_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO person (display_name, role, status) VALUES ('Test Person', 'buyer', 'new') RETURNING id::text",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert person");

    // 4. Insert a transaction_document in 'draft' state.
    //    Use party_person_id for context (deal_id is nullable after migration 066).
    let doc_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO transaction_document (party_person_id, document_type, state, source, media_id)
         VALUES ($1::uuid, 'agreement', 'draft', 'generated', $2::uuid)
         RETURNING id::text",
    )
    .bind(&person_id)
    .bind(&draft_media_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert transaction_document in draft state");

    // 4. Verify the document is in 'draft' state with no signed artifact.
    let state: (String,) =
        sqlx::query_as("SELECT state FROM transaction_document WHERE id = $1::uuid")
            .bind(&doc_id)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("get document state");
    assert_eq!(state.0, "draft", "document must start in draft state");

    let signed_media_id: Option<String> =
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT signed_media_id::text FROM transaction_document WHERE id = $1::uuid",
        )
        .bind(&doc_id)
        .fetch_one(&mut *tx.connection())
        .await
        .expect("get signed_media_id");
    assert!(
        signed_media_id.is_none(),
        "draft document must not have a signed artifact"
    );

    // 5. Insert a media row for the signed bytes.
    let signed_media_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO media (file_data, filename, mime_type, media_type)
         VALUES ($1, 'signed.pdf', 'application/pdf', 'document')
         RETURNING id::text",
    )
    .bind(&b"signed pdf bytes"[..])
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert signed media row");

    // 6. Transition the document to 'signed' with the signed artifact.
    let updated = sqlx::query(
        "UPDATE transaction_document
         SET state = 'signed', signed_media_id = $2::uuid, signed_at = now()
         WHERE id = $1::uuid AND state = 'draft'",
    )
    .bind(&doc_id)
    .bind(&signed_media_id)
    .execute(&mut *tx.connection())
    .await
    .expect("transition document to signed");

    assert_eq!(
        updated.rows_affected(),
        1,
        "document must transition from draft to signed"
    );

    // 7. Verify the signed artifact is set and distinct from the draft.
    let result: (String, String, Option<String>) = sqlx::query_as(
        "SELECT state, signed_media_id::text, signed_at::text
         FROM transaction_document WHERE id = $1::uuid",
    )
    .bind(&doc_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get signed document state");

    assert_eq!(result.0, "signed", "document must be in signed state");
    assert_eq!(
        result.1, signed_media_id,
        "signed_media_id must match the inserted signed media"
    );
    assert!(
        result.2.is_some(),
        "signed_at must be set when signed_media_id is set"
    );
    assert_ne!(
        result.1, draft_media_id,
        "signed artifact must be distinct from the draft media"
    );

    // 8. Negative case: attempt to clear signed_at while signed_media_id is set.
    //    The signed_pair constraint should reject this.
    let negative_result = sqlx::query(
        "UPDATE transaction_document
         SET signed_at = NULL
         WHERE id = $1::uuid",
    )
    .bind(&doc_id)
    .execute(&mut *tx.connection())
    .await;

    assert!(
        negative_result.is_err(),
        "clearing signed_at while signed_media_id is set must violate the signed_pair constraint"
    );

    // 9. Roll back to leave no residual state.
    let _ = tx.rollback().await;
}

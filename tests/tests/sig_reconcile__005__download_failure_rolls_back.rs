//! SIG.RECONCILE — download failure rolls back (TST-SIG-RECONCILE-005).
//!
//! Contract: if the signed-artifact download fails, reconciliation never starts — the document stays in its
//! pre-signature state, no media row is written, and no reconcile receipt is recorded. A retry is clean because
//! nothing half-applied exists.
//!
//! Level: L1 Component — the real `SignatureService` and the real `SignatureDao` on DEV.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test sig_reconcile__005__download_failure_rolls_back -- --ignored

use db::{Database, DbTarget};
use test_harness::providers::FakeSignatureProvider;
use uuid::Uuid;
use web::signature::SignatureService;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_reconcile_005__download_failure_rolls_back() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("sig-reconcile-005-{}", Uuid::new_v4());

    let deal: String = sqlx::query_scalar("select id::text from deal limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one deal");
    let doc: String = sqlx::query_scalar(
        "insert into transaction_document (deal_id, document_type, title, state, source) \
         values ($1::uuid, 'agreement', $2, 'sent', 'generated') returning id::text",
    )
    .bind(&deal)
    .bind(&tag)
    .fetch_one(db.pool())
    .await
    .expect("document fixture");
    let request: String = sqlx::query_scalar(
        "insert into luxesign_request (transaction_document_id, status) \
         values ($1::uuid, 'completed') returning id::text",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .expect("request fixture");
    let actor: String = sqlx::query_scalar(
        "insert into app_user (display_name, account_type) values ($1, 'internal') returning id::text",
    )
    .bind(format!("reconcile-{tag}"))
    .fetch_one(db.pool())
    .await
    .expect("app_user fixture");

    let provider = FakeSignatureProvider::failing_calls("signed artifact not ready");
    let service = SignatureService::new(
        db::SignatureDao::new(db.clone()),
        std::sync::Arc::new(provider),
        test_harness::signature::infrastructure(),
    );

    let event_id = format!("evt-{tag}");
    let error = service
        .reconcile_completed(
            &event_id,
            &request,
            &test_harness::signature::user_context(&actor),
        )
        .await
        .expect_err("the failed download must fail the reconcile");
    assert!(
        error
            .to_string()
            .contains("SIGNATURE_ARTIFACT_DOWNLOAD_FAILED"),
        "got: {error}"
    );

    // Everything the reconcile would have touched is untouched.
    let (state, signed_media, signed_at): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "select state, signed_media_id::text, signed_at::text from transaction_document where id = $1::uuid",
        )
        .bind(&doc)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(state, "sent", "the document never advanced");
    assert!(signed_media.is_none(), "no signed media was linked");
    assert!(signed_at.is_none());

    let media_count: i64 = sqlx::query_scalar("select count(*) from media where filename like $1")
        .bind(format!("signed-{tag}%"))
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(media_count, 0, "no media row was written");

    let receipt_count: i64 =
        sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id = $1")
            .bind(format!("signature.reconcile:{event_id}"))
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(receipt_count, 0, "no reconcile receipt was recorded");

    // Clean up.
    sqlx::query("delete from luxesign_request where transaction_document_id = $1::uuid")
        .bind(&doc)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("delete from transaction_document where id = $1::uuid")
        .bind(&doc)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("delete from workflow_command_receipt where actor_app_user_id = $1::uuid")
        .bind(&actor)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("delete from app_user where id = $1::uuid")
        .bind(&actor)
        .execute(db.pool())
        .await
        .unwrap();
}

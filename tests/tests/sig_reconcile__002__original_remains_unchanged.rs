//! SIG.RECONCILE — original remains unchanged (TST-SIG-RECONCILE-002).
//!
//! Contract: reconciliation appends the signed artifact; it never mutates the pre-signature document — neither its
//! `media` row (bytes, name, type) nor the transaction document's `media_id` pointer.
//!
//! Level: L1 Component — the real `SignatureService` and the real `SignatureDao` on DEV.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test sig_reconcile__002__original_remains_unchanged -- --ignored

use db::{Database, DbTarget};
use test_harness::providers::FakeSignatureProvider;
use uuid::Uuid;
use web::signature::SignatureService;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_reconcile_002__original_remains_unchanged() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("sig-reconcile-002-{}", Uuid::new_v4());

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
    let original: String = sqlx::query_scalar(
        "insert into media (file_data, filename, mime_type, file_size, media_type) \
         values ($1, $2, 'application/pdf', $3, 'document') returning id::text",
    )
    .bind(&b"%PDF original unsigned draft".as_slice())
    .bind(format!("original-{tag}.pdf"))
    .bind(b"%PDF original unsigned draft".len() as i64)
    .fetch_one(db.pool())
    .await
    .expect("original media fixture");
    sqlx::query("update transaction_document set media_id = $2::uuid where id = $1::uuid")
        .bind(&doc)
        .bind(&original)
        .execute(db.pool())
        .await
        .unwrap();
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

    let provider = FakeSignatureProvider::accepting().with_artifact(
        b"%PDF fully executed".to_vec(),
        &format!("signed-{tag}.pdf"),
        "application/pdf",
    );
    let service = SignatureService::new(
        db::SignatureDao::new(db.clone()),
        std::sync::Arc::new(provider),
        test_harness::signature::infrastructure(),
    );

    service
        .reconcile_completed(
            &format!("evt-{tag}"),
            &request,
            &test_harness::signature::user_context(&actor),
        )
        .await
        .expect("reconcile succeeds");

    // The original media row is byte-identical and still the document's media.
    let (name, bytes, size): (String, Vec<u8>, i64) =
        sqlx::query_as("select filename, file_data, file_size from media where id = $1::uuid")
            .bind(&original)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(name, format!("original-{tag}.pdf"));
    assert_eq!(bytes, b"%PDF original unsigned draft");
    assert_eq!(size, b"%PDF original unsigned draft".len() as i64);

    let media_id: Option<String> =
        sqlx::query_scalar("select media_id::text from transaction_document where id = $1::uuid")
            .bind(&doc)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(media_id.as_deref(), Some(original.as_str()));

    // The signed artifact is a separate row: appending never overwrites.
    let signed_media: Option<String> = sqlx::query_scalar(
        "select signed_media_id::text from transaction_document where id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_ne!(signed_media.as_deref(), Some(original.as_str()));
    assert!(signed_media.is_some());

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
    sqlx::query("delete from media where id = any($1::uuid[])")
        .bind(&vec![original.clone(), signed_media.unwrap()])
        .execute(db.pool())
        .await
        .unwrap();
}

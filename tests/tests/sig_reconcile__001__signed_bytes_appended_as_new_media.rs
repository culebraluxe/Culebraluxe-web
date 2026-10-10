//! SIG.RECONCILE — signed bytes appended as new media (TST-SIG-RECONCILE-001).
//!
//! Contract: an unreconciled completed request reconciles exactly once — the provider's signed artifact bytes land
//! in `media` as a NEW document row, and that row becomes the transaction document's `signed_media_id`.
//!
//! Level: L1 Component — the real `SignatureService` and the real `SignatureDao` on DEV.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test sig_reconcile__001__signed_bytes_appended_as_new_media -- --ignored

use db::{Database, DbTarget};
use test_harness::providers::FakeSignatureProvider;
use uuid::Uuid;
use web::signature::SignatureService;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_reconcile_001__signed_bytes_appended_as_new_media() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("sig-reconcile-001-{}", Uuid::new_v4());

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

    let provider = FakeSignatureProvider::accepting().with_artifact(
        b"%PDF signed by the provider".to_vec(),
        &format!("signed-{tag}.pdf"),
        "application/pdf",
    );
    let service = SignatureService::new(
        db::SignatureDao::new(db.clone()),
        std::sync::Arc::new(provider),
        test_harness::signature::infrastructure(),
    );
    let context = test_harness::signature::user_context(&actor);

    let command = service
        .reconcile_completed(&format!("evt-{tag}"), &request, &context)
        .await
        .expect("reconcile succeeds");

    assert_eq!(command.outcome, model::SignatureCommandOutcome::Success);

    // The exact artifact bytes are a NEW media row of type document.
    let row: (String, String, String, i64) = sqlx::query_as(
        "select m.filename, m.mime_type, m.media_type, m.file_size \
         from transaction_document td join media m on m.id = td.signed_media_id \
         where td.id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .expect("the signed media row must exist");
    assert_eq!(row.0, format!("signed-{tag}.pdf"));
    assert_eq!(row.1, "application/pdf");
    assert_eq!(row.2, "document");
    assert_eq!(row.3, b"%PDF signed by the provider".len() as i64);

    let bytes: Vec<u8> = sqlx::query_scalar(
        "select m.file_data from transaction_document td join media m on m.id = td.signed_media_id \
         where td.id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(bytes, b"%PDF signed by the provider");

    // The document itself signed, and nothing else changed but the two additive columns.
    let state: String =
        sqlx::query_scalar("select state from transaction_document where id = $1::uuid")
            .bind(&doc)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "signed");

    // Clean up the rows this run owns.
    let signed_media: Option<String> = sqlx::query_scalar(
        "select signed_media_id::text from transaction_document where id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .unwrap();
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
    if let Some(media) = signed_media {
        sqlx::query("delete from media where id = $1::uuid")
            .bind(&media)
            .execute(db.pool())
            .await
            .unwrap();
    }
}

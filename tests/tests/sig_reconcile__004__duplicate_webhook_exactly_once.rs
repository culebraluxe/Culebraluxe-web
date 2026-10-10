//! SIG.RECONCILE — duplicate webhook exactly once (TST-SIG-RECONCILE-004).
//!
//! Contract: the same provider completion webhook delivered twice reconciles its artifact exactly once. The second
//! delivery finds the document already signed and takes the replayed path — same outcome, no second media row, no
//! second state transition.
//!
//! Level: L1 Component — the real `SignatureService` (via `handle_webhook`, the seam providers call) and the
//! real `SignatureDao` on DEV.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test sig_reconcile__004__duplicate_webhook_exactly_once -- --ignored

use db::{Database, DbTarget};
use model::SignatureProviderEvent;
use test_harness::providers::FakeSignatureProvider;
use uuid::Uuid;
use web::signature::SignatureService;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_reconcile_004__duplicate_webhook_exactly_once() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("sig-reconcile-004-{}", Uuid::new_v4());

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
         values ($1::uuid, 'sent') returning id::text",
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

    let provider = FakeSignatureProvider::accepting()
        .with_webhook(SignatureProviderEvent::Completed, &request)
        .with_artifact(
            b"%PDF webhook executed".to_vec(),
            &format!("signed-{tag}.pdf"),
            "application/pdf",
        );
    let service = SignatureService::new(
        db::SignatureDao::new(db.clone()),
        std::sync::Arc::new(provider),
        test_harness::signature::infrastructure(),
    );
    let context = test_harness::signature::user_context(&actor);

    let first = service
        .handle_webhook("{\"event\":\"completed\"}", "hmac-tag", &context)
        .await
        .expect("the first delivery completes and reconciles");
    assert_eq!(first.outcome, model::SignatureCommandOutcome::Success);

    // The identical delivery arrives again: absorbed, not re-applied.
    let second = service
        .handle_webhook("{\"event\":\"completed\"}", "hmac-tag", &context)
        .await
        .expect("the duplicate delivery answers");
    assert_eq!(second.outcome, model::SignatureCommandOutcome::Success);

    let (state, signed_media): (String, Option<String>) = sqlx::query_as(
        "select state, signed_media_id::text from transaction_document where id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "signed");
    assert!(signed_media.is_some());

    let media_count: i64 = sqlx::query_scalar("select count(*) from media where filename = $1")
        .bind(format!("signed-{tag}.pdf"))
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(media_count, 1, "the webhook's artifact lands exactly once");

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
    if let Some(media) = signed_media {
        sqlx::query("delete from media where id = $1::uuid")
            .bind(&media)
            .execute(db.pool())
            .await
            .unwrap();
    }
}

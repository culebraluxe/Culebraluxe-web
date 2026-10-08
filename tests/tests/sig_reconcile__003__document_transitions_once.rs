//! SIG.RECONCILE — document transitions once (TST-SIG-RECONCILE-003).
//!
//! Contract: a completed request's document moves to `signed` exactly once. Re-delivering the same reconcile
//! event is absorbed — the same receipt answers it, no second transition, no second media row, and `signed_at`
//! does not move.
//!
//! Level: L1 Component — the real `SignatureService` and the real `SignatureDao` on DEV.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test sig_reconcile__003__document_transitions_once -- --ignored

use db::{Database, DbTarget};
use test_harness::providers::FakeSignatureProvider;
use uuid::Uuid;
use web::signature::SignatureService;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_reconcile_003__document_transitions_once() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("sig-reconcile-003-{}", Uuid::new_v4());

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
        "insert into signature_request (transaction_document_id, status) \
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
        b"%PDF executed".to_vec(),
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
        .reconcile_completed(&format!("evt-{tag}"), &request, &context)
        .await
        .expect("the first delivery reconciles");
    assert_eq!(first.outcome, model::SignatureCommandOutcome::Success);
    let first_signed_at: Option<String> =
        sqlx::query_scalar("select signed_at::text from transaction_document where id = $1::uuid")
            .bind(&doc)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(first_signed_at.is_some());

    // The same event delivered again is the same command, replayed: no second transition, no new media.
    let second = service
        .reconcile_completed(&format!("evt-{tag}"), &request, &context)
        .await
        .expect("the replay answers");
    assert_eq!(second.outcome, model::SignatureCommandOutcome::Success);

    let (state, signed_at): (String, Option<String>) = sqlx::query_as(
        "select state, signed_at::text from transaction_document where id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "signed");
    assert_eq!(
        signed_at, first_signed_at,
        "signed_at must not move on replay"
    );

    let media_count: i64 = sqlx::query_scalar("select count(*) from media where filename = $1")
        .bind(format!("signed-{tag}.pdf"))
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        media_count, 1,
        "exactly one signed artifact row is ever written"
    );

    // Clean up.
    let signed_media: Option<String> = sqlx::query_scalar(
        "select signed_media_id::text from transaction_document where id = $1::uuid",
    )
    .bind(&doc)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("delete from signature_request where transaction_document_id = $1::uuid")
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

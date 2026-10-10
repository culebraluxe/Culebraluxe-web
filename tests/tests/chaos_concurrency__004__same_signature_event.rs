//! CHAOS.CONCURRENCY — same signature event (TST-CHAOS-CONCURRENCY-004).
//!
//! Contract: two deliveries of the same signature state-change converge to one
//! transition. `SignatureDao::set_status_tx` updates `luxesign_request.status`
//! only from the expected status, so two concurrent `Sent → Signed` deliveries
//! produce exactly one winner; the loser matches zero rows instead of
//! overwriting. A replay of the same move afterwards matches nothing: the
//! event is absorbed, not applied twice.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus a refused replay.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_concurrency__004__same_signature_event -- --ignored

use db::{Database, DbTarget, SignatureDao};
use model::SignatureRequestStatus;
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_concurrency_004__same_signature_event() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(SignatureDao::new(db.clone()));
    let tag = format!("chaos-004-{}.pdf", Uuid::new_v4());

    // Minimal envelope in Sent: the event under test is the Sent → Signed delivery.
    let deal: String = sqlx::query_scalar("select id::text from deal limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one deal");
    let txdoc: String = sqlx::query_scalar(
        "insert into transaction_document (deal_id, document_type, title, state, source) \
         values ($1::uuid, 'agreement', $2, 'draft', 'generated') returning id::text",
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
    .bind(&txdoc)
    .fetch_one(db.pool())
    .await
    .expect("request fixture");

    // The same event delivered twice at once: one transition wins.
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (db, dao, barrier, request) =
            (db.clone(), dao.clone(), barrier.clone(), request.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            let mut tx = db.begin("chaos-proof-transition").await.expect("begin");
            let moved = dao
                .set_status_tx(
                    &mut tx,
                    &request,
                    SignatureRequestStatus::Sent,
                    SignatureRequestStatus::Signed,
                )
                .await
                .expect("transition answers")
                .is_some();
            tx.commit().await.expect("commit");
            moved
        }));
    }
    let mut wins = 0;
    for handle in handles {
        if handle.await.expect("transition panicked") {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "the same event applied twice moves the state once");

    // The replay is absorbed: with the request already Signed, the same move matches nothing.
    let mut tx = db.begin("chaos-proof-replay").await.unwrap();
    assert!(
        dao.set_status_tx(
            &mut tx,
            &request,
            SignatureRequestStatus::Sent,
            SignatureRequestStatus::Signed,
        )
        .await
        .expect("replay answers")
        .is_none(),
        "replaying the event after convergence changes nothing"
    );
    tx.commit().await.unwrap();
    let status: String =
        sqlx::query_scalar("select status from luxesign_request where id = $1::uuid")
            .bind(&request)
            .fetch_one(db.pool())
            .await
            .expect("status read");
    assert_eq!(status, "signed", "one event, one terminal state");

    sqlx::query("delete from luxesign_request where id = $1::uuid")
        .bind(&request)
        .execute(db.pool())
        .await
        .expect("request sweep");
    sqlx::query("delete from transaction_document where id = $1::uuid")
        .bind(&txdoc)
        .execute(db.pool())
        .await
        .expect("document sweep");
}

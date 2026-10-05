//! DB.CONCURRENCY — two signature webhooks (TST-DB-CONCURRENCY-009).
//!
//! Contract: two concurrent webhook deliveries for the same signature request
//! converge to one legal durable state. The `signature_request` table uses
//! a partial unique index on `(transaction_document_id)` where status is
//! active; exactly one webhook creates the request, the other finds the existing one.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__009__two_signature_webhooks -- --ignored

use db::{Database, DbTarget, SignatureDao};
use model::signature::{
    PrepareSignatureRequest, PreparedSignatureRecipient, SendSignatureRequest,
    SignatureCommandOutcome, SignatureRecipient, SignatureRecipientRole,
};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, doc_id: &str) {
    sqlx::query("delete from signature_envelope_recipient where signature_request_id in (select id from signature_request where transaction_document_id = $1::uuid)")
        .bind(doc_id)
        .execute(db.pool())
        .await
        .expect("recipient sweep");
    sqlx::query("delete from signature_request where transaction_document_id = $1::uuid")
        .bind(doc_id)
        .execute(db.pool())
        .await
        .expect("signature request sweep");
    sqlx::query("delete from transaction_document where id = $1::uuid")
        .bind(doc_id)
        .execute(db.pool())
        .await
        .expect("document sweep");
}

async fn create_document(db: &Database, doc_id: &str) {
    sqlx::query(
        r#"
        insert into transaction_document (id, deal_id, state, source_snapshot, created_at, updated_at)
        values ($1::uuid, $2::uuid, 'draft', '{}', now(), now())
        "#,
    )
    .bind(doc_id)
    .bind(Uuid::new_v4().to_string())
    .execute(db.pool())
    .await
    .expect("document fixture");
}

async fn count_signature_requests(db: &Database, doc_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*)::bigint from signature_request where transaction_document_id = $1::uuid",
    )
    .bind(doc_id)
    .fetch_one(db.pool())
    .await
    .expect("signature request count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_009__two_signature_webhooks() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(SignatureDao::new(db.clone()));

    // Test 1: Two webhooks race to prepare the same document
    let doc_id_1 = Uuid::new_v4().to_string();
    sweep(&db, &doc_id_1).await;
    create_document(&db, &doc_id_1).await;

    let recipients = vec![PreparedSignatureRecipient {
        name: "Test Signer".into(),
        email: "signer@example.test".into(),
        role: SignatureRecipientRole::Signer,
        order: 1,
        signing_step: 1,
        execution_role: None,
        execution_slot_id: None,
    }];

    let request = PrepareSignatureRequest {
        transaction_document_id: doc_id_1.clone(),
        recipients,
        message: Some("Please sign".into()),
        created_by_user_id: Some(Uuid::new_v4().to_string()),
    };

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (db_c, dao_c, barrier_c, request_c) =
            (db.clone(), dao.clone(), barrier.clone(), request.clone());
        handles.push(tokio::spawn(async move {
            barrier_c.arrive_and_wait().await;
            // Use a transaction to test the prepare path
            let mut tx = db_c.begin("signature.prepare").await.unwrap();
            dao_c.prepare_tx(&mut tx, &request_c).await
        }));
    }

    let mut created_count = 0;
    let mut existing_count = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(result) => {
                if result.existing {
                    existing_count += 1;
                } else {
                    created_count += 1;
                }
            }
            Err(e) => panic!("prepare failed: {}", e),
        }
    }
    assert_eq!(
        created_count, 1,
        "exactly one webhook creates the signature request"
    );
    assert_eq!(
        existing_count, 1,
        "the other webhook finds the existing request"
    );
    assert_eq!(
        count_signature_requests(&db, &doc_id_1).await,
        1,
        "exactly one signature request exists"
    );

    // Test 2: Two webhooks race to send (create) the same document
    let doc_id_2 = Uuid::new_v4().to_string();
    sweep(&db, &doc_id_2).await;
    create_document(&db, &doc_id_2).await;

    let send_request = SendSignatureRequest {
        command_id: Uuid::new_v4().to_string(),
        transaction_document_id: doc_id_2.clone(),
        recipients: vec![SignatureRecipient {
            name: "Test Signer".into(),
            email: "signer@example.test".into(),
            role: SignatureRecipientRole::Signer,
            order: 1,
            execution_role: None,
            execution_slot_id: None,
        }],
        message: Some("Please sign".into()),
        created_by_user_id: Some(Uuid::new_v4().to_string()),
        execution_role: None,
        execution_slot_id: None,
        slot_recipient_email: None,
        signature_role: None,
        completion_recipient_emails: Vec::new(),
    };

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, request) = (dao.clone(), barrier.clone(), send_request.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.send(&request, None).await
        }));
    }

    let mut success_count = 0;
    let mut conflict_count = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(result) => {
                if result.outcome == SignatureCommandOutcome::Success {
                    success_count += 1;
                } else if result.outcome == SignatureCommandOutcome::Conflict {
                    conflict_count += 1;
                }
            }
            Err(e) => panic!("send failed: {}", e),
        }
    }
    // The unique index on transaction_document_id with active status filter
    // means exactly one succeeds, the other gets Conflict
    assert_eq!(
        success_count + conflict_count,
        2,
        "both webhooks get a response"
    );
    assert_eq!(
        count_signature_requests(&db, &doc_id_2).await,
        1,
        "exactly one signature request exists"
    );

    // Test 3: Fault injection - one webhook crashes
    let doc_id_3 = Uuid::new_v4().to_string();
    sweep(&db, &doc_id_3).await;
    create_document(&db, &doc_id_3).await;

    let fault_request = PrepareSignatureRequest {
        transaction_document_id: doc_id_3.clone(),
        recipients: vec![PreparedSignatureRecipient {
            name: "Test Signer".into(),
            email: "signer@example.test".into(),
            role: SignatureRecipientRole::Signer,
            order: 1,
            signing_step: 1,
            execution_role: None,
            execution_slot_id: None,
        }],
        message: Some("Please sign".into()),
        created_by_user_id: Some(Uuid::new_v4().to_string()),
    };

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "webhook handler died"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (db_c, dao_c, barrier_c, injector_c, fault_request_c) = (
            db.clone(),
            dao.clone(),
            barrier.clone(),
            injector.clone(),
            fault_request.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier_c.arrive_and_wait().await;
            if injector_c.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            let mut tx = db_c.begin("signature.prepare").await.unwrap();
            dao_c
                .prepare_tx(&mut tx, &fault_request_c)
                .await
                .map_err(|e| e.to_string())
        }));
    }

    let mut success_count = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success_count += 1;
        }
    }
    assert_eq!(
        success_count, 1,
        "the survivor still creates the signature request"
    );
    assert_eq!(
        count_signature_requests(&db, &doc_id_3).await,
        1,
        "exactly one signature request exists"
    );

    // Cleanup
    sweep(&db, &doc_id_1).await;
    sweep(&db, &doc_id_2).await;
    sweep(&db, &doc_id_3).await;
}

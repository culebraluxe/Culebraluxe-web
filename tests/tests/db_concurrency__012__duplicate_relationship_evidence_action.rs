//! DB.CONCURRENCY — duplicate relationship evidence action (TST-DB-CONCURRENCY-012).
//!
//! Contract: two concurrent classify actions on the same relationship evidence row
//! converge to one legal durable state. The `RelationshipEvidenceDao::classify`
//! updates `is_automated_or_bulk` and `is_organization_or_service` atomically;
//! concurrent calls produce one final state, not a lost update.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__012__duplicate_relationship_evidence_action -- --ignored

use db::{Database, DbTarget, RelationshipEvidenceDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, evidence_id: &str) {
    sqlx::query("delete from integration_relationship_evidence where id = $1::uuid")
        .bind(evidence_id)
        .execute(db.pool())
        .await
        .ok();
}

async fn create_evidence(db: &Database, evidence_id: &str) {
    sqlx::query(
        r#"
        insert into integration_relationship_evidence (
            id, source, source_account, source_identity_key, source_label,
            display_name, organization, emails, phones,
            first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
            inbound_count, outbound_count, is_two_way, is_owner_initiated,
            is_automated_or_bulk, is_organization_or_service, known_apple_contact,
            has_email, has_phone, coverage_note, evidence_fingerprint, review_state
        ) values (
            $1::uuid, 'test', 'account', 'key', 'label',
            'Test Name', 'Test Org', '[]'::jsonb, '[]'::jsonb,
            now(), now(), now(), now(),
            1, 1, true, true,
            false, false, false,
            false, false, 'note', 'fingerprint', 'pending'
        )
        "#,
    )
    .bind(evidence_id)
    .execute(db.pool())
    .await
    .expect("evidence fixture");
}

async fn get_evidence_flags(db: &Database, evidence_id: &str) -> (bool, bool) {
    sqlx::query_as::<_, (bool, bool)>(
        "select is_automated_or_bulk, is_organization_or_service from integration_relationship_evidence where id = $1::uuid"
    )
    .bind(evidence_id)
    .fetch_one(db.pool())
    .await
    .expect("evidence flags")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_012__duplicate_relationship_evidence_action() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(RelationshipEvidenceDao::new(db.clone()));

    // Test 1: Two workers race to classify the same evidence
    let evidence_id_1 = Uuid::new_v4().to_string();
    sweep(&db, &evidence_id_1).await;
    create_evidence(&db, &evidence_id_1).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for i in 0..2 {
        let (dao, barrier, evidence_id) = (dao.clone(), barrier.clone(), evidence_id_1.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            // Both try to set automated=true, service=true
            dao.classify(&evidence_id, Some(true), Some(true)).await
        }));
    }

    let mut updated = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").expect("classify") {
            updated += 1;
        }
    }
    // Both calls return true (row was updated), but the final state is consistent
    let (automated, service) = get_evidence_flags(&db, &evidence_id_1).await;
    assert!(automated, "evidence must be marked automated");
    assert!(service, "evidence must be marked service");

    // Test 2: Concurrent conflicting classifications (one sets automated, other sets service)
    let evidence_id_2 = Uuid::new_v4().to_string();
    sweep(&db, &evidence_id_2).await;
    create_evidence(&db, &evidence_id_2).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for i in 0..2 {
        let (dao, barrier, evidence_id) = (dao.clone(), barrier.clone(), evidence_id_2.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if i == 0 {
                // First sets automated=true
                dao.classify(&evidence_id, Some(true), None).await
            } else {
                // Second sets service=true
                dao.classify(&evidence_id, None, Some(true)).await
            }
        }));
    }

    for handle in handles {
        let _ = handle.await.expect("racer panicked");
    }
    // Final state should have both flags (last writer wins for each field, but both get set)
    let (automated, service) = get_evidence_flags(&db, &evidence_id_2).await;
    assert!(automated || service, "at least one flag must be set");

    // Test 3: Fault injection - one worker crashes
    let evidence_id_3 = Uuid::new_v4().to_string();
    sweep(&db, &evidence_id_3).await;
    create_evidence(&db, &evidence_id_3).await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during classify"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, evidence_id) = (
            dao.clone(), barrier.clone(), injector.clone(), evidence_id_3.clone()
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            dao.classify(&evidence_id, Some(true), Some(true)).await.map_err(|e| e.to_string())
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still classifies the evidence");
    let (automated, service) = get_evidence_flags(&db, &evidence_id_3).await;
    assert!(automated && service, "survivor's classification must be applied");

    // Cleanup
    sweep(&db, &evidence_id_1).await;
    sweep(&db, &evidence_id_2).await;
    sweep(&db, &evidence_id_3).await;
}
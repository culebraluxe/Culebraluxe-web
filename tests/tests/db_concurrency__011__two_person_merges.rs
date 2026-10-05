//! DB.CONCURRENCY — two person merges (TST-DB-CONCURRENCY-011).
//!
//! Contract: two concurrent attempts to merge the same two person records
//! converge to one legal durable state. The `PropertyDao::merge_parcel_record`
//! uses `FOR UPDATE` on the source record and moves all references atomically;
//! exactly one merge wins, the other finds no source to merge.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__011__two_person_merges -- --ignored

use db::{Database, DbTarget, PropertyDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, target_id: &str, catastro: &str) {
    sqlx::query("delete from property where id = $1::uuid")
        .bind(target_id)
        .execute(db.pool())
        .await
        .ok();
    // Also clean up any property with this catastro
    sqlx::query("delete from property where regexp_replace(coalesce(catastro_number, ''), '[^0-9]', '', 'g') = $1")
        .bind(catastro)
        .execute(db.pool())
        .await
        .ok();
}

async fn create_property(db: &Database, id: &str, name: &str, catastro: &str) {
    sqlx::query(
        r#"
        insert into property (id, name, catastro_number, status, is_active_listing, is_published, created_at, updated_at)
        values ($1::uuid, $2, $3, 'active', false, false, now(), now())
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(catastro)
    .execute(db.pool())
    .await
    .expect("property fixture");
}

async fn count_properties_with_catastro(db: &Database, catastro: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*)::bigint from property where regexp_replace(coalesce(catastro_number, ''), '[^0-9]', '', 'g') = $1"
    )
    .bind(catastro)
    .fetch_one(db.pool())
    .await
    .expect("count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_011__two_person_merges() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(PropertyDao::new(db.clone()));

    // Test 1: Two workers race to merge the same source into the same target
    let catastro = "123456789";
    let target_id = Uuid::new_v4().to_string();
    let source_id = Uuid::new_v4().to_string();
    
    sweep(&db, &target_id, catastro).await;
    create_property(&db, &target_id, "Target Property", catastro).await;
    create_property(&db, &source_id, "Source Property", catastro).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, target_id, catastro) = (dao.clone(), barrier.clone(), target_id.clone(), catastro.to_string());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.merge_parcel_record(&target_id, &catastro).await
        }));
    }

    let mut merged = 0;
    let mut not_found = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(Some(_)) => merged += 1,
            Ok(None) => not_found += 1,
            Err(e) => panic!("merge failed: {}", e),
        }
    }
    assert_eq!(merged, 1, "exactly one merge succeeds");
    assert_eq!(not_found, 1, "the other finds no source to merge");
    assert_eq!(count_properties_with_catastro(&db, catastro).await, 1, "only one property remains with that catastro");

    // Test 2: Fault injection - one worker crashes
    let catastro2 = "987654321";
    let target_id2 = Uuid::new_v4().to_string();
    let source_id2 = Uuid::new_v4().to_string();
    
    sweep(&db, &target_id2, catastro2).await;
    create_property(&db, &target_id2, "Target 2", catastro2).await;
    create_property(&db, &source_id2, "Source 2", catastro2).await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during merge"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, target_id, catastro) = (
            dao.clone(), barrier.clone(), injector.clone(), target_id2.clone(), catastro2.to_string()
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            dao.merge_parcel_record(&target_id, &catastro).await.map_err(|e| e.to_string())
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still completes the merge");
    assert_eq!(count_properties_with_catastro(&db, catastro2).await, 1, "only one property remains");

    // Test 3: Three workers race (more concurrent pressure)
    let catastro3 = "555555555";
    let target_id3 = Uuid::new_v4().to_string();
    let source_id3 = Uuid::new_v4().to_string();
    
    sweep(&db, &target_id3, catastro3).await;
    create_property(&db, &target_id3, "Target 3", catastro3).await;
    create_property(&db, &source_id3, "Source 3", catastro3).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let (dao, barrier, target_id, catastro) = (dao.clone(), barrier.clone(), target_id3.clone(), catastro3.to_string());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.merge_parcel_record(&target_id, &catastro).await
        }));
    }

    let mut merged = 0;
    let mut not_found = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(Some(_)) => merged += 1,
            Ok(None) => not_found += 1,
            Err(e) => panic!("merge failed: {}", e),
        }
    }
    assert_eq!(merged, 1, "exactly one merge succeeds with 3 racers");
    assert_eq!(not_found, 2, "the other two find no source");
    assert_eq!(count_properties_with_catastro(&db, catastro3).await, 1, "only one property remains");

    // Cleanup
    sweep(&db, &target_id, catastro).await;
    sweep(&db, &target_id2, catastro2).await;
    sweep(&db, &target_id3, catastro3).await;
}
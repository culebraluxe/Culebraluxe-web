//! DB.CONCURRENCY — two person merges (TST-DB-CONCURRENCY-011).
//!
//! Contract: two concurrent attempts to merge the same two records converge to ONE legal durable
//! state. `PropertyDao::merge_parcel_record` takes `FOR UPDATE` on the source row and moves every
//! reference atomically, so exactly one merge wins and the other finds no source left to merge.
//!
//! THE RACERS RUN THE PRODUCTION PATH. The merge is called inside `db::service_mutation`, which is
//! what the route does (`web/src/properties/mod.rs`, `merge_parcel_record` inside `service_mutation`)
//! — the DAO alone is not the contract. Called bare on an autocommit connection the same code cannot
//! hold its transaction, which is a property of calling it wrong rather than of the merge; the earlier
//! version of this file did exactly that and measured nothing about production.
//!
//! Fixtures are unique per run: the catastros are derived from a fresh UUID, so a sweep can never
//! reach a property this test did not create, and two lanes running at once cannot collide.
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

/// Nine digits of the run's own UUID — a catastro no other row (and no other lane) can hold.
struct Catastros {
    first: String,
    second: String,
    third: String,
}

impl Catastros {
    fn for_this_run() -> Self {
        let seed = Uuid::new_v4().as_u128();
        let nine = |value: u128| format!("{:09}", value % 1_000_000_000);
        Self {
            first: nine(seed),
            second: nine(seed / 1_000_000_000),
            third: nine(seed / 1_000_000_000_000_000_000),
        }
    }
}

async fn sweep(db: &Database, id: &str, catastro: &str) {
    sqlx::query("delete from property where id = $1::uuid")
        .bind(id)
        .execute(db.pool())
        .await
        .ok();
    sqlx::query(
        "delete from property where regexp_replace(coalesce(catastro_number, ''), '[^0-9]', '', 'g') = $1",
    )
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

/// One racer: the merge exactly as production calls it — inside the service mutation, so the
/// transaction that holds `FOR UPDATE` outlives the DAO call.
async fn race_merge(
    db: Database,
    dao: Arc<PropertyDao>,
    target_id: String,
    catastro: String,
) -> Result<Option<String>, db::DbFailure> {
    db::service_mutation(Some(db), async move {
        dao.merge_parcel_record(&target_id, &catastro).await
    })
    .await
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_011__two_person_merges() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(PropertyDao::new(db.clone()));
    let catastros = Catastros::for_this_run();

    // ---------------------------------------------------------------------------------------------------
    // Test 1: two workers race to merge the same source into the same target, through the service path.
    // ---------------------------------------------------------------------------------------------------
    let catastro = catastros.first.clone();
    let target_id = Uuid::new_v4().to_string();
    let source_id = Uuid::new_v4().to_string();

    sweep(&db, &target_id, &catastro).await;
    create_property(&db, &target_id, "Target Property", &catastro).await;
    create_property(&db, &source_id, "Source Property", &catastro).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (db, dao, barrier, target_id, catastro) = (
            db.clone(),
            dao.clone(),
            barrier.clone(),
            target_id.clone(),
            catastro.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            race_merge(db, dao, target_id, catastro).await
        }));
    }

    let mut merged = 0;
    let mut not_found = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(Some(_)) => merged += 1,
            Ok(None) => not_found += 1,
            Err(error) => panic!("merge failed: {error}"),
        }
    }
    assert_eq!(merged, 1, "exactly one merge succeeds");
    assert_eq!(not_found, 1, "the other finds no source to merge");
    assert_eq!(
        count_properties_with_catastro(&db, &catastro).await,
        1,
        "only one property remains with that catastro"
    );

    // ---------------------------------------------------------------------------------------------------
    // Test 2: one worker crashes mid-race; the survivor still completes the merge through the service path.
    // ---------------------------------------------------------------------------------------------------
    let catastro2 = catastros.second.clone();
    let target_id2 = Uuid::new_v4().to_string();
    let source_id2 = Uuid::new_v4().to_string();

    sweep(&db, &target_id2, &catastro2).await;
    create_property(&db, &target_id2, "Target 2", &catastro2).await;
    create_property(&db, &source_id2, "Source 2", &catastro2).await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during merge"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (db, dao, barrier, injector, target_id, catastro) = (
            db.clone(),
            dao.clone(),
            barrier.clone(),
            injector.clone(),
            target_id2.clone(),
            catastro2.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            race_merge(db, dao, target_id, catastro)
                .await
                .map_err(|error| error.to_string())
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still completes the merge");
    assert_eq!(
        count_properties_with_catastro(&db, &catastro2).await,
        1,
        "only one property remains"
    );

    // ---------------------------------------------------------------------------------------------------
    // Test 3: three workers race, so the losers outnumber the winner.
    // ---------------------------------------------------------------------------------------------------
    let catastro3 = catastros.third.clone();
    let target_id3 = Uuid::new_v4().to_string();
    let source_id3 = Uuid::new_v4().to_string();

    sweep(&db, &target_id3, &catastro3).await;
    create_property(&db, &target_id3, "Target 3", &catastro3).await;
    create_property(&db, &source_id3, "Source 3", &catastro3).await;

    let barrier = Arc::new(ConcurrencyBarrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let (db, dao, barrier, target_id, catastro) = (
            db.clone(),
            dao.clone(),
            barrier.clone(),
            target_id3.clone(),
            catastro3.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            race_merge(db, dao, target_id, catastro).await
        }));
    }

    let mut merged = 0;
    let mut not_found = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(Some(_)) => merged += 1,
            Ok(None) => not_found += 1,
            Err(error) => panic!("merge failed: {error}"),
        }
    }
    assert_eq!(merged, 1, "exactly one merge succeeds with 3 racers");
    assert_eq!(not_found, 2, "the other two find no source");
    assert_eq!(
        count_properties_with_catastro(&db, &catastro3).await,
        1,
        "only one property remains"
    );

    // Cleanup — the fixtures this run created, matched by its own unique catastros.
    sweep(&db, &target_id, &catastros.first).await;
    sweep(&db, &target_id2, &catastros.second).await;
    sweep(&db, &target_id3, &catastros.third).await;
}

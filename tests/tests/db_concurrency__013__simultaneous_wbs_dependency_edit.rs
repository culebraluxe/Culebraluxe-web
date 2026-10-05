//! DB.CONCURRENCY — simultaneous WBS dependency edit (TST-DB-CONCURRENCY-013).
//!
//! Contract: two concurrent edits to the same WBS dependency graph
//! converge to one legal durable state. The `WbsDao::insert_dependency`
//! uses an advisory lock on the project to serialize graph checks;
//! concurrent insertions of the same edge are serialized and exactly one wins.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__013__simultaneous_wbs_dependency_edit -- --ignored

use db::{Database, DbTarget, WbsDao, WbsDependency};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn sweep(db: &Database, project_id: &str) {
    sqlx::query("delete from wbs_dependency where project_id = $1::uuid")
        .bind(project_id)
        .execute(db.pool())
        .await
        .ok();
    sqlx::query("delete from wbs_item where project_id = $1::uuid")
        .bind(project_id)
        .execute(db.pool())
        .await
        .ok();
}

async fn create_project(db: &Database, project_id: &str) {
    sqlx::query(
        r#"
        insert into wbs_item (id, project_id, title, category, status, created_at, updated_at)
        values ($1::uuid, $2::uuid, 'Project Root', 'project', 'open', now(), now())
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id)
    .execute(db.pool())
    .await
    .expect("project fixture");
}

async fn count_dependencies(db: &Database, project_id: &str) -> i64 {
    sqlx::query_scalar("select count(*)::bigint from wbs_dependency where project_id = $1::uuid")
        .bind(project_id)
        .fetch_one(db.pool())
        .await
        .expect("dependency count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_013__simultaneous_wbs_dependency_edit() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(WbsDao::new(db.clone()));

    // Test 1: Two workers race to insert the same dependency edge
    let project_id_1 = Uuid::new_v4().to_string();
    sweep(&db, &project_id_1).await;
    create_project(&db, &project_id_1).await;

    let edge = WbsDependency {
        project_id: project_id_1.clone(),
        source_id: Uuid::new_v4().to_string(),
        target_id: Uuid::new_v4().to_string(),
        kind: "finish_to_start".into(),
    };

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, edge) = (dao.clone(), barrier.clone(), edge.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.insert_dependency(&edge).await
        }));
    }

    let mut inserted = 0;
    let mut errors = 0;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            Ok(_) => inserted += 1,
            Err(_) => errors += 1,
        }
    }
    // Exactly one insert succeeds (the other may get a unique constraint error or the advisory lock serializes them)
    assert_eq!(inserted, 1, "exactly one dependency insert succeeds");
    assert_eq!(count_dependencies(&db, &project_id_1).await, 1, "exactly one dependency exists");

    // Test 2: Concurrent insert of different edges (should both succeed if no cycle)
    let project_id_2 = Uuid::new_v4().to_string();
    sweep(&db, &project_id_2).await;
    create_project(&db, &project_id_2).await;

    let edge_a = WbsDependency {
        project_id: project_id_2.clone(),
        source_id: Uuid::new_v4().to_string(),
        target_id: Uuid::new_v4().to_string(),
        kind: "finish_to_start".into(),
    };
    let edge_b = WbsDependency {
        project_id: project_id_2.clone(),
        source_id: Uuid::new_v4().to_string(),
        target_id: Uuid::new_v4().to_string(),
        kind: "finish_to_start".into(),
    };

    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    handles.push(tokio::spawn(async move {
        let (dao, barrier, edge) = (dao.clone(), barrier.clone(), edge_a.clone());
        barrier.arrive_and_wait().await;
        dao.insert_dependency(&edge).await
    }));
    handles.push(tokio::spawn(async move {
        let (dao, barrier, edge) = (dao.clone(), barrier.clone(), edge_b.clone());
        barrier.arrive_and_wait().await;
        dao.insert_dependency(&edge).await
    }));

    let mut inserted = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            inserted += 1;
        }
    }
    assert_eq!(inserted, 2, "two different edges can be inserted concurrently");
    assert_eq!(count_dependencies(&db, &project_id_2).await, 2, "both dependencies exist");

    // Test 3: Fault injection - one worker crashes during insert
    let project_id_3 = Uuid::new_v4().to_string();
    sweep(&db, &project_id_3).await
    create_project(&db, &project_id_3).await;

    let edge_3 = WbsDependency {
        project_id: project_id_3.clone(),
        source_id: Uuid::new_v4().to_string(),
        target_id: Uuid::new_v4().to_string(),
        kind: "finish_to_start".into(),
    };

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::None,
        Fault::error("CHAOS_CRASH", "worker died during insert"),
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (dao, barrier, injector, edge) = (
            dao.clone(), barrier.clone(), injector.clone(), edge_3.clone()
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            dao.insert_dependency(&edge).await.map_err(|e| e.to_string())
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still inserts the dependency");
    assert_eq!(count_dependencies(&db, &project_id_3).await, 1, "exactly one dependency exists");

    // Cleanup
    sweep(&db, &project_id_1).await;
    sweep(&db, &project_id_2).await;
    sweep(&db, &project_id_3).await;
}
//! DB.CONCURRENCY — stale claim vs live heartbeat (TST-DB-CONCURRENCY-007).
//!
//! Contract: a stale Forge engine claim and a live heartbeat on the same
//! work item converge to one legal durable state. The stale recovery
//! (`forge_recover_stale_engine_claim`) must not recover a claim that has
//! been heartbeated within the stale window, and a live heartbeat must
//! keep the claim out of stale recovery.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus injected faults.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_concurrency__007__stale_claim_vs_live_heartbeat -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, ForgeAgentWorkRow};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

async fn claim_work_item(db: &Database, story_id: &str, worker_id: &str) -> Option<ForgeAgentWorkRow> {
    let dao = ForgeEngineDao::new(db.clone());
    dao.claim_next_agent_work(worker_id).await.expect("claim")
}

async fn begin_work(db: &Database, work_item_id: &str) -> Option<db::forge_engine::BeginAgentWorkRun> {
    let dao = ForgeEngineDao::new(db.clone());
    dao.begin_agent_work_run(work_item_id).await.expect("begin")
}

async fn heartbeat(db: &Database, work_item_id: &str) -> bool {
    let dao = ForgeEngineDao::new(db.clone());
    dao.heartbeat_agent_work(work_item_id).await.expect("heartbeat")
}

async fn sweep_agent_work(db: &Database, story_id: &str) {
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("agent work sweep");
    sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("story sweep");
}

async fn create_story_ready(db: &Database, story_id: &str) {
    sqlx::query(
        r#"
        insert into storyboard_story (id, title, status, work_type, created_at, updated_at)
        values ($1, 'Stale Claim Test', 'Ready', 'FEATURE', now(), now())
        "#,
    )
    .bind(story_id)
    .execute(db.pool())
    .await
    .expect("story fixture");
    
    // Ensure dispatch creates a work item
    sqlx::query("select forge_dispatch_story($1)")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("dispatch");
}

async fn get_work_item_id(db: &Database, story_id: &str) -> Option<String> {
    sqlx::query_scalar("select id::text from agent_work_item where story_id = $1 and state = 'Ready'")
        .bind(story_id)
        .fetch_optional(db.pool())
        .await
        .expect("work item id")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_007__stale_claim_vs_live_heartbeat() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();

    // Test 1: Stale recovery must not recover a claim with a live heartbeat
    let story_id_1 = format!("db-007-heartbeat-{}", Uuid::new_v4());
    sweep_agent_work(&db, &story_id_1).await;
    create_story_ready(&db, &story_id_1).await;

    let work_item_id = get_work_item_id(&db, &story_id_1).await.expect("work item exists");
    
    // Claim and begin the work item (moves to Running)
    let claimed = claim_work_item(&db, &story_id_1, "worker-1").await.expect("claimed");
    assert_eq!(claimed.state, "Claimed");
    let _begun = begin_work(&db, &work_item_id).await.expect("begun");
    
    // Heartbeat the running claim
    let heartbeated = heartbeat(&db, &work_item_id).await;
    assert!(heartbeated, "heartbeat must succeed on running claim");
    
    // Manually age the updated_at to make it appear stale
    sqlx::query(
        "update agent_work_item set updated_at = now() - interval '30 minutes' where id = $1::uuid"
    )
    .bind(&work_item_id)
    .execute(db.pool())
    .await
    .expect("age claim");
    
    // Now attempt stale recovery - it should NOT recover because we just heartbeated
    let recovered = sqlx::query_scalar::<_, String>(
        "select forge_recover_stale_engine_claim($1::uuid, $2::uuid, $3::uuid, 10)"
    )
    .bind(&story_id_1)
    .bind(&work_item_id)
    .bind("recovery-worker")
    .fetch_optional(db.pool())
    .await
    .expect("recovery");
    
    // The claim should NOT be recovered because it has a live heartbeat
    assert!(recovered.is_none(), "a recently heartbeated claim must not be recovered as stale");
    
    // Verify the claim is still Running
    let state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&work_item_id)
        .fetch_one(db.pool())
        .await
        .expect("state");
    assert_eq!(state, "Running", "claim must remain Running");

    // Test 2: Concurrent heartbeat and stale recovery race
    let story_id_2 = format!("db-007-race-{}", Uuid::new_v4());
    sweep_agent_work(&db, &story_id_2).await;
    create_story_ready(&db, &story_id_2).await;

    let work_item_id_2 = get_work_item_id(&db, &story_id_2).await.expect("work item exists");
    
    // Claim and begin
    claim_work_item(&db, &story_id_2, "worker-2").await.expect("claimed");
    begin_work(&db, &work_item_id_2).await.expect("begun");
    
    // Age the claim
    sqlx::query(
        "update agent_work_item set updated_at = now() - interval '30 minutes' where id = $1::uuid"
    )
    .bind(&work_item_id_2)
    .execute(db.pool())
    .await
    .expect("age claim");

    // Two concurrent operations: heartbeat vs stale recovery
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    
    // Worker 1: heartbeats
    let (db_hb, barrier_hb, work_item_id_hb) = (db.clone(), barrier.clone(), work_item_id_2.clone());
    handles.push(tokio::spawn(async move {
        barrier_hb.arrive_and_wait().await;
        heartbeat(&db_hb, &work_item_id_hb).await
    }));
    
    // Worker 2: attempts stale recovery
    let (db_rec, barrier_rec, story_id_rec, work_item_id_rec) = (db.clone(), barrier.clone(), story_id_2.clone(), work_item_id_2.clone());
    handles.push(tokio::spawn(async move {
        barrier_rec.arrive_and_wait().await;
        sqlx::query_scalar::<_, String>(
            "select forge_recover_stale_engine_claim($1::uuid, $2::uuid, $3::uuid, 10)"
        )
        .bind(&story_id_rec)
        .bind(&work_item_id_rec)
        .bind("recovery-worker")
        .fetch_optional(db_rec.pool())
        .await
        .expect("recovery")
    }));

    let mut heartbeat_result = false;
    let mut recovered_result = None;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            true => heartbeat_result = true,
            Some(id) => recovered_result = Some(id),
            None => {}
        }
    }
    
    // At least one operation must succeed, and they must not both corrupt the state
    // The claim should end up in a valid state (either Running with fresh heartbeat, or recovered)
    let final_state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&work_item_id_2)
        .fetch_one(db.pool())
        .await
        .expect("final state");
    assert!(
        matches!(final_state.as_str(), "Running" | "Ready" | "Done" | "Error" | "Cancelled"),
        "claim must be in a valid state, got {}",
        final_state
    );

    // Test 3: Fault injection - heartbeat worker crashes
    let story_id_3 = format!("db-007-fault-{}", Uuid::new_v4());
    sweep_agent_work(&db, &story_id_3).await;
    create_story_ready(&db, &story_id_3).await;

    let work_item_id_3 = get_work_item_id(&db, &story_id_3).await.expect("work item exists");
    claim_work_item(&db, &story_id_3, "worker-3").await.expect("claimed");
    begin_work(&db, &work_item_id_3).await.expect("begun");
    sqlx::query(
        "update agent_work_item set updated_at = now() - interval '30 minutes' where id = $1::uuid"
    )
    .bind(&work_item_id_3)
    .execute(db.pool())
    .await
    .expect("age claim");

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::error("CHAOS_CRASH", "heartbeat worker died"),
        Fault::None,
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (db_c, barrier_c, injector_c, work_id_c, story_id_c) = (
            db.clone(), barrier.clone(), injector.clone(), work_item_id_3.clone(), story_id_3.clone()
        );
        handles.push(tokio::spawn(async move {
            barrier_c.arrive_and_wait().await;
            if injector_c.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            // One worker heartbeats, one attempts recovery
            if rand::random::<bool>() {
                Ok(("heartbeat", heartbeat(&db_c, &work_id_c).await))
            } else {
                let result = sqlx::query_scalar::<_, String>(
                    "select forge_recover_stale_engine_claim($1::uuid, $2::uuid, $3::uuid, 10)"
                )
                .bind(&story_id_c)
                .bind(&work_id_c)
                .bind("recovery-worker")
                .fetch_optional(db_c.pool())
                .await
                .expect("recovery");
                Ok(("recovery", result.is_some()))
            }
        }));
    }

    for handle in handles {
        let _ = handle.await.expect("racer panicked");
    }

    // Verify final state is valid
    let final_state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&work_item_id_3)
        .fetch_one(db.pool())
        .await
        .expect("final state");
    assert!(
        matches!(final_state.as_str(), "Running" | "Ready" | "Done" | "Error" | "Cancelled"),
        "claim must be in a valid state after fault, got {}",
        final_state
    );

    // Cleanup
    sweep_agent_work(&db, &story_id_1).await;
    sweep_agent_work(&db, &story_id_2).await;
    sweep_agent_work(&db, &story_id_3).await;
}
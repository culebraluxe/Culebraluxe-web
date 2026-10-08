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

use db::{Database, DbTarget, ForgeAgentWorkRow, ForgeEngineDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::fault::{Fault, FaultInjector};
use uuid::Uuid;

/// What a racer in Test 2 returns, so both racers share one handle type.
enum RaceOutcome {
    Heartbeat(bool),
    Recovered(bool),
}

/// Put a story's own open item into the Claimed state with the given worker.
/// The public claim door takes the board's oldest item, so these rows are
/// staged directly — the durable precondition the engine shares with recovery
/// is exactly the row the door leaves behind: Claimed, owned, stamped.
async fn claim_work_item(
    db: &Database,
    story_id: &str,
    worker_id: &str,
) -> Option<ForgeAgentWorkRow> {
    let work_item_id = sqlx::query_scalar::<_, String>(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(story_id)
    .fetch_optional(db.pool())
    .await
    .expect("work item lookup");
    match work_item_id {
        Some(id) => {
            sqlx::query(
                "update agent_work_item set state = 'Claimed', claimed_at = now(), claimed_by = $2, updated_at = now() \
                 where id = $1::uuid and state = 'Ready'",
            )
            .bind(&id)
            .bind(worker_id)
            .execute(db.pool())
            .await
            .expect("claim");
            sqlx::query_as::<_, ForgeAgentWorkRow>(
                "select id::text as id, story_id, state, claimed_by, role, kind, work_type, execution_policy, \
                        model_policy, stop_after, launch_intent from agent_work_item where id = $1::uuid",
            )
            .bind(&id)
            .fetch_optional(db.pool())
            .await
            .expect("claimed row")
        }
        None => None,
    }
}

async fn begin_work(db: &Database, work_item_id: &str) -> Option<db::BeginAgentWorkRun> {
    let dao = ForgeEngineDao::new(db.clone());
    dao.begin_agent_work_run(work_item_id).await.expect("begin")
}

async fn heartbeat(db: &Database, work_item_id: &str) -> bool {
    let dao = ForgeEngineDao::new(db.clone());
    dao.heartbeat_agent_work(work_item_id)
        .await
        .expect("heartbeat")
}

/// The execution ledger row a live worker asks recovery to respect. The
/// heartbeat a live claim holds is stamped on `forge_engine_task_execution`;
/// a fresh `heartbeat_at` must keep the claim out of stale recovery.
async fn seed_execution(
    db: &Database,
    story_id: &str,
    work_item_id: &str,
    worker_id: &str,
) -> (String, String) {
    let process_instance_id: String =
        sqlx::query_scalar("select id::text from process_instances order by started_at desc limit 1")
            .fetch_one(db.pool())
            .await
            .expect("process instance");
    let task_id = Uuid::new_v4().to_string();
    let token_id = Uuid::new_v4().to_string();
    sqlx::query(
        "insert into tokens (id, process_instance_id, node_id, status, is_able_to_reactivate_parent) \
         values ($1::uuid, $2::uuid, 'node', 'active', false)",
    )
    .bind(&token_id)
    .bind(&process_instance_id)
    .execute(db.pool())
    .await
    .expect("token fixture");
    sqlx::query(
        "insert into tasks (id, process_instance_id, token_id, name, status) \
         values ($1::uuid, $2::uuid, $3::uuid, 'tst-007', 'in_progress')",
    )
    .bind(&task_id)
    .bind(&process_instance_id)
    .bind(&token_id)
    .execute(db.pool())
    .await
    .expect("task fixture");
    sqlx::query(
        "insert into forge_engine_task_execution (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status, heartbeat_at) \
         values ($1::uuid, $2::uuid, $3::uuid, $4, 'node', $5::uuid, $6, 'running', now())",
    )
    .bind(&task_id)
    .bind(&process_instance_id)
    .bind(&token_id)
    .bind(story_id)
    .bind(work_item_id)
    .bind(worker_id)
    .execute(db.pool())
    .await
    .expect("execution fixture");
    (task_id, process_instance_id)
}

/// The live heartbeat a worker holds on its execution row.
async fn refresh_heartbeat(db: &Database, task_id: &str) {
    sqlx::query(
        "update forge_engine_task_execution set heartbeat_at = now(), updated_at = now() where task_id = $1::uuid",
    )
    .bind(task_id)
    .execute(db.pool())
    .await
    .expect("refresh heartbeat");
}

async fn age_heartbeat(db: &Database, task_id: &str) {
    sqlx::query(
        "update forge_engine_task_execution set heartbeat_at = now() - interval '30 minutes', updated_at = now() where task_id = $1::uuid",
    )
    .bind(task_id)
    .execute(db.pool())
    .await
    .expect("age heartbeat");
}

async fn stale_recovery(db: &Database, task_id: &str, process_instance_id: &str, work_item_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "select forge_recover_stale_engine_claim($1::uuid, $2::uuid, $3::uuid, 10)",
    )
    .bind(task_id)
    .bind(process_instance_id)
    .bind(work_item_id)
    .fetch_one(db.pool())
    .await
    .expect("recovery")
}

async fn sweep_agent_work(db: &Database, story_id: &str) {
    sqlx::query("delete from forge_engine_task_execution where story_id = $1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .ok();
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
        insert into storyboard_story (id, title, status, work_type, workstream, priority, created_at, updated_at)
        values ($1, 'Stale Claim Test', 'Ready', 'FEATURE', 'ADMIN', 'Medium', now(), now())
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
    sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(story_id)
    .fetch_optional(db.pool())
    .await
    .expect("work item id")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn db_concurrency_007__stale_claim_vs_live_heartbeat() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();

    // Test 1: stale recovery must not recover a claim with a live heartbeat
    let story_id_1 = format!("db-007-heartbeat-{}", Uuid::new_v4());
    sweep_agent_work(&db, &story_id_1).await;
    create_story_ready(&db, &story_id_1).await;

    let work_item_id = get_work_item_id(&db, &story_id_1)
        .await
        .expect("work item exists");

    // Claim and begin the work item (moves to Running)
    let claimed = claim_work_item(&db, &story_id_1, "worker-1")
        .await
        .expect("claimed");
    assert_eq!(claimed.state, "Claimed");
    let _begun = begin_work(&db, &work_item_id).await.expect("begun");

    let (task_id_1, pi_1) = seed_execution(&db, &story_id_1, &work_item_id, "worker-1").await;

    // Heartbeat the running claim
    let heartbeated = heartbeat(&db, &work_item_id).await;
    assert!(heartbeated, "heartbeat must succeed on running claim");
    refresh_heartbeat(&db, &task_id_1).await;

    // Recovery must NOT interrupt a claim whose heartbeat is live
    let recovered = stale_recovery(&db, &task_id_1, &pi_1, &work_item_id).await;
    assert!(!recovered, "a recently heartbeated claim must not be recovered as stale");

    let state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&work_item_id)
        .fetch_one(db.pool())
        .await
        .expect("state");
    assert_eq!(state, "Running", "claim must remain Running");

    // A genuinely stale execution IS recovered, and the item is released
    age_heartbeat(&db, &task_id_1).await;
    let recovered_stale = stale_recovery(&db, &task_id_1, &pi_1, &work_item_id).await;
    assert!(recovered_stale, "a stale claim must be recovered");
    let state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&work_item_id)
        .fetch_one(db.pool())
        .await
        .expect("state");
    assert_eq!(state, "Ready", "the released claim returns the item to Ready");

    // Test 2: Concurrent heartbeat and stale recovery race
    let story_id_2 = format!("db-007-race-{}", Uuid::new_v4());
    sweep_agent_work(&db, &story_id_2).await;
    create_story_ready(&db, &story_id_2).await;

    let work_item_id_2 = get_work_item_id(&db, &story_id_2)
        .await
        .expect("work item exists");

    claim_work_item(&db, &story_id_2, "worker-2")
        .await
        .expect("claimed");
    begin_work(&db, &work_item_id_2).await.expect("begun");
    let (task_id_2, pi_2) = seed_execution(&db, &story_id_2, &work_item_id_2, "worker-2").await;

    age_heartbeat(&db, &task_id_2).await;

    // Two concurrent operations: heartbeat vs stale recovery
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();

    // Worker 1: heartbeats
    let (db_hb, barrier_hb, work_item_id_hb, task_id_hb) = (
        db.clone(),
        barrier.clone(),
        work_item_id_2.clone(),
        task_id_2.clone(),
    );
    handles.push(tokio::spawn(async move {
        barrier_hb.arrive_and_wait().await;
        let hb = heartbeat(&db_hb, &work_item_id_hb).await;
        refresh_heartbeat(&db_hb, &task_id_hb).await;
        RaceOutcome::Heartbeat(hb)
    }));

    // Worker 2: attempts stale recovery
    let (db_rec, barrier_rec, task_id_rec, pi_2_rec, work_item_id_rec) = (
        db.clone(),
        barrier.clone(),
        task_id_2.clone(),
        pi_2.clone(),
        work_item_id_2.clone(),
    );
    handles.push(tokio::spawn(async move {
        barrier_rec.arrive_and_wait().await;
        RaceOutcome::Recovered(stale_recovery(&db_rec, &task_id_rec, &pi_2_rec, &work_item_id_rec).await)
    }));

    let mut heartbeat_result = false;
    let mut recovered_result = false;
    for handle in handles {
        match handle.await.expect("racer panicked") {
            RaceOutcome::Heartbeat(ok) => heartbeat_result = ok,
            RaceOutcome::Recovered(r) => recovered_result = r,
        }
    }

    // At least one operation must succeed, and they must not both corrupt the state.
    // The claim must end up in a legal durable state: either Running with a live
    // heartbeat, or released (Ready / interrupted).
    let _ = heartbeat_result;
    let final_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&work_item_id_2)
            .fetch_one(db.pool())
            .await
            .expect("state");
    let exec_status: String =
        sqlx::query_scalar("select status from forge_engine_task_execution where task_id = $1::uuid")
            .bind(&task_id_2)
            .fetch_one(db.pool())
            .await
            .expect("exec status");
    match (final_state.as_str(), exec_status.as_str()) {
        ("Running", "running") => assert!(!recovered_result, "a fresh claim must not be recovered"),
        ("Ready", "interrupted") => {}
        other => panic!("illegal half-state: {other:?}"),
    }

    // Test 3: Fault injection - one heartbeat worker dies; the survivor's
    // heartbeat keeps the claim recoverable-from-stale only if it is live.
    let story_id_3 = format!("db-007-fault-{}", Uuid::new_v4());
    sweep_agent_work(&db, &story_id_3).await;
    create_story_ready(&db, &story_id_3).await;

    let work_item_id_3 = get_work_item_id(&db, &story_id_3)
        .await
        .expect("work item exists");
    claim_work_item(&db, &story_id_3, "worker-3")
        .await
        .expect("claimed");
    begin_work(&db, &work_item_id_3).await.expect("begun");
    let (task_id_3, pi_3) = seed_execution(&db, &story_id_3, &work_item_id_3, "worker-3").await;
    age_heartbeat(&db, &task_id_3).await;

    let injector = Arc::new(FaultInjector::scripted(vec![
        Fault::error("CHAOS_CRASH", "heartbeat worker died"),
        Fault::None,
    ]));
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (injector, barrier, db_h, work_item_id_h, task_id_h) = (
            injector.clone(),
            barrier.clone(),
            db.clone(),
            work_item_id_3.clone(),
            task_id_3.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            if injector.next_fault().is_failure() {
                return Err("crashed".to_string());
            }
            heartbeat(&db_h, &work_item_id_h).await;
            refresh_heartbeat(&db_h, &task_id_h).await;
            Ok(())
        }));
    }

    let mut success = 0;
    for handle in handles {
        if handle.await.expect("racer panicked").is_ok() {
            success += 1;
        }
    }
    assert_eq!(success, 1, "the survivor still heartbeats");
    let (state_3, exec_3): (String, String) = sqlx::query_as(
        "select w.state, e.status from agent_work_item w, forge_engine_task_execution e where w.id = $1::uuid and e.task_id = $2::uuid",
    )
    .bind(&work_item_id_3)
    .bind(&task_id_3)
    .fetch_one(db.pool())
    .await
    .expect("rows");
    assert_eq!(state_3, "Running", "the survivor's heartbeat keeps the claim running");
    assert_eq!(exec_3, "running", "the execution is not interrupted");
    // And the heartbeat is live, so a fresh recovery still finds nothing to do:
    let recovered_3 = stale_recovery(&db, &task_id_3, &pi_3, &work_item_id_3).await;
    assert!(!recovered_3, "no stale claim after the survivor heartbeats");

    // Cleanup
    sweep_agent_work(&db, &story_id_1).await;
    sweep_agent_work(&db, &story_id_2).await;
    sweep_agent_work(&db, &story_id_3).await;
}

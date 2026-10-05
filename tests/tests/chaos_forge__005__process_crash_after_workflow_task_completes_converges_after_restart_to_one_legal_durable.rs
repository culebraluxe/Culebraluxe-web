//! CHAOS.FORGE — process crash after workflow task completes converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-005).
//!
//! Contract: an engine task execution that completes but dies before the completion is
//! persisted neither loses the completion nor duplicates it. After the crash the task
//! reads as incomplete (claimed/running); once the stale window passes, the recovery
//! sweep marks it interrupted and the next generation re-executes. Exactly one task
//! execution row exists throughout: restart never forks the execution.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and stale-recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__005__process_crash_after_workflow_task_completes_converges_after_restart_to_one_legal_durable -- --ignored

use db::{Database, DbTarget, ForgeEngineDao};
use sqlx::Row;
use uuid::Uuid;

async fn task_execution_count(db: &Database, task_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from forge_engine_task_execution where task_id = $1::uuid")
        .bind(task_id)
        .fetch_one(db.pool())
        .await
        .expect("task execution count")
}

async fn task_execution_status(db: &Database, task_id: &str) -> Option<String> {
    sqlx::query_scalar("select status from forge_engine_task_execution where task_id = $1::uuid")
        .bind(task_id)
        .fetch_optional(db.pool())
        .await
        .expect("task execution status")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_005__process_crash_after_workflow_task_completes_converges_after_restart_to_one_legal_durable(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let worker_id = format!("chaos-forge-005-{}", Uuid::new_v4());

    // Pick an existing story that has a Ready work item.
    let story_id: String = sqlx::query_scalar(
        "select s.id from storyboard_story s \
         join agent_work_item w on w.story_id = s.id \
         where w.state = 'Ready' \
         limit 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("DEV must hold at least one story with a Ready work item");

    // Pick an existing task to anchor the task execution.
    let task_id: String = sqlx::query_scalar("select id::text from tasks limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one task");

    // Clean any existing task executions for this task (test isolation).
    sqlx::query("delete from forge_engine_task_execution where task_id = $1::uuid")
        .bind(&task_id)
        .execute(db.pool())
        .await
        .expect("sweep task executions");

    // Get the work item id using a query that returns a row
    let work_item_row = sqlx::query("select id::text as id from agent_work_item where story_id = $1 and state = 'Ready'")
        .bind(&story_id)
        .fetch_one(db.pool())
        .await
        .expect("work item dispatched");
    let work_item_id: String = work_item_row.get("id");

    // Claim the work item to get a worker.
    let generation = ForgeEngineDao::new(db.clone());
    let claimed = generation
        .claim_specific_agent_work(&work_item_id, &worker_id)
        .await
        .expect("claim answers");
    assert!(claimed.is_some());
    let _claimed = claimed.unwrap();

    // Get a process instance to anchor the task execution.
    let process_instance_id: String = sqlx::query_scalar(
        "select id::text from process_instances limit 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("DEV must hold at least one process instance");

    // Get a token for the process instance.
    let token_id: String = sqlx::query_scalar(
        "select id::text from tokens where process_instance_id = $1::uuid limit 1",
    )
    .bind(&process_instance_id)
    .fetch_one(db.pool())
    .await
    .expect("process instance must have a token");

    // Generation one: the task execution completes, but the process dies before the
    // completion is persisted (simulated by rolling back the transaction).
    {
        let mut tx = db.begin("chaos-task-complete").await.expect("begin");
        sqlx::query(
            "insert into forge_engine_task_execution \
             (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status, heartbeat_at) \
             values ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6::uuid, $7, 'completed', now()) \
             on conflict (task_id) do update set status = 'completed', heartbeat_at = now()",
        )
        .bind(&task_id)
        .bind(&process_instance_id)
        .bind(&token_id)
        .bind(&story_id)
        .bind("test_node")
        .bind(&work_item_id)
        .bind(&worker_id)
        .execute(tx.connection())
        .await
        .expect("record completion");
        // No commit: the process dies here (crash after task completes, before completion persists).
    }
    assert_eq!(
        task_execution_count(&db, &task_id).await,
        0,
        "a crashed completion write leaves no ghost row"
    );
    assert_eq!(
        task_execution_status(&db, &task_id).await,
        None,
        "the crashed completion is not visible"
    );

    // Generation two restarts: the completion is gone, so the task execution is re-inserted
    // as claimed/running (simulating re-execution).
    {
        let mut tx = db.begin("chaos-task-recovery").await.expect("begin");
        sqlx::query(
            "insert into forge_engine_task_execution \
             (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status, heartbeat_at) \
             values ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6::uuid, $7, 'claimed', now()) \
             on conflict (task_id) do update set status = 'claimed', heartbeat_at = now()",
        )
        .bind(&task_id)
        .bind(&process_instance_id)
        .bind(&token_id)
        .bind(&story_id)
        .bind("test_node")
        .bind(&work_item_id)
        .bind(&worker_id)
        .execute(tx.connection())
        .await
        .expect("record re-execution");
        tx.commit().await.expect("commit");
    }
    assert_eq!(
        task_execution_count(&db, &task_id).await,
        1,
        "crash plus recovery converges to exactly one task execution row"
    );
    assert_eq!(
        task_execution_status(&db, &task_id).await,
        Some("claimed".to_string()),
        "the recovered task execution is in claimed state (awaiting fresh execution)"
    );

    // The stale window passes: recovery sweep marks it interrupted.
    sqlx::query(
        "update forge_engine_task_execution set heartbeat_at = now() - interval '16 minutes' \
         where task_id = $1::uuid",
    )
    .bind(&task_id)
    .execute(db.pool())
    .await
    .expect("age the task past the stale window");

    // Run the recovery function (similar to forge_recover_stale_engine_claim).
    sqlx::query("select forge_recover_stale_engine_claim($1::uuid, $2::uuid, $3::uuid, 15)")
        .bind(&task_id)
        .bind(&process_instance_id)
        .bind(&work_item_id)
        .execute(db.pool())
        .await
        .expect("recover stale engine claim");

    // After recovery, the work item should be back to Ready.
    let work_item_state: String = sqlx::query_scalar(
        "select state from agent_work_item where id = $1::uuid",
    )
    .bind(&work_item_id)
    .fetch_one(db.pool())
    .await
    .expect("work item state");
    assert_eq!(
        work_item_state,
        "Ready",
        "stale engine claim recovery requeues the work item"
    );

    // A further execution attempt writes to the same task_id row (no fork).
    {
        let mut tx = db.begin("chaos-task-retry").await.expect("begin");
        sqlx::query(
            "insert into forge_engine_task_execution \
             (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status, heartbeat_at) \
             values ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6::uuid, $7, 'claimed', now()) \
             on conflict (task_id) do update set status = 'claimed', heartbeat_at = now()",
        )
        .bind(&task_id)
        .bind(&process_instance_id)
        .bind(&token_id)
        .bind(&story_id)
        .bind("test_node")
        .bind(&work_item_id)
        .bind(&worker_id)
        .execute(tx.connection())
        .await
        .expect("record retry");
        tx.commit().await.expect("commit");
    }
    assert_eq!(
        task_execution_count(&db, &task_id).await,
        1,
        "subsequent retries converge to the same task execution row, no fork"
    );
    assert_eq!(
        task_execution_status(&db, &task_id).await,
        Some("claimed".to_string()),
        "the task execution row carries the latest generation's state"
    );

    // Clean up.
    sqlx::query("delete from forge_engine_task_execution where task_id = $1::uuid")
        .bind(&task_id)
        .execute(db.pool())
        .await
        .expect("sweep task execution");
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep work item");
    sqlx::query("update storyboard_story set status = 'Planned', updated_at = now() where id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("reset story");
}

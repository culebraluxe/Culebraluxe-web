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
//! Adapted to the fenced generation (migration 279, Slice 4 of FORGE-B1), because two of this test's
//! old moves now describe a database that no longer exists:
//!   * `forge_recover_stale_engine_claim(uuid, uuid, uuid, integer)` was DROPPED by 279 (the same migration
//!     dropped `forge_requeue_stale_work`). Recovery runs the CAS-fenced `forge_recover_stale_work`, so the
//!     test drives the engine's own sweep, `ForgeResetDao::recover_stale_engine_claims`, instead of naming a
//!     function that is gone.
//!   * A ledger row is now fenced by `forge_bind_engine_execution_generation`: a BEFORE INSERT trigger that
//!     binds `claim_generation` from the item's ACTIVE owner and refuses an insert whose `worker_id` owns
//!     nothing. That makes the claim a precondition of every insert (and it fires on the conflict path too).
//! Recovery also has two clocks, not one: the sweep's candidate query reads the EXECUTION heartbeat while the
//! fenced recovery decides staleness from the WORK ITEM's `updated_at` — aging one and not the other leaves the
//! recovery answering `no_longer_stale`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__005__process_crash_after_workflow_task_completes_converges_after_restart_to_one_legal_durable -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, ForgeResetDao};
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

    // Pick an existing story that can legally carry a run — not `Complete` (its board state is terminal and
    // `forge_settlement_pair`, migration 263, refuses an open item beside it) and not `Hold` (recovery would send
    // the item to `Error` rather than `Ready`, which is a different test's contract).
    let story_id: String = sqlx::query_scalar(
        "select id from storyboard_story where status not in ('Complete', 'Hold') limit 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("DEV must hold at least one story to anchor the work item");

    // Isolate the story: executions first (their `work_item_id` FK is RESTRICT), then the work items.
    sqlx::query("delete from forge_engine_task_execution where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep story executions");
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep story work items");

    // Dispatch through the board's own verb. `forge_dispatch_story` (migration 265) bounces the story
    // `Ready → Planned → Ready` itself and raises if the Ready change creates no item — the dispatch trigger fires
    // on a *change* into `Ready`, so forcing `Ready` onto a story that is already `Ready` dispatches nothing at
    // all (observed: `RowNotFound` on the work item lookup).
    let work_item_id: String = sqlx::query_scalar("select item from forge_dispatch_story($1)")
        .bind(&story_id)
        .fetch_one(db.pool())
        .await
        .expect("the board dispatches exactly one work item for the story");

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

    // Claim the work item to get a worker. The claim is not optional bookkeeping: the ledger row's generation is
    // bound from the item's ACTIVE owner by `forge_bind_engine_execution_generation` (migration 279), so an
    // execution inserted without a held claim is refused by the database.
    {
        let generation = ForgeEngineDao::new(db.clone());
        let claimed = generation
            .claim_specific_agent_work(&work_item_id, &worker_id)
            .await
            .expect("claim answers");
        assert!(
            claimed.is_some(),
            "generation one holds the work item the execution ledger needs"
        );
        let claimed = claimed.unwrap();
        assert_eq!(
            claimed.state, "Claimed",
            "generation one moves item to Claimed"
        );
    }

    // Get a process instance to anchor the task execution.
    let process_instance_id: String =
        sqlx::query_scalar("select id::text from process_instances limit 1")
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

    // The stale window passes. BOTH clocks have to age: the sweep's candidate query reads the EXECUTION
    // heartbeat (`ForgeResetDao::recover_stale_engine_claims`), while the fenced recovery decides staleness from
    // the WORK ITEM's `updated_at` (migration 279). Aging one and not the other leaves the recovery answering
    // `no_longer_stale`, which reads as a skipped sweep and leaves the item held forever.
    sqlx::query(
        "update agent_work_item set updated_at = now() - interval '16 minutes' where id = $1::uuid",
    )
    .bind(&work_item_id)
    .execute(db.pool())
    .await
    .expect("age the work item past the stale window");
    sqlx::query(
        "update forge_engine_task_execution set heartbeat_at = now() - interval '16 minutes' \
         where task_id = $1::uuid",
    )
    .bind(&task_id)
    .execute(db.pool())
    .await
    .expect("age the task past the stale window");

    // Recovery sweep: the engine's own sweep, so this exercises the same path the runtime uses rather than a
    // hand-written statement. The un-fenced `forge_recover_stale_engine_claim` this test used to call by name was
    // DROPPED in migration 279 (with `forge_requeue_stale_work`); it now delegates to the CAS-fenced
    // `forge_recover_stale_work`. The sweep is fleet-wide, so `recovered >= 1` is the honest assertion — DEV may
    // hold other leftovers — and the facts below are asserted against THIS task's row.
    let (recovered, _skipped) = ForgeResetDao::new(db.clone())
        .recover_stale_engine_claims(15, 10)
        .await
        .expect("recovery sweep answers");
    assert!(
        recovered >= 1,
        "the sweep recovers the stale engine claim (1 observed, {recovered} reported)"
    );

    // After recovery, the work item should be back to Ready, and the execution row it held must be interrupted:
    // a ledger row left 'claimed' beside an unowned item is the fork this test exists to forbid.
    let work_item_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&work_item_id)
            .fetch_one(db.pool())
            .await
            .expect("work item state");
    assert_eq!(
        work_item_state, "Ready",
        "stale engine claim recovery requeues the work item"
    );
    assert_eq!(
        task_execution_status(&db, &task_id).await,
        Some("interrupted".to_string()),
        "the recovered execution is interrupted, not silently completed"
    );
    assert_eq!(
        task_execution_count(&db, &task_id).await,
        1,
        "recovery leaves exactly one execution row for the task"
    );

    // A further execution attempt writes to the same task_id row (no fork). The fresh generation holds the item
    // again first: the generation-binding trigger fires on the INSERT attempt of this upsert too, so an
    // execution write with no held claim is refused even when the row already exists.
    {
        let generation = ForgeEngineDao::new(db.clone());
        let reclaimed = generation
            .claim_specific_agent_work(&work_item_id, &worker_id)
            .await
            .expect("recovery claim answers");
        assert!(
            reclaimed.is_some(),
            "generation three reclaims the requeued item the ledger write needs"
        );
    }
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
    sqlx::query("delete from forge_engine_task_execution where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep story executions");
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

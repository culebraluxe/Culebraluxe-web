//! SEAM-DB-002 / 004 / 005 / 006 — the Forge execution seam against DEV.
//!
//! Run single-threaded (these all write the same queue/board and read counts back):
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_seam_execution_dev -- --ignored --test-threads=1
//!
//! These are the real-database proofs under the Forge vertical smoke stack, the ones that catch defects component
//! mocks cannot: a story that never leaves `Planned`, a `story_run_id` that stays NULL, an engine fault that
//! condemns a story instead of clearing it, a broken engine that spins one story forever, and a stale execution
//! that is recovered twice. Every transition is the database's stored routine; the Rust here only drives the
//! production DAO and asserts the committed rows.

use db::{
    AgentWorkOutcome, Database, DbTarget, EnsureDispatch, ForgeControlDao, ForgeEngineDao,
    ForgeResetDao,
};

async fn delete_story(pool: &sqlx::PgPool, story_id: &str) {
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(pool)
        .await;
}

/// SEAM-DB-002 — a story walks `Planned → Ready → claimed → running` with exactly one work item and a Story Run
/// that is actually opened, stamped on the item, and committed.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn story_dispatches_through_run_open() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-DISPATCH-{tag}");

    // A Planned story with a real specification, so the run snapshot can be told from NULL.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes, goal, scope)
         values ($1, 'PROOF', 'Dispatch proof', 'High', 'Planned', '', 'prove the dispatch', 'rust/core/db')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert Planned story");

    // Dispatch is the board's own door (`forge_dispatch_story`, migration 265): Planned -> Ready, one item.
    let item = match engine
        .ensure_story_dispatched(&story)
        .await
        .expect("dispatch")
    {
        EnsureDispatch::Queued { item } => item,
        other => panic!("expected Queued, got {other:?}"),
    };
    let board: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(&story)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(board, "Ready");
    let ready_items: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(ready_items, 1, "exactly one Ready work item");

    // Claim, then begin. The begin is the write that opens the Story Run and stamps it on the item.
    let claimed = engine
        .claim_specific_agent_work(&item, "proof-worker")
        .await
        .unwrap()
        .expect("claim");
    assert_eq!(claimed.state, "Claimed");
    let begun = engine
        .begin_agent_work_run(&item)
        .await
        .unwrap()
        .expect("begin");

    // Committed database truth, not the returned struct.
    let (state, story_run_id): (String, Option<String>) =
        sqlx::query_as("select state, story_run_id::text from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(state, "Running");
    let run_id = story_run_id.expect("story_run_id stamped at begin");
    assert_eq!(
        run_id, begun.story_run_id,
        "the item names the run begin opened"
    );
    let (run_story, run_status, run_ended, run_goal): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select story_id, result_status::text, ended_at::text, goal_snapshot
           from storyboard_story_run where id = $1::uuid",
    )
    .bind(&run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(run_story, story, "the run belongs to the claim's story");
    assert!(run_status.is_none(), "an unruled run is not a verdict");
    assert!(run_ended.is_none(), "a run that just started has not ended");
    assert_eq!(
        run_goal.as_deref(),
        Some("prove the dispatch"),
        "the run snapshots the story's spec"
    );

    delete_story(pool, &story).await;
}

/// SEAM-DB-004 — an engine fault clears the pair back into the queue instead of condemning the story.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn engine_fault_clears_the_pair_and_rules_nothing() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-CLEAR-{tag}");

    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Clear proof', 'High', 'Ready', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert Ready story");
    let item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .expect("the Ready trigger created one item");

    engine
        .claim_specific_agent_work(&item, "proof-worker")
        .await
        .unwrap()
        .expect("claim");
    engine
        .begin_agent_work_run(&item)
        .await
        .unwrap()
        .expect("begin");

    // The engine's own plumbing failed mid-run. `Abandoned` is the settlement for that: the story is decided
    // nothing, so the pair goes back to `Ready`.
    let cleared = engine
        .finish_agent_work_run(
            &item,
            AgentWorkOutcome::Abandoned,
            Some("DatabaseUnavailable during workflow.step (sqlstate 25P03)"),
        )
        .await
        .unwrap()
        .expect("the engine-fault settle must land");
    assert_eq!(cleared.item_state, "Ready");
    assert_eq!(cleared.story_status.as_deref(), Some("Ready"));

    let (state, claimed_by, claimed_at, started, finished, error_text): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select state, claimed_by, claimed_at::text, started_at::text, finished_at::text, error_text
           from agent_work_item where id = $1::uuid",
    )
    .bind(&item)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(state, "Ready");
    assert_eq!(claimed_by, None, "a cleared row names nobody");
    assert_eq!(claimed_at, None);
    assert_eq!(started, None, "a cleared row is not a run that started");
    assert_eq!(finished, None, "and it is not finished either");
    assert!(error_text.unwrap_or_default().contains("25P03"));

    // The claim's attempt is retained (not reset), so the retry budget keeps counting.
    let attempts: i32 =
        sqlx::query_scalar("select attempts from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);

    // The Story Run carries no false verdict: an engine fault ruled nothing, so result_status stays NULL even
    // though the run ended.
    let run_status: Option<String> = sqlx::query_scalar(
        "select r.result_status::text from agent_work_item i
          join storyboard_story_run r on r.id = i.story_run_id
         where i.id = $1::uuid",
    )
    .bind(&item)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(run_status, None, "an engine fault is not a story verdict");

    // The same item is genuinely reclaimable.
    assert!(
        engine
            .claim_specific_agent_work(&item, "proof-worker")
            .await
            .unwrap()
            .is_some(),
        "a cleared item must be claimable again"
    );

    delete_story(pool, &story).await;
}

/// SEAM-DB-005 — retry exhaustion: a broken engine retries up to `max_attempts`, then terminalizes, never looping.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn retry_exhaustion_terminates_the_pair() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-EXHAUST-{tag}");

    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Exhaust proof', 'High', 'Ready', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert Ready story");
    let item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .expect("the Ready trigger created one item");

    async fn attempts(pool: &sqlx::PgPool, item: &str) -> i32 {
        sqlx::query_scalar::<_, i32>("select attempts from agent_work_item where id = $1::uuid")
            .bind(item)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    async fn board(pool: &sqlx::PgPool, story: &str) -> String {
        sqlx::query_scalar::<_, String>("select status from storyboard_story where id = $1")
            .bind(story)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    // max_attempts is the item's default 3 (migration 028). Two cleared runs retry; the third is terminal.
    for expected_attempt in 1..=3 {
        engine
            .claim_specific_agent_work(&item, "proof-worker")
            .await
            .unwrap()
            .expect("claim");
        assert_eq!(
            attempts(pool, &item).await,
            expected_attempt,
            "a claim counts its attempt"
        );
        engine
            .begin_agent_work_run(&item)
            .await
            .unwrap()
            .expect("begin");
        let settled = engine
            .finish_agent_work_run(&item, AgentWorkOutcome::Abandoned, Some("still broken"))
            .await
            .unwrap()
            .expect("settle");
        if expected_attempt < 3 {
            assert_eq!(
                settled.item_state, "Ready",
                "attempt {expected_attempt} retries"
            );
            assert_eq!(settled.story_status.as_deref(), Some("Ready"));
        } else {
            assert_eq!(
                settled.item_state, "Error",
                "attempt {expected_attempt} is terminal"
            );
            assert_eq!(settled.story_status.as_deref(), Some("Hold"));
        }
    }

    // Final item/story/run state, together.
    let (state, attempts_now): (String, i32) =
        sqlx::query_as("select state, attempts from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((state.as_str(), attempts_now), ("Error", 3));
    assert_eq!(board(pool, &story).await, "Hold");

    // No infinite queue loop: a terminal item is neither claimable nor re-settleable.
    assert!(
        engine
            .claim_specific_agent_work(&item, "proof-worker")
            .await
            .unwrap()
            .is_none(),
        "an exhausted item must not be claimable"
    );
    assert!(
        engine
            .finish_agent_work_run(&item, AgentWorkOutcome::Abandoned, Some("again"))
            .await
            .unwrap()
            .is_none(),
        "an exhausted item must not settle again"
    );

    delete_story(pool, &story).await;
}

/// One persistence chain behind a `forge_engine_task_execution` row: a process definition, an active process
/// instance, a token and a task. Returns the ids the caller must clean up.
async fn process_chain(
    pool: &sqlx::PgPool,
    tag: &str,
    story_id: &str,
) -> (String, String, String, String) {
    let definition_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "insert into process_definitions (id, key, name, definition, status)
         values ($1::uuid, $2, 'proof', '{}'::jsonb, 'active')",
    )
    .bind(&definition_id)
    .bind(format!("def-{tag}"))
    .execute(pool)
    .await
    .expect("def");
    let instance_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "insert into process_instances (id, definition_id, status, subject_type, subject_id)
         values ($1::uuid, $2::uuid, 'active', 'story', $3)",
    )
    .bind(&instance_id)
    .bind(&definition_id)
    .bind(story_id)
    .execute(pool)
    .await
    .expect("instance");
    let token_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("insert into tokens (id, process_instance_id, node_id) values ($1::uuid, $2::uuid, 'smith')")
        .bind(&token_id)
        .bind(&instance_id)
        .execute(pool)
        .await
        .expect("token");
    let task_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "insert into tasks (id, process_instance_id, token_id, name, status)
         values ($1::uuid, $2::uuid, $3::uuid, 'smith', 'ready')",
    )
    .bind(&task_id)
    .bind(&instance_id)
    .bind(&token_id)
    .execute(pool)
    .await
    .expect("task");
    (definition_id, instance_id, token_id, task_id)
}

/// SEAM-DB-006 (a) — Agent Work ownership: a stale claimed item is read by `stale_agent_work` and requeued by
/// `requeue_stale_work`, clearing the owner and moving the pair together.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn stale_agent_work_owner_is_cleared_and_requeued() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let control = ForgeControlDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-STALE-AW-{tag}");

    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Stale agent work proof', 'High', 'Ready', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert Ready story");
    let item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .expect("the Ready trigger created one item");

    engine
        .claim_specific_agent_work(&item, "dead-worker")
        .await
        .unwrap()
        .expect("claim");
    // Backdate the claim so it reads as stale.
    sqlx::query(
        "update agent_work_item set updated_at = now() - interval '10 minutes' where id = $1::uuid",
    )
    .bind(&item)
    .execute(pool)
    .await
    .unwrap();

    let stale = control.stale_agent_work(1).await.unwrap();
    assert!(
        stale.iter().any(|row| row.id == item),
        "the backdated claim must read as stale"
    );

    control
        .requeue_stale_work(&item, &story)
        .await
        .expect("requeue");

    let (state, owner): (String, Option<String>) =
        sqlx::query_as("select state, claimed_by from agent_work_item where id = $1::uuid")
            .bind(&item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(state, "Ready");
    assert_eq!(owner, None, "a requeued claim names nobody");
    let board: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(&story)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(board, "Ready", "the story goes back with its item");

    delete_story(pool, &story).await;
}

/// SEAM-DB-006 (b) — durable engine-claim ownership: the engine's own recovery path interrupts a stale execution,
/// releases its item, skips a fresh heartbeat, and is a no-op the second time.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn stale_engine_claim_is_interrupted_and_never_repeats() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let reset = ForgeResetDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();

    // One story/claim for the stale execution, one for the fresh heartbeat.
    let stale_story = format!("ENG-PROOF-STALE-EC-{tag}");
    let fresh_story = format!("ENG-PROOF-FRESH-EC-{tag}");
    for story in [&stale_story, &fresh_story] {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes)
             values ($1, 'PROOF', 'Stale engine claim proof', 'High', 'Ready', '')",
        )
        .bind(story)
        .execute(pool)
        .await
        .expect("insert Ready story");
    }
    let stale_item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&stale_story)
    .fetch_one(pool)
    .await
    .expect("stale item");
    let fresh_item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&fresh_story)
    .fetch_one(pool)
    .await
    .expect("fresh item");

    for item in [&stale_item, &fresh_item] {
        engine
            .claim_specific_agent_work(item, "dead-worker")
            .await
            .unwrap()
            .expect("claim");
    }

    // A process chain + engine-claim row for each, one stale heartbeat and one fresh.
    let (_, stale_instance, stale_token, stale_task) =
        process_chain(pool, &format!("stale-{tag}"), &stale_story).await;
    let (_, fresh_instance, fresh_token, fresh_task) =
        process_chain(pool, &format!("fresh-{tag}"), &fresh_story).await;
    sqlx::query(
        "insert into forge_engine_task_execution
             (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status, heartbeat_at)
         values ($1::uuid, $2::uuid, $3::uuid, $4, 'smith', $5::uuid, 'dead-worker', 'claimed', now() - interval '10 minutes')",
    )
    .bind(&stale_task)
    .bind(&stale_instance)
    .bind(&stale_token)
    .bind(&stale_story)
    .bind(&stale_item)
    .execute(pool)
    .await
    .expect("stale engine claim");
    sqlx::query(
        "insert into forge_engine_task_execution
             (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status, heartbeat_at)
         values ($1::uuid, $2::uuid, $3::uuid, $4, 'smith', $5::uuid, 'dead-worker', 'claimed', now())",
    )
    .bind(&fresh_task)
    .bind(&fresh_instance)
    .bind(&fresh_token)
    .bind(&fresh_story)
    .bind(&fresh_item)
    .execute(pool)
    .await
    .expect("fresh engine claim");

    // The engine's own recovery path: the stale execution is interrupted and its item released; the fresh one is
    // untouched.
    reset
        .recover_stale_engine_claims(1, 100)
        .await
        .expect("recover");

    let stale_execution: String = sqlx::query_scalar(
        "select status from forge_engine_task_execution where task_id = $1::uuid",
    )
    .bind(&stale_task)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        stale_execution, "interrupted",
        "a stale execution is interrupted"
    );
    let (stale_item_state, stale_owner): (String, Option<String>) =
        sqlx::query_as("select state, claimed_by from agent_work_item where id = $1::uuid")
            .bind(&stale_item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(stale_item_state, "Ready");
    assert_eq!(stale_owner, None, "the released item names nobody");

    let fresh_execution: String = sqlx::query_scalar(
        "select status from forge_engine_task_execution where task_id = $1::uuid",
    )
    .bind(&fresh_task)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        fresh_execution, "claimed",
        "a fresh heartbeat is never recovered"
    );
    let fresh_item_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&fresh_item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        fresh_item_state, "Claimed",
        "a live claim survives untouched"
    );

    // Second recovery is a no-op on the already-interrupted execution.
    reset
        .recover_stale_engine_claims(1, 100)
        .await
        .expect("recover again");
    let stale_execution_after: String = sqlx::query_scalar(
        "select status from forge_engine_task_execution where task_id = $1::uuid",
    )
    .bind(&stale_task)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        stale_execution_after, "interrupted",
        "second recovery changes nothing"
    );

    // Clean up in FK order: execution rows, then the item/story, then the process chain.
    let _ = sqlx::query("delete from forge_engine_task_execution where task_id = any($1::uuid[])")
        .bind(vec![stale_task.clone(), fresh_task.clone()])
        .execute(pool)
        .await;
    for story in [&stale_story, &fresh_story] {
        delete_story(pool, story).await;
    }
    for instance in [&stale_instance, &fresh_instance] {
        let _ = sqlx::query("delete from tasks where process_instance_id = $1::uuid")
            .bind(instance)
            .execute(pool)
            .await;
        let _ = sqlx::query("delete from tokens where process_instance_id = $1::uuid")
            .bind(instance)
            .execute(pool)
            .await;
        let _ = sqlx::query("delete from process_instances where id = $1::uuid")
            .bind(instance)
            .execute(pool)
            .await;
    }
}

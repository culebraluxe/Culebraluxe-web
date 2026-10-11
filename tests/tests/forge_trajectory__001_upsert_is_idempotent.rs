//! FORGE TRAJECTORY — one turn's trajectory row replaces itself; a retried settlement cannot double a turn.
//!
//! Ignored by default: it writes rows against an existing Story Run on DEV and removes them again.
//! Run with: APP_ENV=development cargo test -p test-harness --test forge_trajectory__001_upsert_is_idempotent -- --ignored
use db::{Database, DbTarget, ForgeEngineDao, RunTrajectoryCounts, RunTrajectoryTurn};

fn turn(attempt_key: &str) -> RunTrajectoryTurn {
    RunTrajectoryTurn {
        attempt_key: attempt_key.to_string(),
        profile_id: "smith-structured-v1".to_string(),
        profile_version: Some(1),
        generation_id: None,
        task_id: Some("task-1".to_string()),
        role_attempt: Some(0),
        workflow_node: Some("smith".to_string()),
        role: Some("forge.smith".to_string()),
        model: None,
        provider: None,
        turn_started_at: None,
        turn_ended_at: None,
        outcome: Some("completed".to_string()),
        first_edit_latency_ms: Some(150),
        candidate_sha: Some("candidate-a".to_string()),
        counts: RunTrajectoryCounts {
            tool_call_count: 3,
            edit_call_count: 1,
            shell_call_count: 2,
            structured_call_count: 0,
            first_edit_at_ms: Some(200),
            last_edit_at_ms: Some(200),
            verification_calls_after_last_edit: 2,
        },
        usage: None,
        usage_unknown: false,
    }
}

#[tokio::test]
#[ignore = "writes one trajectory row against an existing DEV Story Run"]
async fn a_repeated_trajectory_replaces_the_summary_for_its_run_and_turn() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let run_id: String = sqlx::query_scalar(
        "select id::text from storyboard_story_run order by created_at desc limit 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("a Story Run exists on DEV");
    let attempt_key = "test-1:role-attempt:0";
    let dao = ForgeEngineDao::new(db.clone());

    dao.upsert_run_trajectory_turn(&run_id, &turn(attempt_key))
        .await
        .expect("the first trajectory is recorded");
    // The same turn settled again: the summary is replaced, not added to.
    dao.upsert_run_trajectory_turn(&run_id, &turn(attempt_key))
        .await
        .expect("a retried trajectory is recorded");

    let (rows, tool_calls, outcome): (i64, i32, Option<String>) = sqlx::query_as(
        "select count(*), max(tool_call_count), max(outcome) from storyboard_run_trajectory
          where run_id = $1::uuid and attempt_key = $2",
    )
    .bind(&run_id)
    .bind(attempt_key)
    .fetch_one(db.pool())
    .await
    .expect("the row is read");
    assert_eq!(rows, 1, "one row per run and turn");
    assert_eq!(tool_calls, 3, "the retry does not double the counts");
    assert_eq!(outcome.as_deref(), Some("completed"));

    sqlx::query(
        "delete from storyboard_run_trajectory where run_id = $1::uuid and attempt_key = $2",
    )
    .bind(&run_id)
    .bind(attempt_key)
    .execute(db.pool())
    .await
    .expect("the test row is removed");
}

//! FORGE TRAJECTORY — the trajectory row for a run and profile replaces itself; a retried settlement cannot double it.
//!
//! Ignored by default: it writes one row against an existing Story Run on DEV and removes it again.
//! Run with: APP_ENV=development cargo test -p test-harness --test forge_trajectory__001_upsert_is_idempotent -- --ignored
use db::{Database, DbTarget, ForgeEngineDao, RunTrajectoryCounts};

#[tokio::test]
#[ignore = "writes one trajectory row against an existing DEV Story Run"]
async fn a_repeated_trajectory_replaces_the_summary_for_its_run_and_profile() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let run_id: String = sqlx::query_scalar(
        "select id::text from storyboard_story_run order by created_at desc limit 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("a Story Run exists on DEV");
    let profile = "smith-structured-v1-test";
    let dao = ForgeEngineDao::new(db.clone());

    let first = RunTrajectoryCounts {
        tool_call_count: 3,
        edit_call_count: 1,
        shell_call_count: 2,
        structured_call_count: 0,
        first_edit_at_ms: Some(200),
        last_edit_at_ms: Some(200),
        verification_calls_after_last_edit: 2,
    };
    dao.upsert_run_trajectory(&run_id, profile, &first)
        .await
        .expect("the first trajectory is recorded");
    // The same turn settled again: the summary is replaced, not added to.
    dao.upsert_run_trajectory(&run_id, profile, &first)
        .await
        .expect("a retried trajectory is recorded");

    let (rows, tool_calls): (i64, i32) = sqlx::query_as(
        "select count(*), max(tool_call_count) from storyboard_run_trajectory
          where run_id = $1::uuid and harness_profile_id = $2",
    )
    .bind(&run_id)
    .bind(profile)
    .fetch_one(db.pool())
    .await
    .expect("the row is read");
    assert_eq!(rows, 1, "one row per run and profile");
    assert_eq!(tool_calls, 3, "the retry does not double the counts");

    sqlx::query(
        "delete from storyboard_run_trajectory where run_id = $1::uuid and harness_profile_id = $2",
    )
    .bind(&run_id)
    .bind(profile)
    .execute(db.pool())
    .await
    .expect("the test row is removed");
}

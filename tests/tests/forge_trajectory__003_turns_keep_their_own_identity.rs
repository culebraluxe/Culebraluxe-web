//! FORGE TRAJECTORY — a turn's row says WHICH turn it measured, and unknown spend stays unknown.
//!
//! The claim 293 exists for: two turns of one run and one profile are two rows, and each keeps its own identity. The
//! claim 292 could not make: every same-profile turn collapsed into one row and each overwrote the last.
//!
//! Ignored by default: it writes rows against an existing Story Run on DEV and removes them again.
//! Run with: APP_ENV=development cargo test -p test-harness --test forge_trajectory__003_turns_keep_their_own_identity -- --ignored
use db::{Database, DbTarget, ForgeEngineDao, RunTrajectoryCounts, RunTrajectoryTurn, TurnUsage};

/// A distinctive attempt-key prefix, so the cleanup can only remove this test's rows.
const MARK: &str = "trajectory-003";

/// This case's own key prefix. Both cases below run in parallel in one binary against the same newest Story Run, so a
/// mark they share lets one case count the other's rows — or delete them mid-assertion. Each case therefore writes,
/// reads and clears only under its own prefix.
fn case_keys(case: &str) -> String {
    format!("{MARK}:{case}:")
}

fn turn(case: &str, attempt_key: &str, role_attempt: i32) -> RunTrajectoryTurn {
    RunTrajectoryTurn {
        attempt_key: format!("{}{attempt_key}", case_keys(case)),
        profile_id: "smith-structured-v1".to_string(),
        profile_version: Some(1),
        generation_id: Some("00000000-0000-0000-0000-0000000000ff".to_string()),
        task_id: Some("trajectory-003-task".to_string()),
        role_attempt: Some(role_attempt),
        workflow_node: Some("smith".to_string()),
        role: Some("forge.smith".to_string()),
        model: None,
        provider: None,
        turn_started_at: None,
        turn_ended_at: None,
        outcome: Some("completed".to_string()),
        first_edit_latency_ms: Some(120),
        candidate_sha: Some("trajectory-003-candidate".to_string()),
        counts: RunTrajectoryCounts {
            tool_call_count: 2,
            edit_call_count: 1,
            shell_call_count: 1,
            structured_call_count: 0,
            first_edit_at_ms: Some(150),
            last_edit_at_ms: Some(150),
            verification_calls_after_last_edit: 1,
        },
        usage: None,
        usage_unknown: false,
    }
}

async fn a_run_id(db: &Database) -> String {
    sqlx::query_scalar("select id::text from storyboard_story_run order by created_at desc limit 1")
        .fetch_one(db.pool())
        .await
        .expect("a Story Run exists on DEV")
}

async fn clear(db: &Database, run_id: &str, case: &str) {
    sqlx::query(
        "delete from storyboard_run_trajectory where run_id = $1::uuid and attempt_key like $2",
    )
    .bind(run_id)
    .bind(format!("{}%", case_keys(case)))
    .execute(db.pool())
    .await
    .expect("the test rows are removed");
}

#[tokio::test]
#[ignore = "writes trajectory rows against an existing DEV Story Run"]
async fn a_repair_turn_and_a_replan_turn_each_keep_their_own_row() {
    let case = "turns";
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let run_id = a_run_id(&db).await;
    clear(&db, &run_id, case).await;
    let dao = ForgeEngineDao::new(db.clone());

    // The first turn of the role, the repair turn after a rejection, and a second turn of the first attempt paid by a
    // new generation (a REPLAN). Three execution identities; 292 could hold one of them.
    let first = turn(case, "task-1:role-attempt:0@gen-1", 0);
    let mut repair = turn(case, "task-1:role-attempt:1@gen-1", 1);
    repair.outcome = Some("failed".to_string());
    let mut replan = turn(case, "task-1:role-attempt:0@gen-2", 0);
    replan.generation_id = Some("00000000-0000-0000-0000-0000000000fe".to_string());
    replan.candidate_sha = None;

    for row in [&first, &repair, &replan] {
        dao.upsert_run_trajectory_turn(&run_id, row)
            .await
            .expect("the turn is recorded");
    }
    // And the first turn settles a second time, as a retry would.
    dao.upsert_run_trajectory_turn(&run_id, &first)
        .await
        .expect("the retried settlement is recorded");

    let turns = dao
        .run_trajectory_turns(&run_id)
        .await
        .expect("the run's turns are read");
    let mine: Vec<_> = turns
        .iter()
        .filter(|t| t.attempt_key.starts_with(&case_keys(case)))
        .collect();
    assert_eq!(
        mine.len(),
        3,
        "three turns, three rows — not one per profile"
    );
    assert_eq!(
        mine.iter()
            .filter(|t| t.outcome.as_deref() == Some("failed"))
            .count(),
        1,
        "the repair turn's failure survived"
    );
    assert_eq!(
        mine.iter().filter(|t| t.candidate_sha.is_none()).count(),
        1,
        "the turn that produced no candidate says so, and was not overwritten by one that did"
    );
    assert_eq!(
        mine.iter().filter(|t| t.role_attempt == Some(1)).count(),
        1,
        "the repair turn is readable as attempt 1"
    );

    clear(&db, &run_id, case).await;
}

#[tokio::test]
#[ignore = "writes trajectory rows against an existing DEV Story Run"]
async fn unknown_spend_is_neither_zero_nor_a_missing_reading() {
    let case = "usage";
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let run_id = a_run_id(&db).await;
    clear(&db, &run_id, case).await;
    let dao = ForgeEngineDao::new(db.clone());

    // Never settled: nothing was measured, and nothing claims to have been.
    let mut unsettled = turn(case, "usage:none", 0);
    unsettled.generation_id = None;
    // Settled as explicitly unknown: a reading was asked for and could not be taken.
    let mut unknown = turn(case, "usage:unknown", 0);
    unknown.generation_id = None;
    unknown.usage_unknown = true;
    // A vendor reading on a turn with no attempt ledger to hold it.
    let mut priced = turn(case, "usage:priced", 0);
    priced.generation_id = None;
    priced.usage = Some(TurnUsage {
        tokens_input: 100,
        tokens_output: 50,
        cost_usd: 0.25,
    });
    for row in [&unsettled, &unknown, &priced] {
        dao.upsert_run_trajectory_turn(&run_id, row)
            .await
            .expect("the turn is recorded");
    }

    let (never_settled, settled_unknown, vendor): (i64, i64, i64) = sqlx::query_as(
        "select count(*) filter (where usage_known is null and tokens_input is null),
                count(*) filter (where usage_known is false and tokens_input is null),
                count(*) filter (where usage_known is true and tokens_input = 100 and cost_usd = 0.25)
           from storyboard_run_trajectory
          where run_id = $1::uuid and attempt_key like $2",
    )
    .bind(&run_id)
    .bind(format!("{}%", case_keys(case)))
    .fetch_one(db.pool())
    .await
    .expect("the rows are read");
    assert_eq!(
        never_settled, 1,
        "a turn with no reading leaves the numbers null"
    );
    assert_eq!(
        settled_unknown, 1,
        "an unknown settlement says unknown, with no invented zero"
    );
    assert_eq!(vendor, 1, "a vendor reading is stored and marked known");

    clear(&db, &run_id, case).await;
}

/// The read the Flight Recorder's Turns view is built on: an instance resolves to its Story Run, and the run's turns
/// come back with their own three-valued spend. A read keyed the wrong way would still compile; only a database can
/// say whether the instance→run rule used here is the one the turns were written under.
#[tokio::test]
#[ignore = "writes trajectory rows against an existing DEV Story Run"]
async fn a_process_instance_resolves_to_its_run_with_that_runs_turns() {
    let case = "recorder";
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = ForgeEngineDao::new(db.clone());

    // The instance→run pair the read itself resolves: each instance's EARLIEST task execution, which is the rule both
    // the header read and the turns select's lateral subquery use. Asking the database for it keeps the test honest
    // about DEV's own shape instead of assuming one.
    let (instance_id, run_id): (String, String) = sqlx::query_as(
        "select distinct on (ex.process_instance_id) ex.process_instance_id::text, ex.story_run_id::text
           from forge_engine_task_execution ex
          order by ex.process_instance_id, ex.created_at",
    )
    .fetch_one(db.pool())
    .await
    .expect("DEV holds at least one engine task execution");
    clear(&db, &run_id, case).await;

    // One turn that settled as explicitly unknown, one with a vendor reading: the two answers a reader must not merge.
    let mut unknown = turn(case, "unknown", 0);
    unknown.generation_id = None;
    unknown.usage_unknown = true;
    let mut priced = turn(case, "priced", 1);
    priced.generation_id = None;
    priced.usage = Some(TurnUsage {
        tokens_input: 12,
        tokens_output: 3,
        cost_usd: 0.5,
    });
    for row in [&unknown, &priced] {
        dao.upsert_run_trajectory_turn(&run_id, row)
            .await
            .expect("the turn is recorded");
    }

    let read = dao
        .instance_story_run_turns(&instance_id, 50)
        .await
        .expect("the instance's run is read")
        .expect("the instance belongs to a Story Run");
    assert_eq!(
        read.run_id, run_id,
        "the console is handed the run behind its own instance"
    );
    let mine: Vec<_> = read
        .turns
        .iter()
        .filter(|t| t.attempt_key.starts_with(&case_keys(case)))
        .collect();
    assert_eq!(mine.len(), 2, "both turns of the run are readable");
    assert_eq!(
        mine.iter()
            .filter(|t| t.usage_known == Some(false) && t.cost_usd.is_none())
            .count(),
        1,
        "a turn that settled unknown carries no number to read as zero"
    );
    assert_eq!(
        mine.iter()
            .filter(|t| t.usage_known == Some(true) && t.cost_usd == Some(0.5))
            .count(),
        1,
        "the measured turn keeps its reading"
    );

    // An instance that does not exist is `None` — not an empty run. The console must be able to tell "this is not a
    // Forge run" from "this run recorded nothing", and an id that is not an id is not an instance either.
    assert!(dao
        .instance_story_run_turns("00000000-0000-0000-0000-000000000000", 50)
        .await
        .expect("the read succeeds")
        .is_none());
    assert!(dao
        .instance_story_run_turns("not-an-instance", 50)
        .await
        .expect("the read succeeds")
        .is_none());

    clear(&db, &run_id, case).await;
}

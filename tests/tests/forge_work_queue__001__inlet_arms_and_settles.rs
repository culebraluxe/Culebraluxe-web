//! FORGE.WORK-QUEUE — the batch inlet arms and settles (migration 270).
//!
//! CONTRACT. `forge_work_queue` (269) is an inlet, not a third queue. The sweep the worker already runs once per
//! pass (`forge_reconcile_dispatch_queue`, 265) closes finished door rows and arms free ones, and arming invents no
//! path: it calls `forge_dispatch_story`, the board's own ENGINE RUN Q verb, so the story goes `Ready` and the
//! trigger (025, restated 146/259) queues the one item — with `execution_policy = 'Unattended OK'`, the column
//! default and the policy the unattended poller claims on, so an armed story is runnable with nothing added.
//!
//! What this pins, on DEV:
//!   1. FIFO — with `arm_limit = 1` the oldest `Pending` row arms first, then the next once its story settles;
//!   2. pacing — a pass that has no free slot arms nothing;
//!   3. settle Complete — the row completes with the run's commit and `ran_as` (the run's own `model_used`);
//!   4. settle Failed — terminal at the door, carrying the engine's reason;
//!   5. run ended — the story is back on the board with no item, so the row returns to the FIFO (backed off);
//!   6. attempts spent — the row goes `Error` rather than reading as `Pending` forever.
//!
//! Level: L1 executable. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_work_queue__001__inlet_arms_and_settles -- --ignored

use db::{Database, DbTarget};
use sqlx::PgPool;

const ROW_ONE: &str = "TST-INLET-001";
const ROW_TWO: &str = "TST-INLET-002";
const ROW_THREE: &str = "TST-INLET-003";

async fn sweep(pool: &PgPool) -> (i64, i64, i64) {
    sqlx::query_as("select queued, restated, cleared from forge_reconcile_dispatch_queue()")
        .fetch_one(pool)
        .await
        .expect("the sweep runs")
}

async fn arm_limit(pool: &PgPool, limit: i32) {
    sqlx::query(
        "update forge_work_queue_config set arm_limit = $1, updated_at = now() where id = 1",
    )
    .bind(limit)
    .execute(pool)
    .await
    .expect("arm_limit");
}

/// A door row whose `created_at` the test chooses, so FIFO is a fact and not a clock resolution.
async fn enqueue(pool: &PgPool, story: &str, minutes_ago: i32) -> String {
    let id: String = sqlx::query_scalar(
        "select forge_enqueue_work($1, $2, 'opencode', 'deepseek/deepseek-flash')::text",
    )
    .bind(format!("inlet-test-{story}"))
    .bind(story)
    .fetch_one(pool)
    .await
    .expect("enqueue");
    sqlx::query(
        "update forge_work_queue set created_at = now() - make_interval(mins => $2) where id = $1::uuid",
    )
    .bind(&id)
    .bind(minutes_ago)
    .execute(pool)
    .await
    .expect("created_at");
    id
}

async fn door_state(
    pool: &PgPool,
    story: &str,
) -> (String, i32, Option<String>, Option<String>, Option<String>) {
    sqlx::query_as(
        "select state, attempts, commit_sha, ran_as, last_error from forge_work_queue where story_id = $1",
    )
    .bind(story)
    .fetch_one(pool)
    .await
    .expect("door row")
}

async fn story_state(pool: &PgPool, story: &str) -> (String, i64) {
    sqlx::query_as(
        "select s.status, \
                (select count(*) from agent_work_item w where w.story_id = s.id \
                  and w.state in ('Ready','Claimed','Running','Paused')) \
           from storyboard_story s where s.id = $1",
    )
    .bind(story)
    .fetch_one(pool)
    .await
    .expect("story")
}

async fn open_item(pool: &PgPool, story: &str) -> (String, String, Option<String>) {
    sqlx::query_as(
        "select state, execution_policy, work_type from agent_work_item \
          where story_id = $1 and state in ('Ready','Claimed','Running','Paused')",
    )
    .bind(story)
    .fetch_one(pool)
    .await
    .expect("open item")
}

async fn finish_story(pool: &PgPool, story: &str, run_result: &str, model: &str) {
    sqlx::query("update storyboard_story set status = $2, updated_at = now() where id = $1")
        .bind(story)
        .bind(run_result)
        .execute(pool)
        .await
        .expect("story status");
    sqlx::query(
        "insert into storyboard_story_run (story_id, started_at, ended_at, result_status, commit_hash, model_used) \
         values ($1, now() - interval '2 minutes', now(), 'Complete', $2, $3)",
    )
    .bind(story)
    .bind(format!("sha-{story}"))
    .bind(model)
    .execute(pool)
    .await
    .expect("run fixture");
}

async fn clean(pool: &PgPool) {
    let deletes = [
        "delete from agent_work_item where story_id like 'TST-INLET-%'",
        "delete from forge_work_queue where story_id like 'TST-INLET-%'",
        "delete from storyboard_story_run where story_id like 'TST-INLET-%'",
        "delete from storyboard_story where id like 'TST-INLET-%'",
    ];
    for statement in deletes {
        let _ = sqlx::query(statement).execute(pool).await;
    }
    let _ = arm_limit(pool, 3).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn forge_work_queue_001__inlet_arms_and_settles() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let pool = db.pool();
    clean(pool).await;

    for story in [ROW_ONE, ROW_TWO, ROW_THREE] {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, status, work_type, priority) \
             values ($1, 'inlet-test', $1, 'Planned', 'FAST', 'Critical')",
        )
        .bind(story)
        .execute(pool)
        .await
        .expect("story fixture");
    }
    enqueue(pool, ROW_ONE, 3).await;
    enqueue(pool, ROW_TWO, 2).await;
    enqueue(pool, ROW_THREE, 1).await;

    // 1 + 2: one slot, so the oldest row arms and nothing else does.
    arm_limit(pool, 1).await;
    sweep(pool).await;
    assert_eq!(
        door_state(pool, ROW_ONE).await.0,
        "Running",
        "oldest Pending row arms first"
    );
    assert_eq!(
        door_state(pool, ROW_TWO).await.0,
        "Pending",
        "the slot is held"
    );
    assert_eq!(
        story_state(pool, ROW_ONE).await,
        ("Ready".to_string(), 1),
        "the story is dispatched exactly once"
    );
    let (item_state, policy, work_type) = open_item(pool, ROW_ONE).await;
    assert_eq!(item_state, "Ready");
    assert_eq!(
        policy, "Unattended OK",
        "the policy the unattended poller claims on"
    );
    assert_eq!(
        work_type.as_deref(),
        Some("FAST"),
        "the work type travels with the story"
    );

    assert_eq!(
        sweep(pool).await.0,
        0,
        "pacing: no free slot, nothing armed"
    );
    assert_eq!(
        door_state(pool, ROW_TWO).await.1,
        0,
        "a row that waited is not charged an attempt"
    );

    // 3: the story finished — the row completes with the run's commit and what actually ran.
    finish_story(pool, ROW_ONE, "Complete", "smoke-agent-model").await;
    sqlx::query(
        "update agent_work_item set state = 'Done', finished_at = now() where story_id = $1",
    )
    .bind(ROW_ONE)
    .execute(pool)
    .await
    .expect("item settled");
    sweep(pool).await;
    let (state, attempts, sha, ran_as, _) = door_state(pool, ROW_ONE).await;
    assert_eq!((state.as_str(), attempts), ("Complete", 1));
    assert_eq!(
        sha.as_deref(),
        Some("sha-TST-INLET-001"),
        "the run's commit lands on the row"
    );
    assert_eq!(
        ran_as.as_deref(),
        Some("smoke-agent-model"),
        "what actually ran is recorded"
    );
    assert_eq!(
        door_state(pool, ROW_TWO).await.0,
        "Running",
        "the freed slot arms the next row"
    );

    // 4: the story failed — terminal at the door, carrying the engine's own reason.
    sqlx::query(
        "update storyboard_story set status = 'Failed', forge_last_failure_reason = 'inlet-test: qa refused', \
         updated_at = now() where id = $1",
    )
    .bind(ROW_TWO)
    .execute(pool)
    .await
    .expect("failure fixture");
    sqlx::query(
        "update agent_work_item set state = 'Done', finished_at = now() where story_id = $1",
    )
    .bind(ROW_TWO)
    .execute(pool)
    .await
    .expect("item settled");
    sweep(pool).await;
    let (state, _, _, _, error) = door_state(pool, ROW_TWO).await;
    assert_eq!(state, "Error");
    assert_eq!(error.as_deref(), Some("inlet-test: qa refused"));
    assert_eq!(
        door_state(pool, ROW_THREE).await.0,
        "Running",
        "the FIFO keeps moving"
    );

    // 5: the run ended and the story is back on the board with nothing holding it — the row returns to the FIFO.
    sqlx::query("update storyboard_story set status = 'Planned', updated_at = now() where id = $1")
        .bind(ROW_THREE)
        .execute(pool)
        .await
        .expect("story back on the board");
    sqlx::query(
        "update agent_work_item set state = 'Cancelled', finished_at = now() where story_id = $1",
    )
    .bind(ROW_THREE)
    .execute(pool)
    .await
    .expect("item cancelled");
    sweep(pool).await;
    let (state, _, _, _, error) = door_state(pool, ROW_THREE).await;
    assert_eq!(state, "Pending");
    assert_eq!(error.as_deref(), Some("run ended without settling"));
    let backed_off: bool =
        sqlx::query_scalar("select available_at > now() from forge_work_queue where story_id = $1")
            .bind(ROW_THREE)
            .fetch_one(pool)
            .await
            .expect("backoff");
    assert!(backed_off, "the row waits before it can be armed again");

    // 6: attempts spent — nothing will claim it again, so it must not read as Pending.
    sqlx::query("update forge_work_queue set attempts = max_attempts where story_id = $1")
        .bind(ROW_THREE)
        .execute(pool)
        .await
        .expect("attempts");
    sweep(pool).await;
    let (state, _, _, _, error) = door_state(pool, ROW_THREE).await;
    assert_eq!(state, "Error");
    assert_eq!(error.as_deref(), Some("run ended without settling"));

    clean(pool).await;
    let left: i64 = sqlx::query_scalar(
        "select (select count(*) from forge_work_queue where story_id like 'TST-INLET-%') \
              + (select count(*) from storyboard_story where id like 'TST-INLET-%')",
    )
    .fetch_one(pool)
    .await
    .expect("residue");
    assert_eq!(left, 0, "the test leaves DEV as it found it");
}

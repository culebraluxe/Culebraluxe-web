//! FORGE.WORK-QUEUE — a run that ends settles its own door row (migration 271).
//!
//! CONTRACT. The attempt's own writer (`forge_finish_agent_work_run`, 263) is where a claim ends, so it is also where
//! the door row that armed it must end. A `Done` the board does not confirm is refused by 263 (item `Error`, story
//! `Hold`) — and before 271 that refusal left the door row `Running`, so `arm_limit` counted a run nobody was
//! running, `forge_arm_work_queue` returned 0 on every pass, and the inlet was held shut by its own pacing. Three
//! PROD rows sat that way from 23:54 to the fix on 2026-10-05.
//!
//! What this pins, on DEV:
//!   1. the refused `Done` (263's premise, unchanged): item `Error`, story `Hold`, the refusal named in the reason;
//!   2. the door row is terminal in the SAME transaction — asserted with no sweep in between;
//!   3. its reason is the settlement's own words ('Done refused'), not a paraphrase;
//!   4. the slot the attempt held arms the next FIFO row there and then, so a run that ends cannot leave the inlet
//!      closed behind it.
//!
//! Fixtures are `TST-D1Q-*` and this file is standalone on purpose: cargo runs test binaries one at a time, so it
//! shares nothing — not even the single `forge_work_queue_config` row — with
//! `forge_work_queue__001__inlet_arms_and_settles.rs`.
//!
//! Level: L1 executable. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_work_queue__002__a_refused_done_settles_its_own_row -- --ignored

use db::{Database, DbTarget};
use sqlx::PgPool;

const ATTEMPT: &str = "TST-D1Q-001";
const WAITING: &str = "TST-D1Q-002";

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
    .bind(format!("d1q-test-{story}"))
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

async fn door(pool: &PgPool, story: &str) -> (String, Option<String>) {
    sqlx::query_as("select state, last_error from forge_work_queue where story_id = $1")
        .bind(story)
        .fetch_one(pool)
        .await
        .expect("door row")
}

async fn clean(pool: &PgPool) {
    let deletes = [
        "delete from agent_work_item where story_id like 'TST-D1Q-%'",
        "delete from forge_work_queue where story_id like 'TST-D1Q-%'",
        "delete from storyboard_story_run where story_id like 'TST-D1Q-%'",
        "delete from storyboard_story where id like 'TST-D1Q-%'",
    ];
    for statement in deletes {
        let _ = sqlx::query(statement).execute(pool).await;
    }

    let _ = arm_limit(pool, 3).await;
}
#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn forge_work_queue_002__a_refused_done_settles_its_own_row() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let pool = db.pool();
    clean(pool).await;

    for story in [ATTEMPT, WAITING] {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, status, work_type, priority) \
             values ($1, 'd1q-test', $1, 'Planned', 'FAST', 'Critical')",
        )
        .bind(story)
        .execute(pool)
        .await
        .expect("story fixture");
    }
    enqueue(pool, ATTEMPT, 2).await;
    enqueue(pool, WAITING, 1).await;

    // One slot: the oldest row is the attempt, and the other waits on it.
    arm_limit(pool, 1).await;
    sweep(pool).await;
    assert_eq!(door(pool, ATTEMPT).await.0, "Running");
    assert_eq!(door(pool, WAITING).await.0, "Pending");

    // The claim 262 makes: the item is claimed, its run is open, and the board is In Progress — the exact shape of
    // the three PROD runs that jammed the inlet.
    let item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(ATTEMPT)
    .fetch_one(pool)
    .await
    .expect("the armed story holds one item");
    sqlx::query(
        "insert into storyboard_story_run (story_id, started_at, model_used) \
         values ($1, now() - interval '23 minutes', 'smoke-agent-model')",
    )
    .bind(ATTEMPT)
    .execute(pool)
    .await
    .expect("open run fixture");
    sqlx::query(
        "update agent_work_item set state = 'Claimed', claimed_by = 'd1q-test', claimed_at = now(), \
                started_at = now(), updated_at = now(), \
                story_run_id = (select id from storyboard_story_run where story_id = $2 \
                                 order by started_at desc limit 1) \
          where id = $1::uuid",
    )
    .bind(&item)
    .bind(ATTEMPT)
    .execute(pool)
    .await
    .expect("claim fixture");
    sqlx::query(
        "update storyboard_story set status = 'In Progress', updated_at = now() where id = $1",
    )
    .bind(ATTEMPT)
    .execute(pool)
    .await
    .expect("board fixture");

    // 1: the engine says Done; the board still expects a run, so 263 refuses it and holds the story.
    let (item_state, story_status, reason): (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(
            "select item_state, story_status, reason \
               from forge_finish_agent_work_run($1::uuid, 'Done', 'no summary')",
        )
        .bind(&item)
        .fetch_one(pool)
        .await
        .expect("the settlement runs");
    assert_eq!(
        item_state.as_deref(),
        Some("Error"),
        "a Done the board does not confirm is refused"
    );
    assert_eq!(story_status.as_deref(), Some("Hold"));
    let refusal = reason.unwrap_or_default();
    assert!(
        refusal.contains("Done refused"),
        "the refusal says what it refused: {refusal}"
    );

    // 2 + 3: no sweep has run since the finish, and the door row is already terminal — in the settlement's words.
    let (state, error) = door(pool, ATTEMPT).await;
    assert_eq!(
        state, "Error",
        "the attempt that ended settles the row it was armed from, in its own transaction"
    );
    let door_reason = error.unwrap_or_default();
    assert!(
        door_reason.contains("Done refused"),
        "the door's reason is the settlement's words, not a paraphrase: {door_reason}"
    );

    // 4: the slot the attempt held is free in that same transaction, so the inlet keeps moving.
    assert_eq!(
        door(pool, WAITING).await.0,
        "Running",
        "the armed slot takes the next FIFO row without waiting for a sweep"
    );

    clean(pool).await;
    let left: i64 = sqlx::query_scalar(
        "select (select count(*) from forge_work_queue where story_id like 'TST-D1Q-%') \
              + (select count(*) from storyboard_story where id like 'TST-D1Q-%')",
    )
    .fetch_one(pool)
    .await
    .expect("residue");
    assert_eq!(left, 0, "the test leaves DEV as it found it");
    let limit: i32 =
        sqlx::query_scalar("select arm_limit from forge_work_queue_config where id = 1")
            .fetch_one(pool)
            .await
            .expect("arm_limit");
    assert_eq!(limit, 3, "the pacing knob is left as the test found it");
}

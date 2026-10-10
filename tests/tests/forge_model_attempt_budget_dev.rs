//! Cross-connection proof for durable generation model-attempt reservations.
//! Run explicitly against DEV after migration 282:
//!   cargo test -p test-harness --test forge_model_attempt_budget_dev -- --ignored

use db::{Database, DbTarget, ForgeEngineDao};
use std::sync::Arc;

async fn proof_run(pool: &sqlx::PgPool) -> (String, String) {
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-BUDGET-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes) \
         values ($1, 'PROOF', 'Model attempt budget proof', 'High', 'In Progress', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");
    let run: String = sqlx::query_scalar(
        "insert into storyboard_story_run (story_id, started_at, execution_environment, run_type) \
         values ($1, now(), 'DEV', 'dispatch') returning id::text",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .expect("insert proof generation");
    (story, run)
}

async fn cleanup(pool: &sqlx::PgPool, story: &str) {
    let _ = sqlx::query("delete from storyboard_story where id=$1")
        .bind(story)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV and migration 286"]
async fn automatic_redispatch_keeps_one_budget_across_story_runs() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let (story, first_run) = proof_run(pool).await;
    let second_run: String = sqlx::query_scalar(
        "insert into storyboard_story_run (story_id, started_at, execution_environment, run_type) \
         values ($1, now(), 'DEV', 'automatic-redispatch') returning id::text",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .expect("insert redispatch Story Run");
    let generation = uuid::Uuid::new_v4().to_string();
    let dao = Arc::new(ForgeEngineDao::new(database.clone()));

    let initial = dao
        .ensure_model_generation_budget(&generation, &story, 3)
        .await
        .unwrap();
    assert_eq!(
        (initial.used, initial.cap, initial.uncertain),
        (0, 3, false)
    );
    let before_reclaim = dao
        .reserve_model_generation_attempt(&generation, &story, &first_run, 3, "task-before", 0)
        .await
        .unwrap();
    assert!(before_reclaim.authorized);
    let second_before_reclaim = dao
        .reserve_model_generation_attempt(&generation, &story, &first_run, 3, "task-before-2", 0)
        .await
        .unwrap();
    assert!(second_before_reclaim.authorized);
    let left_dao = Arc::clone(&dao);
    let right_dao = Arc::clone(&dao);
    let left_generation = generation.clone();
    let right_generation = generation.clone();
    let left_story = story.clone();
    let right_story = story.clone();
    let left_run = second_run.clone();
    let right_run = second_run.clone();
    let (left, right) = tokio::join!(
        async move {
            left_dao
                .reserve_model_generation_attempt(
                    &left_generation,
                    &left_story,
                    &left_run,
                    99,
                    "task-after-a",
                    0,
                )
                .await
                .unwrap()
        },
        async move {
            right_dao
                .reserve_model_generation_attempt(
                    &right_generation,
                    &right_story,
                    &right_run,
                    3,
                    "task-after-b",
                    0,
                )
                .await
                .unwrap()
        },
    );
    assert_ne!(left.authorized, right.authorized);
    let after_reclaim = if left.authorized { left } else { right };
    assert!(after_reclaim.authorized);
    assert_eq!(
        (after_reclaim.budget.used, after_reclaim.budget.cap),
        (3, 3)
    );
    let excess = dao
        .reserve_model_generation_attempt(&generation, &story, &second_run, 3, "task-excess", 0)
        .await
        .unwrap();
    assert!(
        !excess.authorized,
        "a fourth launch is rejected across dispatches"
    );
    assert_eq!(excess.budget.used, 3);
    let frozen = dao
        .ensure_model_generation_budget(&generation, &story, 99)
        .await
        .unwrap();
    assert_eq!((frozen.used, frozen.cap), (3, 3));

    cleanup(pool, &story).await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV and migration 282"]
async fn reservations_are_fixed_unique_and_atomic_across_connections() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let (story, run) = proof_run(pool).await;
    let engine = Arc::new(ForgeEngineDao::new(database.clone()));

    let initial = engine.ensure_model_attempt_budget(&run, 2).await.unwrap();
    let changed_env = engine.ensure_model_attempt_budget(&run, 99).await.unwrap();
    assert_eq!(
        (initial.cap, changed_env.cap),
        (2, 2),
        "resume cannot reset the fixed cap"
    );

    let first = engine
        .reserve_model_attempt(&run, 99, "task-a", 0)
        .await
        .unwrap();
    assert!(first.authorized);
    let duplicate = engine
        .reserve_model_attempt(&run, 2, "task-a", 0)
        .await
        .unwrap();
    assert!(
        !duplicate.authorized && duplicate.duplicate,
        "the same attempt cannot relaunch"
    );

    let left_engine = Arc::clone(&engine);
    let right_engine = Arc::clone(&engine);
    let left_run = run.clone();
    let right_run = run.clone();
    let (left, right) = tokio::join!(
        async move {
            left_engine
                .reserve_model_attempt(&left_run, 2, "task-left", 0)
                .await
                .unwrap()
        },
        async move {
            right_engine
                .reserve_model_attempt(&right_run, 2, "task-right", 0)
                .await
                .unwrap()
        },
    );
    assert_ne!(
        left.authorized, right.authorized,
        "only the last remaining attempt has one winner"
    );
    let final_budget = engine.ensure_model_attempt_budget(&run, 2).await.unwrap();
    assert_eq!((final_budget.used, final_budget.cap), (2, 2));
    let refused = engine
        .reserve_model_attempt(&run, 2, "task-extra", 0)
        .await
        .unwrap();
    assert!(!refused.authorized && !refused.duplicate);

    cleanup(pool, &story).await;
}

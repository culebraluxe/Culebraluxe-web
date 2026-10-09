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

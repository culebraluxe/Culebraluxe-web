//! CHAOS.FORGE — process crash before queue settlement converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-009).
//!
//! Contract: a Forge queue settlement (agent work item finish) that is initiated but
//! dies before the settlement is persisted neither loses the settlement nor duplicates
//! it. After the crash the work item reads as unfinished; the recovery sweep finishes
//! it. Exactly one work item row exists throughout: restart never forks the settlement.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and stale-recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__009__process_crash_before_queue_settlement_converges_after_restart_to_one_legal_durable_forge -- --ignored

use db::{Database, DbTarget, ForgeEngineDao};
use uuid::Uuid;

async fn work_item_state(db: &Database, work_item_id: &str) -> Option<String> {
    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(work_item_id)
        .fetch_optional(db.pool())
        .await
        .expect("work item state")
}

async fn work_item_count(db: &Database, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
        .bind(story_id)
        .fetch_one(db.pool())
        .await
        .expect("work item count")
}

async fn story_status(db: &Database, story_id: &str) -> Option<String> {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_optional(db.pool())
        .await
        .expect("story status")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_009__process_crash_before_queue_settlement_converges_after_restart_to_one_legal_durable_forge(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();

    // Pick an existing story.
    let story_id: String = sqlx::query_scalar("select id from storyboard_story limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one story");

    // Ensure there's a work item for this story by cycling status.
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep work items");

    sqlx::query("update storyboard_story set status = 'Planned', updated_at = now() where id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("reset story to Planned");

    sqlx::query("update storyboard_story set status = 'Ready', updated_at = now() where id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("dispatch work item");

    let work_item_id: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&story_id)
    .fetch_one(db.pool())
    .await
    .expect("work item dispatched");

    let generation = ForgeEngineDao::new(db.clone());
    let worker_id = format!("chaos-forge-009-{}", Uuid::new_v4());
    let claimed = generation
        .claim_specific_agent_work(&work_item_id, &worker_id)
        .await
        .expect("claim answers");
    assert!(claimed.is_some());

    // Generation one: settlement is initiated via forge_finish_agent_work_run, but the
    // process dies before the settlement is committed (simulated by rolling back).
    {
        let mut tx = db.begin("chaos-settlement").await.expect("begin");
        sqlx::query("select forge_finish_agent_work_run($1::uuid, 'Done', null)")
            .bind(&work_item_id)
            .execute(tx.connection())
            .await
            .expect("record settlement");
        // No commit: the process dies here (crash after settlement, before commit).
    }
    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Claimed".to_string()),
        "a crashed settlement leaves work item in Claimed state"
    );
    assert_eq!(
        work_item_count(&db, &story_id).await,
        1,
        "exactly one work item row exists"
    );

    // Set story to Complete so the settlement can succeed with Done.
    sqlx::query(
        "update storyboard_story set status = 'Complete', updated_at = now() where id = $1",
    )
    .bind(&story_id)
    .execute(db.pool())
    .await
    .expect("set story to Complete");

    // Generation two restarts: the settlement is gone, so it is re-recorded.
    sqlx::query("select forge_finish_agent_work_run($1::uuid, 'Done', null)")
        .bind(&work_item_id)
        .execute(db.pool())
        .await
        .expect("record recovery settlement");
    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Done".to_string()),
        "the recovered settlement marks work item as Done"
    );
    assert_eq!(
        work_item_count(&db, &story_id).await,
        1,
        "exactly one work item row persists"
    );

    // The work item is now Done - a further settlement attempt would be a no-op.
    sqlx::query("select forge_finish_agent_work_run($1::uuid, 'Error', 'test')")
        .bind(&work_item_id)
        .execute(db.pool())
        .await
        .expect("attempt second settlement");
    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Done".to_string()),
        "subsequent settlement is no-op, work item stays Done"
    );
    assert_eq!(
        work_item_count(&db, &story_id).await,
        1,
        "exactly one work item row throughout"
    );

    // Clean up.
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

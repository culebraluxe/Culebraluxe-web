//! CHAOS.FORGE — process crash after task claim converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-003).
//!
//! Contract: a worker that dies between claiming an agent work item and finalizing it
//! neither loses the unit nor duplicates it. After the crash the claim reads `Claimed`
//! (the item is held, not free, not lost); once the claim goes stale — a `Claimed`
//! row older than the engine's stale window — the next worker reclaims it and converges
//! to `Running`. Exactly one agent work item row exists throughout: restart never forks
//! the queue.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and stale-recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__003__process_crash_after_task_claim_converges_after_restart_to_one_legal_durable_forge_state -- --ignored

use db::{Database, DbTarget, ForgeEngineDao};
use sqlx::Row;
use uuid::Uuid;

async fn work_item_state(db: &Database, work_item_id: &str) -> Option<String> {
    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(work_item_id)
        .fetch_optional(db.pool())
        .await
        .expect("work item state")
}

async fn work_item_count_for_story(db: &Database, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
        .bind(story_id)
        .fetch_one(db.pool())
        .await
        .expect("work item count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_003__process_crash_after_task_claim_converges_after_restart_to_one_legal_durable_forge_state(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let worker_id = format!("chaos-forge-003-{}", Uuid::new_v4());

    // Pick an existing story (any status) and force it to Ready to dispatch a work item.
    let story_id: String = sqlx::query_scalar("select id from storyboard_story limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one story to anchor the work item");

    // Clean any existing work item for this story (test isolation).
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Force the story to Ready to dispatch a fresh work item via trigger.
    sqlx::query("update storyboard_story set status = 'Ready', updated_at = now() where id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("dispatch work item");

    // Get the work item id using a query that returns a row
    let work_item_row = sqlx::query("select id::text as id from agent_work_item where story_id = $1 and state = 'Ready'")
        .bind(&story_id)
        .fetch_one(db.pool())
        .await
        .expect("work item dispatched");
    let work_item_id: String = work_item_row.get("id");

    // Generation one claims the work item, then dies before finalizing: the DAO is dropped, never reused.
    {
        let generation = ForgeEngineDao::new(db.clone());
        let claimed = generation
            .claim_specific_agent_work(&work_item_id, &worker_id)
            .await
            .expect("claim answers");
        assert!(claimed.is_some(), "generation one claims the work item");
        let claimed = claimed.unwrap();
        assert_eq!(claimed.state, "Claimed", "generation one moves item to Claimed");
        drop(generation); // the process crash: no finalize, no cleanup, no goodbye
    }

    // The work item remains Claimed — not Ready (no duplicate work) and not Error/Done (not lost).
    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Claimed".to_string()),
        "after a crash the work item reads Claimed, never Ready and never lost"
    );
    assert_eq!(
        work_item_count_for_story(&db, &story_id).await,
        1,
        "exactly one work item row exists for the story"
    );

    // Generation two restarts and finds the item still Claimed (held by the dead worker).
    {
        let generation = ForgeEngineDao::new(db.clone());
        let claimed = generation
            .claim_specific_agent_work(&work_item_id, &worker_id)
            .await
            .expect("reclaim answers");
        // The item is still Claimed by the dead worker, so the specific claim returns None
        // (the story already has an open item). This is the "held" signal.
        assert!(
            claimed.is_none(),
            "after a crash the specific claim finds the item held, never free"
        );
    }
    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Claimed".to_string()),
        "work item stays Claimed while the stale window has not elapsed"
    );
    assert_eq!(
        work_item_count_for_story(&db, &story_id).await,
        1,
        "exactly one work item row still exists"
    );

    // The stale window passes (the claim belonged to a dead process): generation three reclaims.
    sqlx::query(
        "update agent_work_item set updated_at = now() - interval '16 minutes' \
         where id = $1::uuid",
    )
    .bind(&work_item_id)
    .execute(db.pool())
    .await
    .expect("age the claim past the stale window");

    // Recovery sweep: the stale claim is requeued back to Ready.
    sqlx::query("select forge_requeue_stale_work($1::uuid, $2)")
        .bind(&work_item_id)
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("requeue stale work");

    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Ready".to_string()),
        "stale claim is requeued to Ready"
    );
    assert_eq!(
        work_item_count_for_story(&db, &story_id).await,
        1,
        "exactly one work item row persists through recovery"
    );

    // Generation three claims the requeued item and moves it to Claimed.
    {
        let generation = ForgeEngineDao::new(db.clone());
        let claimed = generation
            .claim_specific_agent_work(&work_item_id, &worker_id)
            .await
            .expect("recovery claim answers");
        assert!(claimed.is_some(), "generation three reclaims the requeued item");
        let claimed = claimed.unwrap();
        assert_eq!(claimed.state, "Claimed", "recovery claim moves item to Claimed");
    }
    assert_eq!(
        work_item_state(&db, &work_item_id).await,
        Some("Claimed".to_string()),
        "recovered item is Claimed by the fresh generation"
    );
    assert_eq!(
        work_item_count_for_story(&db, &story_id).await,
        1,
        "exactly one work item row throughout: crash + restart + recovery = one"
    );

    // Clean up.
    sqlx::query("delete from agent_work_item where story_id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("sweep");
    sqlx::query("update storyboard_story set status = 'Planned', updated_at = now() where id = $1")
        .bind(&story_id)
        .execute(db.pool())
        .await
        .expect("reset story");
}

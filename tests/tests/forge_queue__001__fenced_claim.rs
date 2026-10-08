//! FORGE.QUEUE — fenced leases, visible attempts, caller idempotency keys (FORGE-FIX-008).
//!
//! CONTRACT. Three defects in the queue's ownership seam, closed together:
//!
//!   1. the heartbeat wrote `updated_at = now()` with no owner, so any process could freshen any
//!      claim; it is now fenced to `claimed_by` and refreshes `heartbeat_at` / `lease_expires_at`;
//!   2. `attempts` / `max_attempts` lived only on the row below the read model, and the claim had no
//!      expiry, so a dead worker held its item until the sweep; the claim now stamps a lease and an
//!      expired lease is reclaimable on the next claim, without waiting for a sweep;
//!   3. artifact submits and settles had no caller idempotency key, so a retried submit wrote a
//!      second row; both now accept a caller key and a retry returns the first row.
//!
//! Level: L1 executable. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_queue__001__fenced_claim -- --ignored

use db::{Database, DbTarget, ForgeEngineDao, NewToolArtifact};
use sqlx::PgPool;

const STORY_ONE: &str = "TST-QUEUE-LEASE-001";
const STORY_TWO: &str = "TST-QUEUE-LEASE-002";
const STORY_THREE: &str = "TST-QUEUE-LEASE-003";

async fn add_story(pool: &PgPool, story: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, status, work_type, priority) \
         values ($1, 'queue-lease-test', $1, 'Ready', 'FAST', 'Critical')",
    )
    .bind(story)
    .execute(pool)
    .await
    .expect("story");
}

/// The one open item the Ready trigger opens for a story.
async fn open_item(pool: &PgPool, story: &str) -> String {
    sqlx::query_scalar(
        "select id::text from agent_work_item \
         where story_id = $1 and state in ('Ready','Claimed','Running','Paused')",
    )
    .bind(story)
    .fetch_one(pool)
    .await
    .expect("open item")
}

async fn clean(pool: &PgPool) {
    for statement in [
        "delete from forge_tool_artifact where story_id like 'TST-QUEUE-LEASE-%'",
        "delete from agent_work_item where story_id like 'TST-QUEUE-LEASE-%'",
        "delete from storyboard_story_run where story_id like 'TST-QUEUE-LEASE-%'",
        "delete from storyboard_story where id like 'TST-QUEUE-LEASE-%'",
    ] {
        let _ = sqlx::query(statement).execute(pool).await;
    }
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn forge_queue_001__fenced_claim() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let pool = db.pool();
    clean(pool).await;
    let engine = ForgeEngineDao::new(db.clone());

    // --- 1. Two workers racing one item: exactly one owns it, and the loser's heartbeat is refused.
    add_story(pool, STORY_ONE).await;
    let item_one = open_item(pool, STORY_ONE).await;
    let first = engine
        .claim_specific_agent_work(&item_one, "worker-a")
        .await
        .unwrap()
        .expect("worker-a claims");
    assert_eq!(first.id, item_one);
    // A racing worker running the queue sweep must not land on the same item.
    while let Some(row) = engine.claim_next_agent_work("worker-b").await.unwrap() {
        assert_ne!(
            row.id, item_one,
            "worker-b must not double-claim worker-a's item"
        );
        // Put whatever worker-b won back, so the queue is as found.
        engine
            .finish_agent_work_run(
                &row.id,
                db::AgentWorkOutcome::Abandoned,
                Some("proof borrow"),
                None,
            )
            .await
            .unwrap();
    }

    // The fence: worker-b's heartbeat on the row it does NOT own is refused, worker-a's is held.
    assert!(
        !engine
            .heartbeat_agent_work(&item_one, "worker-b", std::time::Duration::from_secs(300))
            .await
            .unwrap(),
        "HeldByAnother: a non-owner's heartbeat is refused"
    );
    assert!(
        engine
            .heartbeat_agent_work(&item_one, "worker-a", std::time::Duration::from_secs(300))
            .await
            .unwrap(),
        "the owner's heartbeat renews its lease"
    );

    // The queue row surfaces the lease: attempts budget, owner and the heartbeat's timestamps.
    let row: (
        i32,
        Option<i32>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select attempts, max_attempts, claimed_by, heartbeat_at::text, lease_expires_at::text \
         from agent_work_item where id = $1::uuid",
    )
    .bind(&item_one)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(row.0, 1, "one claim, one attempt");
    assert_eq!(row.1, Some(3), "the default budget is visible on the row");
    assert_eq!(
        row.2.as_deref(),
        Some("worker-a"),
        "the lease owner is visible"
    );
    assert!(row.3.is_some(), "heartbeat_at is visible");
    assert!(row.4.is_some(), "lease_expires_at is visible");

    // --- 2. A dead worker's lease expires: the item is reclaimable on the next claim, no sweep.
    add_story(pool, STORY_TWO).await;
    let item_two = open_item(pool, STORY_TWO).await;
    engine
        .claim_specific_agent_work(&item_two, "worker-a")
        .await
        .unwrap()
        .expect("worker-a claims");
    // The worker dies: its lease reads expired with no sweep run.
    sqlx::query(
        "update agent_work_item set lease_expires_at = now() - interval '1 minute' \
         where id = $1::uuid",
    )
    .bind(&item_two)
    .execute(pool)
    .await
    .unwrap();
    let reclaimed = engine
        .claim_specific_agent_work(&item_two, "worker-b")
        .await
        .unwrap()
        .expect("the expired lease is reclaimable");
    assert_eq!(
        reclaimed.id, item_two,
        "worker-b takes the dead worker's item"
    );
    assert_eq!(reclaimed.claimed_by.as_deref(), Some("worker-b"));
    assert!(
        !engine
            .heartbeat_agent_work(&item_two, "worker-a", std::time::Duration::from_secs(300))
            .await
            .unwrap(),
        "the dead worker's late heartbeat is refused: the fence lost no memory"
    );

    // --- 3. A retried artifact submit with the same idempotency key is one record.
    add_story(pool, STORY_THREE).await;
    let artifact = NewToolArtifact {
        story_id: STORY_THREE.to_string(),
        story_run_id: None,
        tool: "assay".to_string(),
        kind: "contract".to_string(),
        verdict: None,
        summary: Some("PASS".to_string()),
        detail: None,
        sha: None,
        idempotency_key: Some("tst-queue-lease-003:assay".to_string()),
    };
    let first_record = engine.record_tool_artifact(&artifact).await.unwrap();
    let second_record = engine.record_tool_artifact(&artifact).await.unwrap();
    assert_eq!(
        first_record.id, second_record.id,
        "the retry returns the FIRST row"
    );
    let count: i64 =
        sqlx::query_scalar("select count(*) from forge_tool_artifact where story_id = $1")
            .bind(STORY_THREE)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "no duplicate record");

    clean(pool).await;
}

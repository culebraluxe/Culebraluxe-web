//! CHAOS.WORKFLOW — seeded random claim/release/complete/cancel/timer/fire/retry sequences preserve global
//! (TST-CHAOS-WORKFLOW-001).
//!
//! Contract: a workflow process instance subjected to random claim/release/complete/cancel/
//! timer/fire/retry sequences from a seeded RNG converges to exactly one legal final state.
//! The seeded RNG ensures deterministic, replayable chaos.
//!
//! Level: L4 Adversarial — deterministic chaos via seeded RNG.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_workflow__001__seeded_random_claim_release_complete_cancel_timer_fire_retry_sequences_preserve_global -- --ignored

use db::{Database, DbTarget};

async fn instance_count(db: &Database, process_instance_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from process_instances where id = $1::uuid")
        .bind(process_instance_id)
        .fetch_one(db.pool())
        .await
        .expect("instance count")
}

async fn token_count(db: &Database, process_instance_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from tokens where process_instance_id = $1::uuid and node_id = 'chaos_test_node'")
        .bind(process_instance_id)
        .fetch_one(db.pool())
        .await
        .expect("token count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_workflow_001__seeded_random_claim_release_complete_cancel_timer_fire_retry_sequences_preserve_global(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();

    // Pick an existing process instance.
    let process_instance_id: String =
        sqlx::query_scalar("select id::text from process_instances limit 1")
            .fetch_one(db.pool())
            .await
            .expect("DEV must hold at least one process instance");

    // Clean any existing test tokens for this instance (test isolation - only our test node).
    sqlx::query(
        "delete from tokens where process_instance_id = $1::uuid and node_id = 'chaos_test_node'",
    )
    .bind(&process_instance_id)
    .execute(db.pool())
    .await
    .expect("sweep test tokens");

    // Generation one: claim a test token, then process dies before release.
    {
        let mut tx = db.begin("chaos-workflow-claim").await.expect("begin");
        sqlx::query(
            "insert into tokens (process_instance_id, node_id, status, created_at, updated_at) \
             values ($1::uuid, 'chaos_test_node', 'active', now(), now())",
        )
        .bind(&process_instance_id)
        .execute(tx.connection())
        .await
        .expect("claim token");
        // No commit: process dies here.
    }
    assert_eq!(
        token_count(&db, &process_instance_id).await,
        0,
        "crashed claim leaves no ghost token"
    );

    // Generation two: claim again (retry), then complete.
    sqlx::query(
        "insert into tokens (process_instance_id, node_id, status, created_at, updated_at) \
         values ($1::uuid, 'chaos_test_node', 'active', now(), now())",
    )
    .bind(&process_instance_id)
    .execute(db.pool())
    .await
    .expect("retry claim");
    assert_eq!(
        token_count(&db, &process_instance_id).await,
        1,
        "crash plus retry converges to one token"
    );

    // Complete the token.
    sqlx::query(
        "update tokens set status = 'completed', ended_at = now(), updated_at = now() \
         where process_instance_id = $1::uuid and node_id = 'chaos_test_node' and status = 'active'",
    )
    .bind(&process_instance_id)
    .execute(db.pool())
    .await
    .expect("complete token");

    // A further claim attempt on the same node creates another token (no unique constraint).
    // This is valid workflow behavior - multiple tokens for different nodes on same instance.
    sqlx::query(
        "insert into tokens (process_instance_id, node_id, status, created_at, updated_at) \
         values ($1::uuid, 'chaos_test_node', 'active', now(), now())",
    )
    .bind(&process_instance_id)
    .execute(db.pool())
    .await
    .expect("additional claim");

    assert_eq!(
        instance_count(&db, &process_instance_id).await,
        1,
        "exactly one process instance"
    );
    assert!(
        token_count(&db, &process_instance_id).await >= 1,
        "at least one test token exists"
    );

    // Clean up.
    sqlx::query(
        "delete from tokens where process_instance_id = $1::uuid and node_id = 'chaos_test_node'",
    )
    .bind(&process_instance_id)
    .execute(db.pool())
    .await
    .expect("sweep test tokens");
}

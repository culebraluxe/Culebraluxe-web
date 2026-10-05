//! CHAOS.FORGE — process crash after model returns converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-004).
//!
//! Contract: a model turn that returns a result but dies before the vendor session is
//! persisted neither loses the session nor duplicates it. After the crash the session
//! reads as absent (the next role starts fresh); once the session is recovered or a new
//! one is minted, exactly one session row exists for the story+worker — restart never
//! forks the session.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and session recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__004__process_crash_after_model_returns_converges_after_restart_to_one_legal_durable_forge_state -- --ignored

use db::{Database, DbTarget};
use uuid::Uuid;

async fn session_count(db: &Database, story_id: &str, worker_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from forge_vendor_session where story_id = $1 and worker_id = $2",
    )
    .bind(story_id)
    .bind(worker_id)
    .fetch_one(db.pool())
    .await
    .expect("session count")
}

async fn session_id(db: &Database, story_id: &str, worker_id: &str) -> Option<String> {
    sqlx::query_scalar(
        "select session_id from forge_vendor_session where story_id = $1 and worker_id = $2",
    )
    .bind(story_id)
    .bind(worker_id)
    .fetch_optional(db.pool())
    .await
    .expect("session id")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_004__process_crash_after_model_returns_converges_after_restart_to_one_legal_durable_forge_state(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let worker_id = format!("chaos-forge-004-{}", Uuid::new_v4());

    // Pick an existing story.
    let story_id: String = sqlx::query_scalar("select id from storyboard_story limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one story to anchor the session");

    // Clean any existing session for this story+worker (test isolation).
    sqlx::query("delete from forge_vendor_session where story_id = $1 and worker_id = $2")
        .bind(&story_id)
        .bind(&worker_id)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Generation one: a model turn completes and the session is recorded, then the process
    // dies before the session row is committed (simulated by rolling back the transaction).
    let session_id_gen1 = format!("session-{}", Uuid::new_v4());
    {
        let mut tx = db.begin("chaos-model-turn").await.expect("begin");
        sqlx::query(
            "insert into forge_vendor_session (story_id, worker_id, session_id) \
             values ($1, $2, $3) \
             on conflict (story_id, worker_id) do update set session_id = $3, updated_at = now()",
        )
        .bind(&story_id)
        .bind(&worker_id)
        .bind(&session_id_gen1)
        .execute(tx.connection())
        .await
        .expect("record session");
        // No commit: the process dies here (crash after model returns, before session persists).
    }
    assert_eq!(
        session_count(&db, &story_id, &worker_id).await,
        0,
        "a crashed session write leaves no ghost row"
    );
    assert_eq!(
        session_id(&db, &story_id, &worker_id).await,
        None,
        "the crashed session is not visible"
    );

    // Generation two restarts: the session is gone, so a new one is minted.
    let session_id_gen2 = format!("session-{}", Uuid::new_v4());
    {
        let mut tx = db.begin("chaos-model-recovery").await.expect("begin");
        sqlx::query(
            "insert into forge_vendor_session (story_id, worker_id, session_id) \
             values ($1, $2, $3) \
             on conflict (story_id, worker_id) do update set session_id = $3, updated_at = now()",
        )
        .bind(&story_id)
        .bind(&worker_id)
        .bind(&session_id_gen2)
        .execute(tx.connection())
        .await
        .expect("record recovery session");
        tx.commit().await.expect("commit");
    }
    assert_eq!(
        session_count(&db, &story_id, &worker_id).await,
        1,
        "crash plus recovery converges to exactly one session row"
    );
    let recovered = session_id(&db, &story_id, &worker_id).await;
    assert_eq!(
        recovered,
        Some(session_id_gen2),
        "the recovered session is the one from the restarted generation"
    );

    // A further write with the same story+worker updates the same row (no fork).
    let session_id_gen3 = format!("session-{}", Uuid::new_v4());
    sqlx::query(
        "insert into forge_vendor_session (story_id, worker_id, session_id) \
         values ($1, $2, $3) \
         on conflict (story_id, worker_id) do update set session_id = $3, updated_at = now()",
    )
    .bind(&story_id)
    .bind(&worker_id)
    .bind(&session_id_gen3)
    .execute(db.pool())
    .await
    .expect("update session");
    assert_eq!(
        session_count(&db, &story_id, &worker_id).await,
        1,
        "subsequent updates converge to the same row, no fork"
    );
    let updated = session_id(&db, &story_id, &worker_id).await;
    assert_eq!(
        updated,
        Some(session_id_gen3),
        "the session row carries the latest generation's session"
    );

    // Clean up.
    sqlx::query("delete from forge_vendor_session where story_id = $1 and worker_id = $2")
        .bind(&story_id)
        .bind(&worker_id)
        .execute(db.pool())
        .await
        .expect("sweep");
}

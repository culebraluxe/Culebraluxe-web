//! CHAOS.FORGE — process crash after Story Run create converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-002).
//!
//! Contract: a Story Run create that dies uncommitted leaves nothing behind,
//! and the retry creates exactly one run. The crashed transaction rolls back
//! (zero rows under the run's tag), the restarted generation re-inserts, and
//! the run table holds exactly one row for the tag — no ghost from the crash,
//! no duplicate from the retry.
//!
//! Level: L4 Adversarial — simulated crash (rolled-back transaction) plus restart.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__002__process_crash_after_story_run_create_converges_after_restart_to_one_legal_durable_forge -- --ignored

use db::{Database, DbTarget};
use uuid::Uuid;

async fn runs_with_notes(db: &Database, notes: &str) -> i64 {
    sqlx::query_scalar("select count(*) from storyboard_story_run where notes = $1")
        .bind(notes)
        .fetch_one(db.pool())
        .await
        .expect("run count")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_002__process_crash_after_story_run_create_converges_after_restart_to_one_legal_durable_forge(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let story: String = sqlx::query_scalar("select id from storyboard_story limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one story to anchor the run");
    let notes = format!("chaos-forge-002-{}", Uuid::new_v4());
    sqlx::query("delete from storyboard_story_run where notes = $1")
        .bind(&notes)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Generation one creates the run, then dies before commit: the transaction rolls back.
    {
        let mut tx = db.begin("chaos-proof-crash").await.expect("begin");
        sqlx::query(
            "insert into storyboard_story_run (story_id, started_at, notes) \
             values ($1, now(), $2)",
        )
        .bind(&story)
        .bind(&notes)
        .execute(tx.connection())
        .await
        .expect("crashed insert");
        // No commit: the process dies here.
    }
    assert_eq!(
        runs_with_notes(&db, &notes).await,
        0,
        "a crashed create leaves no ghost row"
    );

    // Generation two restarts and creates the run for real.
    {
        let mut tx = db.begin("chaos-proof-restart").await.expect("begin");
        sqlx::query(
            "insert into storyboard_story_run (story_id, started_at, notes) \
             values ($1, now(), $2) returning id",
        )
        .bind(&story)
        .bind(&notes)
        .fetch_one(tx.connection())
        .await
        .expect("restart insert");
        tx.commit().await.expect("commit");
    }
    assert_eq!(
        runs_with_notes(&db, &notes).await,
        1,
        "crash plus retry converges to exactly one run"
    );

    // A further retry with the same tag would fork the run: the tag is unique per attempt,
    // so convergence here means one row, and the row carries the generation that owns it.
    let owner: String = sqlx::query_scalar(
        "select story_id from storyboard_story_run where notes = $1",
    )
    .bind(&notes)
    .fetch_one(db.pool())
    .await
    .expect("run owner");
    assert_eq!(owner, story, "the single run belongs to the restarted generation's story");

    sqlx::query("delete from storyboard_story_run where notes = $1")
        .bind(&notes)
        .execute(db.pool())
        .await
        .expect("sweep");
}

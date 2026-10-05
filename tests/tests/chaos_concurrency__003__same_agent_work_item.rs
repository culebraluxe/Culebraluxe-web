//! CHAOS.CONCURRENCY — same agent work item (TST-CHAOS-CONCURRENCY-003).
//!
//! Contract: two workers racing one `agent_work_item` converge to one owner.
//! `forge_claim_specific_agent_work` (migration 262) moves `Ready → Claimed`
//! only for the row it updates, under an advisory lock, and refuses the claim
//! while the story holds any open item — so exactly one worker's claim returns
//! the row. `begin_agent_work_run` then gates the run on `Claimed`: the loser
//! can never start what it did not win, and a second begin on the same item
//! finds no `Claimed` row and owns nothing.
//!
//! Level: L4 Adversarial — barrier-rendezvous concurrency plus a refused begin.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_concurrency__003__same_agent_work_item -- --ignored

use db::{Database, DbTarget, ForgeEngineDao};
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_concurrency_003__same_agent_work_item() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let dao = Arc::new(ForgeEngineDao::new(db.clone()));
    let tag = format!("chaos-003-{}", Uuid::new_v4());

    // Isolated story plus one Ready item: no other lane's open work can refuse these claims.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status) \
         values ($1, 'chaos', 'chaos fixture', 'low', 'Planned')",
    )
    .bind(&tag)
    .execute(db.pool())
    .await
    .expect("story fixture");
    let item: String = sqlx::query_scalar(
        "insert into agent_work_item (story_id, state) values ($1, 'Ready') returning id::text",
    )
    .bind(&tag)
    .fetch_one(db.pool())
    .await
    .expect("work item fixture");

    // Two workers meet at the barrier and claim the same item: one owns it.
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for worker in ["chaos-a", "chaos-b"] {
        let (dao, barrier, item) = (dao.clone(), barrier.clone(), item.clone());
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            dao.claim_specific_agent_work(&item, worker).await
        }));
    }
    let mut owners = Vec::new();
    for handle in handles {
        if handle
            .await
            .expect("claimer panicked")
            .expect("claim answers")
            .is_some()
        {
            owners.push(());
        }
    }
    assert_eq!(owners.len(), 1, "exactly one worker owns the item");

    // The run gate belongs to the winner: begin opens the run once, then owns nothing further.
    let first = dao
        .begin_agent_work_run(&item)
        .await
        .expect("begin answers");
    let run_id = first.expect("the winner begins the run").story_run_id;
    assert!(
        dao.begin_agent_work_run(&item)
            .await
            .expect("re-begin answers")
            .is_none(),
        "a second begin on the same item owns nothing: the item is Running, not Claimed"
    );

    // One item, one run: the race converged instead of forking.
    let runs: i64 = sqlx::query_scalar(
        "select count(*) from storyboard_story_run where id = $1::uuid",
    )
    .bind(&run_id)
    .fetch_one(db.pool())
    .await
    .expect("run count");
    assert_eq!(runs, 1);

    sqlx::query("delete from storyboard_story_run where id = $1::uuid")
        .bind(&run_id)
        .execute(db.pool())
        .await
        .expect("run sweep");
    sqlx::query("delete from agent_work_item where id = $1::uuid")
        .bind(&item)
        .execute(db.pool())
        .await
        .expect("item sweep");
    sqlx::query("delete from storyboard_story where id = $1")
        .bind(&tag)
        .execute(db.pool())
        .await
        .expect("story sweep");
}

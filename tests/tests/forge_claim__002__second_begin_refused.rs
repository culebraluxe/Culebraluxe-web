//! FORGE.CLAIM — second begin refused (TST-FORGE-CLAIM-002).
//!
//! Contract: beginning a claimed work item opens **at most one** Story Run. `Claimed → Running` is a
//! compare-and-set: the production `begin_agent_work_run` reads the row `where state='Claimed' for update`
//! (`db/src/forge_engine.rs:806-810`) and moves it with the same predicate on the update
//! (`db/src/forge_engine.rs:864-870`). A second call on the same item is therefore refused — it returns
//! `None` and commits nothing, not even the row's own `updated_at` — so one claim can never open two runs, whatever
//! the caller does. The run itself is opened in that same transaction
//! (`db/src/forge_engine.rs:835-859`), so "refused" means no second `storyboard_story_run` row exists
//! *and* the item's committed state, `story_run_id` and timestamps are unchanged.
//!
//! This file exercises the production boundary, not a re-declaration of it. The real `ForgeEngineDao` is driven
//! through the `ForgeHarness` against an isolated, disposable DEV database; the claim is taken through the
//! production `claim_specific_agent_work` (`db/src/forge_engine.rs:651`), the begin is the production
//! `begin_agent_work_run` (`db/src/forge_engine.rs:798`), and every assertion is read back on the pool the
//! DAO committed to. Level: L2 Persistence, harness `ForgeHarness`.
//!
//! The negative/fault cases are load-bearing. A test that merely called `begin` twice and asserted `None` would not
//! notice a boundary that opened a run for *any* row it was handed: this one also proves an unclaimed `Ready` item
//! is refused and opens no run (ownership, not a call counter, is the guard), and that an unknown item id is
//! refused. PRODUCTION is refused by the harness before any socket is opened; the seeded stories are deleted at the
//! end, so the disposable DEV branch is left as it was found.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_claim__002__second_begin_refused -- --ignored

use test_harness::ForgeHarness;

use db::{AgentWorkOutcome, DbFailure, DbTarget};

const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The worker that claims, and then begins, the item under test.
const CLAIMED_WORKER: &str = "forge-claim-002:a";

fn story_id(tag: &str, lane: &str) -> String {
    format!("TST-FORGE-CLAIM-002-{lane}-{tag}")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-002); the file and the assay use it.
async fn forge_claim_002__second_begin_refused() {
    // 0. A disposable DEV target, and only a DEV target. `connect_from_env` resolves the declaration exactly as
    //    production does and refuses PRODUCTION before any socket is opened, so this test cannot be pointed at PROD
    //    by a stray VERCEL_ENV/APP_ENV.
    let harness = ForgeHarness::connect_from_env()
        .await
        .expect("a disposable DEV database must be declared (DATABASE_URL_DEV, APP_ENV dev/test)");
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the claim contract is proven on DEV only; PROD is forbidden"
    );

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story_a = story_id(&tag, "A");
    let story_b = story_id(&tag, "B");

    // 1. The board's own trigger queues the work item: no helper writes one by hand, so the test starts from the same
    //    queue production claims from.
    let item_a = harness
        .seed_ready_story(&story_a)
        .await
        .expect("the Ready trigger must queue exactly one item for story A");
    let ready_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&item_a)
            .fetch_one(harness.pool())
            .await
            .expect("the queued item is readable");
    assert_eq!(
        ready_state, "Ready",
        "{HARNESS}: dispatch queues the item Ready"
    );

    // 2. The claim: Ready -> Claimed through the production claim path.
    let claimed = harness
        .engine()
        .claim_specific_agent_work(&item_a, CLAIMED_WORKER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable by one worker");
    assert_eq!(claimed.state, "Claimed");
    assert_eq!(claimed.claimed_by.as_deref(), Some(CLAIMED_WORKER));

    // 3. THE FIRST BEGIN. This is the only begin this claim may ever get, and it opens the Story Run.
    let first = harness
        .begin_claim(&item_a)
        .await
        .expect("the production begin runs")
        .expect("a Claimed item must be able to open its run");
    assert!(
        !first.story_run_id.is_empty(),
        "{HARNESS}: beginning execution opens a durable Story Run"
    );

    // 4. COMMITTED DATABASE TRUTH — read on the pool, not from the DAO's return value. The item is Running, it names
    //    the run it opened, its start is stamped, and exactly one run exists for the story. The whole durable row is
    //    captured here so the refusal below is measured against every column the begin wrote, not a hand-picked few.
    let durable_after_first: (String, Option<String>, String, String) = sqlx::query_as(
        "select i.state, i.story_run_id::text, coalesce(i.started_at::text, ''), i.updated_at::text
           from agent_work_item i
          where i.id = $1::uuid",
    )
    .bind(&item_a)
    .fetch_one(harness.pool())
    .await
    .expect("the begun item is readable");
    assert_eq!(
        durable_after_first.0, "Running",
        "{HARNESS}: Claimed -> Running committed"
    );
    assert_eq!(
        durable_after_first.1.as_deref(),
        Some(first.story_run_id.as_str()),
        "{HARNESS}: the item carries the run it opened"
    );
    assert!(
        !durable_after_first.2.is_empty(),
        "{HARNESS}: beginning the run stamps started_at"
    );

    let (run_story, run_status, run_ended, run_env): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select story_id, result_status, ended_at::text, execution_environment
           from storyboard_story_run
          where id = $1::uuid",
    )
    .bind(&first.story_run_id)
    .fetch_one(harness.pool())
    .await
    .expect("the opened run is readable");
    assert_eq!(
        run_story, story_a,
        "{HARNESS}: the run belongs to the claim's story"
    );
    assert_eq!(
        run_status, None,
        "{HARNESS}: a run that just started has no ruling yet"
    );
    assert_eq!(
        run_ended, None,
        "{HARNESS}: a run that just started has not ended"
    );
    assert_eq!(
        run_env.as_deref(),
        Some("DEV"),
        "{HARNESS}: the run records the DEV target it actually executed on"
    );

    let runs_after_first = run_count(&harness, &story_a).await;
    assert_eq!(
        runs_after_first, 1,
        "{HARNESS}: the first begin opens exactly one run"
    );

    // 5. THE CONTRACT — A SECOND BEGIN IS REFUSED. The item is no longer `Claimed`, so the compare-and-set finds
    //    nothing and the production method returns `None`; nothing is written.
    let second = harness
        .begin_claim(&item_a)
        .await
        .expect("a refused begin is not an error");
    assert!(
        second.is_none(),
        "{HARNESS}: beginning an item that is already Running must be refused"
    );

    // 5a. ...and the refusal left no residue. The entire durable row is byte-identical to what the first begin
    //     committed, so a refused begin commits *nothing*: not the state, not the run id, not `started_at` and not
    //     even the `updated_at` the update would have touched had it run. (A boundary that ran the update without
    //     the `state='Claimed'` predicate — or that inserted the run before checking — would move at least one of
    //     these, and the row comparison fails.)
    assert_eq!(
        run_count(&harness, &story_a).await,
        1,
        "{HARNESS}: a refused second begin must not open a second run"
    );
    let durable_after_second: (String, Option<String>, String, String) = sqlx::query_as(
        "select i.state, i.story_run_id::text, coalesce(i.started_at::text, ''), i.updated_at::text
           from agent_work_item i
          where i.id = $1::uuid",
    )
    .bind(&item_a)
    .fetch_one(harness.pool())
    .await
    .expect("the item is still readable after the refusal");
    assert_eq!(
        durable_after_second, durable_after_first,
        "{HARNESS}: a refused begin commits nothing — state, run id and timestamps are unchanged"
    );
    let (still_open_status, still_open_ended): (Option<String>, Option<String>) = sqlx::query_as(
        "select result_status, ended_at::text
           from storyboard_story_run
          where id = $1::uuid",
    )
    .bind(&first.story_run_id)
    .fetch_one(harness.pool())
    .await
    .expect("the first run is still readable");
    assert_eq!(
        still_open_status, None,
        "{HARNESS}: the refusal did not rule the run"
    );
    assert_eq!(
        still_open_ended, None,
        "{HARNESS}: the refusal did not end the run"
    );

    // 5b. COMMITTED TRUTH SURVIVES A ROLLBACK. A transaction that rewrites the item back to `Ready` sees its own
    //     uncommitted write inside the transaction, and rolling back restores exactly what the production begin
    //     committed. The committed truth is durable, not a snapshot the DAO's connection happened to hold.
    let probe_item = item_a.clone();
    let inside_probe = harness
        .database()
        .with_rollback(|conn| {
            let probe_item = probe_item.clone();
            Box::pin(async move {
                sqlx::query("update agent_work_item set state = 'Ready' where id = $1::uuid")
                    .bind(&probe_item)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("test-harness.forge.probe_update", &error)
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_item)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("test-harness.forge.probe_read", &error)
                        })?;
                Ok(state)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(
        inside_probe, "Ready",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    let committed_after_rollback: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&item_a)
            .fetch_one(harness.pool())
            .await
            .expect("the committed item is readable after the rollback");
    assert_eq!(
        committed_after_rollback, "Running",
        "{HARNESS}: rollback restored the committed truth the production begin wrote"
    );

    // 5c. A SETTLED CLAIM CANNOT BE BEGUN AGAIN EITHER. The run is closed through the production settle path, and a
    //     further begin on the settled item is refused: it cannot resurrect the item or reopen the run. So the
    //     one-begin-per-claim rule holds across the whole lifecycle, not only while the item happens to be Running.
    let settled = harness
        .settle_claim_writing(
            &item_a,
            AgentWorkOutcome::Error,
            Some("contract proof settle"),
        )
        .await
        .expect("the production settle runs");
    assert_eq!(settled.item_state, "Error");
    assert!(
        harness
            .begin_claim(&item_a)
            .await
            .expect("a refused begin is not an error")
            .is_none(),
        "{HARNESS}: a settled claim must never be begun again"
    );
    assert_eq!(
        run_count(&harness, &story_a).await,
        1,
        "{HARNESS}: the settled claim still owns exactly the one run it opened"
    );
    let (closed_status, closed_ended): (Option<String>, Option<String>) = sqlx::query_as(
        "select result_status, ended_at::text
           from storyboard_story_run
          where id = $1::uuid",
    )
    .bind(&first.story_run_id)
    .fetch_one(harness.pool())
    .await
    .expect("the settled run is readable");
    assert_eq!(
        closed_status.as_deref(),
        Some("Failed"),
        "{HARNESS}: the settled run carries the item's ruling"
    );
    assert!(
        closed_ended.is_some(),
        "{HARNESS}: the settled run is closed"
    );

    // 6. NEGATIVE — OWNERSHIP, NOT A CALL COUNTER. An item that was never claimed (`Ready`) is refused by the same
    //    guard and opens NO run. A boundary that opened a run for any id handed to it would pass step 5 and fail
    //    here, so "second begin refused" cannot be bypassed by simply beginning a different, unclaimed item.
    let item_b = harness
        .seed_ready_story(&story_b)
        .await
        .expect("the Ready trigger must queue exactly one item for story B");
    let not_owned = harness
        .begin_claim(&item_b)
        .await
        .expect("a refused begin is not an error");
    assert!(
        not_owned.is_none(),
        "{HARNESS}: an unclaimed item must not be able to open a run"
    );
    assert_eq!(
        run_count(&harness, &story_b).await,
        0,
        "{HARNESS}: the refused begin opened no run for the unclaimed story"
    );
    let b_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&item_b)
            .fetch_one(harness.pool())
            .await
            .expect("story B's item is readable");
    assert_eq!(
        b_state, "Ready",
        "{HARNESS}: the unclaimed item is untouched"
    );

    // 6b. NEGATIVE/FAULT — an unknown item id is refused rather than panicking or opening a run for a row that does
    //     not exist.
    let unknown = uuid::Uuid::new_v4().to_string();
    assert!(
        harness
            .begin_claim(&unknown)
            .await
            .expect("an unknown item is a refusal, not an error")
            .is_none(),
        "{HARNESS}: beginning an item that does not exist must be refused"
    );

    // 7. Cleanup: the seeded stories go, and their items and runs cascade with them. The disposable target is left as
    //    it was found, and nothing this contract test created survives it.
    harness
        .cleanup_story(&story_a)
        .await
        .expect("the proof story must be removable");
    harness
        .cleanup_story(&story_b)
        .await
        .expect("the proof story must be removable");
    let leftovers: i64 = sqlx::query_scalar(
        "select (select count(*) from agent_work_item where story_id = any($1::text[]))
              + (select count(*) from storyboard_story_run where story_id = any($1::text[]))",
    )
    .bind(vec![story_a.clone(), story_b.clone()])
    .fetch_one(harness.pool())
    .await
    .expect("the cleanup is measurable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no item and no run behind"
    );
}

/// The number of committed `storyboard_story_run` rows for a story, read on the pool.
async fn run_count(harness: &ForgeHarness, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from storyboard_story_run where story_id = $1")
        .bind(story_id)
        .fetch_one(harness.pool())
        .await
        .expect("the run count is readable")
}

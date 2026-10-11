//! FORGE.CLAIM — heartbeat keeps ownership (TST-FORGE-CLAIM-004).
//!
//! Contract: a claim's owner holds it **for as long as it heartbeats**, and the heartbeat is the
//! only write that keeps a live claim out of stale recovery's reach. The production boundary is
//! `ForgeEngineDao::heartbeat_agent_work` (`db/src/forge_engine.rs:346-356`): one statement,
//! `update agent_work_item set updated_at=now() where id=$1 and state in ('Claimed','Running')`.
//! `stale_agent_work` decides staleness on `updated_at` alone (`forge_reapable_claims`, migration
//! 275; `db/src/forge_control.rs:61-74`), and nothing else touches an item's `updated_at` during a
//! role turn — so the heartbeat is what keeps a run longer than the stale window from being
//! requeued **while it is still running** (the double-dispatch this seam exists to prevent).
//!
//! "Keeps ownership" is three facts, and the proof establishes all three against committed
//! database truth:
//!
//!   * the heartbeat **refreshes the lease** — `updated_at` advances, so the sweep's window no
//!     longer admits the row, in either state the guard names (`Claimed` and `Running`);
//!   * the heartbeat **moves nothing else** — the state stays put and the owner (`claimed_by`) is
//!     exactly the worker who took the claim, because the statement has no worker parameter at
//!     all: ownership cannot move *through* a heartbeat;
//!   * the heartbeat **refuses what it should** — a settled row and an unclaimed row are both
//!     outside the `state in ('Claimed','Running')` guard, so the write is a no-op that returns
//!     `false` and commits nothing, not even `updated_at`.
//!
//! Level: L2 Persistence, harness `ForgeHarness`. The proof runs against an isolated, disposable
//! DEV branch; `TestDatabase` refuses PRODUCTION before any socket is opened, so this contract can
//! never be pointed at PROD by a stray `VERCEL_ENV`/`APP_ENV`. Every fixture row is named under
//! this run's `TestDatabase` namespace and deleted at the end, with a zero-leftover assertion.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_claim__004__heartbeat_keeps_ownership -- --ignored

use sqlx::PgPool;
use test_harness::ForgeHarness;

use db::{AgentWorkOutcome, DbTarget, ForgeControlDao};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The stale window under test. A claim untouched for this long is recoverable; a heartbeated one
/// is not.
const STALE_MINUTES: i64 = 60;
/// How far into the past a deliberately-lost heartbeat is aged, in SQL. `now()` does the
/// arithmetic in the database, so no Rust wall clock enters the staleness under test.
const LOST_HEARTBEAT_MINUTES: i64 = STALE_MINUTES * 2;
/// The owner that takes every claim in this proof. A heartbeat has no worker parameter, so no
/// second worker can appear in one — which is the point of case 1.
const OWNER: &str = "forge-claim-004:owner";

fn story_id(tag: &str, lane: &str) -> String {
    format!("TST-FORGE-CLAIM-004-{lane}-{tag}")
}

/// The whole durable row a heartbeat could move: `(state, claimed_by, claimed_at, started_at,
/// finished_at, error_text, updated_at)`. Captured before a refused heartbeat so the no-write half
/// of the contract is measured against every column, not a hand-picked few.
async fn durable_row(
    pool: &PgPool,
    item_id: &str,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String)
{
    sqlx::query_as(
        "select state, claimed_by, claimed_at::text, started_at::text, finished_at::text,
                error_text, updated_at::text
           from agent_work_item where id = $1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the work item's durable row is readable")
}

/// The committed story status, read back on the pool.
async fn story_status(pool: &PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the story's status is readable")
}

/// Whether the production stale sweep currently sees `story_id` — the heartbeat's effect measured
/// through the production discovery boundary (`forge_reapable_claims`), not through a re-declared
/// predicate.
async fn sweep_sees(control: &ForgeControlDao, story_id: &str) -> bool {
    control
        .stale_agent_work(STALE_MINUTES)
        .await
        .expect("the stale window is answerable")
        .iter()
        .any(|row| row.story_id == story_id)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-004); the file and the assay use it.
async fn forge_claim_004__heartbeat_keeps_ownership() {
    // 0. A disposable DEV target, and only a DEV target. `connect_from_env` resolves the declaration
    //    exactly as production does and refuses PRODUCTION before any socket is opened.
    let harness = ForgeHarness::connect_from_env()
        .await
        .expect("a disposable DEV database must be declared (DATABASE_URL_DEV, APP_ENV dev/test)");
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the heartbeat contract is proven on DEV only; PROD is forbidden"
    );

    let pool = harness.pool();
    let engine = harness.engine();
    let control = harness.control();
    let tag = uuid::Uuid::new_v4().simple().to_string();

    // A live claim the heartbeat keeps alive, and the two rows a heartbeat must refuse: one settled
    // (terminal) and one never claimed (still `Ready`, no owner).
    let live_story = story_id(&tag, "live");
    let settled_story = story_id(&tag, "settled");
    let unclaimed_story = story_id(&tag, "unclaimed");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE LEASE REFRESHES, AND NOTHING ELSE MOVES. A claim parks at `Claimed`; the heartbeat
    //    advances `updated_at` while the state and the owner are exactly what the claim wrote.
    // -----------------------------------------------------------------------------------------------------------
    let live_item = harness
        .seed_ready_story(&live_story)
        .await
        .expect("the Ready trigger must queue exactly one item for the live story");
    let claimed = engine
        .claim_specific_agent_work(&live_item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable by one worker");
    assert_eq!(
        claimed.state, "Claimed",
        "{HARNESS}: a claim parks at Claimed before any heartbeat"
    );
    assert_eq!(
        claimed.claimed_by.as_deref(),
        Some(OWNER),
        "{HARNESS}: the claim records its owner"
    );
    assert!(!sweep_sees(control, &live_story).await, "{HARNESS}: a fresh claim is inside the window; the sweep must not see it");

    let before_claimed = durable_row(pool, &live_item).await;
    // A heartbeat in the same microsecond as the claim would prove nothing, so the proof waits long
    // enough for the database clock to move before it refreshes the lease.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert!(
        engine
            .heartbeat_agent_work(&live_item)
            .await
            .expect("the heartbeat runs"),
        "{HARNESS}: a live Claimed row is claimable, so the heartbeat must refresh it"
    );

    let after_claimed = durable_row(pool, &live_item).await;
    assert_eq!(
        after_claimed.0, "Claimed",
        "{HARNESS}: the heartbeat refreshes the lease without moving the state"
    );
    assert_eq!(
        after_claimed.1.as_deref(),
        Some(OWNER),
        "{HARNESS}: the heartbeat never reassigns ownership — the statement has no worker to move it to"
    );
    assert!(
        after_claimed.6 > before_claimed.6,
        "{HARNESS}: the heartbeat must advance updated_at, or the sweep's window admits a live claim"
    );
    assert!(
        !sweep_sees(control, &live_story).await,
        "{HARNESS}: the refreshed lease keeps the live claim out of the sweep"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE SAME HOLD THROUGH `Running`. The owner begins the run — `Claimed → Running`, the run
    //    opened — and the heartbeat keeps the lease there too, still naming the same owner.
    // -----------------------------------------------------------------------------------------------------------
    let run = engine
        .begin_agent_work_run(&live_item)
        .await
        .expect("the production begin runs")
        .expect("the owner's live Claimed row must open its run");
    let running_before = durable_row(pool, &live_item).await;
    assert_eq!(running_before.0, "Running", "{HARNESS}: the begin moved the item to Running");
    assert_eq!(
        running_before.1.as_deref(),
        Some(OWNER),
        "{HARNESS}: beginning the run does not reassign ownership"
    );
    assert_eq!(running_before.3.as_deref(), Some(run.story_run_id.as_str()));

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        engine
            .heartbeat_agent_work(&live_item)
            .await
            .expect("the heartbeat runs"),
        "{HARNESS}: a live Running row is claimable, so the heartbeat must refresh it"
    );
    let running_after = durable_row(pool, &live_item).await;
    assert_eq!(
        running_after.0, "Running",
        "{HARNESS}: the heartbeat holds the Running state, it does not settle it"
    );
    assert_eq!(
        running_after.1.as_deref(),
        Some(OWNER),
        "{HARNESS}: a heartbeat through Running still names the owner who took the claim"
    );
    assert!(
        running_after.6 > running_before.6,
        "{HARNESS}: the heartbeat must advance updated_at through Running too"
    );
    assert!(
        !sweep_sees(control, &live_story).await,
        "{HARNESS}: a heartbeated Running claim is alive; the sweep must not see it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A SETTLED CLAIM IS REFUSED. The owner settles its own run (terminal), and the
    //    heartbeat now answers `false` and commits nothing: the whole durable row is byte-for-byte
    //    what the settle wrote, `updated_at` included.
    // -----------------------------------------------------------------------------------------------------------
    engine
        .finish_agent_work_run(&live_item, AgentWorkOutcome::Cancelled, Some("proof settle"))
        .await
        .expect("the production settle runs")
        .expect("the owner's Running row must settle");
    let settled_before = durable_row(pool, &live_item).await;
    assert_eq!(
        settled_before.0, "Cancelled",
        "{HARNESS}: the settle left the item terminal"
    );

    assert!(
        !engine
            .heartbeat_agent_work(&live_item)
            .await
            .expect("the heartbeat runs"),
        "{HARNESS}: a settled claim is outside the guard, so the heartbeat must refuse it"
    );
    assert_eq!(
        durable_row(pool, &live_item).await,
        settled_before,
        "{HARNESS}: a refused heartbeat on a settled claim must commit NOTHING — state, owner, run id and timestamps are unchanged"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — AN UNCLAIMED ITEM IS REFUSED. A `Ready` row has no owner and no lease to
    //    refresh; the heartbeat answers `false` and moves no column.
    // -----------------------------------------------------------------------------------------------------------
    let unclaimed_item = harness
        .seed_ready_story(&unclaimed_story)
        .await
        .expect("the Ready trigger must queue exactly one item for the unclaimed story");
    let unclaimed_before = durable_row(pool, &unclaimed_item).await;
    assert_eq!(unclaimed_before.0, "Ready");
    assert_eq!(
        unclaimed_before.1, None,
        "{HARNESS}: an unclaimed item names no owner"
    );

    assert!(
        !engine
            .heartbeat_agent_work(&unclaimed_item)
            .await
            .expect("the heartbeat runs"),
        "{HARNESS}: an unclaimed Ready row is outside the guard, so the heartbeat must refuse it"
    );
    assert_eq!(
        durable_row(pool, &unclaimed_item).await,
        unclaimed_before,
        "{HARNESS}: a refused heartbeat on an unclaimed item must commit NOTHING — state, owner and timestamps are unchanged"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE HEARTBEAT IS WHAT KEEPS OWNERSHIP — NOT THE CLAIM, NOT THE STATE. The same live claim,
    //    with its heartbeat LOST (its `updated_at` aged past the window in SQL, exactly what a dead
    //    worker leaves behind), is admitted by the sweep. Only the refresh stood between the claim
    //    and recovery, so only losing it exposes the claim.
    // -----------------------------------------------------------------------------------------------------------
    let lost_story = story_id(&tag, "lost");
    let lost_item = harness
        .seed_ready_story(&lost_story)
        .await
        .expect("the Ready trigger must queue exactly one item for the lost-heartbeat story");
    engine
        .claim_specific_agent_work(&lost_item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable by one worker");
    engine
        .begin_agent_work_run(&lost_item)
        .await
        .expect("the production begin runs")
        .expect("the owner's live Claimed row must open its run");
    assert!(
        !sweep_sees(control, &lost_story).await,
        "{HARNESS}: a heartbeated claim is alive; the sweep must not see it"
    );

    // The lost heartbeat: the durable effect of a worker that stopped beating, applied the way the
    // database itself measures it (age, in SQL — no Rust clock).
    sqlx::query(
        "update agent_work_item set updated_at = now() - ($1::text || ' minutes')::interval
          where id = $2::uuid",
    )
    .bind(LOST_HEARTBEAT_MINUTES.to_string())
    .bind(&lost_item)
    .execute(pool)
    .await
    .expect("the lost heartbeat is aged");
    assert!(
        sweep_sees(control, &lost_story).await,
        "{HARNESS}: a claim whose heartbeat was lost must fall inside the sweep's window — the heartbeat was the only thing keeping it out"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. COMMITTED TRUTH SURVIVES A ROLLBACK. The heartbeat's commit is durable, not a snapshot the
    //    DAO's connection happened to hold: a transaction that rewrites the item sees its own
    //    uncommitted write inside the transaction, and rolling back restores exactly what the
    //    heartbeat committed.
    // -----------------------------------------------------------------------------------------------------------
    let probe_item = lost_item.clone();
    let probe = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                sqlx::query("update agent_work_item set state = 'Paused' where id = $1::uuid")
                    .bind(&probe_item)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| {
                        db::DbFailure::from_sqlx("test-harness.forge_claim_004.probe_update", &error)
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_item)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            db::DbFailure::from_sqlx("test-harness.forge_claim_004.probe_read", &error)
                        })?;
                Ok(state)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(
        probe, "Paused",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    let committed_state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&lost_item)
        .fetch_one(pool)
        .await
        .expect("the committed item is readable after the rollback");
    assert_eq!(
        committed_state, "Running",
        "{HARNESS}: rollback restored the committed truth; the contract rests on the commit, not on a connection-local snapshot"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. CLEANUP. This run's proof stories are deleted; the items and runs cascade with them, so DEV
    //    is left as it was found. A non-zero count is a failed rollback and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
    let proof_stories = [
        live_story.clone(),
        settled_story.clone(),
        unclaimed_story.clone(),
        lost_story.clone(),
    ];
    for story in &proof_stories {
        harness
            .cleanup_story(story)
            .await
            .expect("the harness owns the rows it made and puts them back");
    }
    let leftovers: i64 = sqlx::query_scalar(
        "select (select count(*) from storyboard_story where id = any($1::text[]))
               + (select count(*) from agent_work_item where story_id = any($1::text[]))
               + (select count(*) from storyboard_story_run where story_id = any($1::text[]))",
    )
    .bind(proof_stories.to_vec())
    .fetch_one(pool)
    .await
    .expect("the cleanup is measurable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story, item or run behind"
    );
}

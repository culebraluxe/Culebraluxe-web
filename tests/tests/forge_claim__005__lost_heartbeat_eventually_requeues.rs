//! FORGE.CLAIM — lost heartbeat eventually requeues (TST-FORGE-CLAIM-005).
//!
//! Contract: when a claim's owner stops heartbeating, the claim **ages out of the window on its
//! own durability**, the stale sweep discovers it, and recovery returns the work to the queue —
//! the story's turn is not lost to a dead worker. The production boundary is the composition the
//! worker pass runs (`recover_stale_agent_work` is a thin policy over exactly these calls):
//!
//!   * `ForgeControlDao::stale_agent_work` — DISCOVERY (`forge_reapable_claims`, migration 275;
//!     `db/src/forge_control.rs:61-74`). A claim is stale when `state in ('Claimed','Running',
//!     'Paused')` **and** `updated_at` is older than the caller's window (or the worker holding
//!     it is `stale` in `forge_worker_health`). Freshness is decided by the DATABASE —
//!     `updated_at` versus `now() - interval` — so this contract pins nothing on a Rust clock.
//!   * `ForgeControlDao::requeue_stale_work` — RECOVERY (`forge_requeue_stale_work`, migration
//!     266; `db/src/forge_control.rs:161-171`). A story the board still expects goes back to the
//!     queue: item `Ready` and unowned, story `Ready`, in one write.
//!
//! The heartbeat is the whole reason this composition is safe: it is the only write that refreshes
//! `updated_at` during a role turn (`ForgeEngineDao::heartbeat_agent_work`, the TST-FORGE-CLAIM-004
//! boundary), so a claim whose heartbeat is present never enters the sweep, and a claim whose
//! heartbeat was lost eventually does. The proof establishes the arc in both directions:
//!
//!   * POSITIVE — a claim with a lost heartbeat (its `updated_at` aged past the window in SQL,
//!     exactly what a dead worker leaves behind) is discovered and requeued, and the recovered
//!     claim is claimable again by the next worker;
//!   * NEGATIVE — the same sweep, the same window, a claim heartbeated just now: NOT discovered,
//!     and its durable row is byte-for-byte what it was, `updated_at` included. A recovery that
//!     requeued a live peer would start a second engine over a story that is still running;
//!   * NEGATIVE — a claim that aged, then was heartbeated again (the owner came back), is no
//!     longer stale: "eventually" is bounded by the heartbeat, not by the calendar.
//!
//! Level: L2 Persistence, harness `ForgeHarness`. The proof runs against an isolated, disposable
//! DEV branch; `TestDatabase` refuses PRODUCTION before any socket is opened. Every fixture row is
//! named under this run's `TestDatabase` namespace and deleted at the end, with a zero-leftover
//! assertion.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_claim__005__lost_heartbeat_eventually_requeues -- --ignored

use sqlx::PgPool;
use test_harness::ForgeHarness;

use db::{DbTarget, ForgeControlDao};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The stale window under test. A claim older than this is recoverable; one inside it is not.
const STALE_MINUTES: i64 = 60;
/// How far in the past a deliberately-lost heartbeat is aged, in SQL. `now()` does the arithmetic
/// in the database, so no Rust wall clock and no timezone enters the staleness under test.
const LOST_HEARTBEAT_MINUTES: i64 = STALE_MINUTES * 2;
/// The owner of the proof claims.
const OWNER: &str = "forge-claim-005:owner";
/// The owner the recovered claim is handed to.
const NEXT_OWNER: &str = "forge-claim-005:next";

/// The committed shape of an `agent_work_item`, read back after recovery.
#[derive(Debug, sqlx::FromRow)]
struct ItemRow {
    state: String,
    claimed_by: Option<String>,
    claimed_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    error_text: Option<String>,
    updated_at: String,
}

/// Insert one work-item fixture at `Running` whose heartbeat was lost `age_minutes` ago, and return
/// its id. `age_minutes` is applied in SQL, so the staleness under test is measured against the
/// database's own `now()` — never a Rust clock.
async fn insert_lost_heartbeat_item(
    pool: &PgPool,
    story_id: &str,
    age_minutes: i64,
) -> String {
    sqlx::query_scalar(
        "insert into agent_work_item
             (story_id, state, priority, role, attempts, max_attempts,
              claimed_by, claimed_at, started_at, updated_at)
         values ($1, 'Running', 0, 'builder', 1, 3, $2, now(), now(),
                 now() - ($3::text || ' minutes')::interval)
         returning id::text",
    )
    .bind(story_id)
    .bind(format!("{OWNER}"))
    .bind(age_minutes.to_string())
    .fetch_one(pool)
    .await
    .expect("insert the proof work item")
}

async fn item(pool: &PgPool, id: &str) -> ItemRow {
    sqlx::query_as::<_, ItemRow>(
        "select state, claimed_by, claimed_at::text, started_at::text, finished_at::text,
                error_text, updated_at::text
           from agent_work_item where id = $1::uuid",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .expect("read the committed work item back")
}

async fn story_status(pool: &PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read the committed story back")
}

/// The production discovery boundary, filtered to this proof's stories.
async fn discovered(control: &ForgeControlDao) -> Vec<db::StaleAgentWorkRow> {
    control
        .stale_agent_work(STALE_MINUTES)
        .await
        .expect("the stale window is answerable")
}

/// Insert one proof story at a board status that still expects a run. A non-`Ready` status keeps
/// the board's own dispatch trigger from firing, so every item in this proof is written by hand and
/// has exactly the state under test.
async fn insert_story(pool: &PgPool, id: &str, status: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'FORGE.CLAIM lost heartbeat proof', 'High', $2, '')",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert the proof story");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-005); the file and the assay use it.
async fn forge_claim_005__lost_heartbeat_eventually_requeues() {
    // 0. A disposable DEV target, and only a DEV target.
    let harness = ForgeHarness::connect_from_env()
        .await
        .expect("a disposable DEV database must be declared (DATABASE_URL_DEV, APP_ENV dev/test)");
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the lost-heartbeat contract is proven on DEV only; PROD is forbidden"
    );

    let pool = harness.pool();
    let engine = harness.engine();
    let control = harness.control();
    let tag = harness.database().namespace().to_owned();

    // Three claims in one window: the lost heartbeat under test, a live peer that must survive the
    // sweep untouched, and an aged claim the owner came back to (heartbeated again after aging).
    let lost_story = format!("FORGE-CLAIM-005-LOST-{tag}");
    let fresh_story = format!("FORGE-CLAIM-005-FRESH-{tag}");
    let rescued_story = format!("FORGE-CLAIM-005-RESCUED-{tag}");

    insert_story(pool, &lost_story, "In Progress").await;
    let lost_item = insert_lost_heartbeat_item(pool, &lost_story, LOST_HEARTBEAT_MINUTES).await;
    insert_story(pool, &fresh_story, "In Progress").await;
    let fresh_item = insert_lost_heartbeat_item(pool, &fresh_story, 0).await;
    insert_story(pool, &rescued_story, "In Progress").await;
    let rescued_item = insert_lost_heartbeat_item(pool, &rescued_story, LOST_HEARTBEAT_MINUTES).await;

    // The live peer's heartbeat is the production heartbeat itself, not a fixture: the owner beats
    // through the real boundary, so the proof exercises the same write it depends on.
    assert!(
        engine
            .heartbeat_agent_work(&fresh_item)
            .await
            .expect("the live peer's heartbeat runs"),
        "{HARNESS}: the live peer's claim is claimable, so its heartbeat must refresh it"
    );
    assert!(
        engine
            .heartbeat_agent_work(&rescued_item)
            .await
            .expect("the rescued claim's heartbeat runs"),
        "{HARNESS}: the owner who came back heartbeats again, so the claim must refresh"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. DISCOVERY — the lost heartbeat is what the sweep admits. Same window, same sweep: the aged
    //    claim is stale, the heartbeated twin is alive, and the aged-but-reheartbeated claim is
    //    alive again. Freshness is the DATABASE's reading of `updated_at`, read back through the
    //    production boundary.
    // -----------------------------------------------------------------------------------------------------------
    let sweep = discovered(control).await;
    let seen: std::collections::BTreeSet<&str> = sweep.iter().map(|row| row.story_id.as_str()).collect();

    assert!(
        seen.contains(lost_story.as_str()),
        "{HARNESS}: a claim whose heartbeat was lost must be discovered as stale"
    );
    assert!(
        !seen.contains(fresh_story.as_str()),
        "{HARNESS}: a claim heartbeated inside the window is ALIVE; the sweep must not see it — \
         requeuing a live peer would start a second engine over a story that is still running"
    );
    assert!(
        !seen.contains(rescued_story.as_str()),
        "{HARNESS}: an aged claim the owner heartbeated again is ALIVE; eventual requeue is bounded \
         by the heartbeat, not by the calendar"
    );

    // The discovery row names why the claim is reapable, and carries the attempt accounting the
    // recovery policy reads.
    let lost_row = sweep
        .iter()
        .find(|row| row.story_id == lost_story)
        .expect("the lost-heartbeat proof row is present");
    assert!(
        lost_row.why.contains("claim untouched") && lost_row.why.contains("no worker heartbeat"),
        "{HARNESS}: the discovery must name the lost heartbeat as the reason, got: {}",
        lost_row.why
    );
    assert_eq!(lost_row.attempts, 1, "{HARNESS}: the attempt count rides the discovery row");
    assert_eq!(lost_row.max_attempts, 3);

    // The rows recovery must leave BYTE-FOR-BYTE alone. A recovery that rewrote every row it was
    // handed — or that requeued a live peer — would still satisfy a state-only assertion, so the
    // no-write half of the contract is measured against the full committed row, captured here.
    let fresh_before = item(pool, &fresh_item).await;
    let rescued_before = item(pool, &rescued_item).await;

    // -----------------------------------------------------------------------------------------------------------
    // 2. RECOVERY — the lost claim goes back to the queue, and the board moves with it. Both rows move
    //    or neither: the item is `Ready` and unowned, the story is `Ready`, and the recovered claim
    //    is claimable again by the next worker — the story's turn was not lost.
    // -----------------------------------------------------------------------------------------------------------
    let before = item(pool, &lost_item).await;
    control
        .requeue_stale_work(&lost_item, &lost_story)
        .await
        .expect("a stale claim the board still expects must be recoverable");

    let recovered = item(pool, &lost_item).await;
    assert_eq!(
        recovered.state, "Ready",
        "{HARNESS}: recovery returns the claim to the queue"
    );
    assert_eq!(
        recovered.claimed_by, None,
        "{HARNESS}: a recovered claim names nobody"
    );
    assert_eq!(recovered.claimed_at, None);
    assert_eq!(
        recovered.started_at, None,
        "{HARNESS}: a recovered row is not a run that started"
    );
    assert_eq!(
        recovered.finished_at, None,
        "{HARNESS}: and it is not finished either"
    );
    assert_eq!(
        recovered.error_text, None,
        "{HARNESS}: a fresh attempt starts without a stale reason attached"
    );
    assert!(
        recovered.updated_at > before.updated_at,
        "{HARNESS}: recovery moves `updated_at`, or the recovered row reads as stale again immediately"
    );
    assert_eq!(
        story_status(pool, &lost_story).await,
        "Ready",
        "{HARNESS}: the board moves with the item, or nothing dispatches the recovered work"
    );

    // The recovered claim is claimable again: the next worker takes it through the production claim
    // door, which is the whole point of "eventually requeues".
    let retaken = engine
        .claim_specific_agent_work(&lost_item, NEXT_OWNER)
        .await
        .expect("the production claim runs")
        .expect("a recovered claim must be claimable by the next worker");
    assert_eq!(
        retaken.state, "Claimed",
        "{HARNESS}: the recovered work is back in the queue, owned by the next worker"
    );
    assert_eq!(
        retaken.claimed_by.as_deref(),
        Some(NEXT_OWNER),
        "{HARNESS}: the retaken claim records its new owner"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — THE LIVE PEERS SURVIVE, BYTE-FOR-BYTE. The sweep never saw them, recovery was
    //    never handed them, and their durable rows are exactly what they were — `updated_at` included.
    //    A boundary that requeued every row it was handed would pass the state assertion above and
    //    fail here.
    // -----------------------------------------------------------------------------------------------------------
    let fresh_after = item(pool, &fresh_item).await;
    assert_eq!(
        fresh_after.state, "Running",
        "{HARNESS}: a claim heartbeated inside the window must NOT be recovered — otherwise the next \
         tick starts a second engine over a story that is still running"
    );
    assert_eq!(
        fresh_after.claimed_by.as_deref(),
        Some(OWNER),
        "{HARNESS}: the live peer's ownership survives the sweep"
    );
    assert_eq!(
        fresh_after.updated_at, fresh_before.updated_at,
        "{HARNESS}: the sweep must not touch a live peer at all — not even its `updated_at`"
    );
    assert_eq!(
        story_status(pool, &fresh_story).await,
        "In Progress",
        "{HARNESS}: the live peer's board is untouched"
    );
    let rescued_after = item(pool, &rescued_item).await;
    assert_eq!(
        rescued_after.updated_at, rescued_before.updated_at,
        "{HARNESS}: a claim heartbeated again after aging is alive; recovery must not touch it"
    );
    assert_eq!(
        story_status(pool, &rescued_story).await,
        "In Progress",
        "{HARNESS}: the rescued claim's board is untouched"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. COMMITTED TRUTH SURVIVES A ROLLBACK. Recovery committed on the pool; a later transaction that
    //    rewrites the recovered row sees its own uncommitted write and, rolled back, restores exactly
    //    what recovery committed. The contract reads committed rows, so durable truth — not a
    //    connection-local snapshot — is what the assertions rest on.
    // -----------------------------------------------------------------------------------------------------------
    let probe_item = lost_item.clone();
    let uncommitted = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                sqlx::query("update agent_work_item set state = 'Running' where id = $1::uuid")
                    .bind(&probe_item)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| {
                        db::DbFailure::from_sqlx("test-harness.forge_claim_005.probe_update", &error)
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_item)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            db::DbFailure::from_sqlx("test-harness.forge_claim_005.probe_read", &error)
                        })?;
                Ok(state)
            })
        })
        .await
        .expect("the rolled-back probe must run");
    assert_eq!(
        uncommitted, "Running",
        "{HARNESS}: inside its own transaction the probe sees the write it made"
    );
    let committed_after_rollback = item(pool, &lost_item).await;
    assert_eq!(
        committed_after_rollback.state, "Claimed",
        "{HARNESS}: rollback restored the committed truth recovery and the retake wrote; the \
         contract rests on the commit, not on a connection-local snapshot"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP. This run's proof stories are deleted; the items cascade with them, so DEV is left
    //    as it was found. A non-zero count is a failed rollback and fails the proof.
    // -----------------------------------------------------------------------------------------------------------
    let proof_stories = [lost_story.clone(), fresh_story.clone(), rescued_story.clone()];
    for story in &proof_stories {
        sqlx::query("delete from storyboard_story where id = $1")
            .bind(*story)
            .execute(pool)
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

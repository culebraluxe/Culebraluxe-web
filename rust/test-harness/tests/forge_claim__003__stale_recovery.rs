//! FORGE.CLAIM — stale recovery (TST-FORGE-CLAIM-003).
//!
//! Contract: a claim whose owner stopped heartbeating is **recovered** from the durable queue, and recovery never
//! touches a claim that is still alive. The production boundary is the Forge control-plane DAO the worker pass calls
//! through (`recover_stale_agent_work`, `rust/forge/src/engine/worker.rs:141-189`, is a thin policy over exactly these
//! three methods):
//!
//!   * `ForgeControlDao::stale_agent_work` — DISCOVERY. A claim is stale when `state in ('Claimed','Running',
//!     'Paused')` **and** `updated_at` is older than the caller's window
//!     (`rust/core/db/src/forge_control.rs:39-54`). Freshness is decided by the DATABASE (`updated_at` versus
//!     `now() - interval`), so this contract pins nothing on a Rust clock.
//!   * `ForgeControlDao::requeue_stale_work` — RECOVERY, board-driven: a story the board still expects goes back to
//!     the queue (`Ready`/`Ready`), landed work settles `Done` without being rerun, and a human-held story settles
//!     `Error` without being reopened (`rust/core/db/src/forge_control.rs:117-202`).
//!   * `ForgeControlDao::hold_stale_work` — RECOVERY, terminal: a claim that must not be retried (an assay role, or
//!     one out of attempts) is terminalized and the board is moved to `Hold` in the same transaction
//!     (`rust/core/db/src/forge_control.rs:78-108`).
//!
//! WHY A REAL DATABASE. The subject *is* a SQL predicate and a committed write: which rows a window admits, and the
//! pair (item, story) a recovery leaves behind. An in-memory fake would assert a re-statement of the predicate, not
//! the predicate. So the test runs at the production DAO boundary against a **disposable DEV database**, and the
//! harness refuses PRODUCTION before a socket is opened (`test_harness::TestDatabase::guard_target`). The taxonomy
//! harness is ForgeHarness; the rows here are the production control-plane rows a Forge worker recovers.
//! The recovery's own transaction commits, and the assertions read the committed rows back across a fresh checkout —
//! that committed truth is the contract. A rollback probe confirms the truth is durable rather than a
//! connection-local snapshot: an uncommitted rewrite is visible inside its transaction and gone after rollback
//! (`TestDatabase::with_rollback`).
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path rust/Cargo.toml -p test-harness \
//!       --test forge_claim__003__stale_recovery -- --ignored
//!
//! THE NEGATIVE CASES ARE LOAD-BEARING, not garnish. A live peer (fresh `updated_at`) must survive the sweep; an
//! already-settled claim must not be recovered twice; and a stale claim over landed or human-held work must NOT be
//! requeued into a rerun. Remove any of those and the test still passes on the easy path — which is exactly the
//! vacuous pass this file refuses. Level: L2 Persistence, harness ForgeHarness.

use db::{DbFailure, ForgeControlDao};
use sqlx::{FromRow, PgPool};
use test_harness::TestDatabase;

/// The stale window under test. A claim older than this is recoverable; one inside it is not.
const STALE_MINUTES: i64 = 60;
/// How far in the past a deliberately-stale claim's `updated_at` is set. `now()` does the arithmetic in the database.
const STALE_AGE_MINUTES: i64 = STALE_MINUTES * 2;

/// The committed shape of an `agent_work_item`, read back after a recovery.
#[derive(Debug, FromRow)]
struct ItemRow {
    state: String,
    claimed_by: Option<String>,
    claimed_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    error_text: Option<String>,
    updated_at: String,
}

async fn insert_story(pool: &PgPool, id: &str, status: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'FORGE.CLAIM stale recovery proof', 'High', $2, '')",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert the proof story");
}

/// Insert one work-item fixture and return its id.
///
/// `age_minutes` is applied in SQL (`now() - ($6::text || ' minutes')::interval`), so no Rust wall clock and no
/// timezone enters the staleness under test — the window is measured against the database's own `now()`.
async fn insert_item(
    pool: &PgPool,
    story_id: &str,
    state: &str,
    role: &str,
    attempts: i32,
    age_minutes: i64,
) -> String {
    sqlx::query_scalar(
        "insert into agent_work_item
             (story_id, state, priority, role, attempts, max_attempts,
              claimed_by, claimed_at, started_at, updated_at)
         values ($1, $2, 0, $3, $4, 3, $5, now(), now(),
                 now() - ($6::text || ' minutes')::interval)
         returning id::text",
    )
    .bind(story_id)
    .bind(state)
    .bind(role)
    .bind(attempts)
    .bind(format!("proof-worker-{role}"))
    .bind(age_minutes.to_string())
    .fetch_one(pool)
    .await
    .expect("insert the proof work item")
}

async fn item(pool: &PgPool, id: &str) -> ItemRow {
    sqlx::query_as::<_, ItemRow>(
        "select state, claimed_by, claimed_at::text, started_at::text, finished_at::text,
                error_text, updated_at::text
           from agent_work_item
          where id = $1::uuid",
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_003__stale_recovery() {
    const HARNESS: &str = "ForgeHarness/L2 Persistence";

    let harness = TestDatabase::connect_declared(None, Some("test")).await.expect(
        "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
    );
    assert_eq!(
        harness.target().as_str(),
        "dev",
        "{HARNESS}: a stale-recovery contract must never run against PROD"
    );

    let pool = harness.database().pool();
    let control = ForgeControlDao::new(harness.database().clone());
    let tag = harness.namespace().to_owned();

    // Six proof stories, each isolating one clause of the contract. Inserting at a non-`Ready` status avoids the
    // board's Ready-dispatch trigger, so every item below is written by hand and has exactly the state under test.
    let stale_story = format!("FORGE-CLAIM-003-STALE-{tag}");
    let fresh_story = format!("FORGE-CLAIM-003-FRESH-{tag}");
    let landed_story = format!("FORGE-CLAIM-003-LANDED-{tag}");
    let held_story = format!("FORGE-CLAIM-003-HELD-{tag}");
    let settled_story = format!("FORGE-CLAIM-003-SETTLED-{tag}");
    let hold_recovery_story = format!("FORGE-CLAIM-003-HOLDREC-{tag}");

    // A live-but-silent claim the board still expects: the primary recovery target.
    insert_story(pool, &stale_story, "In Progress").await;
    let stale_item = insert_item(
        pool,
        &stale_story,
        "Running",
        "builder",
        0,
        STALE_AGE_MINUTES,
    )
    .await;
    // A claim heartbeated just now: a live peer.
    insert_story(pool, &fresh_story, "In Progress").await;
    let fresh_item = insert_item(pool, &fresh_story, "Running", "builder", 0, 0).await;
    // A stale claim whose work already landed on the board.
    insert_story(pool, &landed_story, "Complete").await;
    let landed_item = insert_item(
        pool,
        &landed_story,
        "Running",
        "builder",
        0,
        STALE_AGE_MINUTES,
    )
    .await;
    // A stale claim a human has taken ownership of.
    insert_story(pool, &held_story, "Hold").await;
    let held_item = insert_item(
        pool,
        &held_story,
        "Running",
        "builder",
        0,
        STALE_AGE_MINUTES,
    )
    .await;
    // A claim that is already settled — never a recovery candidate, however old.
    insert_story(pool, &settled_story, "Complete").await;
    let settled_item = insert_item(
        pool,
        &settled_story,
        "Done",
        "builder",
        0,
        STALE_AGE_MINUTES,
    )
    .await;
    // A stale claim out of attempts: the hold (terminal) recovery outcome.
    insert_story(pool, &hold_recovery_story, "In Progress").await;
    let hold_recovery_item = insert_item(
        pool,
        &hold_recovery_story,
        "Running",
        "builder",
        3,
        STALE_AGE_MINUTES,
    )
    .await;

    // ---------------------------------------------------------------------------------------------------------
    // DISCOVERY — the window and the state filter are the SQL predicate, read back from the database.
    // ---------------------------------------------------------------------------------------------------------
    let discovered = control
        .stale_agent_work(STALE_MINUTES)
        .await
        .expect("the stale window is answerable");
    let stories: std::collections::BTreeSet<&str> =
        discovered.iter().map(|row| row.story_id.as_str()).collect();

    assert!(
        stories.contains(stale_story.as_str()),
        "{HARNESS}: a claim silently older than the window must be discovered as stale"
    );
    assert!(
        !stories.contains(fresh_story.as_str()),
        "{HARNESS}: a claim heartbeated inside the window is ALIVE; the sweep must not see it"
    );
    assert!(
        !stories.contains(settled_story.as_str()),
        "{HARNESS}: a settled (`Done`) claim is not a recovery candidate, however old"
    );
    assert!(
        stories.contains(landed_story.as_str()) && stories.contains(held_story.as_str()),
        "{HARNESS}: staleness is about the claim's liveness, not the board's; landed and held claims are discovered \
         and it is recovery (below) that decides their outcome"
    );

    let stale_row = discovered
        .iter()
        .find(|row| row.story_id == stale_story)
        .expect("the stale proof row is present");
    assert_eq!(stale_row.role.as_deref(), Some("builder"));
    assert_eq!(
        stale_row.attempts, 0,
        "{HARNESS}: the attempt count rides the discovery row"
    );
    assert_eq!(stale_row.max_attempts, 3);

    // ---------------------------------------------------------------------------------------------------------
    // POSITIVE — recovery of a stale claim the board still expects: item back to the queue, story with it.
    // ---------------------------------------------------------------------------------------------------------
    let before = item(pool, &stale_item).await;
    control
        .requeue_stale_work(&stale_item, &stale_story)
        .await
        .expect("a stale claim the board still expects must be recoverable");

    let recovered = item(pool, &stale_item).await;
    assert_eq!(
        recovered.state, "Ready",
        "{HARNESS}: recovery returns the claim to the queue"
    );
    assert_eq!(
        recovered.claimed_by, None,
        "{HARNESS}: a recovered row names nobody"
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
        story_status(pool, &stale_story).await,
        "Ready",
        "{HARNESS}: the board moves with the item, or nothing dispatches the recovered work"
    );

    // ---------------------------------------------------------------------------------------------------------
    // COMMITTED TRUTH SURVIVES A ROLLBACK. Recovery committed on the pool; a later transaction that rewrites the
    // recovered row sees its own uncommitted write and, rolled back, restores exactly what recovery committed. The
    // contract reads committed rows, so durable truth — not a connection-local snapshot — is what the assertions
    // rest on. `with_rollback` can only roll back, so this probe changes no canonical row.
    // ---------------------------------------------------------------------------------------------------------
    let probe_id = stale_item.clone();
    let uncommitted = harness
        .with_rollback(move |conn| {
            Box::pin(async move {
                sqlx::query("update agent_work_item set state = 'Running' where id = $1::uuid")
                    .bind(&probe_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("test-harness.forge_claim_003.probe_update", &error)
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_id)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("test-harness.forge_claim_003.probe_read", &error)
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
    let committed_after_rollback = item(pool, &stale_item).await;
    assert_eq!(
        committed_after_rollback.state, "Ready",
        "{HARNESS}: rollback restored the committed truth recovery wrote; the contract rests on the commit, not on a \
         connection-local snapshot"
    );

    // ---------------------------------------------------------------------------------------------------------
    // NEGATIVE — the live peer survives untouched. This is the whole reason recovery keys on a heartbeat.
    // ---------------------------------------------------------------------------------------------------------
    let survivor = item(pool, &fresh_item).await;
    assert_eq!(
        survivor.state, "Running",
        "{HARNESS}: a claim heartbeated inside the window must NOT be recovered — otherwise the next tick starts a \
         second engine over a story that is still running"
    );
    assert_eq!(survivor.claimed_by.as_deref(), Some("proof-worker-builder"));
    assert_eq!(story_status(pool, &fresh_story).await, "In Progress");

    // ---------------------------------------------------------------------------------------------------------
    // NEGATIVE — landed work is not rerun; a held story is not reopened. Both are board-driven refusals inside the
    // production recovery write, and both are the mirror image of dispatching unowned work.
    // ---------------------------------------------------------------------------------------------------------
    control
        .requeue_stale_work(&landed_item, &landed_story)
        .await
        .expect("recovery must settle a stale claim over landed work, not error");
    let landed = item(pool, &landed_item).await;
    assert_eq!(
        landed.state, "Done",
        "{HARNESS}: a stale claim over work the board says is Complete must settle Done, never Ready (a rerun)"
    );
    assert_eq!(story_status(pool, &landed_story).await, "Complete");

    control
        .requeue_stale_work(&held_item, &held_story)
        .await
        .expect("recovery must settle a stale claim over held work, not error");
    let held = item(pool, &held_item).await;
    assert_eq!(
        held.state, "Error",
        "{HARNESS}: a stale claim over a story a human holds must settle Error, never Ready"
    );
    assert_eq!(
        held.error_text.as_deref(),
        Some("stale claim on a story a human holds")
    );
    assert_eq!(
        story_status(pool, &held_story).await,
        "Hold",
        "{HARNESS}: the human gate is not reopened by a bookkeeping sweep"
    );

    // NEGATIVE (rollback / no-op): a settled claim is not a recovery candidate, so the recovery write must change
    // nothing rather than resurrect it. The guard is the `state in (...)` predicate of the recovery's own select.
    control
        .requeue_stale_work(&settled_item, &settled_story)
        .await
        .expect("recovering an already-settled claim is a no-op, not an error");
    let settled = item(pool, &settled_item).await;
    assert_eq!(
        settled.state, "Done",
        "{HARNESS}: a terminal claim must not be recovered twice"
    );
    assert_eq!(story_status(pool, &settled_story).await, "Complete");

    // ---------------------------------------------------------------------------------------------------------
    // POSITIVE (hold outcome) — the other recovery write: a claim that must not be retried is terminalized and the
    // board moves with it, atomically.
    // ---------------------------------------------------------------------------------------------------------
    let reason = "stale worker: no heartbeat; process/host presumed terminated";
    control
        .hold_stale_work(&hold_recovery_item, &hold_recovery_story, reason)
        .await
        .expect("a claim that must not be retried is held, not requeued");
    let terminal = item(pool, &hold_recovery_item).await;
    assert_eq!(
        terminal.state, "Error",
        "{HARNESS}: a held recovery terminalizes the claim"
    );
    assert_eq!(terminal.error_text.as_deref(), Some(reason));
    assert!(
        terminal.finished_at.is_some(),
        "{HARNESS}: a terminal claim records when it ended"
    );
    assert_eq!(
        story_status(pool, &hold_recovery_story).await,
        "Hold",
        "{HARNESS}: the board moves with the held claim, in the same write"
    );

    // ---------------------------------------------------------------------------------------------------------
    // CLOSED LOOP — after recovery, none of the proof stories is stale any more. Recovery removed them from the
    // sweep's own input, not merely from the test's expectations.
    // ---------------------------------------------------------------------------------------------------------
    let after = control
        .stale_agent_work(STALE_MINUTES)
        .await
        .expect("the stale window is answerable");
    let proof_stories = [
        stale_story.as_str(),
        fresh_story.as_str(),
        landed_story.as_str(),
        held_story.as_str(),
        settled_story.as_str(),
        hold_recovery_story.as_str(),
    ];
    for row in &after {
        assert!(
            !proof_stories.contains(&row.story_id.as_str()),
            "{HARNESS}: {} is still stale after recovery — the sweep and the recovery disagree",
            row.story_id
        );
    }

    // DEV is left as it was found: the proof stories and everything that cascades with them are removed.
    for story in &proof_stories {
        sqlx::query("delete from storyboard_story where id = $1")
            .bind(*story)
            .execute(pool)
            .await
            .expect("the harness owns the rows it made and puts them back");
    }
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id = any($1::text[])")
            .bind(
                proof_stories
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            )
            .fetch_one(pool)
            .await
            .expect("the cleanup is readable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
}

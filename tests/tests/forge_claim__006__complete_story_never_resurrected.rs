//! FORGE.CLAIM — Complete story never resurrected (TST-FORGE-CLAIM-006).
//!
//! Contract: a story whose work **landed** (`status = 'Complete'`) is the truth, and no
//! bookkeeping write in the claim lifecycle puts it back to work. The board rule lives in
//! one place — `forge_settlement_pair` (migration 263, `db/migrations/263_forge_agent_work_settlement.sql:30-63`):
//! `v_hold_the_board` is `Hold` only over a board that still expects a run (`Ready` /
//! `In Progress`); over `Complete` every outcome leaves the board alone (a `Done` is
//! accepted, everything else settles the item without touching the story). The recovery
//! door agrees (`forge_requeue_stale_work`, migration 266: a `Complete` board settles the
//! stale claim `Done` — bookkeeping, never a rerun), and the claim doors only take `Ready`
//! items (`forge_claim_specific_agent_work` / `forge_claim_story`, migrations 262/275),
//! which a settled claim is not.
//!
//! Why a real database: the rule is a SQL predicate over committed rows, and the
//! resurrection it fences is exactly what a crashed run leaves behind — a stranded
//! `Running` item over a board that already landed. The production DAOs are driven through
//! the `ForgeHarness` against an isolated, disposable DEV branch; `TestDatabase` refuses
//! PRODUCTION before any socket is opened.
//!
//! The proof covers every door that could put a landed story back to work:
//!
//!   * the settle door (`ForgeEngineDao::finish_agent_work_run`) with **each** outcome —
//!     `Done` (accepted, board stays `Complete`), `Error`, `Cancelled`, `Abandoned`
//!     (item settles, board stays `Complete`);
//!   * the recovery door (`ForgeControlDao::requeue_stale_work`) — settles `Done`, board
//!     stays `Complete`;
//!   * the dispatch door (`ForgeEngineDao::ensure_story_dispatched`) over a landed story
//!     with a stranded claim — answers `AlreadyQueued` and moves nothing;
//!   * the claim doors — a settled item is refused by `claim_specific_agent_work`, and the
//!     poller's door (`claim_next_agent_work`) never claims a `Complete` story's item;
//!   * the board trigger (`agent_work_item_dispatch`, migration 025) fires only on a
//!     **change into** `Ready`, and no door above produces one, so no item is re-queued.
//!
//! The NEGATIVE control is load-bearing: the same recovery door over an `In Progress`
//! story — a board that still expects a run — **does** return the claim to the queue
//! (`Ready`/`Ready`). The refusal over `Complete` is the board's status, not a dead door.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_claim__006__complete_story_never_resurrected -- --ignored

use sqlx::PgPool;
use test_harness::ForgeHarness;

use db::{AgentWorkOutcome, DbTarget, EnsureDispatch, ForgeControlDao};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The stale window under test, for the recovery door's discovery input.
const STALE_MINUTES: i64 = 60;
/// How far into the past a deliberately-lost heartbeat is aged, in SQL.
const LOST_HEARTBEAT_MINUTES: i64 = STALE_MINUTES * 2;

/// Insert one proof story at `status`. A non-`Ready` status keeps the board's own dispatch
/// trigger from firing, so every item in this proof is written by hand and has exactly the
/// state under test (a crashed run's leftover: a stranded `Running` item over a landed board).
async fn insert_story(pool: &PgPool, id: &str, status: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'FORGE.CLAIM complete-proof', 'High', $2, '')",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert the proof story");
}

/// Insert one stranded `Running` claim with the run it opened, and return `(item_id,
/// run_id)`. The run row is what the settle door closes, so its ruling is part of the
/// contract under test.
async fn insert_running_claim(pool: &PgPool, story_id: &str) -> (String, String) {
    let run_id: String = sqlx::query_scalar(
        "insert into storyboard_story_run (story_id, started_at)
         values ($1, now()) returning id::text",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("insert the proof run");
    let item_id: String = sqlx::query_scalar(
        "insert into agent_work_item
             (story_id, state, priority, role, attempts, max_attempts,
              claimed_by, claimed_at, started_at, story_run_id, updated_at)
         values ($1, 'Running', 0, 'builder', 1, 3, 'proof-worker-builder',
                 now(), now(), $2, now())
         returning id::text",
    )
    .bind(story_id)
    .bind(&run_id)
    .fetch_one(pool)
    .await
    .expect("insert the proof work item");
    (item_id, run_id)
}

/// The committed pair a settlement leaves behind: `(item_state, story_status)`.
async fn pair(pool: &PgPool, item_id: &str, story_id: &str) -> (String, String) {
    let item_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(item_id)
            .fetch_one(pool)
            .await
            .expect("the settled item is readable");
    let story_status: String =
        sqlx::query_scalar("select status from storyboard_story where id = $1")
            .bind(story_id)
            .fetch_one(pool)
            .await
            .expect("the board is readable");
    (item_state, story_status)
}

/// The run's committed ruling: `(ended_at is not null, result_status)`.
async fn run_ruling(pool: &PgPool, run_id: &str) -> (bool, Option<String>) {
    sqlx::query_as(
        "select ended_at is not null, result_status from storyboard_story_run where id = $1::uuid",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("the run's ruling is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-006); the file and the assay use it.
async fn forge_claim_006__complete_story_never_resurrected() {
    // 0. A disposable DEV target, and only a DEV target.
    let harness = ForgeHarness::connect_from_env()
        .await
        .expect("a disposable DEV database must be declared (DATABASE_URL_DEV, APP_ENV dev/test)");
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the complete-story contract is proven on DEV only; PROD is forbidden"
    );

    let pool = harness.pool();
    let engine = harness.engine();
    let control = harness.control();
    let tag = harness.database().namespace().to_owned();

    // Five landed stories, one per settle outcome plus the recovery case, and the control:
    // a story the board still expects a run for, whose stale claim the same recovery door
    // DOES return to the queue.
    let done_story = format!("FORGE-CLAIM-006-DONE-{tag}");
    let error_story = format!("FORGE-CLAIM-006-ERROR-{tag}");
    let cancelled_story = format!("FORGE-CLAIM-006-CANCELLED-{tag}");
    let abandoned_story = format!("FORGE-CLAIM-006-ABANDONED-{tag}");
    let requeued_story = format!("FORGE-CLAIM-006-REQUEUED-{tag}");
    let stranded_story = format!("FORGE-CLAIM-006-STRANDED-{tag}");
    let control_story = format!("FORGE-CLAIM-006-CONTROL-{tag}");

    insert_story(pool, &done_story, "Complete").await;
    let (done_item, done_run) = insert_running_claim(pool, &done_story).await;
    insert_story(pool, &error_story, "Complete").await;
    let (error_item, error_run) = insert_running_claim(pool, &error_story).await;
    insert_story(pool, &cancelled_story, "Complete").await;
    let (cancelled_item, cancelled_run) = insert_running_claim(pool, &cancelled_story).await;
    insert_story(pool, &abandoned_story, "Complete").await;
    let (abandoned_item, abandoned_run) = insert_running_claim(pool, &abandoned_story).await;
    insert_story(pool, &requeued_story, "Complete").await;
    let (requeued_item, _requeued_run) = insert_running_claim(pool, &requeued_story).await;
    // A landed story that still holds the claim a crashed run left behind: the
    // dispatch door's input for the `AlreadyQueued` answer.
    insert_story(pool, &stranded_story, "Complete").await;
    let (stranded_item, _stranded_run) = insert_running_claim(pool, &stranded_story).await;
    insert_story(pool, &control_story, "In Progress").await;
    let (control_item, _control_run) = insert_running_claim(pool, &control_story).await;
    // The control's claim is as stale as the recovery case's, so the same door faces the
    // same input and answers differently only because the board differs.
    sqlx::query(
        "update agent_work_item set updated_at = now() - ($1::text || ' minutes')::interval
          where id = $2::uuid",
    )
    .bind(LOST_HEARTBEAT_MINUTES.to_string())
    .bind(&control_item)
    .execute(pool)
    .await
    .expect("age the control's claim");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE SETTLE DOOR, EVERY OUTCOME. `forge_settlement_pair` holds the board rule: over a
    //    landed board a `Done` is accepted and every other outcome settles the item WITHOUT
    //    moving the story. The run each claim opened closes with its own ruling, once.
    // -----------------------------------------------------------------------------------------------------------
    let settled = engine
        .finish_agent_work_run(&done_item, AgentWorkOutcome::Done, None)
        .await
        .expect("the settle door runs")
        .expect("the owner's Running row must settle");
    assert_eq!(
        settled.item_state, "Done",
        "{HARNESS}: a Done over a landed board is accepted"
    );
    assert_eq!(
        settled.story_status, None,
        "{HARNESS}: a landed board is the truth — the settle door must not move it"
    );
    assert_eq!(
        pair(pool, &done_item, &done_story).await,
        ("Done".to_string(), "Complete".to_string()),
        "{HARNESS}: the item settled Done and the board stayed Complete"
    );
    assert_eq!(
        run_ruling(pool, &done_run).await,
        (true, Some("Complete".to_string())),
        "{HARNESS}: the run the Done claim opened closes ruled Complete"
    );

    for (story, item, run, outcome) in [
        (&error_story, &error_item, &error_run, AgentWorkOutcome::Error),
        (&cancelled_story, &cancelled_item, &cancelled_run, AgentWorkOutcome::Cancelled),
        (&abandoned_story, &abandoned_item, &abandoned_run, AgentWorkOutcome::Abandoned),
    ] {
        let settled = engine
            .finish_agent_work_run(item, outcome, Some("proof settle"))
            .await
            .expect("the settle door runs")
            .expect("the owner's Running row must settle");
        let expected_item = match outcome {
            AgentWorkOutcome::Error => "Error",
            _ => "Cancelled",
        };
        assert_eq!(
            settled.item_state, expected_item,
            "{HARNESS}: a {outcome} settle over a landed board settles the item {expected_item}"
        );
        assert_eq!(
            settled.story_status, None,
            "{HARNESS}: no outcome may move a landed board — the settle door returned a board move"
        );
        assert_eq!(
            pair(pool, item, story).await,
            (expected_item.to_string(), "Complete".to_string()),
            "{HARNESS}: the item settled and the board stayed Complete"
        );
        assert_eq!(
            run_ruling(pool, run).await,
            (true, Some(if outcome == AgentWorkOutcome::Error {
                "Failed".to_string()
            } else {
                "Cancelled".to_string()
            })),
            "{HARNESS}: the run closed with its own ruling"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RECOVERY DOOR. A stale claim over landed work is bookkeeping: the claim settles
    //    `Done` and the board stays `Complete` — never `Ready`, which would be a rerun.
    // -----------------------------------------------------------------------------------------------------------
    control
        .requeue_stale_work(&requeued_item, &requeued_story)
        .await
        .expect("recovery must settle a stale claim over landed work, not error");
    assert_eq!(
        pair(pool, &requeued_item, &requeued_story).await,
        ("Done".to_string(), "Complete".to_string()),
        "{HARNESS}: recovery over a landed board settles Done and leaves the board Complete"
    );

    // The dispatch door over a landed story that still holds the claim a crashed run
    // left behind answers `AlreadyQueued` — it names the open item and moves
    // nothing. (A landed story with no open item is the operator's own ENGINE RUN
    // Q re-run move, a deliberate human action outside this contract; the claim
    // lifecycle never produces that shape.)
    let dispatched = engine
        .ensure_story_dispatched(&stranded_story)
        .await
        .expect("the dispatch door runs");
    match dispatched {
        EnsureDispatch::AlreadyQueued { item } => assert_eq!(
            item, stranded_item,
            "{HARNESS}: the dispatch door must name the stranded claim, not re-queue the story"
        ),
        other => panic!(
            "{HARNESS}: the dispatch door over a landed story with a stranded claim must answer \
             AlreadyQueued, answered {other:?}"
        ),
    }
    assert_eq!(
        pair(pool, &stranded_item, &stranded_story).await,
        ("Running".to_string(), "Complete".to_string()),
        "{HARNESS}: the dispatch door moved nothing on a landed board"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE CLAIM DOORS REFUSE THE SETTLED. Every item above is terminal, so the claim door
    //    returns `None` — and the poller's door, which additionally requires the story to be
    //    `Ready` on the board, never claims a `Complete` story's item.
    // -----------------------------------------------------------------------------------------------------------
    for (story, item) in [
        (&done_story, &done_item),
        (&error_story, &error_item),
        (&cancelled_story, &cancelled_item),
        (&abandoned_story, &abandoned_item),
        (&requeued_story, &requeued_item),
    ] {
        let refused = engine
            .claim_specific_agent_work(item, "forge-claim-006:late")
            .await
            .expect("the claim door runs");
        assert!(
            refused.is_none(),
            "{HARNESS}: a settled claim over {story} must not be claimable — a landed story \
             cannot be resurrected by a claim"
        );
    }
    // The poller's door, over the whole shared board: whatever it claims (if anything), it is
    // never one of this proof's landed stories.
    if let Some(claimed) = engine
        .claim_next_agent_work("forge-claim-006:poller")
        .await
        .expect("the poller's claim door runs")
    {
        for story in [
            &done_story,
            &error_story,
            &cancelled_story,
            &abandoned_story,
            &requeued_story,
        ] {
            assert_ne!(
                claimed.story_id, *story,
                "{HARNESS}: the poller's door claimed a landed story's work — a Complete story \
                 was resurrected"
            );
        }
    }
    for (story, item) in [
        (&done_story, &done_item),
        (&error_story, &error_item),
        (&cancelled_story, &cancelled_item),
        (&abandoned_story, &abandoned_item),
        (&requeued_story, &requeued_item),
    ] {
        let (item_state, story_status) = pair(pool, item, story).await;
        assert_eq!(
            story_status, "Complete",
            "{HARNESS}: {story} must still be Complete after the claim doors ran"
        );
        assert_ne!(
            item_state, "Ready",
            "{HARNESS}: {story}'s claim must never return to the queue — that is a resurrection"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE CONTROL — THE SAME DOOR OPENS FOR A BOARD THAT EXPECTS A RUN. The control
    //    story is `In Progress` with a stale claim: the same recovery door that settled the
    //    landed claim returns THIS one to the queue. The refusal above is the board's status,
    //    not a dead door — without this case the proof would pass on a recovery that requeued
    //    nothing at all.
    // -----------------------------------------------------------------------------------------------------------
    control
        .requeue_stale_work(&control_item, &control_story)
        .await
        .expect("a stale claim the board still expects must be recoverable");
    assert_eq!(
        pair(pool, &control_item, &control_story).await,
        ("Ready".to_string(), "Ready".to_string()),
        "{HARNESS}: the control's stale claim must be requeued — the recovery door works, and \
         only the landed board is refused"
    );
    // The recovered control claim is claimable again: the door the landed stories' settled
    // items were refused by opens for work the board still expects.
    let retaken = engine
        .claim_specific_agent_work(&control_item, "forge-claim-006:next")
        .await
        .expect("the claim door runs")
        .expect("a recovered claim the board still expects must be claimable");
    assert_eq!(
        retaken.state, "Claimed",
        "{HARNESS}: the control's recovered work is back in the queue"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. COMMITTED TRUTH SURVIVES A ROLLBACK. The recovery's commit is durable, not a snapshot
    //    the DAO's connection happened to hold: a transaction that rewrites the control item
    //    sees its own uncommitted write, and rolling back restores exactly what recovery
    //    committed.
    // -----------------------------------------------------------------------------------------------------------
    let probe_item = control_item.clone();
    let uncommitted = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                sqlx::query("update agent_work_item set state = 'Running' where id = $1::uuid")
                    .bind(&probe_item)
                    .execute(&mut *conn)
                    .await
                    .map_err(|error| {
                        db::DbFailure::from_sqlx("test-harness.forge_claim_006.probe_update", &error)
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_item)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            db::DbFailure::from_sqlx("test-harness.forge_claim_006.probe_read", &error)
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
    let committed_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&control_item)
            .fetch_one(pool)
            .await
            .expect("the committed item is readable after the rollback");
    assert_eq!(
        committed_state, "Claimed",
        "{HARNESS}: rollback restored the committed truth recovery and the retake wrote"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP. This run's proof stories are deleted; the items and runs cascade with them,
    //    so DEV is left as it was found.
    // -----------------------------------------------------------------------------------------------------------
    let proof_stories = vec![
        done_story.clone(),
        error_story.clone(),
        cancelled_story.clone(),
        abandoned_story.clone(),
        requeued_story.clone(),
        stranded_story.clone(),
        control_story.clone(),
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
    .bind(proof_stories)
    .fetch_one(pool)
    .await
    .expect("the cleanup is measurable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no story, item or run behind"
    );
}

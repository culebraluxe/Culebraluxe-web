//! FORGE.CLAIM — Hold story never reopened (TST-FORGE-CLAIM-007).
//!
//! Contract: a story a human holds (`status = 'Hold'`) is a human gate, and no
//! bookkeeping write in the claim lifecycle reopens it. The board rule lives in
//! `forge_settlement_pair` (migration 263,
//! `db/migrations/263_forge_agent_work_settlement.sql:30-63`): a `Done` is
//! accepted only when the board confirms it — `Complete` **or `Hold`** ("a human
//! gate ended the turn") — and every other outcome settles the item with
//! `v_hold_the_board = NULL`, because a board that is already `Hold` is never
//! moved. The recovery doors agree (`forge_requeue_stale_work` /
//! `forge_hold_stale_work`, migration 266, `db/migrations/266_forge_stale_recovery.sql`):
//! a stale claim over a held story settles `Error` with
//! `'stale claim on a story a human holds'`, and the board is not reopened.
//!
//! Why a real database: the rule is a SQL predicate over committed rows, and the
//! shape it fences is a crashed run's leftover — a stranded `Running` claim over a
//! story a human is holding. The production DAOs are driven through the
//! `ForgeHarness` against an isolated, disposable DEV branch; `TestDatabase`
//! refuses PRODUCTION before any socket is opened.
//!
//! The proof covers every door that could reopen a held story:
//!
//!   * the settle door (`ForgeEngineDao::finish_agent_work_run`) with **each**
//!     outcome — `Done` (accepted, board stays `Hold`), `Error`, `Cancelled`,
//!     `Abandoned` (item settles, board stays `Hold`);
//!   * the recovery doors — `requeue_stale_work` settles the claim `Error`
//!     ("a story a human holds") and `hold_stale_work` holds it; the board is
//!     `Hold` after both;
//!   * the dispatch door (`ForgeEngineDao::ensure_story_dispatched`) over a held
//!     story with a stranded claim — answers `AlreadyQueued` and moves nothing;
//!   * the claim doors — a settled claim is refused by `claim_specific_agent_work`,
//!     and the poller's door (`claim_next_agent_work`, which additionally requires
//!     the story to be `Ready` on the board) never claims a `Hold` story's item;
//!   * the board trigger (`agent_work_item_dispatch`, migration 025) fires only on
//!     a **change into** `Ready`, and no door above produces one.
//!
//! The NEGATIVE control is load-bearing: the same recovery door over an `In
//! Progress` story — a board that still expects a run — **does** return the claim
//! to the queue (`Ready`/`Ready`). The refusal over `Hold` is the human gate, not
//! a dead door.
//!
//! Level: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_claim__007__hold_story_never_reopened -- --ignored

use sqlx::PgPool;
use test_harness::ForgeHarness;

use db::{AgentWorkOutcome, DbTarget, EnsureDispatch};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The stale window under test, for the recovery door's discovery input.
const STALE_MINUTES: i64 = 60;
/// How far into the past a deliberately-lost heartbeat is aged, in SQL.
const LOST_HEARTBEAT_MINUTES: i64 = STALE_MINUTES * 2;
/// The reason the recovery door writes for a held story's stale claim.
const HELD_REASON: &str = "stale claim on a story a human holds";

/// Insert one proof story at `status`. A non-`Ready` status keeps the board's own
/// dispatch trigger from firing, so every item in this proof is written by hand and
/// has exactly the state under test (a crashed run's leftover: a stranded `Running`
/// claim over a held board).
async fn insert_story(pool: &PgPool, id: &str, status: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'FORGE.CLAIM hold-proof', 'High', $2, '')",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert the proof story");
}

/// Insert one stranded `Running` claim with the run it opened, and return
/// `(item_id, run_id)`.
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

/// The settled item's reason and finish: `(error_text, finished_at is not null)`.
async fn settled_why(pool: &PgPool, item_id: &str) -> (Option<String>, bool) {
    sqlx::query_as(
        "select error_text, finished_at is not null from agent_work_item where id = $1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the settled claim's reason is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-007); the file and the assay use it.
async fn forge_claim_007__hold_story_never_reopened() {
    // 0. A disposable DEV target, and only a DEV target.
    let harness = ForgeHarness::connect_from_env()
        .await
        .expect("a disposable DEV database must be declared (DATABASE_URL_DEV, APP_ENV dev/test)");
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the hold-story contract is proven on DEV only; PROD is forbidden"
    );

    let pool = harness.pool();
    let engine = harness.engine();
    let control = harness.control();
    let tag = harness.database().namespace().to_owned();

    // Six held stories: one per settle outcome, the two recovery doors, and the
    // stranded-claim shape the dispatch door answers — plus the control: a story
    // the board still expects a run for, whose stale claim the same recovery door
    // DOES return to the queue.
    let done_story = format!("FORGE-CLAIM-007-DONE-{tag}");
    let error_story = format!("FORGE-CLAIM-007-ERROR-{tag}");
    let cancelled_story = format!("FORGE-CLAIM-007-CANCELLED-{tag}");
    let abandoned_story = format!("FORGE-CLAIM-007-ABANDONED-{tag}");
    let requeued_story = format!("FORGE-CLAIM-007-REQUEUED-{tag}");
    let held_story = format!("FORGE-CLAIM-007-HELD-{tag}");
    let stranded_story = format!("FORGE-CLAIM-007-STRANDED-{tag}");
    let control_story = format!("FORGE-CLAIM-007-CONTROL-{tag}");

    let mut fixtures = Vec::new();
    for story in [
        &done_story,
        &error_story,
        &cancelled_story,
        &abandoned_story,
        &requeued_story,
        &held_story,
        &stranded_story,
    ] {
        insert_story(pool, story, "Hold").await;
        fixtures.push(insert_running_claim(pool, story).await);
    }
    insert_story(pool, &control_story, "In Progress").await;
    let (control_item, _control_run) = insert_running_claim(pool, &control_story).await;
    // The control's claim is as stale as the recovery cases', so the same door
    // faces the same input and answers differently only because the board differs.
    sqlx::query(
        "update agent_work_item set updated_at = now() - ($1::text || ' minutes')::interval
          where id = $2::uuid",
    )
    .bind(LOST_HEARTBEAT_MINUTES.to_string())
    .bind(&control_item)
    .execute(pool)
    .await
    .expect("age the control's claim");

    let (done_item, done_run) = fixtures[0].clone();
    let (error_item, error_run) = fixtures[1].clone();
    let (cancelled_item, cancelled_run) = fixtures[2].clone();
    let (abandoned_item, abandoned_run) = fixtures[3].clone();
    let (requeued_item, _requeued_run) = fixtures[4].clone();
    let (held_item, _held_run) = fixtures[5].clone();
    let (stranded_item, _stranded_run) = fixtures[6].clone();

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE SETTLE DOOR, EVERY OUTCOME. `forge_settlement_pair` holds the board rule:
    //    over a held board a `Done` is accepted (the human gate ended the turn) and
    //    every other outcome settles the item WITHOUT moving the story — `v_hold_the_board`
    //    is `NULL` over `Hold`, because a held board is never moved, only confirmed.
    // -----------------------------------------------------------------------------------------------------------
    let settled = engine
        .finish_agent_work_run(&done_item, AgentWorkOutcome::Done, None)
        .await
        .expect("the settle door runs")
        .expect("the owner's Running row must settle");
    assert_eq!(
        settled.item_state, "Done",
        "{HARNESS}: a Done over a held board is accepted — the human gate ended the turn"
    );
    assert_eq!(
        settled.story_status, None,
        "{HARNESS}: a held board is never moved by a settle — not even to confirm it"
    );
    assert_eq!(
        pair(pool, &done_item, &done_story).await,
        ("Done".to_string(), "Hold".to_string()),
        "{HARNESS}: the item settled Done and the board stayed Hold"
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>(
            "select result_status from storyboard_story_run where id = $1::uuid"
        )
        .bind(&done_run)
        .fetch_one(pool)
        .await
        .expect("the run's ruling is readable"),
        Some("Complete".to_string()),
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
            "{HARNESS}: a {outcome} settle over a held board settles the item {expected_item}"
        );
        assert_eq!(
            settled.story_status, None,
            "{HARNESS}: no outcome may reopen a held board — the settle door returned a board move"
        );
        assert_eq!(
            pair(pool, item, story).await,
            (expected_item.to_string(), "Hold".to_string()),
            "{HARNESS}: the item settled and the board stayed Hold"
        );
        assert_eq!(
            sqlx::query_scalar::<_, Option<String>>(
                "select result_status from storyboard_story_run where id = $1::uuid"
            )
            .bind(run)
            .fetch_one(pool)
            .await
            .expect("the run's ruling is readable"),
            Some(if outcome == AgentWorkOutcome::Error {
                "Failed".to_string()
            } else {
                "Cancelled".to_string()
            }),
            "{HARNESS}: the run closed with its own ruling"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE RECOVERY DOORS. A stale claim over a held story is not retried and the
    //    gate is not reopened: `requeue_stale_work` settles the claim `Error` naming
    //    the human gate, and `hold_stale_work` terminalizes with the board held.
    // -----------------------------------------------------------------------------------------------------------
    control
        .requeue_stale_work(&requeued_item, &requeued_story)
        .await
        .expect("recovery must settle a stale claim over a held story, not error");
    assert_eq!(
        pair(pool, &requeued_item, &requeued_story).await,
        ("Error".to_string(), "Hold".to_string()),
        "{HARNESS}: recovery over a held board settles the claim Error and leaves the board Hold"
    );
    let (why, finished) = settled_why(pool, &requeued_item).await;
    assert_eq!(
        why.as_deref(),
        Some(HELD_REASON),
        "{HARNESS}: the settled claim must name the human gate it refused to reopen"
    );
    assert!(
        finished,
        "{HARNESS}: a settled claim records when it ended"
    );

    control
        .hold_stale_work(&held_item, &held_story, "proof: a claim that must not be retried")
        .await
        .expect("a claim that must not be retried is held, not requeued");
    assert_eq!(
        pair(pool, &held_item, &held_story).await,
        ("Error".to_string(), "Hold".to_string()),
        "{HARNESS}: the hold recovery terminalizes the claim with the board held"
    );

    // The dispatch door over a held story that still holds the claim a crashed run
    // left behind answers `AlreadyQueued` — it names the open item and moves
    // nothing. (A held story with no open item is the operator's own ENGINE RUN Q
    // re-run move, a deliberate human action outside this contract; the claim
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
            "{HARNESS}: the dispatch door over a held story with a stranded claim must answer \
             AlreadyQueued, answered {other:?}"
        ),
    }
    assert_eq!(
        pair(pool, &stranded_item, &stranded_story).await,
        ("Running".to_string(), "Hold".to_string()),
        "{HARNESS}: the dispatch door moved nothing on a held board"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE CLAIM DOORS REFUSE THE SETTLED. Every item above is terminal, so the
    //    claim door returns `None` — and the poller's door, which additionally
    //    requires the story to be `Ready` on the board, never claims a `Hold` story's
    //    item. The board trigger fires only on a change INTO `Ready`, which no door
    //    above produces.
    // -----------------------------------------------------------------------------------------------------------
    for (story, item) in [
        (&done_story, &done_item),
        (&error_story, &error_item),
        (&cancelled_story, &cancelled_item),
        (&abandoned_story, &abandoned_item),
        (&requeued_story, &requeued_item),
        (&held_story, &held_item),
    ] {
        let refused = engine
            .claim_specific_agent_work(item, "forge-claim-007:late")
            .await
            .expect("the claim door runs");
        assert!(
            refused.is_none(),
            "{HARNESS}: a settled claim over {story} must not be claimable — a held story \
             cannot be reopened by a claim"
        );
    }
    // The poller's door, over the whole shared board: whatever it claims (if anything),
    // it is never one of this proof's held stories.
    if let Some(claimed) = engine
        .claim_next_agent_work("forge-claim-007:poller")
        .await
        .expect("the poller's claim door runs")
    {
        for story in [
            &done_story,
            &error_story,
            &cancelled_story,
            &abandoned_story,
            &requeued_story,
            &held_story,
            &stranded_story,
        ] {
            assert_ne!(
                claimed.story_id, *story,
                "{HARNESS}: the poller's door claimed a held story's work — a Hold story \
                 was reopened"
            );
        }
    }
    for story in [
        &done_story,
        &error_story,
        &cancelled_story,
        &abandoned_story,
        &requeued_story,
        &held_story,
        &stranded_story,
    ] {
        let status: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
            .bind(story)
            .fetch_one(pool)
            .await
            .expect("the board is readable");
        assert_eq!(
            status, "Hold",
            "{HARNESS}: {story} must still be Hold after every door ran"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE CONTROL — THE SAME DOOR OPENS FOR A BOARD THAT EXPECTS A RUN. The
    //    control story is `In Progress` with a stale claim: the same recovery door that
    //    settled the held claim returns THIS one to the queue. The refusal above is the
    //    human gate, not a dead door — without this case the proof would pass on a
    //    recovery that requeued nothing at all.
    // -----------------------------------------------------------------------------------------------------------
    control
        .requeue_stale_work(&control_item, &control_story)
        .await
        .expect("a stale claim the board still expects must be recoverable");
    assert_eq!(
        pair(pool, &control_item, &control_story).await,
        ("Ready".to_string(), "Ready".to_string()),
        "{HARNESS}: the control's stale claim must be requeued — the recovery door works, and \
         only the held board is refused"
    );
    // The recovered control claim is claimable again: the door the held stories' settled
    // items were refused by opens for work the board still expects.
    let retaken = engine
        .claim_specific_agent_work(&control_item, "forge-claim-007:next")
        .await
        .expect("the claim door runs")
        .expect("a recovered claim the board still expects must be claimable");
    assert_eq!(
        retaken.state, "Claimed",
        "{HARNESS}: the control's recovered work is back in the queue"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. COMMITTED TRUTH SURVIVES A ROLLBACK. The recovery's commit is durable, not a
    //    snapshot the DAO's connection happened to hold: a transaction that rewrites the
    //    control item sees its own uncommitted write, and rolling back restores exactly
    //    what recovery committed.
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
                        db::DbFailure::from_sqlx("test-harness.forge_claim_007.probe_update", &error)
                    })?;
                let state: String =
                    sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
                        .bind(&probe_item)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            db::DbFailure::from_sqlx("test-harness.forge_claim_007.probe_read", &error)
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
    // 6. CLEANUP. This run's proof stories are deleted; the items and runs cascade
    //    with them, so DEV is left as it was found.
    // -----------------------------------------------------------------------------------------------------------
    let proof_stories = vec![
        done_story.clone(),
        error_story.clone(),
        cancelled_story.clone(),
        abandoned_story.clone(),
        requeued_story.clone(),
        held_story.clone(),
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

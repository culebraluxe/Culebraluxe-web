//! The queue-claim lifecycle proof against DEV.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_work_claim_dev -- --ignored
//!
//! Why this exists: the Rust port kept every queue DAO and dropped the composition that claims one, so for a week
//! no row ever left `Ready` — `Done` newest 2026-09-19, `Error` newest 2026-09-18, zero `Claimed`/`Running`/
//! `Paused` — and the single-active index, the priority ordering, `attempts` and stale recovery were all inert.
//! Unit tests cannot see that: the defect is a write nobody performed, and only a real database has a queue to
//! write to. This test walks one item `Ready → Claimed → Running → Done` on DEV and proves, in the same run:
//! dispatch never claims a story the board says is being worked; the claim is exclusive at the database; a live
//! claim cannot be read as stale (the guard against requeueing a running story); a settled item cannot be settled
//! twice; `Done` is refused when the board never confirmed the work, with the story moving to `Hold` in the same
//! write; and the configuration-rejection path writes a state the live CHECK accepts, holding the board with it.
//!
//! It leaves DEV as it found it: both proof stories are deleted (their items cascade), and if the claim fell to a
//! pre-existing DEV item, that item's prior shape is restored exactly.

use db::{AgentWorkOutcome, Database, DbTarget, ForgeControlDao, ForgeEngineDao};

async fn cleanup_story(pool: &sqlx::PgPool, story_id: &str) {
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_claimed_item_walks_ready_to_done_and_never_settles_twice() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let control = ForgeControlDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let ready_story = format!("ENG-PROOF-READY-{tag}");
    let busy_story = format!("ENG-PROOF-BUSY-{tag}");

    // Precondition, stated rather than assumed: this proof needs the system-wide single-active slot free, because
    // that slot is one of the things it is measuring.
    let active_before: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where state in ('Claimed','Running','Paused')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        active_before, 0,
        "DEV already has an active claim; this proof needs the single-active slot free"
    );

    // 1. The board is the authority for dispatch: authorizing a story creates exactly one item, by trigger.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Queue claim proof', 'High', 'Ready', '')",
    )
    .bind(&ready_story)
    .execute(pool)
    .await
    .expect("insert proof story");
    let _ready_item: String =
        sqlx::query_scalar("select id::text from agent_work_item where story_id = $1 and state = 'Ready'")
            .bind(&ready_story)
            .fetch_one(pool)
            .await
            .expect("the Ready trigger created exactly one item");

    // 2. A leftover `Ready` item for a story the board says is `In Progress` — the shape every item in the queue
    //    actually has today, because nothing ever claimed one. Deliberately the highest-priority row in the table,
    //    so a selector that ignored the board would pick this one.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Queue claim proof (busy)', 'High', 'In Progress', '')",
    )
    .bind(&busy_story)
    .execute(pool)
    .await
    .expect("insert busy proof story");
    let busy_item: String = sqlx::query_scalar(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 999) returning id::text",
    )
    .bind(&busy_story)
    .fetch_one(pool)
    .await
    .expect("insert leftover Ready item");

    // 3. Dispatch claims — and never launches a story the board says is already being worked.
    let claimed = engine
        .claim_next_agent_work("proof-worker")
        .await
        .unwrap()
        .expect("a story the board has at Ready is claimable");
    assert_ne!(
        claimed.story_id, busy_story,
        "a `Ready` item whose story is `In Progress` must never be dispatched: that is the rerun of a live story"
    );
    assert_eq!(claimed.state, "Claimed");
    assert_eq!(claimed.claimed_by.as_deref(), Some("proof-worker"));
    let claimed_story_status: String =
        sqlx::query_scalar("select status from storyboard_story where id = $1")
            .bind(&claimed.story_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        claimed_story_status, "Ready",
        "only a story the board still has at Ready may be claimed"
    );
    let attempts: i32 = sqlx::query_scalar("select attempts from agent_work_item where id = $1::uuid")
        .bind(&claimed.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(attempts >= 1, "a claim must count the attempt");

    // DEV may hold Ready items this proof has no business terminalizing, so a borrowed claim is put back at the end.
    let borrowed = claimed.story_id != ready_story;

    // 4. The claim is exclusive at the database, not by courtesy.
    assert!(
        engine
            .claim_next_agent_work("second-worker")
            .await
            .unwrap()
            .is_none(),
        "a second worker cannot claim while one item is active (single-active backstop)"
    );
    assert!(
        engine
            .claim_specific_agent_work(&claimed.id, "second-worker")
            .await
            .unwrap()
            .is_none(),
        "an item that is already claimed cannot be claimed again"
    );

    // 5. Claimed → Running, and the heartbeat keeps it out of stale recovery. This is the write that makes a long
    //    run survivable: without it, `stale_agent_work` requeues a live run and the next tick launches a twin.
    assert!(
        engine.begin_agent_work_run(&claimed.id).await.unwrap(),
        "Claimed -> Running must settle exactly one row"
    );
    assert!(
        !engine.begin_agent_work_run(&claimed.id).await.unwrap(),
        "a row that is no longer `Claimed` must refuse to open: otherwise the engine drives a claim it does not own"
    );
    let running: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&claimed.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(running, "Running");
    let (before, started): (String, Option<String>) = sqlx::query_as(
        "select updated_at::text, started_at::text from agent_work_item where id = $1::uuid",
    )
    .bind(&claimed.id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(started.is_some(), "a run must record when it started");

    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    assert!(
        engine.heartbeat_agent_work(&claimed.id).await.unwrap(),
        "a running claim must accept a heartbeat"
    );
    let after: String = sqlx::query_scalar("select updated_at::text from agent_work_item where id = $1::uuid")
        .bind(&claimed.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(
        after > before,
        "a heartbeat must move updated_at forward, or a long run is requeued while it is still alive"
    );
    let stale = control.stale_agent_work(10).await.unwrap();
    assert!(
        stale.iter().all(|row| row.id != claimed.id),
        "a claim that was just heartbeated must not be readable as stale"
    );

    // 6. The one terminal write — and it cannot be written twice. The board is put where a finished run leaves it
    //    first: `Done` is accepted only when the board confirms the work, so a settle over a `Ready`/`In Progress`
    //    story is refused (proved in 6b) instead of being credited to a run that never finished.
    sqlx::query("update storyboard_story set status='Complete', completion=100, updated_at=now() where id=$1")
        .bind(&claimed.story_id)
        .execute(pool)
        .await
        .expect("board: the story completed");
    let settled = engine
        .finish_agent_work_run(&claimed.id, AgentWorkOutcome::Done, None)
        .await
        .unwrap()
        .expect("the first settle must land");
    assert_eq!(settled.item_state, "Done");
    assert_eq!(
        settled.story_status, None,
        "a board that already confirms completion needs no move"
    );
    let (state, finished): (String, Option<String>) =
        sqlx::query_as("select state, finished_at::text from agent_work_item where id = $1::uuid")
            .bind(&claimed.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(state, "Done");
    assert!(finished.is_some(), "a terminal write must stamp finished_at");
    assert!(
        engine
            .finish_agent_work_run(&claimed.id, AgentWorkOutcome::Error, Some("late second verdict"))
            .await
            .unwrap()
            .is_none(),
        "a settled item must not be settled again"
    );
    let (state_after, error_after): (String, Option<String>) =
        sqlx::query_as("select state, error_text from agent_work_item where id = $1::uuid")
            .bind(&claimed.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(state_after, "Done", "a second settle must not overwrite the verdict");
    assert_eq!(error_after, None, "a second settle must not write its reason either");

    // 6b. `Ok` from the engine is not completion. A run that stops `exhausted`, hits the step cap, or blocks on a
    //     missing ready task returns `Ok` with the story still `In Progress`, and crediting that as `Done` leaves a
    //     live workflow with no `Ready` row to resume it. The refusal therefore lives in the transaction, for every
    //     caller: the item records `Error` and the board moves with it, in one write.
    let stuck_story = format!("ENG-PROOF-STUCK-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Queue claim proof (stuck)', 'High', 'In Progress', '')",
    )
    .bind(&stuck_story)
    .execute(pool)
    .await
    .expect("insert stuck proof story");
    let stuck_item: String = sqlx::query_scalar(
        "insert into agent_work_item (story_id, state, priority, started_at)
         values ($1, 'Running', 1, now()) returning id::text",
    )
    .bind(&stuck_story)
    .fetch_one(pool)
    .await
    .expect("insert a running item over a story the board says is being worked");
    let refused = engine
        .finish_agent_work_run(&stuck_item, AgentWorkOutcome::Done, None)
        .await
        .unwrap()
        .expect("a run that ends must still settle its claim");
    assert_eq!(
        refused.item_state, "Error",
        "`Ok` is not completion: `Done` must be refused when the board never confirmed the work"
    );
    assert_eq!(
        refused.story_status,
        Some("Hold"),
        "the board moves with the item, in the same transaction"
    );
    assert!(refused.reason.unwrap().contains("Done refused"));
    let (stuck_item_state, stuck_story_status): (String, String) = sqlx::query_as(
        "select i.state, s.status from agent_work_item i
           join storyboard_story s on s.id = i.story_id where i.id = $1::uuid",
    )
    .bind(&stuck_item)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(stuck_item_state, "Error");
    assert_eq!(
        stuck_story_status, "Hold",
        "a story the board still expected must not be left `In Progress` beside a terminal item"
    );

    // 7. The configuration-rejection path writes a state the live CHECK accepts. It wrote `Failed` until
    //    2026-09-29 — a state the CHECK does not allow, which threw on the first caller ever to reach it.
    engine
        .reject_agent_work_configuration(&busy_item, "invalid --work-type PROOF")
        .await
        .expect("rejecting a claim's configuration must write a legal state");
    let busy_state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&busy_item)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(busy_state, "Error");
    let busy_story_status: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(&busy_story)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        busy_story_status, "Hold",
        "refusing a claim's configuration must move the board too: a `Ready` story beside a terminal item is \
         dispatched by nothing, which is the strand migration 258 repaired eight of"
    );
    assert!(
        !engine.heartbeat_agent_work(&busy_item).await.unwrap(),
        "a terminal item is not claimable and must not accept a heartbeat"
    );

    // 8. The queue is left as it was found: nothing active.
    let active_after: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where state in ('Claimed','Running','Paused')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(active_after, 0, "this proof must end with an empty single-active slot");

    if borrowed {
        // Put the borrowed DEV item back the way it was: a freshly queued, unclaimed item. The item goes first and
        // the board second — the Ready trigger fires on the story update, and the open row has to be there for its
        // `on conflict ... do nothing` to hold.
        sqlx::query(
            "update agent_work_item
                set state='Ready', claimed_at=null, claimed_by=null, started_at=null, finished_at=null,
                    updated_at=now()
              where id=$1::uuid",
        )
        .bind(&claimed.id)
        .execute(pool)
        .await
        .expect("restore the borrowed Ready item");
        // The claim gate guarantees this story was `Ready` when the proof claimed it; step 6 moved it to `Complete`
        // to settle the run, so the board is put back too. The trigger sees the open item and inserts nothing.
        sqlx::query(
            "update storyboard_story
                set status='Ready', completion=0, completed_at=null, updated_at=now()
              where id=$1",
        )
        .bind(&claimed.story_id)
        .execute(pool)
        .await
        .expect("restore the borrowed story status");
        eprintln!(
            "proof: claim fell to pre-existing DEV item {} (story {}); item and board restored",
            claimed.id, claimed.story_id
        );
    }
    cleanup_story(pool, &ready_story).await;
    cleanup_story(pool, &busy_story).await;
    cleanup_story(pool, &stuck_story).await;

    eprintln!(
        "proof: item {} walked Ready->Claimed->Running->Done; a `Ready` item over an `In Progress` story was not dispatched",
        claimed.id
    );
}

/// The engine-fault and pre-run-sweep proof against DEV.
///
/// Run with the first walk, single-threaded, because both occupy the system-wide single-active slot:
///   DATABASE_URL_DEV=... cargo test -p db --test forge_work_claim_dev -- --ignored --test-threads=1
///
/// What it proves that unit tests cannot: an engine fault **clears the pair back into the queue** instead of holding
/// the story (the captain's rule, 2026-09-29 — a broken engine must not cost a story its turn or leave a row that
/// reads like a verdict), a broken engine that will not stop stops spinning at `max_attempts`, and the sweep run
/// before every worker pass repairs the three shapes of junk: a story whose run is gone while the board says one is
/// happening, a `Ready` story with no item to dispatch, and an item whose story no longer expects a run.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn engine_faults_clear_the_pair_and_the_plane_is_swept_before_each_run() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();

    // 1. The stranded pair: the board says a run is happening and nothing anywhere holds it — no claimed item, no
    //    live instance. This is the shape that sat undispatchable, and the sweep puts the story back where a claim
    //    can reach it.
    let stranded = format!("ENG-PROOF-STRANDED-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Sweep proof (stranded)', 'High', 'In Progress', '')",
    )
    .bind(&stranded)
    .execute(pool)
    .await
    .expect("insert stranded proof story");
    let stranded_item: String = sqlx::query_scalar(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 0) returning id::text",
    )
    .bind(&stranded)
    .fetch_one(pool)
    .await
    .expect("insert the stranded item by hand");

    let swept = engine.reconcile_dispatch_queue().await.unwrap();
    assert!(
        swept.restated >= 1,
        "the sweep must put a story whose run is gone back on the board"
    );
    let board: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(&stranded)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(board, "Ready", "the board must move back with the item");
    let open_items: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&stranded)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        open_items, 1,
        "and the dispatch trigger must not open a second item for a story that already has one"
    );



    // 2. An engine fault clears the pair. The claim is taken through the real DAO, so the guard being measured is
    //    the one that runs in production.
    engine
        .claim_specific_agent_work(&stranded_item, "proof-worker")
        .await
        .unwrap()
        .expect("claim the proof item");
    assert!(
        engine.begin_agent_work_run(&stranded_item).await.unwrap(),
        "the run must be able to open its claim"
    );
    let cleared = engine
        .finish_agent_work_run(
            &stranded_item,
            AgentWorkOutcome::Abandoned,
            Some("DatabaseUnavailable during workflow.step (sqlstate 25P03)"),
        )
        .await
        .unwrap()
        .expect("the engine-fault settle must land");
    assert_eq!(
        cleared.item_state, "Ready",
        "an engine fault clears the item back into the queue"
    );
    assert_eq!(
        cleared.story_status,
        Some("Ready"),
        "and the story goes back with it, or nothing dispatches the pair again"
    );
    let (state, claimed_by, claimed_at, started, finished, error_text): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select state, claimed_by, claimed_at::text, started_at::text, finished_at::text, error_text
           from agent_work_item where id = $1::uuid",
    )
    .bind(&stranded_item)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(state, "Ready");
    assert_eq!(claimed_by, None, "a cleared row names nobody");
    assert_eq!(claimed_at, None);
    assert_eq!(started, None, "a cleared row is not a run that started");
    assert_eq!(finished, None, "and it is not finished either");
    assert!(
        error_text.unwrap_or_default().contains("25P03"),
        "the reason stays on the row, so the fault is readable after the fact"
    );

    // 3. It is genuinely back in the queue: the same item can be claimed again.
    assert!(
        engine
            .claim_specific_agent_work(&stranded_item, "proof-worker")
            .await
            .unwrap()
            .is_some(),
        "a cleared item must be claimable again"
    );

    // 4. A broken engine that will not stop stops spinning: at `max_attempts` the pair stops clearing and holds the
    //    story where a human will see it, instead of cycling the same story through the queue forever.
    sqlx::query("update agent_work_item set attempts = coalesce(max_attempts, 3) where id = $1::uuid")
        .bind(&stranded_item)
        .execute(pool)
        .await
        .unwrap();
    let exhausted = engine
        .finish_agent_work_run(&stranded_item, AgentWorkOutcome::Abandoned, Some("still broken"))
        .await
        .unwrap()
        .expect("the exhausted settle must land");
    assert_eq!(exhausted.item_state, "Error");
    assert_eq!(exhausted.story_status, Some("Hold"));


    // 5. The other two shapes: an item whose story no longer expects a run is cleared, and a `Ready` story whose item
    //    went away gets one — the case the dispatch trigger cannot see, because it fires on a *change* to `Ready`.
    let settled_story = format!("ENG-PROOF-SETTLED-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Sweep proof (settled)', 'High', 'Complete', '')",
    )
    .bind(&settled_story)
    .execute(pool)
    .await
    .expect("insert settled proof story");
    let junk_item: String = sqlx::query_scalar(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 0) returning id::text",
    )
    .bind(&settled_story)
    .fetch_one(pool)
    .await
    .expect("insert the junk item");

    let requeued_story = format!("ENG-PROOF-REQUEUED-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Sweep proof (requeued)', 'High', 'Ready', '')",
    )
    .bind(&requeued_story)
    .execute(pool)
    .await
    .expect("insert requeued proof story");
    sqlx::query("update agent_work_item set state='Cancelled', finished_at=now() where story_id = $1")
        .bind(&requeued_story)
        .execute(pool)
        .await
        .expect("cancel the item the trigger made, leaving a Ready story with nothing to dispatch");

    let swept = engine.reconcile_dispatch_queue().await.unwrap();
    assert!(swept.cleared >= 1, "an item over a settled story is junk");
    let junk_state: String = sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
        .bind(&junk_item)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(junk_state, "Cancelled");
    assert!(
        swept.queued >= 1,
        "a Ready story with no item must be given one"
    );
    let requeued_items: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where story_id = $1 and state = 'Ready'",
    )
    .bind(&requeued_story)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(requeued_items, 1, "and exactly one, never a duplicate");

    // DEV is left as it was found: the proof stories go, and their items cascade.
    for story in [&stranded, &settled_story, &requeued_story] {
        cleanup_story(pool, story).await;
    }
}

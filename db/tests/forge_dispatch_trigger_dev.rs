//! WHO WRITES A DISPATCHED WORK ITEM — the database, and only the database.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_dispatch_trigger_dev -- --ignored
//!
//! Why this exists: `agent_work_item_dispatch()` owns the dispatch rule — a story that BECOMES `Ready` gets exactly
//! one open work item, scored by `story_priority_score()`, arbitrated by the partial unique index
//! (`db/migrations/025_agent_work_queue.sql:101`, restated in `146_fix_storyboard_ready_dispatch_arbiter.sql:36`).
//! The Rust port spelled that rule a second time and wrote the row itself (until 2026-09-29), which is how a `Ready`
//! story could end up beside a terminal item with nothing left to dispatch it — the shape
//! `258_reopen_stranded_ready_work_items.sql` had to repair. A unit test cannot see this: the defect was a write
//! performed by the wrong owner, and only a real database has a trigger to perform it instead.
//!
//! So this proof walks the one case the trigger cannot see — a story that is ALREADY `Ready` when its item went
//! terminal, so there was no change of status to fire it — and shows the repair restoring the queue slot through the
//! database's own rule: the item it produces carries the database's score, the story's status is what the board
//! already wrote, a second sweep adds nothing, and the partial unique index arbitrates a manual re-dispatch. The
//! source-level half of the same rule is enforced by `cli`'s dispatch-owner guard
//! (`cli/src/forge/repo_guards.rs`).
//!
//! The sweep under test is the real one (`ForgeEngineDao::reconcile_dispatch_queue`), so it repairs whatever else is
//! stranded on DEV; that is its contract — a `Ready` story with no open item IS junk — and DEV's Forge data is
//! disposable. Every row this proof creates is deleted at the end (the story's items cascade with it).

use db::{Database, DbTarget, EnsureDispatch, ForgeEngineDao};

/// The open **dispatch slot(s)** for a story: the rows the partial unique index arbitrates on —
/// `state in ('Ready','Claimed','Running','Paused') and parallel_group_id is null`, added by migration
/// 143 and read through 146. Written out statically here (sqlx 0.9 refuses a dynamically built
/// statement) and read as a FIXTURE, not re-derived: if the index changes, the database's own trigger
/// is what moves, and this proof moves with it.
async fn open_slots(pool: &sqlx::PgPool, story_id: &str) -> Vec<(String, i32, Option<String>)> {
    sqlx::query_as(
        "select state, priority, claimed_by from agent_work_item
          where story_id=$1
            and state in ('Ready','Claimed','Running','Paused')
            and parallel_group_id is null",
    )
    .bind(story_id)
    .fetch_all(pool)
    .await
    .expect("read the open dispatch slots")
}

async fn story_status(pool: &sqlx::PgPool, story_id: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id=$1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("read story status")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_stranded_ready_story_is_redispatched_by_the_database_rule_not_by_the_sweep() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-DISPATCH-{tag}");
    let scored: i32 = sqlx::query_scalar("select story_priority_score($1)")
        .bind("High")
        .fetch_one(pool)
        .await
        .expect("the database scores its own priorities");
    assert!(
        scored > 0,
        "the score table this proof compares against went quiet: story_priority_score('High') = {scored}"
    );

    // 1. A story that is NOT `Ready` holds no dispatch slot: the trigger fires on a change INTO `Ready`.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Dispatch owner proof', 'High', 'Planned', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");
    assert_eq!(
        open_slots(pool, &story).await.len(),
        0,
        "a `Planned` story must not be dispatched"
    );

    // 2. The transition the trigger owns, and it is the DATABASE that writes the row and scores it.
    sqlx::query("update storyboard_story set status='Ready', updated_at=now() where id=$1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("dispatch the proof story");
    let (state, priority): (String, i32) =
        sqlx::query_as("select w.state, w.priority from agent_work_item w where w.story_id=$1")
            .bind(&story)
            .fetch_one(pool)
            .await
            .expect("the Ready trigger created exactly one item");
    println!("dispatch trigger: state={state} priority={priority} (score('High')={scored})");
    assert_eq!(state, "Ready", "a dispatched item is queued, not claimed");
    assert_eq!(
        priority, scored,
        "the item must carry the database's own score; a Rust-side constant would not equal it"
    );

    // 3. The stranding shape: the item goes terminal while the story stays `Ready`. Nothing dispatches this, and the
    //    trigger has nothing to fire on — this is the row migration 258 existed to repair.
    sqlx::query(
        "update agent_work_item set state='Cancelled', finished_at=now(), updated_at=now() where story_id=$1",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("strand the proof story");
    assert_eq!(
        open_slots(pool, &story).await.len(),
        0,
        "the stranded story has nothing to dispatch it"
    );
    assert_eq!(story_status(pool, &story).await, "Ready");

    // 4. The sweep repairs it — and the row it produces is the database's, not the sweep's.
    let swept = engine
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue");
    println!(
        "reconcile_dispatch_queue: queued={} restated={} cleared={}",
        swept.queued, swept.restated, swept.cleared
    );
    let repaired = open_slots(pool, &story).await;
    assert_eq!(
        repaired.len(),
        1,
        "the repair restores exactly one open slot, and the database's trigger is what creates it"
    );
    assert_eq!(repaired[0].0, "Ready");
    assert_eq!(
        repaired[0].1, scored,
        "the repaired item carries the database's score — a Rust hand-insert would carry whatever it was told"
    );
    assert_eq!(
        repaired[0].2, None,
        "a dispatch is a queue slot, not a claim: nothing holds it until the engine claims it"
    );
    assert_eq!(
        story_status(pool, &story).await,
        "Ready",
        "the repair restores the CHANGE the trigger fires on, inside one transaction, so the board sees only `Ready`"
    );

    // 5. Idempotent through the database's own arbiter: a second sweep, and a manual off/on cycle, both add nothing.
    engine
        .reconcile_dispatch_queue()
        .await
        .expect("sweep the DEV queue a second time");
    assert_eq!(
        open_slots(pool, &story).await.len(),
        1,
        "a second sweep must not add a second slot"
    );
    sqlx::query("update storyboard_story set status='Planned', updated_at=now() where id=$1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("stage the proof story off Ready");
    sqlx::query("update storyboard_story set status='Ready', updated_at=now() where id=$1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("dispatch the proof story again");
    assert_eq!(
        open_slots(pool, &story).await.len(),
        1,
        "the partial unique index arbitrates a duplicate dispatch away"
    );

    // 6. Leave DEV as it was found.
    sqlx::query("delete from storyboard_story where id=$1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("delete the proof story");
    let left: i64 = sqlx::query_scalar("select count(*) from agent_work_item where story_id=$1")
        .bind(&story)
        .fetch_one(pool)
        .await
        .expect("count leftover items");
    assert_eq!(left, 0, "the proof cleaned up after itself (items cascade)");
}

/// The board's **"into ENGINE RUN Q"** move, from the DAO the server calls, on the shape a bench round trip leaves
/// behind: the story is ALREADY `Ready` (moving to the bench withdraws the item and keeps the status) and holds no
/// slot. `set status='Ready'` dispatches nothing here — the trigger fires on a change INTO `Ready` — so the old code
/// moved the card, told the human it was queued, and dispatched nothing. `ensure_story_dispatched` restores the
/// change, so the slot exists and the id the caller is given is the row the database wrote.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn handing_an_already_ready_story_to_the_engine_still_queues_a_slot() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-ENGINE-{tag}");
    let scored: i32 = sqlx::query_scalar("select story_priority_score($1)")
        .bind("High")
        .fetch_one(pool)
        .await
        .expect("the database scores its own priorities");

    // 1. A story in the engine queue, then withdrawn to the bench: status stays `Ready`, the slot is cancelled.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Board dispatch proof', 'High', 'Ready', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");
    sqlx::query(
        "update agent_work_item set state='Cancelled', finished_at=now(), updated_at=now() where story_id=$1",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("bench the proof story");
    assert_eq!(story_status(pool, &story).await, "Ready");
    assert_eq!(
        open_slots(pool, &story).await.len(),
        0,
        "the benched story holds no slot, and its status still says `Ready`"
    );

    // 2. The board hands it to the engine — and the database is what queues it.
    let handed = engine
        .ensure_story_dispatched(&story)
        .await
        .expect("hand the proof story to the engine");
    println!("ensure_story_dispatched (benched, still Ready): {handed:?}");
    let queued = match handed {
        EnsureDispatch::Queued { item } => item,
        other => panic!("a story with no slot must be queued, not {other:?}"),
    };
    let slot = open_slots(pool, &story).await;
    assert_eq!(slot.len(), 1, "exactly one slot, created by the database");
    assert_eq!(slot[0].0, "Ready");
    assert_eq!(slot[0].1, scored, "scored by the database's own table");
    assert_eq!(slot[0].2, None, "a dispatch is not a claim");

    // 3. Asking twice confirms the same row — a dispatch is never duplicated, and the id is read back, not invented.
    let again = engine
        .ensure_story_dispatched(&story)
        .await
        .expect("hand the proof story over again");
    println!("ensure_story_dispatched (already queued): {again:?}");
    match again {
        EnsureDispatch::AlreadyQueued { item } => assert_eq!(
            item, queued,
            "the second call must confirm the database's row, not create a second one"
        ),
        other => panic!("a queued story must read as already queued, not {other:?}"),
    }
    assert_eq!(open_slots(pool, &story).await.len(), 1);

    // 4. A story that does not exist is never invented into a dispatch.
    let missing = engine
        .ensure_story_dispatched(&format!("ENG-PROOF-ABSENT-{tag}"))
        .await
        .expect("ask about an absent story");
    println!("ensure_story_dispatched (absent story): {missing:?}");
    assert_eq!(missing, EnsureDispatch::Missing);

    // 5. Leave DEV as it was found.
    sqlx::query("delete from storyboard_story where id=$1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("delete the proof story");
    let left: i64 = sqlx::query_scalar("select count(*) from agent_work_item where story_id=$1")
        .bind(&story)
        .fetch_one(pool)
        .await
        .expect("count leftover items");
    assert_eq!(left, 0, "the proof cleaned up after itself (items cascade)");
}

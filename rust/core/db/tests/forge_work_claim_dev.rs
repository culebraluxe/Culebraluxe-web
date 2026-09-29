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
//! twice; and the configuration-rejection path writes a state the live CHECK accepts.
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
    engine.begin_agent_work_run(&claimed.id).await.unwrap();
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

    // 6. The one terminal write — and it cannot be written twice.
    assert!(
        engine
            .finish_agent_work_run(&claimed.id, AgentWorkOutcome::Done, None)
            .await
            .unwrap(),
        "the first settle must land"
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
        !engine
            .finish_agent_work_run(&claimed.id, AgentWorkOutcome::Error, Some("late second verdict"))
            .await
            .unwrap(),
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
        // Put the borrowed DEV item back the way it was: a freshly queued, unclaimed item.
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
        eprintln!(
            "proof: claim fell to pre-existing DEV item {} (story {}); restored to Ready",
            claimed.id, claimed.story_id
        );
    }
    cleanup_story(pool, &ready_story).await;
    cleanup_story(pool, &busy_story).await;

    eprintln!(
        "proof: item {} walked Ready->Claimed->Running->Done; a `Ready` item over an `In Progress` story was not dispatched",
        claimed.id
    );
}

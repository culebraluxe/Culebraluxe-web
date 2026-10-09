//! The queue-claim lifecycle proof against DEV.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_work_claim_dev -- --ignored
//!
//! Why this exists: the Rust port kept every queue DAO and dropped the composition that claims one, so for a week
//! no row ever left `Ready` — `Done` newest 2026-09-19, `Error` newest 2026-09-18, zero `Claimed`/`Running`/
//! `Paused` — and the per-story serial index, the priority ordering, `attempts` and stale recovery were all inert.
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

/// The authority the item row holds — the fence every claim write takes (migration 278). It is READ from the
/// row rather than invented here: a test that guesses a generation proves something about the guess.
async fn fence_of(pool: &sqlx::PgPool, item_id: &str) -> db::ClaimFence {
    let (owner, generation): (Option<String>, i64) = sqlx::query_as(
        "select claimed_by, claim_generation from agent_work_item where id = $1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the claim fence reads back");
    db::ClaimFence::new(owner.unwrap_or_default(), generation)
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

    // Precondition, stated rather than assumed: this proof needs no global quiet. The queue is serial per STORY
    // since 2026-09-29 — the system-wide governor is gone — so a peer story running beside this proof is normal
    // rather than a conflict, and the claim below is scoped to the proof story by construction. A borrowed DEV item
    // is put back at the end.

    // 1. The board is the authority for dispatch: authorizing a story creates exactly one item, by trigger. The
    //    story carries a real specification, because the run is meant to snapshot it and a proof story with no
    //    specification could not tell a copy from a NULL.
    sqlx::query(
        "insert into storyboard_story
             (id, workstream, title, priority, status, notes, goal, test_mode, assay_commands,
              acceptance_criteria, preconditions, postconditions, architect_brief, context_refs,
              dependencies, scope, operating_surface, packet_sha)
         values ($1, 'PROOF', 'Queue claim proof', 'High', 'Ready', '',
                 '  prove the queue claim  ', 'SCOPED', 'cargo test -p db',
                 'the run records what it executed', 'a free single-active slot', 'the claim is settled',
                 'brief for the run', 'docs/agent/packets/ENG-PROOF.md', 'none', 'db',
                 'NEXUS', 'sha256:proof')",
    )
    .bind(&ready_story)
    .execute(pool)
    .await
    .expect("insert proof story");
    let _ready_item: String = sqlx::query_scalar(
        "select id::text from agent_work_item where story_id = $1 and state = 'Ready'",
    )
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
    let attempts: i32 =
        sqlx::query_scalar("select attempts from agent_work_item where id = $1::uuid")
            .bind(&claimed.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(attempts >= 1, "a claim must count the attempt");

    // DEV may hold Ready items this proof has no business terminalizing, so a borrowed claim is put back at the end.
    let borrowed = claimed.story_id != ready_story;

    // 4. The claim is exclusive at the database — for THIS story. Since the governor was removed (2026-09-29) a
    //    second worker may claim a DIFFERENT ready story in the same instant; that is the queue behaving exactly as
    //    its own indexes describe (`agent_work_item_one_serial_active_per_story`), and what may never happen is a
    //    second writer on this one. Both halves are asserted: the by-id path refuses outright, and the next-item
    //    path may hand out a peer story but never this one.
    assert!(
        engine
            .claim_specific_agent_work(&claimed.id, "second-worker")
            .await
            .unwrap()
            .is_none(),
        "an item that is already claimed cannot be claimed again"
    );
    if let Some(peer) = engine.claim_next_agent_work("second-worker").await.unwrap() {
        assert_ne!(
            peer.story_id, claimed.story_id,
            "two workers must never hold one story: the queue is serial per story, not per system"
        );
        engine
            .finish_agent_work_run(
                &peer.id,
                &fence_of(&pool, &peer.id).await,
                AgentWorkOutcome::Abandoned,
                Some("proof borrow"),
            )
            .await
            .expect("a borrowed DEV claim must be put back");
    }

    // 5. Claimed → Running, and the heartbeat keeps it out of stale recovery. This is the write that makes a long
    //    run survivable: without it, `stale_agent_work` requeues a live run and the next tick launches a twin.
    //    The transition also reports the claimed row's `execution_policy`, because that policy decides whether the
    //    run may be unattended at all (migration 029).
    let begin = engine
        .begin_agent_work_run(&claimed.id, &fence_of(&pool, &claimed.id).await)
        .await
        .unwrap()
        .expect("Claimed -> Running must settle exactly one row and report the item's policy");
    assert!(
        !begin.execution_policy.trim().is_empty(),
        "the durable execution policy rides the claim"
    );
    // 5b. Execution beginning IS the Story Run row (migration 025 §2), and the claim is stamped with it in the same
    //     write. The port shipped for a week with `story_run_id` null on every claim: the lane ran, and nothing
    //     durable was created for it to be read against.
    let (stamped, run_story, run_status, run_ended): (
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select i.story_run_id::text, r.story_id, r.result_status, r.ended_at::text
           from agent_work_item i
           join storyboard_story_run r on r.id = i.story_run_id
          where i.id = $1::uuid",
    )
    .bind(&claimed.id)
    .fetch_one(pool)
    .await
    .expect("beginning execution must open a storyboard_story_run row for the claim");
    assert_eq!(
        stamped.as_deref(),
        Some(begin.story_run_id.as_str()),
        "the claim must carry the run it opened"
    );
    assert_eq!(
        run_story, claimed.story_id,
        "the run belongs to the claim's story"
    );
    assert!(
        run_status.is_none(),
        "a run that just started has no ruling yet — an unruled run is not a verdict"
    );
    assert!(run_ended.is_none(), "a run that just started has not ended");
    // 5c. The run carries the story's specification, and it was copied by the insert rather than passed in: every
    //     specification column the run holds must equal the story row's, by the same `nullif(trim(…), '')` rule the
    //     insert uses. Measured as one comparison against the row, so a borrowed DEV claim (a story with no
    //     specification) proves the NULL case while the proof story proves the copy.
    let spec_matches_row: bool = sqlx::query_scalar(
        "select (r.goal_snapshot              is not distinct from nullif(trim(s.goal), ''))
            and (r.preconditions_snapshot      is not distinct from nullif(trim(s.preconditions), ''))
            and (r.architect_brief_snapshot    is not distinct from nullif(trim(s.architect_brief), ''))
            and (r.context_refs_snapshot       is not distinct from nullif(trim(s.context_refs), ''))
            and (r.acceptance_criteria_snapshot is not distinct from nullif(trim(s.acceptance_criteria), ''))
            and (r.postconditions_snapshot     is not distinct from nullif(trim(s.postconditions), ''))
            and (r.dependencies_snapshot       is not distinct from nullif(trim(s.dependencies), ''))
            and (r.scope_snapshot              is not distinct from nullif(trim(s.scope), ''))
            and (r.operating_surface_snapshot  is not distinct from nullif(trim(s.operating_surface), ''))
            and (r.test_mode_snapshot          is not distinct from nullif(trim(s.test_mode), ''))
            and (r.assay_commands_snapshot     is not distinct from nullif(trim(s.assay_commands), ''))
            and (r.packet_sha_snapshot         is not distinct from nullif(trim(s.packet_sha), ''))
           from storyboard_story_run r
           join storyboard_story s on s.id = r.story_id
          where r.id = $1::uuid",
    )
    .bind(&begin.story_run_id)
    .fetch_one(pool)
    .await
    .expect("the run's specification must be readable beside the story it came from");
    assert!(
        spec_matches_row,
        "every specification column on the run must be the story row's, trimmed the same way the insert trims it"
    );
    if !borrowed {
        let (run_goal, run_assay, run_sha): (Option<String>, Option<String>, Option<String>) =
            sqlx::query_as(
                "select goal_snapshot, assay_commands_snapshot, packet_sha_snapshot
                   from storyboard_story_run where id = $1::uuid",
            )
            .bind(&begin.story_run_id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            run_goal.as_deref(),
            Some("prove the queue claim"),
            "the run must snapshot the goal of the story it is executing, trimmed"
        );
        assert_eq!(run_assay.as_deref(), Some("cargo test -p db"));
        assert_eq!(run_sha.as_deref(), Some("sha256:proof"));
    }
    assert!(
        engine
            .begin_agent_work_run(&claimed.id, &fence_of(&pool, &claimed.id).await)
            .await
            .unwrap()
            .is_none(),
        "a row that is no longer `Claimed` must refuse to open: otherwise the engine drives a claim it does not own"
    );
    let running: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
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
        engine
            .heartbeat_agent_work(
                &claimed.id,
                &fence_of(&pool, &claimed.id).await,
                std::time::Duration::from_secs(300)
            )
            .await
            .unwrap(),
        "a running claim must accept a heartbeat"
    );
    let after: String =
        sqlx::query_scalar("select updated_at::text from agent_work_item where id = $1::uuid")
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
        .finish_agent_work_run(
            &claimed.id,
            &fence_of(&pool, &claimed.id).await,
            AgentWorkOutcome::Done,
            None,
        )
        .await
        .unwrap();
    assert!(
        settled.wrote(),
        "the first settle must land (got {})",
        settled.name()
    );
    let settled = settled
        .settlement()
        .cloned()
        .expect("a written settle carries its pair");
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
    assert!(
        finished.is_some(),
        "a terminal write must stamp finished_at"
    );
    // 6a. The run the claim opened ends with the claim, in the same write, and it ends with the item's ruling.
    let (closed_status, closed_at): (Option<String>, Option<String>) = sqlx::query_as(
        "select r.result_status, r.ended_at::text
           from agent_work_item i
           join storyboard_story_run r on r.id = i.story_run_id
          where i.id = $1::uuid",
    )
    .bind(&claimed.id)
    .fetch_one(pool)
    .await
    .expect("the settled claim still names its run");
    assert_eq!(
        closed_status.as_deref(),
        Some("Complete"),
        "a `Done` claim rules its run Complete"
    );
    assert!(closed_at.is_some(), "a settled claim closes its run");
    assert!(
        !engine
            .finish_agent_work_run(
                &claimed.id,
                &fence_of(&pool, &claimed.id).await,
                AgentWorkOutcome::Error,
                Some("late second verdict"),
            )
            .await
            .unwrap()
            .wrote(),
        "a settled item must not be settled again"
    );
    let (state_after, error_after): (String, Option<String>) =
        sqlx::query_as("select state, error_text from agent_work_item where id = $1::uuid")
            .bind(&claimed.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        state_after, "Done",
        "a second settle must not overwrite the verdict"
    );
    assert_eq!(
        error_after, None,
        "a second settle must not write its reason either"
    );

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
        .finish_agent_work_run(
            &stuck_item,
            &fence_of(&pool, &stuck_item).await,
            AgentWorkOutcome::Done,
            None,
        )
        .await
        .unwrap();
    assert!(
        refused.wrote(),
        "a run that ends must still settle its claim (got {})",
        refused.name()
    );
    let refused = refused
        .settlement()
        .cloned()
        .expect("a written settle carries its pair");
    assert_eq!(
        refused.item_state, "Error",
        "`Ok` is not completion: `Done` must be refused when the board never confirmed the work"
    );
    assert_eq!(
        refused.story_status.as_deref(),
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
    let busy_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&busy_item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(busy_state, "Error");
    let busy_story_status: String =
        sqlx::query_scalar("select status from storyboard_story where id = $1")
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
        !engine
            .heartbeat_agent_work(
                &busy_item,
                &fence_of(&pool, &busy_item).await,
                std::time::Duration::from_secs(300)
            )
            .await
            .unwrap(),
        "a terminal item is not claimable and must not accept a heartbeat"
    );

    // 8. The queue is left as it was found: nothing of THIS proof's is open. The count is scoped to the proof
    //    stories deliberately — since 2026-09-29 a peer story running beside this proof is normal (the queue is
    //    serial per story, not per system), and a global count would report someone else's live run as this proof's
    //    failure. Nothing here touches another story's rows.
    let active_after: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item
          where state in ('Claimed','Running','Paused')
            and story_id = any($1::text[])",
    )
    .bind(vec![
        ready_story.clone(),
        busy_story.clone(),
        stuck_story.clone(),
    ])
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        active_after, 0,
        "this proof must end with none of its own claims open"
    );

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
/// Run with the first walk, single-threaded, because both write the same queue and the same board and read counts
/// back out of them:
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
        engine
            .begin_agent_work_run(&stranded_item, &fence_of(&pool, &stranded_item).await)
            .await
            .unwrap()
            .is_some(),
        "the run must be able to open its claim"
    );
    let cleared = engine
        .finish_agent_work_run(
            &stranded_item,
            &fence_of(&pool, &stranded_item).await,
            AgentWorkOutcome::Abandoned,
            Some("DatabaseUnavailable during workflow.step (sqlstate 25P03)"),
        )
        .await
        .unwrap();
    assert!(
        cleared.wrote(),
        "the engine-fault settle must land (got {})",
        cleared.name()
    );
    let cleared = cleared
        .settlement()
        .cloned()
        .expect("a written settle carries its pair");
    assert_eq!(
        cleared.item_state, "Ready",
        "an engine fault clears the item back into the queue"
    );
    assert_eq!(
        cleared.story_status.as_deref(),
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
    sqlx::query(
        "update agent_work_item set attempts = coalesce(max_attempts, 3) where id = $1::uuid",
    )
    .bind(&stranded_item)
    .execute(pool)
    .await
    .unwrap();
    let exhausted = engine
        .finish_agent_work_run(
            &stranded_item,
            &fence_of(&pool, &stranded_item).await,
            AgentWorkOutcome::Abandoned,
            Some("still broken"),
        )
        .await
        .unwrap();
    assert!(
        exhausted.wrote(),
        "the exhausted settle must land (got {})",
        exhausted.name()
    );
    let exhausted = exhausted
        .settlement()
        .cloned()
        .expect("a written settle carries its pair");
    assert_eq!(exhausted.item_state, "Error");
    assert_eq!(exhausted.story_status.as_deref(), Some("Hold"));

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
    sqlx::query(
        "update agent_work_item set state='Cancelled', finished_at=now() where story_id = $1",
    )
    .bind(&requeued_story)
    .execute(pool)
    .await
    .expect("cancel the item the trigger made, leaving a Ready story with nothing to dispatch");

    let swept = engine.reconcile_dispatch_queue().await.unwrap();
    assert!(swept.cleared >= 1, "an item over a settled story is junk");
    let junk_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
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

    // 6. GPT's P0 (2026-09-29), and the reason the sweep is allowed near the board at all: an `In Progress` story
    //    with no Forge trace is OPEN — human work, deliberately off the engine. It must be left exactly as it is,
    //    because restating it would fire the dispatch trigger and run a story nobody handed over.
    let open_story = format!("ENG-PROOF-OPEN-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Sweep proof (open human card)', 'High', 'In Progress', '')",
    )
    .bind(&open_story)
    .execute(pool)
    .await
    .expect("insert the open proof story");

    engine.reconcile_dispatch_queue().await.unwrap();
    let open_board: String =
        sqlx::query_scalar("select status from storyboard_story where id = $1")
            .bind(&open_story)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        open_board, "In Progress",
        "an OPEN card must stay off the engine run queue"
    );
    let open_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
            .bind(&open_story)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        open_items, 0,
        "and the sweep must not manufacture a work item for it"
    );

    // DEV is left as it was found: the proof stories go, and their items cascade.
    for story in [&stranded, &settled_story, &requeued_story, &open_story] {
        cleanup_story(pool, story).await;
    }
}

/// One behaviour, one implementation: the claim transaction is the database's (migration 262), and the DAO is an
/// adapter. No database needed — this reads the two sources the claim lives in.
#[test]
fn the_claim_transaction_lives_in_the_database_not_in_rust() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let dao = std::fs::read_to_string(root.join("db/src/forge_engine.rs")).unwrap();
    let routine =
        std::fs::read_to_string(root.join("db/migrations/262_forge_agent_work_claim.sql")).unwrap();
    let settlement =
        std::fs::read_to_string(root.join("db/migrations/263_forge_agent_work_settlement.sql"))
            .unwrap();
    for function in [
        "forge_settlement_pair",
        "forge_run_result_status_for",
        "forge_close_story_run",
        "forge_finish_agent_work_run",
        "forge_reject_agent_work_configuration",
    ] {
        assert!(
            settlement.contains(&format!("create or replace function {function}(")),
            "migration 263 defines {function}"
        );
    }
    let dispatch =
        std::fs::read_to_string(root.join("db/migrations/265_forge_dispatch_reconcile.sql"))
            .unwrap();
    for function in ["forge_dispatch_story", "forge_reconcile_dispatch_queue"] {
        assert!(
            dispatch.contains(&format!("create or replace function {function}(")),
            "migration 265 defines {function}"
        );
        assert!(
            dao.contains(&format!("from {function}(")),
            "the DAO calls {function}"
        );
    }
    let artifact =
        std::fs::read_to_string(root.join("db/migrations/267_forge_tool_artifact_write.sql"))
            .unwrap();
    for function in [
        "forge_verdict_polarity",
        "forge_artifact_verdict_for_run",
        "forge_record_tool_artifact",
    ] {
        assert!(
            artifact.contains(&format!("create or replace function {function}(")),
            "migration 267 defines {function}"
        );
    }
    assert!(
        dao.contains("from forge_record_tool_artifact("),
        "the DAO calls forge_record_tool_artifact"
    );
    let recovery =
        std::fs::read_to_string(root.join("db/migrations/266_forge_stale_recovery.sql")).unwrap();
    let control = std::fs::read_to_string(root.join("db/src/forge_control.rs")).unwrap();
    let reset = std::fs::read_to_string(root.join("db/src/forge_reset.rs")).unwrap();
    for (function, caller) in [
        ("forge_hold_stale_work", &control),
        ("forge_requeue_stale_work", &control),
        ("forge_recover_stale_engine_claim", &reset),
    ] {
        assert!(
            recovery.contains(&format!("create or replace function {function}(")),
            "migration 266 defines {function}"
        );
        assert!(
            caller.contains(&format!("{function}(")),
            "the DAO calls {function}"
        );
    }
    for (source, choreography) in [
        (&control, "begin(\"forge_control.hold_stale_work"),
        (&control, "begin(\"forge_control.requeue_stale_work"),
        (&reset, "begin(\"forge_reset.recover_stale_claim"),
        (&reset, "last_error='stale claim recovered'"),
    ] {
        assert!(
            !source.contains(choreography),
            "stale-recovery choreography `{choreography}` is still in Rust beside migration 266"
        );
    }
    let begin =
        std::fs::read_to_string(root.join("db/migrations/264_forge_agent_work_begin.sql")).unwrap();
    assert!(
        begin.contains("create or replace function forge_begin_agent_work_run("),
        "migration 264 defines forge_begin_agent_work_run"
    );
    assert!(
        dao.contains("from forge_begin_agent_work_run("),
        "the DAO calls forge_begin_agent_work_run"
    );
    for function in [
        "forge_finish_agent_work_run",
        "forge_reject_agent_work_configuration",
    ] {
        assert!(
            dao.contains(&format!("{function}(")),
            "the DAO calls {function}"
        );
    }
    // The specific-item claim (262) and THE claim door (275). `forge_claim_next_agent_work` is still defined by 262 but
    // is a delegation to `forge_claim_story` since 275, so the door the DAO calls — and the one that obeys the brake and
    // the fleet ceiling — is `forge_claim_story`.
    let claim_door =
        std::fs::read_to_string(root.join("db/migrations/275_forge_claim_story_brake.sql"))
            .unwrap();
    assert!(
        routine.contains("create or replace function forge_claim_specific_agent_work("),
        "migration 262 defines forge_claim_specific_agent_work"
    );
    assert!(
        routine.contains("create or replace function forge_claim_next_agent_work("),
        "migration 262 still defines forge_claim_next_agent_work (275 re-points it at the door)"
    );
    assert!(
        claim_door.contains("create or replace function forge_claim_story("),
        "migration 275 defines forge_claim_story"
    );
    for function in ["forge_claim_specific_agent_work", "forge_claim_story"] {
        assert!(
            dao.contains(&format!("from {function}(")),
            "the DAO calls {function}"
        );
    }
    for choreography in [
        "pg_advisory_xact_lock",
        "skip locked",
        "attempts=attempts+1",
        "attempts = attempts + 1",
        "begin(\"forge_engine.claim_",
        "AGENT_CLAIM_LOCK",
        // The settlement (migration 263): the policy, the run close and the terminal transactions.
        "fn settlement_pair",
        "fn run_result_status_for",
        "fn close_story_run_in",
        "begin(\"forge_engine.finish_agent_work_run",
        "begin(\"forge_engine.reject_agent_work_configuration",
        "Done refused",
        // Dispatch and the sweep (migration 265).
        "fn dispatch_story_in",
        "set status='Planned'",
        "begin(\"forge_engine.reconcile_dispatch_queue",
        "begin(\"forge_engine.ensure_story_dispatched",
        // Stale recovery (migration 266) lives in forge_control.rs / forge_reset.rs, checked below.
        // The artifact write (migration 267).
        "fn artifact_verdict_for_run",
        "fn verdict_polarity",
        "insert into forge_tool_artifact",
        "begin(\"forge_engine.record_tool_artifact",
        // The run open (migration 264).
        "insert into storyboard_story_run",
        "begin(\"forge_engine.begin_agent_work_run",
    ] {
        assert!(
            !dao.contains(choreography),
            "forge_engine.rs still carries claim choreography `{choreography}` beside the stored routine"
        );
    }
}

/// The stored routines, against DEV: the claim contract the Rust transactions had, held by the database.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_claim_routines_hold_the_claim_contract() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();

    let installed: i64 = sqlx::query_scalar(
        "select count(*) from pg_proc
          where proname in ('forge_claim_specific_agent_work', 'forge_claim_next_agent_work', 'forge_claim_story')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(installed, 3, "migrations 262 and 275 are applied to DEV");

    // A run that panicked part-way leaves its proof stories behind; these prefixes are this test's alone.
    sqlx::query(
        "delete from storyboard_story where id like 'ENG-PROOF-CLAIM-%' or id like 'ENG-PROOF-NEXT-%'",
    )
    .execute(pool)
    .await
    .expect("clear earlier runs' proof stories");

    // Stories the board says are being worked: `claim_next` never selects their items, so the specific-claim
    // rails below cannot be disturbed by a live DEV worker.
    let mut stories = Vec::new();
    async fn held(pool: &sqlx::PgPool, story: &str, priority: i32) -> String {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes)
             values ($1, 'PROOF', 'Claim routine proof', 'High', 'In Progress', '')",
        )
        .bind(story)
        .execute(pool)
        .await
        .expect("insert proof story");
        sqlx::query_scalar(
            "insert into agent_work_item (story_id, state, priority, role, stop_after, launch_intent)
             values ($1, 'Ready', $2, 'smith', 'lead', 'SOLO') returning id::text",
        )
        .bind(story)
        .bind(priority)
        .fetch_one(pool)
        .await
        .expect("insert proof item")
    }
    async fn claim_shape(
        pool: &sqlx::PgPool,
        item: &str,
    ) -> (String, Option<String>, i32, bool, bool) {
        sqlx::query_as(
            "select state, claimed_by, attempts, claimed_at is not null, started_at is not null
               from agent_work_item where id = $1::uuid",
        )
        .bind(item)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    let story_a = format!("ENG-PROOF-CLAIM-A-{tag}");
    let story_b = format!("ENG-PROOF-CLAIM-B-{tag}");
    let item_a = held(pool, &story_a, 0).await;
    let item_b = held(pool, &story_b, 0).await;
    stories.extend([story_a.clone(), story_b.clone()]);

    // 1 + 3 + 5. One worker claims exactly the item it named, and the claim writes the pre-262 contract: state,
    //    owner, one attempt, a claim time, no start — and the row it returns is the row the database holds, with
    //    the execution envelope carried through.
    let before: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("select now()")
        .fetch_one(pool)
        .await
        .unwrap();
    let claimed = engine
        .claim_specific_agent_work(&item_b, "routine-worker-1")
        .await
        .unwrap()
        .expect("a Ready item on a story with nothing open is claimed");
    assert_eq!(
        claimed.id, item_b,
        "the specific claim claims the item it was asked for"
    );
    assert_eq!(claimed.story_id, story_b);
    assert_eq!(claimed.state, "Claimed");
    assert_eq!(claimed.claimed_by.as_deref(), Some("routine-worker-1"));
    assert_eq!(claimed.role.as_deref(), Some("smith"));
    assert_eq!(claimed.stop_after.as_deref(), Some("lead"));
    assert_eq!(claimed.launch_intent.as_deref(), Some("SOLO"));
    assert_eq!(claimed.execution_policy, "Unattended OK");
    assert_eq!(
        claim_shape(pool, &item_b).await,
        (
            "Claimed".into(),
            Some("routine-worker-1".into()),
            1,
            true,
            false
        )
    );
    let claimed_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("select claimed_at from agent_work_item where id = $1::uuid")
            .bind(&item_b)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(claimed_at >= before, "claimed_at is the claim's own time");
    assert_eq!(
        claim_shape(pool, &item_a).await,
        ("Ready".into(), None, 0, false, false),
        "a sibling item is untouched"
    );

    // 2. A second worker cannot claim it, and the refusal writes nothing.
    assert!(engine
        .claim_specific_agent_work(&item_b, "routine-worker-2")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        claim_shape(pool, &item_b).await,
        (
            "Claimed".into(),
            Some("routine-worker-1".into()),
            1,
            true,
            false
        ),
        "a refused claim must not take the owner or count an attempt"
    );

    // The run open (migration 264): a Ready item is not the caller's to begin, a Claimed one opens exactly one run
    // with its own envelope, role and actual target, and a second begin opens nothing.
    async fn runs(pool: &sqlx::PgPool, story: &str) -> i64 {
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id = $1")
            .bind(story)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    assert!(engine
        .begin_agent_work_run(&item_a, &fence_of(&pool, &item_a).await)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        runs(pool, &story_a).await,
        0,
        "an unclaimed item opens no run"
    );
    let begun = engine
        .begin_agent_work_run(&item_b, &fence_of(&pool, &item_b).await)
        .await
        .unwrap()
        .expect("the claimed item begins");
    assert_eq!(
        (
            begun.execution_policy.as_str(),
            begun.launch_intent.as_deref()
        ),
        ("Unattended OK", Some("SOLO"))
    );
    let (run_item, environment, run_type, state): (String, String, String, String) =
        sqlx::query_as(
            "select i.story_run_id::text, r.execution_environment, r.run_type, i.state
           from agent_work_item i join storyboard_story_run r on r.id = i.story_run_id
          where i.id = $1::uuid",
        )
        .bind(&item_b)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        (
            run_item.as_str(),
            environment.as_str(),
            run_type.as_str(),
            state.as_str()
        ),
        (begun.story_run_id.as_str(), "DEV", "smith", "Running")
    );
    assert!(engine
        .begin_agent_work_run(&item_b, &fence_of(&pool, &item_b).await)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        runs(pool, &story_b).await,
        1,
        "a second begin opens no second run"
    );

    // 6. No lost and no double claim under concurrency: eight workers race for one item, exactly one wins.
    let mut racers = Vec::new();
    for worker in 0..8 {
        let engine = engine.clone();
        let item = item_a.clone();
        racers.push(tokio::spawn(async move {
            engine
                .claim_specific_agent_work(&item, &format!("racer-{worker}"))
                .await
                .unwrap()
        }));
    }
    let mut winners = Vec::new();
    for racer in racers {
        if let Some(row) = racer.await.unwrap() {
            winners.push(row.claimed_by.unwrap());
        }
    }
    assert_eq!(winners.len(), 1, "exactly one racer claims: {winners:?}");
    let (state, owner, attempts, _, _) = claim_shape(pool, &item_a).await;
    assert_eq!(
        (state.as_str(), owner.as_deref(), attempts),
        ("Claimed", Some(winners[0].as_str()), 1)
    );

    // 7. A claim inside a transaction that fails leaves no half-claimed item.
    let story_c = format!("ENG-PROOF-CLAIM-C-{tag}");
    let item_c = held(pool, &story_c, 0).await;
    stories.push(story_c.clone());
    {
        let mut tx = pool.begin().await.unwrap();
        let inside: Option<String> = sqlx::query_scalar(
            "select id::text from forge_claim_specific_agent_work($1::uuid, 'doomed-worker')",
        )
        .bind(&item_c)
        .fetch_optional(&mut *tx)
        .await
        .unwrap();
        assert_eq!(inside.as_deref(), Some(item_c.as_str()));
        assert!(sqlx::query("select 1 / 0").execute(&mut *tx).await.is_err());
        tx.rollback().await.unwrap();
    }
    assert_eq!(
        claim_shape(pool, &item_c).await,
        ("Ready".into(), None, 0, false, false),
        "the failed transaction took the claim with it"
    );

    // 4. Next-claim ordering: priority first, then queue time. Three board-Ready stories far above any real DEV
    //    priority, claimed one at a time.
    let mut ordered = Vec::new();
    for (suffix, priority, offset) in [
        ("LOW", 1_000_000, 0),
        ("HIGH-LATE", 1_000_001, 5),
        ("HIGH-EARLY", 1_000_001, 10),
    ] {
        let story = format!("ENG-PROOF-NEXT-{suffix}-{tag}");
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes)
             values ($1, 'PROOF', 'Claim order proof', 'High', 'Ready', '')",
        )
        .bind(&story)
        .execute(pool)
        .await
        .expect("insert ordered story");
        let item: String = sqlx::query_scalar(
            "update agent_work_item set priority = $2, queued_at = now() - make_interval(secs => $3)
              where story_id = $1 and state = 'Ready' returning id::text",
        )
        .bind(&story)
        .bind(priority)
        .bind(offset as f64)
        .fetch_one(pool)
        .await
        .expect("the Ready trigger created the item");
        stories.push(story);
        ordered.push(item);
    }
    let mut taken = Vec::new();
    for _ in 0..3 {
        let row = engine
            .claim_next_agent_work("routine-next")
            .await
            .unwrap()
            .expect("an eligible item is claimed");
        taken.push(row.id);
    }
    let expected = vec![ordered[2].clone(), ordered[1].clone(), ordered[0].clone()];
    let borrowed: Vec<&String> = taken.iter().filter(|id| !ordered.contains(id)).collect();
    for id in &borrowed {
        sqlx::query(
            "update agent_work_item set state = 'Ready', claimed_by = null, claimed_at = null,
                    attempts = attempts - 1 where id = $1::uuid",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }
    for story in &stories {
        cleanup_story(pool, story).await;
    }
    assert!(
        borrowed.is_empty(),
        "the order proof claimed a real DEV item (restored): {borrowed:?}"
    );
    assert_eq!(
        taken, expected,
        "next claim: highest priority first, then earliest queued"
    );
}

/// The settlement policy, against DEV: the case table the deleted Rust `settlement_pair` unit tests held, asserted
/// against `forge_settlement_pair` and `forge_run_result_status_for` (migration 263) word for word.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_settlement_policy_is_the_databases() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    async fn pair(
        pool: &sqlx::PgPool,
        outcome: &str,
        board: &str,
    ) -> (String, Option<String>, Option<String>) {
        sqlx::query_as("select item_state, story_status, reason from forge_settlement_pair($1, $2)")
            .bind(outcome)
            .bind(board)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    async fn ruling(pool: &sqlx::PgPool, item_state: &str) -> Option<String> {
        sqlx::query_scalar("select forge_run_result_status_for($1)")
            .bind(item_state)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    // A cleared run keeps no ruling: an engine fault never becomes a story verdict.
    assert_eq!(ruling(pool, "Done").await.as_deref(), Some("Complete"));
    assert_eq!(ruling(pool, "Error").await.as_deref(), Some("Failed"));
    assert_eq!(
        ruling(pool, "Cancelled").await.as_deref(),
        Some("Cancelled")
    );
    assert_eq!(ruling(pool, "Ready").await, None);
    assert_eq!(ruling(pool, "Claimed").await, None);
    assert_eq!(ruling(pool, "").await, None);

    // The run ruling follows the pair that settles it, for every outcome the engine settles with.
    for (outcome, board, expected) in [
        (AgentWorkOutcome::Done, "Complete", Some("Complete")),
        (AgentWorkOutcome::Error, "In Progress", Some("Failed")),
        (
            AgentWorkOutcome::Cancelled,
            "In Progress",
            Some("Cancelled"),
        ),
        (AgentWorkOutcome::Abandoned, "In Progress", None),
    ] {
        let (item, _, _) = pair(pool, outcome.as_str(), board).await;
        assert_eq!(
            ruling(pool, &item).await.as_deref(),
            expected,
            "{outcome:?} over {board}"
        );
    }

    // The two honest `Done`s: the board confirms it, or a human gate stopped the run.
    for board in ["Complete", "Hold"] {
        assert_eq!(
            pair(pool, "Done", board).await,
            ("Done".into(), None, None),
            "{board}"
        );
    }

    // `Ok` from the engine is not completion: `Done` over an unfinished board is refused and the story held.
    let (item, story, reason) = pair(pool, "Done", "In Progress").await;
    assert_eq!((item.as_str(), story.as_deref()), ("Error", Some("Hold")));
    assert_eq!(
        reason.as_deref(),
        Some("run ended without the board confirming completion (story status 'In Progress'); Done refused")
    );

    // `Done` over a board that no longer expects a run is still refused, but the board is not moved.
    for board in ["Planned", "Batched"] {
        let (item, story, reason) = pair(pool, "Done", board).await;
        assert_eq!((item.as_str(), story), ("Error", None), "{board}");
        assert!(reason.unwrap().contains("Done refused"), "{board}");
    }
    let (item, story, reason) = pair(pool, "Done", "Ready").await;
    assert_eq!((item.as_str(), story.as_deref()), ("Error", Some("Hold")));
    assert!(
        reason.is_some(),
        "a refused Done over a Ready board says why"
    );

    // An engine fault clears the pair back into the queue while the board expects a run...
    for board in ["Ready", "In Progress"] {
        assert_eq!(
            pair(pool, "Abandoned", board).await,
            ("Ready".into(), Some("Ready".into()), None),
            "{board}"
        );
    }
    // ...and over a settled board only clears the item: no fault demotes `Complete` or reopens a board.
    for board in ["Complete", "Hold", "Planned", "Batched"] {
        assert_eq!(
            pair(pool, "Abandoned", board).await,
            ("Cancelled".into(), None, None),
            "{board}"
        );
    }

    // `Error` / `Cancelled` hold a board that expects a run and leave any other board alone.
    for outcome in ["Error", "Cancelled"] {
        for board in ["Ready", "In Progress"] {
            assert_eq!(
                pair(pool, outcome, board).await,
                (outcome.into(), Some("Hold".into()), None),
                "{outcome} over {board}"
            );
        }
        for board in ["Complete", "Hold", "Planned", "Batched"] {
            assert_eq!(
                pair(pool, outcome, board).await,
                (outcome.into(), None, None),
                "{outcome} over {board}"
            );
        }
    }

    // An outcome the engine does not have is an error, never a default.
    assert!(
        sqlx::query("select * from forge_settlement_pair('Maybe', 'Ready')")
            .execute(pool)
            .await
            .is_err()
    );
}

/// Dispatch refuses rather than shrugs (migration 265): a story whose change into `Ready` creates no item means the
/// trigger this deployment relies on is missing. Proven inside a transaction that is always rolled back — the
/// trigger is disabled only there — so DEV never runs without it.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn dispatch_without_its_trigger_is_a_schema_mismatch_and_moves_nothing() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let story = format!("ENG-PROOF-NOTRIGGER-{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Dispatch trigger proof', 'High', 'Planned', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("alter table storyboard_story disable trigger storyboard_story_ready_dispatch")
        .execute(&mut *tx)
        .await
        .unwrap();
    let refused = sqlx::query("select * from forge_dispatch_story($1)")
        .bind(&story)
        .execute(&mut *tx)
        .await
        .expect_err("no item after the change is refused");
    tx.rollback().await.unwrap();

    let code = match &refused {
        sqlx::Error::Database(error) => error.code().map(|code| code.to_string()),
        _ => None,
    };
    let status: String = sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(&story)
        .fetch_one(pool)
        .await
        .unwrap();
    let enabled: String = sqlx::query_scalar(
        "select tgenabled::text from pg_trigger where tgname = 'storyboard_story_ready_dispatch'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    cleanup_story(pool, &story).await;
    assert_eq!(code.as_deref(), Some("42704"), "{refused}");
    assert!(
        refused.to_string().contains("created no work item"),
        "{refused}"
    );
    assert_eq!(
        status, "Planned",
        "the refused change into Ready did not stick"
    );
    assert_ne!(enabled, "D", "the trigger is enabled again on DEV");
}

/// One stale engine claim recovered (migration 266), proven inside a transaction that is always rolled back: the
/// DAO's sweep would also recover genuine DEV claims, so the routine is driven directly on fixture rows that never
/// commit. A stale claim is interrupted and its item released; a fresh one, and a second pass, write nothing.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_stale_engine_claim_is_recovered_once_and_a_fresh_one_never() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let mut tx = database.pool().begin().await.unwrap();
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let definition: String =
        sqlx::query_scalar("select id::text from process_definitions order by id limit 1")
            .fetch_one(&mut *tx)
            .await
            .expect("DEV holds a process definition");

    // A story with one running claim and one engine execution, whose heartbeat is `age` old.
    async fn claim(
        tx: &mut sqlx::PgConnection,
        definition: &str,
        story: &str,
        age_minutes: i32,
    ) -> (String, String, String) {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes)
             values ($1, 'PROOF', 'Stale engine claim proof', 'High', 'In Progress', '')",
        )
        .bind(story)
        .execute(&mut *tx)
        .await
        .unwrap();
        let item: String = sqlx::query_scalar(
            "insert into agent_work_item (story_id, state, priority, claimed_by, claimed_at)
             values ($1, 'Running', 0, 'stale-worker', now()) returning id::text",
        )
        .bind(story)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        let instance: String = sqlx::query_scalar(
            "insert into process_instances (definition_id, status) values ($1::uuid, 'active') returning id::text",
        )
        .bind(definition)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        let token: String = sqlx::query_scalar(
            "insert into tokens (process_instance_id, node_id) values ($1::uuid, 'smith') returning id::text",
        )
        .bind(&instance)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        let task: String = sqlx::query_scalar(
            "insert into tasks (process_instance_id, token_id, name)
             values ($1::uuid, $2::uuid, 'smith') returning id::text",
        )
        .bind(&instance)
        .bind(&token)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        sqlx::query(
            "insert into forge_engine_task_execution
                 (task_id, process_instance_id, token_id, story_id, node_id, work_item_id, worker_id, status,
                  heartbeat_at)
             values ($1::uuid, $2::uuid, $3::uuid, $4, 'smith', $5::uuid, 'stale-worker', 'running',
                     now() - make_interval(mins => $6))",
        )
        .bind(&task)
        .bind(&instance)
        .bind(&token)
        .bind(story)
        .bind(&item)
        .bind(age_minutes)
        .execute(&mut *tx)
        .await
        .unwrap();
        (task, instance, item)
    }
    async fn recover(tx: &mut sqlx::PgConnection, ids: &(String, String, String)) -> bool {
        sqlx::query_scalar(
            "select forge_recover_stale_engine_claim($1::uuid, $2::uuid, $3::uuid, 10)",
        )
        .bind(&ids.0)
        .bind(&ids.1)
        .bind(&ids.2)
        .fetch_one(&mut *tx)
        .await
        .unwrap()
    }
    async fn shape(
        tx: &mut sqlx::PgConnection,
        ids: &(String, String, String),
    ) -> (
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    ) {
        sqlx::query_as(
            "select e.status, e.last_error, i.state, i.claimed_by, i.error_text
               from forge_engine_task_execution e join agent_work_item i on i.id = e.work_item_id
              where e.task_id = $1::uuid",
        )
        .bind(&ids.0)
        .fetch_one(&mut *tx)
        .await
        .unwrap()
    }

    let stale = claim(&mut tx, &definition, &format!("ENG-PROOF-STALE-{tag}"), 30).await;
    let fresh = claim(&mut tx, &definition, &format!("ENG-PROOF-FRESH-{tag}"), 0).await;

    assert!(
        recover(&mut tx, &stale).await,
        "a claim past the cutoff is recovered"
    );
    assert_eq!(
        shape(&mut tx, &stale).await,
        (
            "interrupted".into(),
            Some("stale claim recovered".into()),
            "Ready".into(),
            None,
            Some("stale claim recovered; awaiting fresh attempt".into())
        )
    );
    assert!(
        !recover(&mut tx, &stale).await,
        "a recovered claim is not recovered twice"
    );

    assert!(
        !recover(&mut tx, &fresh).await,
        "a live heartbeat is left alone"
    );
    assert_eq!(
        shape(&mut tx, &fresh).await,
        (
            "running".into(),
            None,
            "Running".into(),
            Some("stale-worker".into()),
            None
        ),
        "and nothing about it was written"
    );

    tx.rollback().await.unwrap();
}

/// The artifact verdict rule, against DEV: the case table the deleted Rust `artifact_verdict_for_run` /
/// `verdict_polarity` unit tests held (the legacy "agrees by polarity, not by spelling" contract), asserted against
/// migration 267's functions word for word.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_artifact_verdict_rule_is_the_databases() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    async fn kept(
        pool: &sqlx::PgPool,
        kind: &str,
        ruling: Option<&str>,
        verdict: Option<&str>,
    ) -> Option<String> {
        sqlx::query_scalar("select forge_artifact_verdict_for_run($1, $2, $3)")
            .bind(kind)
            .bind(ruling)
            .bind(verdict)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    async fn polarity(pool: &sqlx::PgPool, token: &str) -> Option<String> {
        sqlx::query_scalar("select forge_verdict_polarity($1)")
            .bind(token)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    // The guard agrees by polarity, not by spelling.
    assert_eq!(
        kept(pool, "run-verdict", Some("Complete"), Some("PASS"))
            .await
            .as_deref(),
        Some("PASS")
    );
    assert_eq!(
        kept(pool, "run-verdict", Some("Hold"), Some("Failed"))
            .await
            .as_deref(),
        Some("Failed")
    );
    assert_eq!(
        kept(pool, "run-verdict", Some("Complete"), Some("Hold")).await,
        None,
        "no contradiction"
    );
    assert_eq!(
        kept(pool, "run-verdict", None, Some("Failed")).await,
        None,
        "an unruled run certifies nothing"
    );
    assert_eq!(
        kept(pool, "qa-assay-evidence", None, Some("PASS"))
            .await
            .as_deref(),
        Some("PASS"),
        "an assay's own reading is a measurement, not a claim about the run"
    );

    // A reading that cannot be compared is not kept.
    for ruling in ["Partial", "Deferred", "Cancelled", "Interrupted", ""] {
        assert_eq!(
            kept(pool, "run-verdict", Some(ruling), Some("PASS")).await,
            None,
            "{ruling:?}"
        );
    }
    assert_eq!(
        kept(pool, "run-verdict", Some("Complete"), Some("maybe")).await,
        None
    );
    assert_eq!(
        kept(pool, "run-verdict", Some("Complete"), None).await,
        None
    );
    assert_eq!(kept(pool, "qa-assay-evidence", None, None).await, None);
    assert_eq!(
        kept(pool, " Run-Verdict ", Some("Complete"), Some("Hold")).await,
        None,
        "the kind is compared trimmed and case-blind"
    );

    // Polarity words, both spellings, and the misspelling that must not pass.
    assert_eq!(polarity(pool, " passed ").await.as_deref(), Some("Affirm"));
    assert_eq!(polarity(pool, "COMPLETE").await.as_deref(), Some("Affirm"));
    assert_eq!(polarity(pool, "fail").await.as_deref(), Some("Negative"));
    assert_eq!(polarity(pool, "HOLD").await.as_deref(), Some("Negative"));
    assert_eq!(polarity(pool, "mostly fine").await, None);
}

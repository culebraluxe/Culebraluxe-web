//! WHEN A STORY LEAVES `Batched`, ITS STAGED BATCH ROW GOES WITH IT — and a flight being fired is not mistaken
//! for a departure.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test forge_staged_item_follows_story_status_dev -- --ignored
//!
//! Why this exists: on 2026-10-04 production reported `board vs table: DRIFT (board 0 vs table 1)`. The staging batch
//! `9f80316c` held one `learn` item (`LEARN-SWALLOWED-CATCH-E2E-PORTAL-NAV-SMOKE-MJS-1790666636`) whose story
//! `forge reset` had already returned to `Planned` (`db/src/forge_reset.rs:111`). "Staged in the next flight" lives in
//! two places — `storyboard_story.status = 'Batched'` (what the board reads) and a `forge_batch_item` row with
//! `state = 'Staged'` (what `tech.batch_select()` counts) — and the reset path carried only one of them. Migration 268
//! puts the rule on the status column instead of in the sixth caller: a DEPARTURE from `Batched` withdraws the staged
//! membership, judged at commit.
//!
//! A unit test cannot see any of this. The rule is a deferred constraint trigger, and its deferral is the whole
//! hazard in BOTH launchers. `tech.launch_flight` writes members `Ready` (selecting them from items that are still
//! `Staged`) before it queues those items, so a check that read that interval would delete the flight it was
//! launching — it commits its four writes together. `forge_control.fire_flight`, the path the engine worker's
//! `fire_due_flights` drives for a scheduled night flight, queues each member's item BEFORE that member's story
//! leaves `Batched`, in one transaction per member, for the same reason. Only a real database holds a real trigger, a
//! real commit boundary and the real fire/queue/route statements — those last three are the production constants
//! (`db::LAUNCH_FLIGHT_*`), not copies, and `fire_flight` is called here as the real function.
//!
//! DEV's Forge data is disposable and every row this proof creates is deleted at the end (items and work items
//! cascade with their story, items with their batch).

use db::{
    Database, DbTarget, ForgeControlDao, LAUNCH_FLIGHT_FIRE_STORIES_SQL,
    LAUNCH_FLIGHT_QUEUE_ITEMS_SQL, LAUNCH_FLIGHT_ROUTE_ITEMS_SQL,
};

/// A batch and a story wearing a given pairing, as the writers under test leave them.
struct Fixture {
    story: String,
    batch: String,
}

async fn fixture(
    pool: &sqlx::PgPool,
    story_status: &str,
    batch_status: &str,
    item_state: &str,
) -> Fixture {
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-STAGED-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Staged membership proof', 'High', $2, '')",
    )
    .bind(&story)
    .bind(story_status)
    .execute(pool)
    .await
    .expect("insert proof story");
    let batch: String = sqlx::query_scalar(
        "insert into forge_batch (label, status, note) values ($1::text, $2::text, 'contract proof')
         returning id::text",
    )
    .bind(format!("eng-proof-staged-{tag}"))
    .bind(batch_status)
    .fetch_one(pool)
    .await
    .expect("insert proof batch");
    sqlx::query("insert into forge_batch_item (batch_id, story_id, state, kind) values ($1::uuid, $2, $3, 'fix')")
        .bind(&batch)
        .bind(&story)
        .bind(item_state)
        .execute(pool)
        .await
        .expect("stage the proof story");
    Fixture { story, batch }
}

/// The staged membership of `story` in a batch that has not fired — the fact the board and the batch table must agree
/// on (`tech.batch_select()`'s `story_count` counts exactly this).
async fn live_staged_items(pool: &sqlx::PgPool, story: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "select i.state, b.status from forge_batch_item i join forge_batch b on b.id = i.batch_id
          where i.story_id = $1 and i.state = 'Staged'
            and b.status in ('Staged', 'Scheduled')",
    )
    .bind(story)
    .fetch_all(pool)
    .await
    .expect("read the live staged membership")
}

async fn item_states(pool: &sqlx::PgPool, story: &str) -> Vec<String> {
    sqlx::query_scalar("select state from forge_batch_item where story_id = $1 order by state")
        .bind(story)
        .fetch_all(pool)
        .await
        .expect("read the batch items")
}

async fn story_status(pool: &sqlx::PgPool, story: &str) -> String {
    sqlx::query_scalar("select status from storyboard_story where id = $1")
        .bind(story)
        .fetch_one(pool)
        .await
        .expect("read the story status")
}

async fn cleanup(pool: &sqlx::PgPool, fixture: &Fixture) {
    sqlx::query("delete from forge_batch where id = $1::uuid")
        .bind(&fixture.batch)
        .execute(pool)
        .await
        .expect("delete the proof batch");
    sqlx::query("delete from storyboard_story where id = $1")
        .bind(&fixture.story)
        .execute(pool)
        .await
        .expect("delete the proof story");
    assert_eq!(
        item_states(pool, &fixture.story).await.len(),
        0,
        "the proof cleaned up after itself (items cascade with the batch and the story)"
    );
    let work_items: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id = $1")
            .bind(&fixture.story)
            .fetch_one(pool)
            .await
            .expect("read the work items the dispatch trigger created");
    assert_eq!(
        work_items, 0,
        "and nothing is left in the dispatch table either: a stray queued item is what `forge doctor` reads"
    );
}

/// A proof that dies mid-run must not leave a drift of its own behind: a `Batched` story whose staged item was never
/// created is exactly the contradiction this file tests for, and the first run of this proof left three of those on DEV
/// (2026-10-04). So every test sweeps what an earlier, interrupted run left — rows carrying this proof's own prefixes
/// and older than a minute, so a run that is still in flight is never touched.
async fn sweep_leftovers(pool: &sqlx::PgPool) {
    let batches = sqlx::query(
        "delete from forge_batch where label like 'eng-proof-staged-%' and created_at < now() - interval '1 minute'",
    )
    .execute(pool)
    .await
    .expect("sweep proof batches")
    .rows_affected();
    let stories = sqlx::query(
        "delete from storyboard_story where id like 'ENG-PROOF-STAGED-%' and created_at < now() - interval '1 minute'",
    )
    .execute(pool)
    .await
    .expect("sweep proof stories")
    .rows_affected();
    if batches > 0 || stories > 0 {
        eprintln!(
            "swept {batches} batch(es) and {stories} story(ies) left by an earlier proof run"
        );
    }
}

/// The production drift, reproduced and repaired by the rule: the learn item was staged (`state='Staged'`), its story
/// was `Batched`, and `forge reset` returned the story to `Planned` while the batch row stayed behind.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_story_leaving_batched_withdraws_its_staged_membership() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    sweep_leftovers(pool).await;
    let proof = fixture(pool, "Batched", "Staged", "Staged").await;

    assert_eq!(
        live_staged_items(pool, &proof.story).await,
        vec![("Staged".to_string(), "Staged".to_string())],
        "the board and the batch table start out agreeing: one live staged member"
    );

    // The reset path's own statement (`db/src/forge_reset.rs:111`), committed on its own -- exactly how a hand-run
    // `update` would commit it. Nothing else of the batch is touched by that caller, which is how the drift arose.
    sqlx::query("update storyboard_story set status = 'Planned', completion = 0 where id = $1")
        .bind(&proof.story)
        .execute(pool)
        .await
        .expect("return the proof story to Planned, as `forge reset` does");

    assert_eq!(story_status(pool, &proof.story).await, "Planned");
    assert!(
        live_staged_items(pool, &proof.story).await.is_empty(),
        "a story that is not `Batched` holds no staged membership: board 0, batch table 0"
    );
    assert!(
        item_states(pool, &proof.story).await.is_empty(),
        "and the stale row is gone, not merely uncounted"
    );

    cleanup(pool, &proof).await;
}

/// History is not a departure. A batch that has fired (or was cancelled) keeps every row it has, whatever state its
/// items are in -- the batch screen reads its members back from there, and a later status change to a member is not a
/// reason to lose the record of what flew.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_fired_batch_keeps_its_rows_when_a_member_moves_on() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    sweep_leftovers(pool).await;
    let fired = fixture(pool, "Ready", "Fired", "Staged").await;

    sqlx::query("update storyboard_story set status = 'Planned' where id = $1")
        .bind(&fired.story)
        .execute(pool)
        .await
        .expect("move the fired batch's member on");

    assert_eq!(
        item_states(pool, &fired.story).await,
        vec!["Staged".to_string()],
        "a fired batch is history: its row survives the member's later status"
    );
    assert!(
        live_staged_items(pool, &fired.story).await.is_empty(),
        "and it was never a live staged member -- that batch had already fired"
    );

    cleanup(pool, &fired).await;
}

/// The hazard the deferral exists for: the real fire/queue/route statements, in one transaction, exactly as
/// `tech.launch_flight` runs them. The fire step writes a member `Ready` while its item is still `Staged` -- selecting
/// that member FROM that item -- so a check that read the interval would delete the flight mid-launch.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_real_launch_sequence_queues_its_member_instead_of_withdrawing_it() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    sweep_leftovers(pool).await;
    let proof = fixture(pool, "Batched", "Staged", "Staged").await;

    // `TechCockpitDao::launch_flight`'s four writes, in its order and in one transaction, bound to this proof's own
    // batch -- so no other staged story on DEV is fired by the proof.
    let mut tx = database
        .begin("proof.launch_flight")
        .await
        .expect("open the proof transaction");
    sqlx::query(LAUNCH_FLIGHT_FIRE_STORIES_SQL)
        .bind(&proof.batch)
        .execute(tx.connection())
        .await
        .expect("fire the proof batch's stories");
    let queued = sqlx::query(LAUNCH_FLIGHT_QUEUE_ITEMS_SQL)
        .bind(&proof.batch)
        .execute(tx.connection())
        .await
        .expect("queue the proof batch's items")
        .rows_affected();
    let stamped = sqlx::query(LAUNCH_FLIGHT_ROUTE_ITEMS_SQL)
        .bind(&proof.batch)
        .bind("cheap")
        .execute(tx.connection())
        .await
        .expect("route the proof batch's items")
        .rows_affected();
    sqlx::query("update forge_batch set status='Fired', fired_at=now() where id=$1::uuid and status<>'Fired'")
        .bind(&proof.batch)
        .execute(tx.connection())
        .await
        .expect("fire the proof batch");
    tx.commit().await.expect("commit the proof flight");

    assert_eq!(queued, 1, "the flight queued its one member");
    assert_eq!(
        stamped, 1,
        "and routed the work item the dispatch trigger created for it -- evidence the fire step really ran"
    );
    assert_eq!(story_status(pool, &proof.story).await, "Ready");
    assert_eq!(
        item_states(pool, &proof.story).await,
        vec!["Queued".to_string()],
        "the member is Queued, not withdrawn: the deferred check judged the commit, not the launch's own interval"
    );
    assert!(
        live_staged_items(pool, &proof.story).await.is_empty(),
        "and the live staged membership is empty because that batch has fired"
    );

    cleanup(pool, &proof).await;
}

/// The SECOND launcher, and the one a scheduled night flight takes: `forge_control.fire_flight`, the function
/// `engine::worker::fire_due_flights` calls for every `Scheduled` batch whose hour has come. It is a different shape
/// from `launch_flight` — one member at a time, so one poison member is marked `Skipped` instead of failing the whole
/// flight — which is exactly why it needs its own proof: every write in it is the real function, not a copy of it,
/// and a story-first order there would withdraw the member the flight was queuing.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_scheduled_flight_path_queues_its_member_without_withdrawing_it() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    sweep_leftovers(pool).await;
    let proof = fixture(pool, "Batched", "Scheduled", "Staged").await;

    let flight = ForgeControlDao::new(database.clone())
        .fire_flight(&proof.batch)
        .await
        .expect("fire the scheduled proof flight");

    assert_eq!(flight.queued, 1, "the flight queued its one member");
    assert_eq!(flight.skipped, 0, "and nothing about that member failed");
    assert_eq!(
        flight.stamped, 1,
        "and routed the work item the dispatch trigger created for it — evidence the fire step really ran"
    );
    assert_eq!(story_status(pool, &proof.story).await, "Ready");
    assert_eq!(
        item_states(pool, &proof.story).await,
        vec!["Queued".to_string()],
        "the member is Queued, not withdrawn: its item moved before its story left `Batched`, and the deferred check \
         judged that commit — a story-first order would have deleted this row while the flight still counted it"
    );
    assert!(
        live_staged_items(pool, &proof.story).await.is_empty(),
        "and it is no longer a live staged member: that batch has fired"
    );
    let batch_status: String =
        sqlx::query_scalar("select status from forge_batch where id = $1::uuid")
            .bind(&proof.batch)
            .fetch_one(pool)
            .await
            .expect("read the proof batch status");
    assert_eq!(
        batch_status, "Fired",
        "the scheduled flight marked itself fired"
    );

    cleanup(pool, &proof).await;
}

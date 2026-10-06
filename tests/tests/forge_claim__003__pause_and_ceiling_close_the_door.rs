//! FORGE.CLAIM — a paused door claims nothing and the ceiling caps what a pass may hold (TST-FORGE-CLAIM-003).
//!
//! Contract: `forge_claim_story(worker_id, max)` (migration 275) is the claim door, and it obeys two facts that live
//! in `forge_runtime_control` (274) rather than in the caller's environment. While `paused` is true it claims
//! nothing, and it never hands out more than `global_story_concurrency` minus the stories already in flight. Both are
//! read under the claim mutex (`pg_advisory_xact_lock(9000212)`), so two workers cannot both read one free slot and
//! both spend it. The old door, `forge_claim_next_agent_work` (262), is a delegation to it: one implementation, so
//! the two names cannot drift.
//!
//! Why this is proven against a database and not in a unit test: the brake is a row read inside the claim's own
//! transaction, and the ceiling is a count of in-flight stories committed by other workers. Neither exists outside
//! Postgres. The production `ForgeEngineDao` is driven through the `ForgeHarness` against the disposable DEV branch;
//! the claim assertions go through the production Rust path (`claim_next_agent_work` -> `forge_claim_story`), and the
//! brake assertions run inside a transaction this test ROLLS BACK — so the pause is never left on for the rest of the
//! suite and no other test's claim is refused by our proof.
//!
//! Level: L2 Persistence, harness `ForgeHarness`. PROD is refused by the harness before any socket is opened.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a; cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_claim__003__pause_and_ceiling_close_the_door -- --ignored

use sqlx::PgPool;
use test_harness::ForgeHarness;

use db::DbTarget;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
/// The worker identity that takes every claim in this proof.
const WORKER: &str = "forge-claim-003:a";
/// The brake's author, as every writer of the control row must name itself.
const BRAKE_AUTHOR: &str = "forge-claim-003";

fn story_id(tag: &str, lane: &str) -> String {
    format!("TST-FORGE-CLAIM-003-{lane}-{tag}")
}

/// Seed a story at `Planned` and queue its door row through the production enqueue door.
async fn seed_planned_with_door(pool: &PgPool, story_id: &str) -> String {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'FORGE-CLAIM-CONTRACT', 'claim brake fixture', 'High', 'Planned', '')",
    )
    .bind(story_id)
    .execute(pool)
    .await
    .expect("insert the Planned fixture story");

    // priority 1 puts this row in front of every default-priority row, so the arm below is about THIS row.
    sqlx::query_scalar(
        "select forge_enqueue_work('claim-003-fixture', $1, 'maestro', 'smoke-agent', 1)::text",
    )
    .bind(story_id)
    .fetch_one(pool)
    .await
    .expect("the production enqueue door must queue the fixture")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-CLAIM-003); the file and the assay use it.
async fn forge_claim_003__pause_and_ceiling_close_the_door() {
    // 0. A disposable DEV target, and only a DEV target. `connect_from_env` resolves the declaration exactly as
    //    production does and refuses PRODUCTION before any socket is opened, so this cannot be pointed at PROD by a
    //    stray VERCEL_ENV/APP_ENV.
    let harness = ForgeHarness::connect_from_env()
        .await
        .expect("a disposable DEV database must be declared (DATABASE_URL_DEV, APP_ENV dev/test)");
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the claim brake is proven on DEV only; PROD is forbidden"
    );

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story_a = story_id(&tag, "A");
    let story_b = story_id(&tag, "B");
    let story_c = story_id(&tag, "C");

    // 1. Two Ready stories (the board's own trigger queues exactly one item each) and one Planned story with a door
    //    row waiting to be armed.
    let item_a = harness
        .seed_ready_story(&story_a)
        .await
        .expect("the Ready trigger must queue exactly one item for story A");
    let item_b = harness
        .seed_ready_story(&story_b)
        .await
        .expect("the Ready trigger must queue exactly one item for story B");
    let door_c = seed_planned_with_door(harness.pool(), &story_c).await;

    // 1b. THE CONTROL ROW IS A SINGLETON AND DEV IS SHARED. Every other contract test claims against this same
    //     database, several of them hold a claim right now, and the ceiling counts all of them - so a proof that
    //     needs a free slot must say what it borrowed and put it back. What this proof found is restored in step 7.
    let (prior_paused, prior_ceiling, prior_by): (bool, i32, String) = sqlx::query_as(
        "select paused, global_story_concurrency, updated_by from forge_runtime_control where id = 1",
    )
    .fetch_one(harness.pool())
    .await
    .expect("the control row exists and is readable");
    sqlx::query(
        "update forge_runtime_control set paused = false, global_story_concurrency = 64,
                updated_at = now(), updated_by = $1 where id = 1",
    )
    .bind(BRAKE_AUTHOR)
    .execute(harness.pool())
    .await
    .expect("the proof opens a slot for itself");

    // 2. Claim order stated rather than hoped for, as this suite already does: DEV may hold other claimable work, so
    //    both proof items are lifted above every score the board can produce, A ahead of B. A claim below can then
    //    only be about these stories, and no borrowed row has to be put back.
    for (story, priority) in [(&story_a, 2_000_002i32), (&story_b, 2_000_001i32)] {
        let lifted = sqlx::query(
            "update agent_work_item set priority = $2 where story_id = $1 and state = 'Ready'",
        )
        .bind(story)
        .bind(priority)
        .execute(harness.pool())
        .await
        .expect("lift the proof item above the rest of the queue")
        .rows_affected();
        assert_eq!(
            lifted, 1,
            "{HARNESS}: the board's trigger must have queued exactly one item for {story}"
        );
    }

    // 3. THE DOOR IS OPEN when nothing is paused. This is the production Rust path - `claim_next_agent_work` now
    //    reaches `forge_claim_story` - and the claim is read back on the pool, so the cutover is proven by what
    //    committed, not by what the call returned.
    let claimed_a = harness
        .engine()
        .claim_next_agent_work(WORKER)
        .await
        .expect("a claim is not an error")
        .expect("story A is Ready and nothing is paused; the door must open");
    assert_eq!(
        claimed_a.story_id, story_a,
        "{HARNESS}: the highest-priority ready story claims first"
    );
    assert_eq!(claimed_a.state, "Claimed");
    assert_eq!(claimed_a.claimed_by.as_deref(), Some(WORKER));
    assert_eq!(claimed_a.id, item_a, "{HARNESS}: story A's own item");
    let (a_state, a_by): (String, Option<String>) =
        sqlx::query_as("select state, claimed_by from agent_work_item where id = $1::uuid")
            .bind(&item_a)
            .fetch_one(harness.pool())
            .await
            .expect("story A's item is readable");
    assert_eq!(
        (a_state.as_str(), a_by.as_deref()),
        ("Claimed", Some(WORKER)),
        "{HARNESS}: the claim committed with the worker that took it"
    );

    // 4. THE BRAKE. The control row is written inside a transaction this test rolls back at the end of the step, so
    //    the pause is invisible to every other session and is never left on for the rest of the suite - which is what
    //    a proof of a global switch has to do if it is not to refuse other tests' claims while it runs.
    let mut tx = harness
        .pool()
        .begin()
        .await
        .expect("the brake proof runs in its own transaction");

    sqlx::query(
        "update forge_runtime_control set paused = true, updated_at = now(), updated_by = $1 where id = 1",
    )
    .bind(BRAKE_AUTHOR)
    .execute(&mut *tx)
    .await
    .expect("the control row must accept the brake");

    let claims_while_paused: i64 =
        sqlx::query_scalar("select count(*) from forge_claim_story($1::text, 1)")
            .bind(WORKER)
            .fetch_one(&mut *tx)
            .await
            .expect("a paused door is a refusal, not an error");
    assert_eq!(
        claims_while_paused, 0,
        "{HARNESS}: a paused door must claim nothing, whatever is Ready"
    );
    let b_state: String =
        sqlx::query_scalar("select state from agent_work_item where id = $1::uuid")
            .bind(&item_b)
            .fetch_one(&mut *tx)
            .await
            .expect("story B's item is readable");
    assert_eq!(
        b_state, "Ready",
        "{HARNESS}: the refused claim left story B's item untouched"
    );

    //    The brake is a row the worker can read for itself: the DAO selects exactly these three facts, so a pass that
    //    reports "no work" while this says paused is the bug the brake exists to make impossible.
    let (paused, ceiling, held_by): (bool, i32, String) = sqlx::query_as(
        "select paused, global_story_concurrency, updated_by from forge_runtime_control where id = 1",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("the control row the DAO reads is there");
    assert!(paused, "{HARNESS}: the brake is the row the worker reads");
    assert_eq!(held_by, BRAKE_AUTHOR);
    assert!(ceiling >= 1, "{HARNESS}: the ceiling is a readable fact");

    // 4b. THE CEILING, a second and independent refusal. The brake is lifted inside the proof, so the only thing
    //     refusing below is the ceiling: story A is in flight, and a ceiling of 1 leaves no slot for story B.
    sqlx::query(
        "update forge_runtime_control set paused = false, global_story_concurrency = 1 where id = 1",
    )
    .execute(&mut *tx)
    .await
    .expect("the brake is lifted inside the proof");
    let claims_at_ceiling: i64 =
        sqlx::query_scalar("select count(*) from forge_claim_story($1::text, 1)")
            .bind(WORKER)
            .fetch_one(&mut *tx)
            .await
            .expect("a full ceiling is a refusal, not an error");
    assert_eq!(
        claims_at_ceiling, 0,
        "{HARNESS}: one story in flight against a ceiling of 1 must leave no slot"
    );

    //     Raise the ceiling and the same call claims story B: the refusal above was the ceiling, not an empty queue.
    sqlx::query("update forge_runtime_control set global_story_concurrency = 64 where id = 1")
        .execute(&mut *tx)
        .await
        .expect("the ceiling can be raised");
    let claims_under_ceiling: i64 =
        sqlx::query_scalar("select count(*) from forge_claim_story($1::text, 1)")
            .bind(WORKER)
            .fetch_one(&mut *tx)
            .await
            .expect("a claim under a raised ceiling is a claim");
    assert_eq!(
        claims_under_ceiling, 1,
        "{HARNESS}: with a free slot the door must open and claim the next story"
    );
    let (b_claimed_by, b_claimed_state): (Option<String>, String) =
        sqlx::query_as("select claimed_by, state from agent_work_item where id = $1::uuid")
            .bind(&item_b)
            .fetch_one(&mut *tx)
            .await
            .expect("story B's item is readable");
    assert_eq!(
        (b_claimed_by.as_deref(), b_claimed_state.as_str()),
        (Some(WORKER), "Claimed"),
        "{HARNESS}: the claim names the worker that took it"
    );

    tx.rollback()
        .await
        .expect("the brake must not outlive the proof");

    // 5. ONE IMPLEMENTATION, TWO NAMES. `forge_claim_next_agent_work` is a delegation, so a caller that still uses
    //    262's name reaches the same mutex, brake and ceiling. A second body here would be a second door.
    let old_door: String = sqlx::query_scalar(
        "select prosrc from pg_proc where proname = 'forge_claim_next_agent_work'",
    )
    .fetch_one(harness.pool())
    .await
    .expect("the 262 door still exists for its callers");
    assert!(
        old_door.contains("forge_claim_story"),
        "{HARNESS}: 262's door must delegate to the claim door, not re-implement it"
    );
    let claim_door: String =
        sqlx::query_scalar("select prosrc from pg_proc where proname = 'forge_claim_story'")
            .fetch_one(harness.pool())
            .await
            .expect("the claim door exists");
    for required in [
        "forge_runtime_control",
        "global_story_concurrency",
        "pg_advisory_xact_lock(9000212)",
    ] {
        assert!(
            claim_door.contains(required),
            "{HARNESS}: the claim door must read {required}"
        );
    }

    // 6. THE SECOND DOOR: the arm is refused by the same brake, in a transaction that is rolled back too. A paused
    //    arm moves no Pending row; releasing the brake arms the fixture, which is what makes the refusal mean "the
    //    brake refused" rather than "nothing was claimable anyway".
    let mut tx = harness
        .pool()
        .begin()
        .await
        .expect("the arm proof runs in its own transaction");
    sqlx::query("update forge_runtime_control set paused = true, updated_by = $1 where id = 1")
        .bind(BRAKE_AUTHOR)
        .execute(&mut *tx)
        .await
        .expect("the brake is re-applied inside the proof");
    let armed_while_paused: i64 =
        sqlx::query_scalar("select forge_arm_work_queue(64, 'forge-claim-003')")
            .fetch_one(&mut *tx)
            .await
            .expect("a paused arm is a refusal, not an error");
    assert_eq!(
        armed_while_paused, 0,
        "{HARNESS}: a paused arm must move no Pending row"
    );
    let c_door_paused: String =
        sqlx::query_scalar("select state from forge_work_queue where id = $1::uuid")
            .bind(&door_c)
            .fetch_one(&mut *tx)
            .await
            .expect("the fixture door row is readable");
    assert_eq!(
        c_door_paused, "Pending",
        "{HARNESS}: the refused arm left the fixture waiting"
    );

    sqlx::query("update forge_runtime_control set paused = false where id = 1")
        .execute(&mut *tx)
        .await
        .expect("the brake is released inside the proof");
    let armed_when_open: i64 =
        sqlx::query_scalar("select forge_arm_work_queue(64, 'forge-claim-003')")
            .fetch_one(&mut *tx)
            .await
            .expect("an open arm is an arm");
    assert!(
        armed_when_open >= 1,
        "{HARNESS}: with the brake released the fixture row must arm (armed {armed_when_open})"
    );
    let c_door_open: String =
        sqlx::query_scalar("select state from forge_work_queue where id = $1::uuid")
            .bind(&door_c)
            .fetch_one(&mut *tx)
            .await
            .expect("the fixture door row is readable");
    assert_eq!(
        c_door_open, "Running",
        "{HARNESS}: the armed fixture took its slot"
    );
    tx.rollback()
        .await
        .expect("the arm proof leaves nothing armed");

    // 7. Cleanup: the fixture door rows, then the three stories - their items and runs cascade, and the control row
    //    goes back to exactly what this proof found: the brake and the ceiling belong to the operator, and a test
    //    that left them changed would refuse the next run's claims.
    let stories = vec![story_a.clone(), story_b.clone(), story_c.clone()];
    sqlx::query("delete from forge_work_queue where story_id = any($1::text[])")
        .bind(stories.clone())
        .execute(harness.pool())
        .await
        .expect("the fixture door rows must be removable");
    for story in &stories {
        harness
            .cleanup_story(story)
            .await
            .expect("the proof story must be removable");
    }
    let leftovers: i64 = sqlx::query_scalar(
        "select (select count(*) from agent_work_item where story_id = any($1::text[]))
              + (select count(*) from storyboard_story_run where story_id = any($1::text[]))
              + (select count(*) from forge_work_queue where story_id = any($1::text[]))",
    )
    .bind(stories.clone())
    .fetch_one(harness.pool())
    .await
    .expect("the cleanup is measurable");
    assert_eq!(
        leftovers, 0,
        "{HARNESS}: the proof must leave no item, run or door row behind"
    );
    //    Put the control row back exactly as found, author included, so the next test inherits the operator's row.
    sqlx::query(
        "update forge_runtime_control set paused = $1, global_story_concurrency = $2,
                updated_by = $3, updated_at = now() where id = 1",
    )
    .bind(prior_paused)
    .bind(prior_ceiling)
    .bind(&prior_by)
    .execute(harness.pool())
    .await
    .expect("the borrowed control row must be put back");
    let (restored_paused, restored_ceiling, restored_by): (bool, i32, String) = sqlx::query_as(
        "select paused, global_story_concurrency, updated_by from forge_runtime_control where id = 1",
    )
    .fetch_one(harness.pool())
    .await
    .expect("the control row is readable at the end");
    assert_eq!(
        (restored_paused, restored_ceiling, restored_by.as_str()),
        (prior_paused, prior_ceiling, prior_by.as_str()),
        "{HARNESS}: the brake and the ceiling must be left exactly as this proof found them"
    );
}

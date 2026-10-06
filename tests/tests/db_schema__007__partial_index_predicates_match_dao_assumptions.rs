//! DB.SCHEMA — partial-index predicates match DAO assumptions (TST-DB-SCHEMA-007).
//!
//! CONTRACT. A partial unique index enforces only the rows its `WHERE` predicate admits,
//! so the predicate must cover EXACTLY the set of rows the DAO treats as "open". The
//! Forge control plane's per-story serial claim is the load-bearing case: migration 143
//! (`db/migrations/143_forge_dispatch_parallel_lock.sql:22-30`) defines the pair
//!
//!   agent_work_item_one_serial_active_per_story
//!     on agent_work_item (story_id)
//!     where state in ('Ready','Claimed','Running','Paused')
//!       and parallel_group_id is null
//!
//!   agent_work_item_one_parallel_slot
//!     on agent_work_item (story_id, parallel_group_id, parallel_slot)
//!     where state in ('Ready','Claimed','Running','Paused')
//!       and parallel_group_id is not null
//!
//! and the production DAO counts exactly `('Ready','Claimed','Running','Paused')` as the
//! open set it must never double-book: `db/src/forge_reset.rs:300-301`,
//! `db/src/forge_reset.rs:320-321`, `db/src/forge_reset.rs:354`, `db/src/tech.rs:331`,
//! `db/src/forge_control.rs:275`. The `agent_work_item.state` CHECK constraint
//! (`db/migrations/028_agent_command_runtime.sql:51-52`, current on DEV) partitions the
//! vocabulary into exactly those four open states plus the three terminal ones
//! `('Done','Error','Cancelled')`.
//!
//! If the index predicate were NARROWER than the DAO's open set — drop `Paused`, say — the
//! DAO's "one open item per story" assumption would silently break and two open items could
//! coexist. If it were BROADER — include `Cancelled` — a story could never be re-queued
//! after a terminal item. This test pins both predicates to the DAO's open set and proves
//! the enforcement behaviourally on the database.
//!
//! The proof reads the live schema through the production snapshot reader
//! (`db::schema_parity::read_snapshot`, the reader `pnpm db:parity` uses) and then exercises
//! the real index with writes the harness only rolls back. A fabricated narrower predicate is
//! compared first to show the check discriminates.
//!
//! Level: L2 Persistence — the production snapshot reader and driver against an isolated
//! disposable DEV/Neon target. The harness refuses PRODUCTION before any socket is opened;
//! every write runs inside a transaction the harness only knows how to roll back.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__007__partial_index_predicates_match_dao_assumptions -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a
//! disposable DEV database and the harness will never open a PRODUCTION one.

use std::collections::BTreeSet;

use db::DbTarget;
use test_harness::database::{TestDatabase, TestTransaction};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The states the production DAO treats as OPEN (a live claim the story still owns). Cited
/// above; the partial unique indexes over `agent_work_item` must cover exactly this set.
const DAO_OPEN_STATES: &[&str] = &["Ready", "Claimed", "Running", "Paused"];

/// The per-story serial claim index defined by migration 143.
const SERIAL_INDEX: &str = "agent_work_item.agent_work_item_one_serial_active_per_story";

/// The parallel-slot index defined by migration 143.
const PARALLEL_INDEX: &str = "agent_work_item.agent_work_item_one_parallel_slot";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

fn dao_open_states() -> BTreeSet<String> {
    DAO_OPEN_STATES
        .iter()
        .map(|state| (*state).to_string())
        .collect()
}

/// The single-quoted literals in a partial index's `WHERE` predicate.
///
/// Postgres renders `IN ('a','b')` as `= ANY (ARRAY['a'::text, 'b'::text])`, so the state set
/// is read from the predicate rather than matched as text — the contract is about WHICH states
/// the index admits, not how one server printed them. The value arrives already
/// whitespace-normalized by the snapshot reader; it is normalized again so the helper is
/// self-contained.
fn predicate_literals(indexdef: &str) -> BTreeSet<String> {
    let normalized = indexdef.split_whitespace().collect::<Vec<_>>().join(" ");
    let predicate = normalized
        .split_once(" WHERE ")
        .map(|(_, predicate)| predicate)
        .unwrap_or("");
    let mut literals = BTreeSet::new();
    let mut rest = predicate;
    while let Some(open) = rest.find('\'') {
        let after = &rest[open + 1..];
        match after.find('\'') {
            Some(close) => {
                literals.insert(after[..close].to_string());
                rest = &after[close + 1..];
            }
            None => break,
        }
    }
    literals
}

/// Insert a `Planned` proof story so `agent_work_item.story_id` satisfies its foreign key.
///
/// `Planned` is deliberate: the board's dispatch trigger only queues an item when a story
/// becomes `Ready`, so a `Planned` story leaves the work items to the probe.
async fn insert_proof_story(tx: &mut TestTransaction, story_id: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'PROOF', 'TST-DB-SCHEMA-007 partial-index proof', 'High', 'Planned')",
    )
    .bind(story_id)
    .execute(tx.connection())
    .await
    .expect("a Planned proof story inserts without the dispatch trigger queueing an item");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-007); the file and the assay use it.
async fn db_schema_007__partial_index_predicates_match_dao_assumptions() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the partial-index proof runs only on an isolated DEV target"
    );

    // 1. The production snapshot reader — the same reader the parity gate uses.
    let snapshot = db::schema_parity::read_snapshot(dev.database())
        .await
        .expect("the production snapshot reader reads DEV");

    // 2. Negative control: a predicate that drops an open state must read as a mismatch, so a
    //    comparison that answered "match" for everything could not pass this test.
    let narrowed = "CREATE UNIQUE INDEX probe_narrowed ON public.agent_work_item USING btree \
         (story_id) WHERE ((state = ANY (ARRAY['Ready'::text, 'Claimed'::text, \
         'Running'::text])) AND (parallel_group_id IS NULL))";
    assert_ne!(
        predicate_literals(narrowed),
        dao_open_states(),
        "{HARNESS}: dropping an open state must read as a mismatch — the comparison discriminates"
    );

    // 3. The per-story serial claim index: predicate is exactly the DAO's open set, on the serial slot.
    let serial = snapshot.indexes.get(SERIAL_INDEX).unwrap_or_else(|| {
        panic!("{HARNESS}: the per-story serial claim index {SERIAL_INDEX} must exist")
    });
    assert_eq!(
        predicate_literals(serial),
        dao_open_states(),
        "{HARNESS}: the serial index predicate must cover exactly the DAO open states; indexdef={serial}"
    );
    assert!(
        serial.contains("parallel_group_id IS NULL"),
        "{HARNESS}: the serial index must be partial on the serial slot (parallel_group_id IS NULL); indexdef={serial}"
    );

    // 4. The parallel-slot index: same open set, complementary parallel shape.
    let parallel = snapshot.indexes.get(PARALLEL_INDEX).unwrap_or_else(|| {
        panic!("{HARNESS}: the parallel-slot index {PARALLEL_INDEX} must exist")
    });
    assert_eq!(
        predicate_literals(parallel),
        dao_open_states(),
        "{HARNESS}: the parallel index predicate must cover exactly the DAO open states; indexdef={parallel}"
    );
    assert!(
        parallel.contains("parallel_group_id IS NOT NULL"),
        "{HARNESS}: the parallel index must be partial on the parallel slot (parallel_group_id IS NOT NULL); indexdef={parallel}"
    );

    // 5. Behavioural refusal: a second serial open item for one story is refused BY THIS INDEX,
    //    which can only happen if the predicate admits `Paused` (the state asserted here).
    //
    //    The story is inserted `Planned`, not `Ready`, so the board's own dispatch trigger
    //    (`agent_work_item_dispatch`, `db/migrations/025_agent_work_queue.sql:101-116`) does not
    //    queue an item of its own and this probe is about the inserts below. The whole probe is
    //    one transaction the harness rolls back, so neither the story nor its items survive.
    let refusal_story = format!("TST-DB-SCHEMA-007-{}", uuid::Uuid::new_v4());
    let mut refusal_tx = dev.begin().await.expect("begin refusal probe");
    insert_proof_story(&mut refusal_tx, &refusal_story).await;
    let first = sqlx::query(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Paused', 100)",
    )
    .bind(&refusal_story)
    .execute(refusal_tx.connection())
    .await;
    first.expect("a first open serial item must insert");
    let second = sqlx::query(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 100)",
    )
    .bind(&refusal_story)
    .execute(refusal_tx.connection())
    .await;
    let error = second.expect_err("a second open serial item for one story must be refused");
    let constraint = error
        .as_database_error()
        .and_then(|database_error| database_error.constraint())
        .unwrap_or("")
        .to_string();
    refusal_tx
        .rollback()
        .await
        .expect("roll back the refusal probe");
    assert_eq!(
        constraint, "agent_work_item_one_serial_active_per_story",
        "{HARNESS}: the refusal must be the per-story serial index, not a different failure: {error}"
    );

    // 6. Fault case: a terminal item must NOT block the next open one — the predicate must not
    //    over-cover, or a story could never be re-queued after a `Cancelled`/`Done`/`Error` item.
    let requeue_story = format!("TST-DB-SCHEMA-007-{}", uuid::Uuid::new_v4());
    let mut requeue_tx = dev.begin().await.expect("begin terminal probe");
    insert_proof_story(&mut requeue_tx, &requeue_story).await;
    sqlx::query(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Cancelled', 100)",
    )
    .bind(&requeue_story)
    .execute(requeue_tx.connection())
    .await
    .expect("a terminal item must insert");
    let requeued = sqlx::query(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 100)",
    )
    .bind(&requeue_story)
    .execute(requeue_tx.connection())
    .await;
    requeue_tx
        .rollback()
        .await
        .expect("roll back the terminal probe");
    requeued.expect("a story with only a terminal item must be re-queueable as a new open item");
}

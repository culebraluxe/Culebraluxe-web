//! CRM.COMMS — ordering (TST-CRM-COMMS-005).
//!
//! Contract: the comms timeline is ordered **newest first by `occurred_at`, with the row id as a
//! deterministic tie-break** — `order by occurred_at desc, id desc` in both production reads,
//! `CommsDao::moments` (`db/src/comms.rs:416-424`) and `CommsDao::activity`
//! (`db/src/comms.rs:280`). Two things must therefore hold, and both are asserted against
//! committed rows:
//!
//! - **Order follows the event's own time, never the order rows happened to be written.** The
//!   oldest fixture row is inserted LAST, so a reader that returned insertion order would put it
//!   at the head of the timeline and fail here.
//! - **Equal timestamps are ordered deterministically.** Two events recorded at the same instant
//!   come back in descending id order — a total order, so paging through the same rows twice can
//!   never interleave them differently (that stability is what makes pagination in TST-CRM-COMMS-006
//!   sound).
//!
//! The activity feed and the timeline are separate queries over the same rows; the test asserts
//! they agree on this run's rows, and that `limit` truncates the ORDERED sequence — a narrow
//! feed read is the head of the wide one. (The feed is global, so the truncation is asserted
//! against the feed's own order rather than this run's fixtures: shared DEV holds other
//! writers' rows, and the global head is theirs as often as ours.)
//!
//! Level: L2 Persistence — the isolated, disposable DEV/Neon target; the harness refuses
//! PRODUCTION before any socket is opened. Fixture rows live under a uniquely named person and are
//! deleted at the end with a zero-leftover assertion.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_comms__005__ordering -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use chrono::{DateTime, Utc};
use db::{CommsDao, DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::CrmHarness;
use uuid::Uuid;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "CrmHarness/L2 Persistence";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
async fn connect_dev() -> CrmHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match CrmHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; CrmHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Insert one interaction fixture row; returns its id.
async fn insert_interaction(
    pool: &PgPool,
    person_id: &str,
    channel: &str,
    occurred_at: &str,
    title: &str,
) -> Result<String, DbFailure> {
    sqlx::query_scalar(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title)
         values ($1::uuid, $2, $2, 'inbound', $3::timestamptz, $4)
         returning id::text",
    )
    .bind(person_id)
    .bind(channel)
    .bind(occurred_at)
    .bind(title)
    .fetch_one(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms005.insert", &error))
}

/// A row's uuid bytes, for the id tie-break the contract asserts.
fn uuid_bytes(id: &str) -> [u8; 16] {
    Uuid::parse_str(id)
        .unwrap_or_else(|error| panic!("{HARNESS}: timeline id {id} is a uuid ({error})"))
        .into_bytes()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-COMMS-005); the file and the assay use it.
async fn crm_comms_005__ordering() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the ordering proof runs only on an isolated DEV target"
    );
    let dao = CommsDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCOMMS005-{ns}");
    let person = harness
        .seed_person(&format!("{marker}-person"))
        .await
        .expect("the fixture person seeds");

    // Insertion order is deliberately NOT time order: two rows share one instant to exercise the
    // tie-break, and the oldest row is written LAST, so a reader returning write order would head
    // an evening timeline with the morning email.
    let tied_first = insert_interaction(
        harness.pool(),
        &person,
        "imessage",
        "2026-10-01T12:00:00+00:00",
        "first of the tie",
    )
    .await
    .expect("the first tied interaction commits");
    let tied_second = insert_interaction(
        harness.pool(),
        &person,
        "imessage",
        "2026-10-01T12:00:00+00:00",
        "second of the tie",
    )
    .await
    .expect("the second tied interaction commits");
    let newest = insert_interaction(
        harness.pool(),
        &person,
        "call",
        "2026-10-01T13:00:00+00:00",
        "afternoon call",
    )
    .await
    .expect("the newest interaction commits");
    let oldest = insert_interaction(
        harness.pool(),
        &person,
        "email",
        "2026-10-01T09:00:00+00:00",
        "morning email",
    )
    .await
    .expect("the oldest interaction commits — written last on purpose");
    let page = dao
        .moments(&person, 10, 0)
        .await
        .expect("the production moments read runs");
    assert_eq!(
        page.total, 4,
        "{HARNESS}: every committed interaction is in the timeline"
    );
    let order: Vec<&str> = page
        .moments
        .iter()
        .map(|moment| moment.id.as_str())
        .collect();
    assert_eq!(
        order.first().copied(),
        Some(newest.as_str()),
        "{HARNESS}: the newest event heads the timeline even though it was not written last"
    );
    assert_eq!(
        order.last().copied(),
        Some(oldest.as_str()),
        "{HARNESS}: the oldest event sits at the tail even though it was written last"
    );

    // Times are non-increasing down the page.
    let times: Vec<DateTime<Utc>> = page
        .moments
        .iter()
        .map(|moment| {
            moment
                .occurred_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|error| {
                    panic!(
                        "{HARNESS}: occurred_at {} is not an rfc3339 timestamp ({error})",
                        moment.occurred_at
                    )
                })
        })
        .collect();
    for pair in times.windows(2) {
        assert!(
            pair[0] >= pair[1],
            "{HARNESS}: the timeline never goes back in time: {} then {}",
            pair[0],
            pair[1]
        );
    }

    // The tie: same instant, adjacent, descending id — a total, repeatable order.
    let tied: Vec<&str> = order
        .iter()
        .copied()
        .filter(|id| *id == tied_first.as_str() || *id == tied_second.as_str())
        .collect();
    assert_eq!(tied.len(), 2, "{HARNESS}: both tied events are on the page");
    let first_pos = order
        .iter()
        .position(|id| *id == tied[0])
        .expect("tied[0] is on the page");
    let second_pos = order
        .iter()
        .position(|id| *id == tied[1])
        .expect("tied[1] is on the page");
    assert_eq!(
        second_pos,
        first_pos + 1,
        "{HARNESS}: equal timestamps are adjacent in the ordering"
    );
    assert!(
        uuid_bytes(tied[0]) > uuid_bytes(tied[1]),
        "{HARNESS}: the tie-break is id DESCENDING, got {} before {}",
        tied[0],
        tied[1]
    );
    // Repeating the read cannot shuffle the tie.
    let again = dao
        .moments(&person, 10, 0)
        .await
        .expect("the repeated production moments read runs");
    let repeat: Vec<&str> = again
        .moments
        .iter()
        .map(|moment| moment.id.as_str())
        .collect();
    assert_eq!(
        repeat, order,
        "{HARNESS}: reading the same committed rows twice returns the same order"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE SECOND READER — the activity feed orders this run's rows the same way as the
    //    timeline, and `limit` truncates the feed's own ordered sequence (see the header:
    //    the feed is global, so the head assertion is feed-against-feed).
    // -----------------------------------------------------------------------------------------------------------
    let activity = dao
        .activity(500)
        .await
        .expect("the production activity read runs");
    let feed: Vec<&str> = activity
        .iter()
        .filter(|entry| entry.person_id.as_deref() == Some(person.as_str()))
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        feed, order,
        "{HARNESS}: the activity feed and the timeline order the person's rows identically"
    );
    let narrow = dao
        .activity(3)
        .await
        .expect("the limited activity read runs");
    let narrow_ids: Vec<&str> = narrow
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    let wide_ids: Vec<&str> = activity
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        narrow_ids.len(),
        3,
        "{HARNESS}: limit=3 returns exactly three rows — this run alone committed four"
    );
    assert_eq!(
        narrow_ids,
        wide_ids[..3],
        "{HARNESS}: limit=3 takes the HEAD of the ordered feed — the limit truncates the ordered sequence"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CLEANUP / NO LEFTOVER — the person is deleted (interactions cascade) with a
    //    zero-leftover assertion, so shared DEV keeps no fixture of this run.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("the fixture person is removed");
    assert_eq!(
        removed, 1,
        "{HARNESS}: exactly this run's person is removed"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person or interaction behind"
    );
}

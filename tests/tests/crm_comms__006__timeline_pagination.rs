//! CRM.COMMS — timeline pagination (TST-CRM-COMMS-006).
//!
//! Contract: `CommsDao::moments` (`db/src/comms.rs:395-445`) pages the timeline —
//! `order by occurred_at desc, id desc limit $2 offset $3` with the total counted
//! separately (`comms.moments.count`). Five things must therefore hold, and all are
//! asserted against committed rows:
//!
//! - **Pages partition the timeline.** Page 1, 2 and 3 of size 2 over five events return
//!   `full[0..2]`, `full[2..4]` and `full[4..5]`: the union is the whole timeline in
//!   order, with no overlap and no gap. (Ordering itself is TST-CRM-COMMS-005; the
//!   fixtures here use distinct timestamps so the order is total and the pages are
//!   about slicing, not sorting.)
//! - **The total is page-independent.** Every page — including the empty ones — reports
//!   `total == 5`, so a pager can trust the count it got on page 1 on page 3.
//! - **An offset past the end is an empty page, not an error and not a wrap.**
//!   `offset 10` over five events returns no moments with the total intact.
//! - **`limit 0` is an empty page with the total intact**, not a "no limit" synonym.
//! - **Negative pagination input is refused, never silently reinterpreted.** The DAO
//!   binds `limit`/`offset` straight through (no clamp), so Postgres refuses a
//!   negative `LIMIT`/`OFFSET` and that refusal surfaces as a `DbFailure` — a caller
//!   asking for `limit -1` gets an error, not the whole timeline dumped without a
//!   bound. That refusal is the fault case that stops this test passing vacuously.
//!
//! Level: L2 Persistence — the production `CommsDao` against an isolated, disposable
//! DEV/Neon target; the harness refuses PRODUCTION before any socket is opened.
//! Fixture rows live under a uniquely named person and are deleted at the end, with a
//! zero-leftover assertion.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_comms__006__timeline_pagination -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{CommsDao, DbFailure, DbTarget};
use sqlx::PgPool;
use test_harness::CrmHarness;

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
///
/// Raw SQL is fixture setup only — the pagination under test reads the committed rows back
/// through the production DAO.
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
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms006.insert", &error))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-COMMS-006); the file and the assay use it.
async fn crm_comms_006__timeline_pagination() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the pagination proof runs only on an isolated DEV target"
    );
    let dao = CommsDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCOMMS006-{ns}");
    let person = harness
        .seed_person(&format!("{marker}-person"))
        .await
        .expect("the fixture person seeds");

    // Five events at five distinct instants, so the timeline order is total and the pages
    // below slice it rather than re-sort it. Insertion order is shuffled on purpose: the
    // middle event is written first and the newest last, so a reader returning write order
    // would fail the partition assertions in section 1.
    for (channel, occurred_at, title) in [
        ("email", "2026-10-01T11:00:00+00:00", "mid-morning email"),
        ("call", "2026-10-01T09:00:00+00:00", "morning call"),
        ("imessage", "2026-10-01T13:00:00+00:00", "afternoon message"),
        ("email", "2026-10-01T10:00:00+00:00", "late-morning email"),
        ("call", "2026-10-01T12:00:00+00:00", "midday call"),
    ] {
        insert_interaction(harness.pool(), &person, channel, occurred_at, title)
            .await
            .expect("the fixture interaction commits");
    }

    // The whole timeline, newest first, is the reference every page is sliced against.
    let full = dao
        .moments(&person, 10, 0)
        .await
        .expect("the production moments read runs");
    assert_eq!(
        full.total, 5,
        "{HARNESS}: every committed interaction is in the timeline"
    );
    assert_eq!(
        full.moments.len(),
        5,
        "{HARNESS}: a large limit returns the whole timeline"
    );
    let full_order: Vec<&str> = full
        .moments
        .iter()
        .map(|moment| moment.id.as_str())
        .collect();

    // -----------------------------------------------------------------------------------------------------------
    // 1. PAGES PARTITION THE TIMELINE — page 1/2/3 of size 2 are full[0..2], full[2..4],
    //    full[4..5]: the union is the timeline in order, with no overlap and no gap.
    // -----------------------------------------------------------------------------------------------------------
    let page1 = dao.moments(&person, 2, 0).await.expect("page 1 reads");
    let page2 = dao.moments(&person, 2, 2).await.expect("page 2 reads");
    let page3 = dao.moments(&person, 2, 4).await.expect("page 3 reads");
    let ids1: Vec<&str> = page1
        .moments
        .iter()
        .map(|moment| moment.id.as_str())
        .collect();
    let ids2: Vec<&str> = page2
        .moments
        .iter()
        .map(|moment| moment.id.as_str())
        .collect();
    let ids3: Vec<&str> = page3
        .moments
        .iter()
        .map(|moment| moment.id.as_str())
        .collect();
    assert_eq!(
        ids1,
        full_order[0..2],
        "{HARNESS}: page 1 is the head of the timeline"
    );
    assert_eq!(
        ids2,
        full_order[2..4],
        "{HARNESS}: page 2 continues exactly where page 1 stopped"
    );
    assert_eq!(
        ids3,
        full_order[4..5],
        "{HARNESS}: page 3 is the tail — a short final page, not an error"
    );
    let mut union: Vec<&str> = ids1.iter().chain(&ids2).chain(&ids3).copied().collect();
    let before_dedup = union.len();
    union.sort_unstable();
    union.dedup();
    assert_eq!(
        before_dedup,
        union.len(),
        "{HARNESS}: no event appears on two pages"
    );
    assert_eq!(
        union.len(),
        5,
        "{HARNESS}: the pages union to the whole timeline — no event is skipped"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE TOTAL IS PAGE-INDEPENDENT — every page, full or short, reports total 5.
    // -----------------------------------------------------------------------------------------------------------
    for (label, page) in [("page 1", &page1), ("page 2", &page2), ("page 3", &page3)] {
        assert_eq!(
            page.total, 5,
            "{HARNESS}: {label} reports the timeline total, not the page size"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. AN OFFSET PAST THE END IS AN EMPTY PAGE — no rows, the total intact, no error.
    // -----------------------------------------------------------------------------------------------------------
    let past_end = dao
        .moments(&person, 2, 10)
        .await
        .expect("an offset past the end reads");
    assert!(
        past_end.moments.is_empty(),
        "{HARNESS}: an offset past the end returns no moments"
    );
    assert_eq!(
        past_end.total, 5,
        "{HARNESS}: the past-the-end page still reports the timeline total"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. LIMIT 0 IS AN EMPTY PAGE — not a "no limit" synonym: no rows, the total intact.
    // -----------------------------------------------------------------------------------------------------------
    let zero = dao
        .moments(&person, 0, 0)
        .await
        .expect("a zero-limit read runs");
    assert!(
        zero.moments.is_empty(),
        "{HARNESS}: limit 0 returns no moments"
    );
    assert_eq!(
        zero.total, 5,
        "{HARNESS}: the zero-limit page still reports the timeline total"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / FAULT — a negative LIMIT or OFFSET is refused, never silently
    //    reinterpreted. The DAO binds both straight through (no clamp at
    //    `db/src/comms.rs:426-428`), so Postgres refuses them and the refusal surfaces
    //    as a `DbFailure` rather than a panic or — worse — an unbounded timeline dump.
    // -----------------------------------------------------------------------------------------------------------
    let refused_limit = dao.moments(&person, -1, 0).await;
    assert!(
        refused_limit.is_err(),
        "{HARNESS}: limit -1 must be refused, not reinterpreted as the whole timeline"
    );
    let refused_offset = dao.moments(&person, 2, -1).await;
    assert!(
        refused_offset.is_err(),
        "{HARNESS}: offset -1 must be refused, not reinterpreted as the head of the timeline"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / NO LEFTOVER — the person is deleted (interactions cascade) with a
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

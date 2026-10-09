//! CRM.CLIENT — communication panel (TST-CRM-CLIENT-007).
//!
//! Contract: the client communication panel is what `ClientService::history`
//! (`web/src/clients/mod.rs:283-338`) answers — the person's contact history over
//! `mv_client_contact_history`, burst-grouped by `build_contact_history`
//! (`middle/model/src/client.rs:430-475`), paged, with a `recent` mode. Five things
//! must therefore hold, asserted through the production service (L3 Composition):
//!
//! - **The panel shows the person's own events, newest first.** Three interactions at
//!   three distinct instants come back as three detail rows in descending `ended_at`
//!   order with `total == 3` — the count and the rows agree.
//! - **Pages partition the panel.** Page 1 and page 2 of size 2 are the head and the
//!   tail with no overlap and no gap, and both echo the requested page coordinates.
//! - **`recent` mode is a bounded head, not a page.** It forces the page size to
//!   `CLIENT_RECENT_HISTORY_LIMIT`, offsets to 0, and says so in the result.
//! - **A stranger gets an empty panel, not an error and not someone else's rows.**
//!   An unknown person reads `total == 0` with no rows — and a page past the end of a
//!   real panel is empty with the total intact.
//! - **The panel reads the production read model, rebuilt through the production
//!   seam.** Fixtures are committed to `interaction`, then `LandingDao::
//!   refresh_client_read_models` rebuilds `mv_client_contact_history` before the panel
//!   reads — the same refresh production runs after ingest (`db/src/landing.rs:348`)
//!   — and again after cleanup, so shared DEV keeps no stale projection of this run.
//!
//! Level: L3 Composition — the production client service on an isolated, disposable
//! DEV/Neon target; the harness refuses PRODUCTION before any socket is opened.
//! Fixture rows live under a run-unique marker and are deleted at the end, with a
//! zero-leftover assertion plus a post-cleanup empty-panel read.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_client__007__communication_panel -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L3 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget, LandingDao};
use model::{ClientHistoryRequest, ContactHistoryRow};
use sqlx::PgPool;
use test_harness::ClientHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ClientHarness/L3 Composition";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
async fn connect_dev() -> ClientHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ClientHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; ClientHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Insert one interaction fixture row; returns its id.
///
/// Raw SQL is fixture setup only — the panel under test reads the committed rows back
/// through the production service and its read model.
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
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client007.insert", &error))
}

/// The detail-row ids of a history result, in panel order.
fn detail_ids(rows: &[ContactHistoryRow]) -> Vec<&str> {
    rows.iter()
        .map(|row| match row {
            ContactHistoryRow::Detail(moment) => {
                assert_eq!(
                    moment.kind, "detail",
                    "{HARNESS}: the panel row is a detail moment, got {}",
                    moment.kind
                );
                moment.id.as_str()
            }
            ContactHistoryRow::Aggregate(item) => {
                panic!(
                    "{HARNESS}: the panel holds no aggregate evidence rows with no evidence seeded, got {}",
                    item.id
                )
            }
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-007); the file and the assay use it.
async fn crm_client_007__communication_panel() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the panel proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let landing = LandingDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCLIENT007-{ns}");
    let pool = harness.pool();

    let client = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'active') returning id::text",
    )
    .bind(format!("{marker}-panel-client"))
    .fetch_one(pool)
    .await
    .expect("the fixture client seeds");

    // Three single-channel events (email/call/meeting never burst-group) at three distinct
    // instants. Insertion order is shuffled so the panel must order by event time, not by
    // write order.
    let mid = insert_interaction(
        pool,
        &client,
        "email",
        "2026-10-01T11:00:00+00:00",
        "mid email",
    )
    .await
    .expect("the middle event commits");
    let oldest = insert_interaction(
        pool,
        &client,
        "call",
        "2026-10-01T09:00:00+00:00",
        "old call",
    )
    .await
    .expect("the oldest event commits");
    let newest = insert_interaction(
        pool,
        &client,
        "meeting",
        "2026-10-01T13:00:00+00:00",
        "new meeting",
    )
    .await
    .expect("the newest event commits");

    // The production read-model rebuild — the same seam production runs after ingest.
    landing
        .refresh_client_read_models()
        .await
        .expect("the production read models rebuild before the panel reads");

    let request = |page: i64, page_size: i64, recent: bool| ClientHistoryRequest {
        person_id: client.clone(),
        page,
        page_size,
        recent,
    };

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE PANEL SHOWS THE PERSON'S OWN EVENTS, NEWEST FIRST — three rows, total 3.
    // -----------------------------------------------------------------------------------------------------------
    let panel = harness
        .service()
        .history(&request(1, 50, false), &ctx)
        .await
        .expect("the production panel read runs");
    assert_eq!(
        panel.total, 3,
        "{HARNESS}: the panel counts every committed event"
    );
    assert_eq!(
        panel.page, 1,
        "{HARNESS}: the panel echoes the requested page"
    );
    assert_eq!(
        panel.page_size, 50,
        "{HARNESS}: the panel echoes the requested page size"
    );
    assert!(!panel.recent, "{HARNESS}: the panel echoes non-recent mode");
    assert_eq!(
        detail_ids(&panel.rows),
        vec![newest.as_str(), mid.as_str(), oldest.as_str()],
        "{HARNESS}: the panel orders the person's events newest first, not in write order"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. PAGES PARTITION THE PANEL — page 1 is the head, page 2 the tail, no overlap, no gap.
    // -----------------------------------------------------------------------------------------------------------
    let head = harness
        .service()
        .history(&request(1, 2, false), &ctx)
        .await
        .expect("page 1 reads");
    let tail = harness
        .service()
        .history(&request(2, 2, false), &ctx)
        .await
        .expect("page 2 reads");
    assert_eq!(head.total, 3, "{HARNESS}: page 1 reports the panel total");
    assert_eq!(tail.total, 3, "{HARNESS}: page 2 reports the panel total");
    assert_eq!(
        detail_ids(&head.rows),
        vec![newest.as_str(), mid.as_str()],
        "{HARNESS}: page 1 is the head of the panel"
    );
    assert_eq!(
        detail_ids(&tail.rows),
        vec![oldest.as_str()],
        "{HARNESS}: page 2 is the tail — a short final page, not an error"
    );

    // A page past the end is empty with the total intact.
    let past_end = harness
        .service()
        .history(&request(5, 2, false), &ctx)
        .await
        .expect("a page past the end reads");
    assert!(
        past_end.rows.is_empty(),
        "{HARNESS}: a page past the end holds no rows"
    );
    assert_eq!(
        past_end.total, 3,
        "{HARNESS}: the past-the-end page still reports the panel total"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. RECENT MODE IS A BOUNDED HEAD — page size forced to the recent limit, offset to 0.
    // -----------------------------------------------------------------------------------------------------------
    let recent = harness
        .service()
        .history(&request(3, 2, true), &ctx)
        .await
        .expect("the recent panel reads");
    assert!(recent.recent, "{HARNESS}: the panel echoes recent mode");
    assert_eq!(
        recent.page_size,
        model::CLIENT_RECENT_HISTORY_LIMIT,
        "{HARNESS}: recent mode forces the bounded page size, ignoring the requested one"
    );
    assert_eq!(
        detail_ids(&recent.rows),
        vec![newest.as_str(), mid.as_str(), oldest.as_str()],
        "{HARNESS}: recent mode starts at the head even when asked for page 3"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — a stranger gets an empty panel, not an error and not someone else's rows.
    // -----------------------------------------------------------------------------------------------------------
    let stranger = harness
        .service()
        .history(
            &ClientHistoryRequest {
                person_id: "00000000-0000-0000-0000-000000000000".to_owned(),
                page: 1,
                page_size: 50,
                recent: false,
            },
            &ctx,
        )
        .await
        .expect("a stranger's panel reads rather than erroring");
    assert_eq!(
        stranger.total, 0,
        "{HARNESS}: a stranger's panel counts nothing"
    );
    assert!(
        stranger.rows.is_empty(),
        "{HARNESS}: a stranger's panel shows no rows"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. CLEANUP / NO LEFTOVER — the fixture person is deleted (interactions cascade),
    //    the production read models are rebuilt, and the panel reads empty afterwards.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(
        removed, 1,
        "{HARNESS}: exactly this run's person is removed"
    );
    landing
        .refresh_client_read_models()
        .await
        .expect("the production read models rebuild after cleanup");
    let after = harness
        .service()
        .history(&request(1, 50, false), &ctx)
        .await
        .expect("the panel reads after cleanup");
    assert_eq!(
        (after.rows.len(), after.total),
        (0, 0),
        "{HARNESS}: the rebuilt panel holds no trace of this run"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person behind"
    );
}

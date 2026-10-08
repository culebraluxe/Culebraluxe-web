//! CRM.CLIENT — history (TST-CRM-CLIENT-005).
//!
//! Contract: **the client history a caller asks for is the page it asked for, bounded by the server.** The read runs
//! through `ClientService::history` (`web/src/clients/mod.rs:283-338`), which authorizes `comms.read`, normalises the
//! request, reads the page from `mv_client_contact_history` (`db/src/client/detail.rs:218-261`) and projects it with
//! `build_contact_history` (`middle/model/src/client.rs:430-475`). Three rules are load-bearing, and each is asserted
//! here against the production code:
//!
//! - a page is a page: `page` is at least 1, and `page_size` is clamped to `CLIENT_MAX_PAGE_SIZE` — a caller cannot
//!   ask the server for an unbounded read;
//! - `recent` is a different read, not a page: it reads the newest rows (`offset = 0`) and forces the size to
//!   `CLIENT_RECENT_HISTORY_LIMIT`, then truncates the projected rows to that limit, so a "recent activity" panel
//!   cannot be flooded by a burst of events;
//! - `total` is the **database's** count, not the number of rows returned, so the UI can say "12 of 12" while
//!   showing the 10 it asked for.
//!
//! The projection half is proven on the model's own function with twelve synthetic events — the same function the
//! service calls — because that is where grouping, ordering and the recent limit live; the service half is proven on
//! the real pool for a fixture person this run seeds and owns.
//!
//! The negative case is the clamp: a request for 5000 rows must not be answered with 5000, and `recent` must answer
//! at most `CLIENT_RECENT_HISTORY_LIMIT` rows even when asked for the maximum page. A server that honoured either
//! would let one screen read the whole contact history of a client in one response.
//!
//! What this does not cover, stated so nobody reads more into a green run: the event rows themselves come from the
//! materialized read model `mv_client_contact_history`, which this test does not seed (a refresh of a shared read
//! model is not a test's to trigger); the paging, the limits and the projection are what is proven here, on rows the
//! fixture person does not have.
//!
//! Level: L3 Composition — the production client service against an isolated, disposable DEV/Neon target, plus the
//! model's pure projection. `ClientHarness` refuses PRODUCTION before any socket is opened
//! (`tests/src/database.rs:68-75`). The fixture person is named under a unique run marker and deleted at the end; a
//! zero-leftover count is asserted, so DEV is left as it was found.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_client__005__history -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L3 half needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use model::{
    build_contact_history, ClientHistoryEventRecord, ClientHistoryRequest, ContactHistoryRow,
    CLIENT_MAX_PAGE_SIZE, CLIENT_RECENT_HISTORY_LIMIT,
};
use test_harness::ClientHarness;

const HARNESS: &str = "ClientHarness/L3 Composition";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: `ClientHarness` still refuses PRODUCTION before any socket is opened.
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

fn history_request(person_id: &str, page: i64, page_size: i64, recent: bool) -> ClientHistoryRequest {
    ClientHistoryRequest {
        person_id: person_id.to_owned(),
        page,
        page_size,
        recent,
    }
}

/// Twelve one-hour-apart email events, oldest first, as the read model would hand them back.
///
/// `email` deliberately: only `imessage`, `sms` and `whatsapp` are grouped into bursts
/// (`group_into_bursts`, `middle/model/src/client.rs:485-522`), so twelve of these are twelve moments and the recent
/// limit can be observed on them.
fn twelve_events() -> Vec<ClientHistoryEventRecord> {
    (0..12)
        .map(|index| ClientHistoryEventRecord {
            id: format!("event-{index:02}"),
            channel: "email".to_owned(),
            direction: Some(if index % 2 == 0 { "inbound" } else { "outbound" }.to_owned()),
            occurred_at: format!("2026-09-{:02}T12:00:00+00:00", index + 1),
            title: Some(format!("Message {index}")),
            summary: Some(format!("Summary {index}")),
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-005); the file and the assay use it.
async fn crm_client_005__history() {
    // A. The projection: twelve moments of this person's history, projected by the production function.
    let events = twelve_events();
    let ids = |result: &model::ClientContactHistoryResult| -> Vec<String> {
        result
            .rows
            .iter()
            .map(|row| match row {
                ContactHistoryRow::Detail(moment) => moment.id.clone(),
                ContactHistoryRow::Aggregate(item) => item.id.clone(),
            })
            .collect()
    };

    let page = build_contact_history(
        events.clone(),
        &[],
        &[],
        events.len() as i64,
        2,
        CLIENT_MAX_PAGE_SIZE,
        false,
    );
    assert_eq!(
        page.rows.len(),
        12,
        "{HARNESS}: a paged (non-recent) read projects every event of the page it was given"
    );
    assert_eq!(
        ids(&page).first().map(String::as_str),
        Some("event-11"),
        "{HARNESS}: the newest moment leads the history, however the rows arrived"
    );
    assert_eq!(
        page.page, 2,
        "{HARNESS}: the page the caller asked for is echoed, not the page the caller meant"
    );
    assert_eq!(
        page.total, 12,
        "{HARNESS}: `total` is the count behind the page, not the number of rows returned"
    );

    let recent = build_contact_history(
        events.clone(),
        &[],
        &[],
        events.len() as i64,
        1,
        CLIENT_RECENT_HISTORY_LIMIT,
        true,
    );
    assert_eq!(
        recent.rows.len() as i64,
        CLIENT_RECENT_HISTORY_LIMIT,
        "{HARNESS}: `recent` is bounded — twelve events must not become twelve rows in a recent panel"
    );
    assert_eq!(
        ids(&recent).first().map(String::as_str),
        Some("event-11"),
        "{HARNESS}: and the rows it keeps are the newest ones, or the bound would hide the wrong end"
    );
    assert!(
        !ids(&recent).contains(&"event-00".to_owned()),
        "{HARNESS}: the oldest event is the one the bound drops"
    );
    assert_eq!(
        recent.total, 12,
        "{HARNESS}: the recent view still reports the whole count it was given, not the rows it shows"
    );
    assert!(recent.recent, "{HARNESS}: the read is echoed as the recent one");

    // B. The service, on the real pool: the request is normalised before it reaches the read model.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the history proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let marker = format!("TST-CRM-CLIENT-005-{}-", harness.namespace());
    let person_id = harness
        .seed_person(&format!("{marker}History Client"))
        .await
        .expect("proof: a client fixture row must be insertable on DEV");

    // The negative case: an unbounded page size is not honoured, and a page below the first is the first.
    let huge = harness
        .service()
        .history(&history_request(&person_id, 0, 5000, false), &ctx)
        .await
        .expect("proof: clients.history must answer on the composed service");
    assert_eq!(
        huge.page_size, CLIENT_MAX_PAGE_SIZE,
        "{HARNESS}: a caller cannot ask the server for 5000 rows of one client's history"
    );
    assert_eq!(
        huge.page, 1,
        "{HARNESS}: page 0 is page 1, so a paging control cannot walk off the front"
    );
    assert!(
        huge.rows.len() as i64 <= CLIENT_MAX_PAGE_SIZE,
        "{HARNESS}: and the rows returned obey the clamp, not the request"
    );
    assert_eq!(
        huge.total, 0,
        "{HARNESS}: this fixture person has no committed history, and the count says so"
    );

    let recent = harness
        .service()
        .history(&history_request(&person_id, 1, 5000, true), &ctx)
        .await
        .expect("proof: a recent history read must answer on the composed service");
    assert!(recent.recent, "{HARNESS}: the read is echoed as the recent one");
    assert_eq!(
        recent.page_size, CLIENT_RECENT_HISTORY_LIMIT,
        "{HARNESS}: `recent` forces the small page whatever size the caller asked for"
    );
    assert!(
        recent.rows.len() as i64 <= CLIENT_RECENT_HISTORY_LIMIT,
        "{HARNESS}: and no recent read may exceed the recent limit"
    );

    // A person nobody knows is an empty history, not an error: a stale link must not fail the request.
    let unknown = harness
        .service()
        .history(
            &history_request(&uuid::Uuid::new_v4().to_string(), 1, 20, false),
            &ctx,
        )
        .await
        .expect("proof: an unknown person's history is empty, never a fault");
    assert!(
        unknown.rows.is_empty() && unknown.total == 0,
        "{HARNESS}: an unknown person has no history and the read answers emptily"
    );

    // Teardown: this run's fixture person, and none left behind.
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("proof: the fixture row must be deletable");
    assert_eq!(
        removed, 1,
        "{HARNESS}: teardown must remove exactly the one person this run seeded"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("proof: the leftover count must be readable"),
        0,
        "{HARNESS}: DEV must be left as it was found — zero fixture rows remain"
    );
}


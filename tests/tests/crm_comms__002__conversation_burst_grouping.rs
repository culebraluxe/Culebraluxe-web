//! CRM.COMMS — conversation burst grouping (TST-CRM-COMMS-002).
//!
//! Contract: consecutive message-channel interactions (iMessage, SMS, WhatsApp) belong to ONE
//! conversation burst while they are at most 30 minutes apart, and a wider gap starts a new burst
//! (`group_into_bursts`, `middle/model/src/client.rs:466-513`, threshold
//! `BURST_THRESHOLD_SECONDS = 30 * 60`, `middle/model/src/client.rs:8`). A burst is not a display
//! convenience: it carries the committed counts of the events it groups — inbound, outbound,
//! `two_way`, `count`, `started_at`, `ended_at`, `latest_direction`, `preview` — assembled by
//! `finalize_burst` (`middle/model/src/client.rs:515-560`).
//!
//! Two rules are load-bearing and both are asserted:
//!
//! - **A non-message channel is never grouped.** An `email` event inside the same 30-minute window
//!   stays a single-event row (`group_into_bursts` sends every channel outside
//!   `imessage | sms | whatsapp` down the `singles` path, `:473-479`).
//! - **The gap splits.** Events 10 minutes apart merge; the event 35 minutes after them starts a
//!   second burst, so the first burst's `count` is 2, not 3. A grouping that merged everything (or
//!   split everything) fails here.
//!
//! The events are read through the production persistence boundary — `ClientDao::history_events`
//! (`db/src/client/detail.rs:218-258`) over `mv_client_contact_history`, rebuilt by the production
//! `LandingDao::refresh_client_read_models` — so the grouping consumes committed database truth,
//! not values the test itself constructed in memory. Arrival order is a non-factor: the reader
//! returns newest-first while `group_into_bursts` sorts per channel ascending, and the test proves
//! it by grouping the same events reversed and asserting identical bursts.
//!
//! Level: L2 Persistence — the isolated, disposable DEV/Neon target; the harness refuses
//! PRODUCTION before any socket is opened. Fixture rows are named under a unique run marker,
//! deleted at the end, and the read models are refreshed after cleanup so DEV is left as found.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_comms__002__conversation_burst_grouping -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{ClientDao, DbFailure, DbTarget, LandingDao};
use model::{build_contact_history, ContactHistoryRow};
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
/// Raw SQL is fixture setup only — the grouping under test reads the committed rows back through
/// the production DAO.
async fn insert_interaction(
    pool: &PgPool,
    person_id: &str,
    channel: &str,
    direction: &str,
    occurred_at: &str,
    summary: &str,
) -> Result<String, DbFailure> {
    sqlx::query_scalar(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, summary)
         values ($1::uuid, $2, $2, $3, $4::timestamptz, $5)
         returning id::text",
    )
    .bind(person_id)
    .bind(channel)
    .bind(direction)
    .bind(occurred_at)
    .bind(summary)
    .fetch_one(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms002.insert", &error))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-COMMS-002); the file and the assay use it.
async fn crm_comms_002__conversation_burst_grouping() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the burst proof runs only on an isolated DEV target"
    );
    let dao = ClientDao::new(harness.database().database().clone());
    let landing = LandingDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCOMMS002-{ns}");
    let person = harness
        .seed_person(&format!("{marker}-person"))
        .await
        .expect("the fixture person seeds");

    // A 30-minute conversation (T0 inbound, T0+10m outbound) and a second conversation opened at
    // T0+45m — a 35-minute gap, which is wider than the threshold and must start a NEW burst.
    // An email lands inside the first window and must never join a message burst.
    let burst_one_first = insert_interaction(
        harness.pool(),
        &person,
        "imessage",
        "inbound",
        "2026-10-01T12:00:00+00:00",
        "hola, is the villa still available",
    )
    .await
    .expect("the first message commits");
    let burst_one_last = insert_interaction(
        harness.pool(),
        &person,
        "imessage",
        "outbound",
        "2026-10-01T12:10:00+00:00",
        "yes — Saturday works",
    )
    .await
    .expect("the reply commits");
    let burst_two = insert_interaction(
        harness.pool(),
        &person,
        "imessage",
        "inbound",
        "2026-10-01T12:45:00+00:00",
        "one more question",
    )
    .await
    .expect("the later message commits");
    let email = insert_interaction(
        harness.pool(),
        &person,
        "email",
        "inbound",
        "2026-10-01T12:05:00+00:00",
        "showing confirmation",
    )
    .await
    .expect("the email commits");

    // Committed truth first, then the production read models rebuilt the way production rebuilds
    // them — `history_events` reads `mv_client_contact_history`, a materialized view that only sees
    // committed rows.
    let committed: i64 =
        sqlx::query_scalar("select count(*) from interaction where person_id = $1::uuid")
            .bind(&person)
            .fetch_one(harness.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms002.committed", &error))
            .expect("the committed count reads");
    assert_eq!(committed, 4, "{HARNESS}: four interactions are committed");
    landing
        .refresh_client_read_models()
        .await
        .expect("the production read models rebuild");

    let (events, total) = dao
        .history_events(&person, 50, 0)
        .await
        .expect("the production history read runs");
    assert_eq!(
        total, 4,
        "{HARNESS}: the read model reports exactly the committed events"
    );
    assert_eq!(
        events.len(),
        4,
        "{HARNESS}: the page holds every committed event"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE POSITIVE CONTRACT — two message bursts and one never-grouped email.
    // -----------------------------------------------------------------------------------------------------------
    let history = build_contact_history(events.clone(), &[], &[], total, 1, 50, false);
    assert_eq!(
        (
            history.total,
            history.page,
            history.page_size,
            history.recent
        ),
        (4, 1, 50, false),
        "{HARNESS}: the history result carries the request's paging through untouched"
    );
    assert_eq!(
        history.rows.len(),
        3,
        "{HARNESS}: three rows — two message bursts and the ungrouped email, got {:?}",
        history
            .rows
            .iter()
            .map(|row| match row {
                ContactHistoryRow::Detail(detail) => format!("detail:{}", detail.id),
                ContactHistoryRow::Aggregate(aggregate) => format!("aggregate:{}", aggregate.id),
            })
            .collect::<Vec<_>>()
    );
    assert!(
        history
            .rows
            .iter()
            .all(|row| matches!(row, ContactHistoryRow::Detail(_))),
        "{HARNESS}: a person with no relationship-evidence rows produces detail bursts only"
    );

    let detail = |id: &str| {
        history
            .rows
            .iter()
            .find_map(|row| match row {
                ContactHistoryRow::Detail(detail) => (detail.id == id).then_some(detail),
                ContactHistoryRow::Aggregate(_) => None,
            })
            .unwrap_or_else(|| panic!("{HARNESS}: burst for event {id} exists"))
    };

    let first = detail(&burst_one_first);
    assert_eq!(
        first.channel, "imessage",
        "{HARNESS}: the burst is attributed to its message channel"
    );
    assert_eq!(
        (first.count, first.inbound_count, first.outbound_count),
        (2, 1, 1),
        "{HARNESS}: two messages 10 minutes apart are ONE burst with the committed counts"
    );
    assert!(
        first.two_way,
        "{HARNESS}: one inbound and one outbound makes the burst two-way"
    );
    assert_eq!(
        first.direction.as_deref(),
        Some("two-way"),
        "{HARNESS}: a two-way burst is labelled two-way"
    );
    assert_eq!(
        first.latest_direction.as_deref(),
        Some("outbound"),
        "{HARNESS}: the burst's latest direction is the last committed message's direction"
    );
    assert_eq!(
        (first.started_at.as_str(), first.ended_at.as_str()),
        ("2026-10-01T12:00:00+00:00", "2026-10-01T12:10:00+00:00"),
        "{HARNESS}: the burst spans exactly the events it grouped"
    );
    assert_eq!(
        first.preview.as_deref(),
        Some("yes — Saturday works"),
        "{HARNESS}: the preview is the burst's latest content"
    );

    let second = detail(&burst_two);
    assert_eq!(
        (second.count, second.inbound_count, second.outbound_count),
        (1, 1, 0),
        "{HARNESS}: the 35-minute gap starts a NEW burst — the first burst stays at two"
    );
    assert!(
        !second.two_way,
        "{HARNESS}: a one-message burst is not two-way"
    );
    assert_eq!(
        second.direction.as_deref(),
        Some("inbound"),
        "{HARNESS}: an all-inbound burst is labelled inbound"
    );

    let mail = detail(&email);
    assert_eq!(
        mail.channel, "email",
        "{HARNESS}: the email keeps its own channel"
    );
    assert_eq!(
        mail.count, 1,
        "{HARNESS}: an email inside the 30-minute window is never absorbed into a message burst"
    );

    // Rows are newest-first by their effective date, which is what the history screen renders.
    let order: Vec<&str> = history
        .rows
        .iter()
        .map(|row| match row {
            ContactHistoryRow::Detail(detail) => detail.id.as_str(),
            ContactHistoryRow::Aggregate(_) => "aggregate",
        })
        .collect();
    assert_eq!(
        order,
        vec![burst_two.as_str(), burst_one_first.as_str(), email.as_str()],
        "{HARNESS}: history rows are ordered by their ending time, newest first"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — arrival order must not change the grouping: the reader returns newest-first
    //    while the grouper sorts per channel ascending, so the same events reversed must produce
    //    the same three bursts with the same counts. A grouper that depended on input order would
    //    pass the assertions above and fail here.
    // -----------------------------------------------------------------------------------------------------------
    let mut reversed = events;
    reversed.reverse();
    let regrouped = build_contact_history(reversed, &[], &[], total, 1, 50, false);
    let signature = |history: &model::ClientContactHistoryResult| {
        history
            .rows
            .iter()
            .map(|row| match row {
                ContactHistoryRow::Detail(detail) => format!(
                    "{}|{}|{}|{}|{}",
                    detail.id,
                    detail.count,
                    detail.inbound_count,
                    detail.outbound_count,
                    detail.direction.clone().unwrap_or_default()
                ),
                ContactHistoryRow::Aggregate(aggregate) => format!("aggregate:{}", aggregate.id),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        signature(&regrouped),
        signature(&history),
        "{HARNESS}: grouping does not depend on the order the events arrived in"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. COMMITTED TRUTH / ROLLBACK — the grouper consumes rows that survived a commit; a probe
    //    written inside a transaction only this connection can roll back is invisible afterwards.
    // -----------------------------------------------------------------------------------------------------------
    let probe_person = person.clone();
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                let count: i64 = sqlx::query_scalar(
                    "insert into interaction (person_id, channel, event_type, direction, occurred_at, summary)
                     values ($1::uuid, 'imessage', 'imessage', 'inbound', now(), 'rolled back')
                     returning 1",
                )
                .bind(&probe_person)
                .fetch_one(&mut *conn)
                .await
                .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms002.probe", &error))?;
                Ok(count)
            })
        })
        .await
        .expect("the rolled-back probe runs");
    assert_eq!(
        visible_inside, 1,
        "{HARNESS}: the probe row is visible inside its own transaction"
    );
    let after_probe: i64 =
        sqlx::query_scalar("select count(*) from interaction where person_id = $1::uuid")
            .bind(&person)
            .fetch_one(harness.pool())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("test-harness.crm_comms002.probe_leftover", &error)
            })
            .expect("the post-rollback count reads");
    assert_eq!(
        after_probe, 4,
        "{HARNESS}: a rolled-back transaction commits nothing — the grouper only ever sees four events"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER — the person is deleted (interactions cascade) and the read models
    //    are rebuilt so this run leaves no stale projection behind on shared DEV.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("the fixture person is removed");
    assert_eq!(
        removed, 1,
        "{HARNESS}: exactly this run's person is removed"
    );
    landing
        .refresh_client_read_models()
        .await
        .expect("the production read models rebuild after cleanup");
    let (after_events, after_total) = dao
        .history_events(&person, 50, 0)
        .await
        .expect("the production history read runs after cleanup");
    assert_eq!(
        (after_events.len(), after_total),
        (0, 0),
        "{HARNESS}: the rebuilt read model holds no trace of this run"
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

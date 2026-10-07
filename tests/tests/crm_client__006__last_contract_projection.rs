//! CRM.CLIENT — last-contract projection (TST-CRM-CLIENT-006).
//!
//! Contract: the client detail projects its "last" interaction — the latest one by event
//! time — and nothing else. The projection lives in SQL, in the `last_contact` lateral
//! of `ClientDao::detail` (`db/src/client/detail.rs:58-64`):
//!
//!   select i.channel, i.occurred_at, i.title, i.summary
//!   from interaction i
//!   where i.person_id = p.id
//!   order by i.occurred_at desc
//!   limit 1
//!
//! and `map_detail` (`:367-374`) turns it into `ClientDetail.last_contact`: `Some` only
//! when a channel AND a time both came back, `None` otherwise. (The taxonomy says
//! "contract"; the client boundary's only last-X projection is this last-contact one —
//! there is no contract entity in CRM.CLIENT scope — so this test proves the seam the
//! taxonomy points at: the latest interaction projected onto the client.)
//!
//! Three things must therefore hold, asserted through `ClientService::detail` (L3
//! Composition — the service, the DAO and the relationship-evidence join together):
//!
//! - **The newest event wins, not the last-written row.** The older fixture is inserted
//!   LAST, so a reader returning write order would project the morning email while the
//!   contract demands the afternoon call — channel, summary and timestamp label must
//!   all be the newer row's.
//! - **The projection is absent when there is nothing to project.** A client with no
//!   interactions carries `last_contact: None` — the screen shows no last contact,
//!   never a fabricated one.
//! - **Clients outside the book are not projected at all.** An unknown id and an
//!   archived person both read back as `None` (the query filters
//!   `p.archived_at is null` at `:72`), so the panel cannot render a stranger or a
//!   filed-away client.
//!
//! Level: L3 Composition — the production client service on an isolated, disposable
//! DEV/Neon target; the harness refuses PRODUCTION before any socket is opened.
//! Fixture rows live under run-unique markers and are deleted at the end, with a
//! zero-leftover assertion.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_client__006__last_contract_projection -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L3 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget};
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

/// Insert one interaction fixture row.
///
/// Raw SQL is fixture setup only — the projection under test reads the committed rows back
/// through the production service.
async fn insert_interaction(
    pool: &PgPool,
    person_id: &str,
    channel: &str,
    occurred_at: &str,
    title: &str,
    summary: &str,
) -> Result<String, DbFailure> {
    sqlx::query_scalar(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title, summary)
         values ($1::uuid, $2, $2, 'inbound', $3::timestamptz, $4, $5)
         returning id::text",
    )
    .bind(person_id)
    .bind(channel)
    .bind(occurred_at)
    .bind(title)
    .bind(summary)
    .fetch_one(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client006.insert", &error))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-006); the file and the assay use it.
async fn crm_client_006__last_contract_projection() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the last-contact proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCLIENT006-{ns}");

    // The client whose projection is under test, plus one client with no interactions and
    // one archived client for the negative cases.
    let pool = harness.pool();
    let client = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'active') returning id::text",
    )
    .bind(format!("{marker}-ana-rios"))
    .fetch_one(pool)
    .await
    .expect("the fixture client seeds");
    let quiet = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'new') returning id::text",
    )
    .bind(format!("{marker}-quiet-client"))
    .fetch_one(pool)
    .await
    .expect("the interaction-less client seeds");
    let archived = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status, archived_at)
         values ($1, 'seller', 'active', now()) returning id::text",
    )
    .bind(format!("{marker}-archived-client"))
    .fetch_one(pool)
    .await
    .expect("the archived client seeds");

    // Two interactions for the fixture client. The older one is written LAST on purpose:
    // a projection that returned write order would show the morning email, while the
    // contract demands the afternoon call.
    insert_interaction(
        pool,
        &client,
        "call",
        "2026-10-01T13:00:00+00:00",
        "afternoon call",
        "rates and closing costs",
    )
    .await
    .expect("the newer interaction commits");
    insert_interaction(
        pool,
        &client,
        "email",
        "2026-10-01T09:00:00+00:00",
        "morning email",
        "first hello",
    )
    .await
    .expect("the older interaction commits — written last on purpose");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE NEWEST EVENT IS PROJECTED — channel, summary and timestamp label are the newer row's.
    //    13:00 UTC is 09:00 in America/Puerto_Rico (no DST), rendered by the production
    //    `to_char(... 'Mon FMDD, YYYY HH12:MI AM')` label.
    // -----------------------------------------------------------------------------------------------------------
    let detail = harness
        .service()
        .detail(&client, &ctx)
        .await
        .expect("the production detail read runs")
        .expect("the fixture client is in the book");
    assert_eq!(detail.id, client, "{HARNESS}: the detail is the client's own");
    let last = detail
        .last_contact
        .as_ref()
        .expect("a client with interactions projects a last contact");
    assert_eq!(
        last.channel, "call",
        "{HARNESS}: the projected channel is the NEWER interaction's, not the last-written row's"
    );
    assert_eq!(
        last.summary.as_deref(),
        Some("rates and closing costs"),
        "{HARNESS}: the projected summary is the NEWER interaction's"
    );
    assert_eq!(
        last.occurred_at, "Oct 1, 2026 09:00 AM",
        "{HARNESS}: the projected timestamp is the NEWER interaction's, in the production label format"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NO INTERACTIONS, NO PROJECTION — `last_contact` is absent, never fabricated.
    // -----------------------------------------------------------------------------------------------------------
    let quiet_detail = harness
        .service()
        .detail(&quiet, &ctx)
        .await
        .expect("the quiet detail read runs")
        .expect("the interaction-less client is in the book");
    assert_eq!(
        quiet_detail.last_contact, None,
        "{HARNESS}: a client with no interactions projects no last contact"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — strangers and archived clients are not projected at all. An unknown id
    //    and an archived person both read back as `None`, so the panel cannot render either.
    // -----------------------------------------------------------------------------------------------------------
    let stranger = harness
        .service()
        .detail("00000000-0000-0000-0000-000000000000", &ctx)
        .await
        .expect("an unknown id reads rather than erroring");
    assert_eq!(
        stranger, None,
        "{HARNESS}: an unknown client id projects nothing"
    );
    let filed = harness
        .service()
        .detail(&archived, &ctx)
        .await
        .expect("an archived client reads rather than erroring");
    assert_eq!(
        filed, None,
        "{HARNESS}: an archived client is outside the book and projects nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER — all three fixture persons are deleted (interactions
    //    cascade) with a zero-leftover assertion, so shared DEV keeps no fixture of this run.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(
        removed, 3,
        "{HARNESS}: exactly this run's three persons are removed"
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

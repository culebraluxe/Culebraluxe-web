//! CRM.CLIENT — status indicators (TST-CRM-CLIENT-008).
//!
//! Contract: the client admin row is a set of status indicators that each mean exactly
//! what their query says — no more. `ClientDao::admin_page` (`db/src/client/
//! warm_read_cache.rs:373-488`) projects, per person, `status` straight from the row,
//! `last_interaction_label` from the latest interaction lateral (`:443-449`), and three
//! counts with three different exclusion rules:
//!
//! - `open_task_count` counts only `task.status = 'open'` (`:413`) — a completed task
//!   is work done, not work outstanding, and must not inflate the indicator.
//! - `active_deal_count` counts only deals whose `stage <> 'closed'` with an active
//!   `role = 'client'` participant (`:414-425`) — a closed deal is history, not pipeline.
//! - `interest_count` counts every `property_interest` for the person (`:426`).
//!
//! Four things must therefore hold, asserted through `ClientService::admin` (L3
//! Composition — the service over the production DAO):
//!
//! - **Each indicator reads its own committed truth.** With one open and one completed
//!   task, one interest, one open-stage deal and one closed deal seeded, the row reads
//!   `open_task_count == 1`, `interest_count == 1`, `active_deal_count == 1` — the
//!   completed task and the closed deal are excluded by the rules above, not by luck.
//! - **The status is passed through, not derived.** The row's `status` is the stored
//!   person status (`warm`), and the latest interaction renders its production label.
//! - **An archived client is not indicated at all.** The count and the rows both filter
//!   `p.archived_at is null` (`:450`), so searching for an archived person finds
//!   nothing — the board cannot show a filed-away client as needing attention.
//! - **The indicators survive a client with nothing to show.** (Covered by the
//!   `None` label path: a second person with no interactions, tasks, deals or
//!   interests reads zeros and no label — asserted through the same search.)
//!
//! Level: L3 Composition — the production client service on an isolated, disposable
//! DEV/Neon target; the harness refuses PRODUCTION before any socket is opened.
//! Fixture rows live under run-unique markers. Cleanup deletes dependents in
//! restrict-safe order (participants, deals) before the persons (tasks, interactions
//! and interests cascade), then the fixture property, with zero-leftover assertions.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_client__008__status_indicators -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L3 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget};
use model::ClientAdminPageRequest;
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

async fn seed_person(
    pool: &PgPool,
    display_name: &str,
    archived: bool,
) -> Result<String, DbFailure> {
    sqlx::query_scalar(
        "insert into person (display_name, role, status, archived_at)
         values ($1, 'buyer', 'warm', case when $2 then now() else null end)
         returning id::text",
    )
    .bind(display_name)
    .bind(archived)
    .fetch_one(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.seed_person", &error))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-008); the file and the assay use it.
async fn crm_client_008__status_indicators() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the indicators proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCLIENT008-{ns}");
    let pool = harness.pool();

    // The indicated client, one archived client (negative case), and one client with
    // nothing to show (zeros-and-no-label case).
    let client = seed_person(pool, &format!("{marker}-ana-rios"), false)
        .await
        .expect("the fixture client seeds");
    seed_person(pool, &format!("{marker}-archived-zed"), true)
        .await
        .expect("the archived client seeds");
    let bare = seed_person(pool, &format!("{marker}-bare-client"), false)
        .await
        .expect("the bare client seeds");

    // One open task and one completed task: the indicator must count only the open one.
    for status in ["open", "completed"] {
        sqlx::query("insert into task (person_id, title, status) values ($1::uuid, $2, $3)")
            .bind(&client)
            .bind(format!("{marker} task ({status})"))
            .bind(status)
            .execute(pool)
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.task", &error))
            .expect("the fixture task seeds");
    }

    // One property, one interest in it, one open-stage deal and one closed deal — each
    // with an active client participant, so only the stage rule separates them.
    let property = sqlx::query_scalar::<_, String>(
        "insert into property (name) values ($1) returning id::text",
    )
    .bind(format!("{marker}-casa-prueba"))
    .fetch_one(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.property", &error))
    .expect("the fixture property seeds");
    sqlx::query(
        "insert into property_interest (person_id, property_id) values ($1::uuid, $2::uuid)",
    )
    .bind(&client)
    .bind(&property)
    .execute(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.interest", &error))
    .expect("the fixture interest seeds");
    for stage in ["offer", "closed"] {
        let deal = sqlx::query_scalar::<_, String>(
            "insert into deal (property_id, client_person_id, stage)
             values ($1::uuid, $2::uuid, $3) returning id::text",
        )
        .bind(&property)
        .bind(&client)
        .bind(stage)
        .fetch_one(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.deal", &error))
        .expect("the fixture deal seeds");
        sqlx::query(
            "insert into deal_participant (deal_id, person_id, role, active)
             values ($1::uuid, $2::uuid, 'client', true)",
        )
        .bind(&deal)
        .bind(&client)
        .execute(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.participant", &error))
        .expect("the fixture participant seeds");
    }

    // One interaction, so the label has something to render: 13:00 UTC is Oct 1 in
    // America/Puerto_Rico, rendered by the production `to_char(... 'Mon FMDD, YYYY')`.
    sqlx::query(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title)
         values ($1::uuid, 'email', 'email', 'inbound', '2026-10-01T13:00:00+00:00', 'hello')",
    )
    .bind(&client)
    .execute(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.interaction", &error))
    .expect("the fixture interaction seeds");

    let search = |term: &str| ClientAdminPageRequest {
        search: term.to_owned(),
        page: 1,
        page_size: 50,
    };

    // -----------------------------------------------------------------------------------------------------------
    // 1. EACH INDICATOR READS ITS OWN COMMITTED TRUTH — the completed task and the closed
    //    deal are excluded by their rules, and the status passes through.
    // -----------------------------------------------------------------------------------------------------------
    let page = harness
        .service()
        .admin(&search(&format!("{marker}-ana")), &ctx)
        .await
        .expect("the production admin read runs");
    assert_eq!(
        page.total, 1,
        "{HARNESS}: the search finds exactly the fixture client"
    );
    assert_eq!(page.rows.len(), 1, "{HARNESS}: one admin row");
    let row = &page.rows[0];
    assert_eq!(
        row.id, client,
        "{HARNESS}: the row is the fixture client's own"
    );
    assert_eq!(
        row.status, "warm",
        "{HARNESS}: the status indicator passes the stored status through"
    );
    assert_eq!(
        row.open_task_count, 1,
        "{HARNESS}: the open task counts and the completed one does not"
    );
    assert_eq!(
        row.interest_count, 1,
        "{HARNESS}: the property interest counts"
    );
    assert_eq!(
        row.active_deal_count, 1,
        "{HARNESS}: the open-stage deal counts and the closed one does not"
    );
    assert_eq!(
        row.last_interaction_label.as_deref(),
        Some("Oct 1, 2026"),
        "{HARNESS}: the latest interaction renders its production label"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NOTHING TO SHOW IS ZEROS AND NO LABEL — not nulls, not someone else's counts.
    // -----------------------------------------------------------------------------------------------------------
    let bare_page = harness
        .service()
        .admin(&search(&format!("{marker}-bare")), &ctx)
        .await
        .expect("the bare admin read runs");
    assert_eq!(bare_page.total, 1, "{HARNESS}: the bare client is found");
    let bare_row = &bare_page.rows[0];
    assert_eq!(
        bare_row.id, bare,
        "{HARNESS}: the row is the bare client's own"
    );
    assert_eq!(
        bare_row.open_task_count, 0,
        "{HARNESS}: no tasks reads zero, not null"
    );
    assert_eq!(
        bare_row.active_deal_count, 0,
        "{HARNESS}: no deals reads zero, not null"
    );
    assert_eq!(
        bare_row.interest_count, 0,
        "{HARNESS}: no interests reads zero, not null"
    );
    assert_eq!(
        bare_row.last_interaction_label, None,
        "{HARNESS}: no interactions reads no label, never another client's"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — an archived client is not indicated at all. The count and the rows
    //    both filter `archived_at is null`, so the search finds nothing to show.
    // -----------------------------------------------------------------------------------------------------------
    let archived_page = harness
        .service()
        .admin(&search(&format!("{marker}-archived")), &ctx)
        .await
        .expect("the archived search reads rather than erroring");
    assert_eq!(
        archived_page.total, 0,
        "{HARNESS}: an archived client is outside the book — total zero"
    );
    assert!(
        archived_page.rows.is_empty(),
        "{HARNESS}: an archived client renders no indicator row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER — participants and deals first (`on delete restrict`
    //    forbids deleting the person or the property first), then the persons (tasks,
    //    interactions and interests cascade), then the property, with zero-leftover
    //    assertions on shared DEV.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("delete from deal_participant where person_id = $1::uuid")
        .bind(&client)
        .execute(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.unlink", &error))
        .expect("fixture participants are removed");
    sqlx::query("delete from deal where client_person_id = $1::uuid")
        .bind(&client)
        .execute(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.undeal", &error))
        .expect("fixture deals are removed");
    let removed = harness.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(
        removed, 3,
        "{HARNESS}: exactly this run's three persons are removed"
    );
    sqlx::query("delete from property where id = $1::uuid")
        .bind(&property)
        .execute(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_client008.unproperty", &error))
        .expect("the fixture property is removed");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person behind"
    );
    let property_left: i64 =
        sqlx::query_scalar("select count(*) from property where id = $1::uuid")
            .bind(&property)
            .fetch_one(pool)
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("test-harness.crm_client008.property_left", &error)
            })
            .expect("the property leftover count reads");
    assert_eq!(
        property_left, 0,
        "{HARNESS}: the proof leaves no property behind"
    );
}

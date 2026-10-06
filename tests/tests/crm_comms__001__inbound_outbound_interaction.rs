//! CRM.COMMS — inbound/outbound interaction (TST-CRM-COMMS-001).
//!
//! Contract: an `interaction` row's direction is committed as exactly one of `inbound`, `outbound`
//! or `NULL`, and the production read path reports that committed truth instead of inventing one.
//!
//! - The table refuses any other value: `check (direction is null or direction in ('inbound',
//!   'outbound'))` on `interaction.direction` (`db/migrations/001_initial_schema.sql:266-300`),
//!   which Postgres names `interaction_direction_check`.
//! - `CommsDao::moments` (`db/src/comms.rs:395-445`) maps `inbound` → `CommsDirection::Inbound`,
//!   `outbound` → `CommsDirection::Outbound`, and `NULL` → `None` (`db/src/comms.rs:138-144`) —
//!   a row recorded without a direction is reported as unknown, never guessed.
//! - `CommsDao::activity` (`db/src/comms.rs:253-308`) carries the raw value through untouched, so
//!   the feed the portal renders shows exactly what was stored.
//!
//! The negatives are load-bearing. A direction of `sideways` must be refused by the schema, so no
//! reader can ever see a direction the CRM never recorded: the test asserts the constraint failure
//! AND the unchanged committed count. A write inside a transaction the harness can only roll back
//! must be visible to itself and invisible to both readers afterwards — that is what makes
//! "committed truth" the contract rather than "the value this connection last wrote".
//!
//! Level: L2 Persistence — the production `CommsDao` against an isolated, disposable DEV/Neon
//! target; the harness refuses PRODUCTION before any socket is opened. Fixture rows are inserted
//! under a uniquely named person and deleted at the end, with a zero-leftover assertion.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_comms__001__inbound_outbound_interaction -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{CommsDao, DbFailure, DbFailureKind, DbTarget};
use model::CommsDirection;
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

/// Insert one interaction fixture row through the schema the contract is about.
///
/// Raw SQL is fixture setup only: the rows must land in `interaction` for the production DAO to
/// read them back. `source_system`/`source_external_id` are left NULL so this run can never
/// collide with a concurrent run's dedupe key.
async fn insert_interaction(
    pool: &PgPool,
    person_id: &str,
    channel: &str,
    direction: Option<&str>,
    occurred_at: &str,
    title: &str,
) -> Result<String, DbFailure> {
    sqlx::query_scalar(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title)
         values ($1::uuid, $2, $2, $3, $4::timestamptz, $5)
         returning id::text",
    )
    .bind(person_id)
    .bind(channel)
    .bind(direction)
    .bind(occurred_at)
    .bind(title)
    .fetch_one(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms001.insert", &error))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-COMMS-001); the file and the assay use it.
async fn crm_comms_001__inbound_outbound_interaction() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the interaction proof runs only on an isolated DEV target"
    );
    let dao = CommsDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCOMMS001-{ns}");
    let person = harness
        .seed_person(&format!("{marker}-person"))
        .await
        .expect("the fixture person seeds");

    // Three committed rows: one inbound, one outbound, and one recorded with NO direction (a note
    // is neither) — the third is the case a reader that assumed a direction would get wrong.
    let inbound_id = insert_interaction(
        harness.pool(),
        &person,
        "call",
        Some("inbound"),
        "2026-10-01T14:00:00+00:00",
        "incoming call",
    )
    .await
    .expect("the inbound interaction commits");
    let outbound_id = insert_interaction(
        harness.pool(),
        &person,
        "email",
        Some("outbound"),
        "2026-10-01T15:00:00+00:00",
        "follow-up email",
    )
    .await
    .expect("the outbound interaction commits");
    let undirected_id = insert_interaction(
        harness.pool(),
        &person,
        "note",
        None,
        "2026-10-01T16:00:00+00:00",
        "call note",
    )
    .await
    .expect("the undirected interaction commits");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE POSITIVE CONTRACT — the production timeline reports each direction as recorded.
    //    Ordering is asserted elsewhere (TST-CRM-COMMS-005); here every row is found by id.
    // -----------------------------------------------------------------------------------------------------------
    let page = dao
        .moments(&person, 10, 0)
        .await
        .expect("the production moments read runs");
    assert_eq!(
        page.total, 3,
        "{HARNESS}: exactly this run's three interactions are committed for the person"
    );
    let direction_of = |id: &str| {
        page.moments
            .iter()
            .find(|moment| moment.id == id)
            .unwrap_or_else(|| panic!("{HARNESS}: interaction {id} is in the committed timeline"))
            .direction
    };
    assert_eq!(
        direction_of(&inbound_id),
        Some(CommsDirection::Inbound),
        "{HARNESS}: an inbound interaction is read back as inbound"
    );
    assert_eq!(
        direction_of(&outbound_id),
        Some(CommsDirection::Outbound),
        "{HARNESS}: an outbound interaction is read back as outbound"
    );
    assert_eq!(
        direction_of(&undirected_id),
        None,
        "{HARNESS}: an interaction recorded with no direction is reported as unknown, not guessed"
    );

    // The second production seam: the activity feed carries the raw value unchanged, so what the
    // portal renders is what the row holds.
    let activity = dao
        .activity(500)
        .await
        .expect("the production activity read runs");
    let activity_direction = |id: &str| {
        activity
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("{HARNESS}: interaction {id} is in the activity feed"))
            .direction
            .clone()
    };
    assert_eq!(
        activity_direction(&inbound_id).as_deref(),
        Some("inbound"),
        "{HARNESS}: the activity feed reports the stored direction verbatim"
    );
    assert_eq!(
        activity_direction(&outbound_id).as_deref(),
        Some("outbound"),
        "{HARNESS}: the activity feed reports the stored direction verbatim"
    );
    assert_eq!(
        activity_direction(&undirected_id),
        None,
        "{HARNESS}: the activity feed never invents a direction for a NULL"
    );

    // Committed truth, read from the pool rather than through the DAO: the two readers agree with
    // the table itself.
    let stored: Vec<(String, Option<String>)> = sqlx::query_as(
        "select id::text, direction from interaction
          where person_id = $1::uuid order by occurred_at",
    )
    .bind(&person)
    .fetch_all(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms001.truth", &error))
    .expect("the committed rows read back");
    assert_eq!(
        stored,
        vec![
            (inbound_id.clone(), Some("inbound".to_owned())),
            (outbound_id.clone(), Some("outbound".to_owned())),
            (undirected_id.clone(), None),
        ],
        "{HARNESS}: committed truth is exactly what was inserted"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE / REFUSAL — a direction the CRM never recorded cannot be written at all. If the
    //    check were absent, a reader could be handed `sideways` and every mapping above would be
    //    reporting a fiction; the assertion that NOTHING was written is what makes the refusal real.
    // -----------------------------------------------------------------------------------------------------------
    let refusal = sqlx::query(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title)
         values ($1::uuid, 'email', 'email', 'sideways', now(), 'impossible')",
    )
    .bind(&person)
    .execute(harness.pool())
    .await
    .expect_err("an unknown direction must be refused by the schema");
    let refusal = DbFailure::from_sqlx("test-harness.crm_comms001.refusal", &refusal);
    assert!(
        matches!(refusal.kind, DbFailureKind::Constraint),
        "{HARNESS}: an illegal direction is a constraint failure, got {:?}",
        refusal.kind
    );
    assert!(
        refusal.to_string().contains("interaction_direction_check")
            || refusal
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("interaction_direction_check")),
        "{HARNESS}: the refusal names the direction check, got {refusal:?}"
    );
    let after_refusal: i64 =
        sqlx::query_scalar("select count(*) from interaction where person_id = $1::uuid")
            .bind(&person)
            .fetch_one(harness.pool())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("test-harness.crm_comms001.after_refusal", &error)
            })
            .expect("the count reads");
    assert_eq!(
        after_refusal, 3,
        "{HARNESS}: a refused direction writes no row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. COMMITTED TRUTH / ROLLBACK — a row inserted inside a transaction the harness can only roll
    //    back is visible inside it, and neither reader sees it afterwards.
    // -----------------------------------------------------------------------------------------------------------
    let probe_person = person.clone();
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                let id: String = sqlx::query_scalar(
                    "insert into interaction (person_id, channel, event_type, direction, occurred_at, title)
                     values ($1::uuid, 'sms', 'sms', 'inbound', now(), 'rolled back')
                     returning id::text",
                )
                .bind(&probe_person)
                .fetch_one(&mut *conn)
                .await
                .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms001.probe", &error))?;
                let count: i64 =
                    sqlx::query_scalar("select count(*) from interaction where id = $1::uuid")
                        .bind(&id)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("test-harness.crm_comms001.probe_read", &error)
                        })?;
                Ok(count)
            })
        })
        .await
        .expect("the rolled-back probe runs");
    assert_eq!(
        visible_inside, 1,
        "{HARNESS}: the probe row is visible inside its own transaction"
    );
    let committed: i64 =
        sqlx::query_scalar("select count(*) from interaction where person_id = $1::uuid")
            .bind(&person)
            .fetch_one(harness.pool())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("test-harness.crm_comms001.probe_leftover", &error)
            })
            .expect("the post-rollback count reads");
    assert_eq!(
        committed, 3,
        "{HARNESS}: a rolled-back transaction commits nothing — both readers answer from committed truth"
    );
    let page_after = dao
        .moments(&person, 10, 0)
        .await
        .expect("the production moments read runs after the rollback");
    assert_eq!(
        page_after.total, 3,
        "{HARNESS}: the production timeline does not see the rolled-back interaction"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER — this run's person is deleted and its interactions cascade with it.
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

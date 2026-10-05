//! CRM.COMMS — dedupe (TST-CRM-COMMS-004).
//!
//! Contract: one canonical interaction per `(source_system, source_external_id)` — a replayed
//! intake event never appends a second row, and a stale candidate never overwrites a newer one.
//!
//! The production boundary is `LandingDao::upsert_latest_interaction` (`db/src/landing.rs:224-286`),
//! which the website-intake, WhatsApp and deal-portal writers all lean on:
//!
//! - the partial unique index `interaction_source_identity_unique`
//!   (`db/migrations/005_crm_interaction_task_foundation.sql:41-44`) makes the key pair unique for
//!   every non-NULL pair — that is the dedupe, in the schema, not in application code;
//! - the upsert answers `Inserted` the first time, `Ignored` on a replay (the `where
//!   interaction.occurred_at < excluded.occurred_at` guard returns no row, so nothing is written),
//!   `Updated` when a genuinely newer event for the same key replaces the row in place, and
//!   `Ignored` again for an older candidate — newest evidence wins, and one row is all there ever
//!   is;
//! - a different `source_external_id` under the same system is a different event: dedupe is per
//!   key pair, never per source.
//!
//! The negatives are load-bearing. A raw insert of the same key without the upsert's conflict
//! clause must be refused by the unique index (constraint named in the assertion) with the row
//! count unchanged — if the index were gone, the upsert's answers above would still look right and
//! duplicates would quietly accumulate through any other writer. An older candidate must be
//! ignored rather than allowed to roll the row back in time. And a key inserted inside a
//! transaction only this connection can roll back must leave the committed set untouched.
//!
//! Level: L2 Persistence — the isolated, disposable DEV/Neon target; the harness refuses
//! PRODUCTION before any socket is opened. The key carries a run-unique id, the fixture person is
//! deleted at the end, and a zero-leftover assertion proves DEV was left as found.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_comms__004__dedupe -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use chrono::{DateTime, Utc};
use db::{DbFailure, DbFailureKind, DbTarget, LandingDao, LatestInteraction, LatestInteractionOutcome};
use serde_json::json;
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

/// The candidate the production landing writes for one Person x source key.
fn candidate(person: &str, external_id: &str, occurred_at: &str) -> LatestInteraction {
    LatestInteraction {
        person_id: person.to_owned(),
        channel: "website".into(),
        event_type: "general_enquiry_submitted".into(),
        direction: Some("inbound".into()),
        occurred_at: occurred_at.to_owned(),
        summary: Some("general enquiry".into()),
        duration_seconds: None,
        source_system: "website".into(),
        source_external_id: external_id.to_owned(),
        source_metadata: json!({ "proof": "crm-comms-004" }),
    }
}

/// The committed row for one key: its id and when it happened.
async fn committed_row(
    pool: &sqlx::PgPool,
    external_id: &str,
) -> Result<Option<(String, DateTime<Utc>)>, DbFailure> {
    sqlx::query_as(
        "select id::text, occurred_at from interaction
          where source_system = 'website' and source_external_id = $1",
    )
    .bind(external_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms004.row", &error))
}

/// How many interactions are committed for the fixture person.
async fn committed_count(pool: &sqlx::PgPool, person: &str) -> Result<i64, DbFailure> {
    sqlx::query_scalar("select count(*) from interaction where person_id = $1::uuid")
        .bind(person)
        .fetch_one(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms004.count", &error))
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-COMMS-004); the file and the assay use it.
async fn crm_comms_004__dedupe() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the dedupe proof runs only on an isolated DEV target"
    );
    let landing = LandingDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCOMMS004-{ns}");
    let person = harness
        .seed_person(&format!("{marker}-person"))
        .await
        .expect("the fixture person seeds");
    let external_id = format!("crm-comms-004-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. FIRST LANDING INSERTS; THE SAME EVENT AGAIN IS A NO-OP.
    // -----------------------------------------------------------------------------------------------------------
    let first = landing
        .upsert_latest_interaction(&candidate(&person, &external_id, "2026-10-01T12:00:00+00:00"))
        .await
        .expect("the first landing runs");
    assert_eq!(
        first,
        LatestInteractionOutcome::Inserted,
        "{HARNESS}: the first landing of a key inserts the canonical row"
    );
    let (row_id, stored_at) = committed_row(harness.pool(), &external_id)
        .await
        .expect("the committed row reads")
        .unwrap_or_else(|| panic!("{HARNESS}: the landed interaction is committed"));
    assert_eq!(
        committed_count(harness.pool(), &person).await.expect("the count reads"),
        1,
        "{HARNESS}: one key, one committed row"
    );

    let replay = landing
        .upsert_latest_interaction(&candidate(&person, &external_id, "2026-10-01T12:00:00+00:00"))
        .await
        .expect("the replay runs");
    assert_eq!(
        replay,
        LatestInteractionOutcome::Ignored,
        "{HARNESS}: replaying the identical event writes nothing"
    );
    assert_eq!(
        committed_count(harness.pool(), &person).await.expect("the count reads"),
        1,
        "{HARNESS}: a replay does not append a second row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. A GENUINELY NEWER EVENT FOR THE SAME KEY REPLACES THE ROW IN PLACE; AN OLDER ONE IS
    //    REFUSED. Newest evidence wins, and there is still exactly one row.
    // -----------------------------------------------------------------------------------------------------------
    let newer = landing
        .upsert_latest_interaction(&candidate(&person, &external_id, "2026-10-01T13:00:00+00:00"))
        .await
        .expect("the newer landing runs");
    assert_eq!(
        newer,
        LatestInteractionOutcome::Updated,
        "{HARNESS}: newer evidence for the same key updates the canonical row"
    );
    let (row_after_update, at_after_update) = committed_row(harness.pool(), &external_id)
        .await
        .expect("the committed row reads")
        .expect("the landed interaction is still committed");
    assert_eq!(
        row_after_update, row_id,
        "{HARNESS}: the update replaced the same row — no second row for one key"
    );
    assert_eq!(
        at_after_update,
        "2026-10-01T13:00:00+00:00"
            .parse::<DateTime<Utc>>()
            .expect("the timestamp parses"),
        "{HARNESS}: the row now carries the newer event's time"
    );
    assert_eq!(
        committed_count(harness.pool(), &person).await.expect("the count reads"),
        1,
        "{HARNESS}: an update does not append either"
    );

    let stale = landing
        .upsert_latest_interaction(&candidate(&person, &external_id, "2026-10-01T11:00:00+00:00"))
        .await
        .expect("the stale landing runs");
    assert_eq!(
        stale,
        LatestInteractionOutcome::Ignored,
        "{HARNESS}: an older candidate is refused — stale evidence cannot roll the row back"
    );
    let (_, at_after_stale) = committed_row(harness.pool(), &external_id)
        .await
        .expect("the committed row reads")
        .expect("the landed interaction is still committed");
    assert_eq!(
        at_after_stale, at_after_update,
        "{HARNESS}: the committed row still carries the newest event's time"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. DEDUPE IS PER KEY PAIR — a second event from the same system is a different event.
    // -----------------------------------------------------------------------------------------------------------
    let other_id = format!("{external_id}-second");
    let second_event = landing
        .upsert_latest_interaction(&candidate(&person, &other_id, "2026-10-02T09:00:00+00:00"))
        .await
        .expect(" the second event's landing runs");
    assert_eq!(
        second_event,
        LatestInteractionOutcome::Inserted,
        "{HARNESS}: a different source_external_id is a different event"
    );
    assert_eq!(
        committed_count(harness.pool(), &person).await.expect("the count reads"),
        2,
        "{HARNESS}: two keys from one source system are two canonical rows"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — the schema itself refuses a duplicate key, so no other writer can bypass the
    //    upsert and append a twin. If the unique index were missing, every answer above would still
    //    look correct while duplicates accumulated.
    // -----------------------------------------------------------------------------------------------------------
    let refusal = sqlx::query(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, source_system, source_external_id)
         values ($1::uuid, 'website', 'website', 'inbound', now(), 'website', $2)",
    )
    .bind(&person)
    .bind(&external_id)
    .execute(harness.pool())
    .await
    .expect_err("a raw duplicate key must be refused by the unique index");
    let refusal = DbFailure::from_sqlx("test-harness.crm_comms004.refusal", &refusal);
    assert!(
        matches!(refusal.kind, DbFailureKind::Constraint),
        "{HARNESS}: a duplicate canonical key is a constraint failure, got {:?}",
        refusal.kind
    );
    assert!(
        refusal
            .to_string()
            .contains("interaction_source_identity_unique")
            || refusal
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("interaction_source_identity_unique")),
        "{HARNESS}: the refusal names the dedupe index, got {refusal:?}"
    );
    assert_eq!(
        committed_count(harness.pool(), &person).await.expect("the count reads"),
        2,
        "{HARNESS}: the refused duplicate wrote nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. COMMITTED TRUTH / ROLLBACK — a key landed inside a transaction only this connection can
    //    roll back is visible to itself and gone from the committed set afterwards.
    // -----------------------------------------------------------------------------------------------------------
    let probe_id = format!("{external_id}-probe");
    let probe_person = person.clone();
    let probe_candidate = candidate(&probe_person, &probe_id, "2026-10-03T09:00:00+00:00");
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                let written = sqlx::query(
                    "insert into interaction (person_id, channel, event_type, direction, occurred_at, source_system, source_external_id)
                     values ($1::uuid, 'website', 'website', 'inbound', $2::timestamptz, 'website', $3)",
                )
                .bind(&probe_candidate.person_id)
                .bind(&probe_candidate.occurred_at)
                .bind(&probe_candidate.source_external_id)
                .execute(&mut *conn)
                .await
                .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms004.probe", &error))?;
                Ok(written.rows_affected() as i64)
            })
        })
        .await
        .expect("the rolled-back probe runs");
    assert_eq!(
        visible_inside, 1,
        "{HARNESS}: the probe row is visible inside its own transaction"
    );
    assert_eq!(
        committed_count(harness.pool(), &person).await.expect("the count reads"),
        2,
        "{HARNESS}: a rolled-back key commits nothing — the dedupe counts committed truth only"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / NO LEFTOVER — the person is deleted and its interactions cascade with it.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("the fixture person is removed");
    assert_eq!(removed, 1, "{HARNESS}: exactly this run's person is removed");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person or interaction behind"
    );
}

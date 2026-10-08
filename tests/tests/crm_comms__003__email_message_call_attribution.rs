//! CRM.COMMS — email / message / call attribution (TST-CRM-COMMS-003).
//!
//! Contract: every communication a client has is attributed to exactly ONE channel — email,
//! message (iMessage/SMS/WhatsApp), or call (Phone/FaceTime) — by production mapping, never by
//! guesswork, and evidence that is not yet linked or that is bulk/automated traffic is not
//! attributed to anybody.
//!
//! Two production seams carry the attribution, and both are asserted:
//!
//! - **Source attribution** (who said what, through which integration): `source_channel_for`
//!   (`middle/model/src/comms.rs:266-276`) maps an evidence source to its channel —
//!   `apple_messages` → iMessage, `gmail`/`gmail_contacts`/`icloud_mail` → Email, `apple_calls` →
//!   Phone, `whatsapp` → WhatsApp — and anything else is `Other` rather than a plausible-looking
//!   channel. `source_dto` (`:341-362`) puts that channel on the DTO the panel renders, and the
//!   committed counts come from `CommsDao::sources` (`db/src/comms.rs:310-332`) over
//!   `mv_client_relationship_channels`, which only admits `review_state = 'exact_linked'` rows that
//!   are neither bulk nor organization traffic, and which presents the unified Email
//!   channel as source `'email'` — `gmail`/`gmail_contacts`/`icloud_mail` normalize into
//!   one Person × Email row (migration `100`, the unified Email relationship channel).
//! - **Moment attribution** (one interaction's channel): `moment_dto`
//!   (`middle/model/src/comms.rs:364-387`) maps a stored channel to its taxonomy, and reclassifies
//!   a `call` as FaceTime only when the row's provenance says so (`is_facetime_interaction`,
//!   `:293-296`).
//!
//! The negatives are load-bearing: an evidence row that is `unresolved`, and a `gmail` row marked
//! automated/bulk, must NOT appear as relationship channels (no counts, no channel, no panel
//! presence) — an attribution that counts unlinked or bulk traffic would pass every positive
//! assertion and still be lying. A `website` interaction is reported as an unknown moment channel
//! rather than being coerced into email/message/call.
//!
//! Level: L2 Persistence — the isolated, disposable DEV/Neon target; the harness refuses
//! PRODUCTION before any socket is opened. Fixtures live under a unique run marker, the production
//! read models are rebuilt through `LandingDao::refresh_client_read_models`, and everything is
//! deleted at the end with a zero-leftover assertion.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_comms__003__email_message_call_attribution -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{CommsDao, DbFailure, DbTarget, LandingDao};
use model::{moment_dto, source_channel_for, source_dto, CommsMomentChannel, CommsSourceChannel};
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

/// One evidence fixture row. Raw SQL is fixture setup; the attribution under test reads the rows
/// back through the production DAO and the materialized read model.
#[allow(clippy::too_many_arguments)]
async fn seed_evidence(
    pool: &sqlx::PgPool,
    person_id: &str,
    marker: &str,
    source: &str,
    review_state: &str,
    inbound: Option<i32>,
    outbound: Option<i32>,
    bulk: bool,
) -> Result<(), DbFailure> {
    sqlx::query(
        "insert into integration_relationship_evidence
           (source, source_account, source_identity_key, evidence_fingerprint, review_state,
            canonical_person_id, inbound_count, outbound_count, is_two_way,
            first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
            is_automated_or_bulk)
         values ($1, $8, $2, $2, $3, $4::uuid, $5, $6,
                 coalesce($5, 0) > 0 and coalesce($6, 0) > 0,
                 '2026-09-01T12:00:00+00:00'::timestamptz, '2026-10-01T12:00:00+00:00'::timestamptz,
                 case when coalesce($5,0) > 0 then '2026-10-01T12:00:00+00:00'::timestamptz else null end,
                 case when coalesce($6,0) > 0 then '2026-10-01T11:00:00+00:00'::timestamptz else null end,
                 $7)",
    )
    .bind(source)
    .bind(format!("{marker}-{source}-{review_state}-{bulk}"))
    .bind(review_state)
    .bind(person_id)
    .bind(inbound)
    .bind(outbound)
    .bind(bulk)
    .bind(format!("{marker}-attribution-proof"))
    .execute(pool)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.evidence", &error))?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-COMMS-003); the file and the assay use it.
async fn crm_comms_003__email_message_call_attribution() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the attribution proof runs only on an isolated DEV target"
    );
    let dao = CommsDao::new(harness.database().database().clone());
    let landing = LandingDao::new(harness.database().database().clone());
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCOMMS003-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. SOURCE ATTRIBUTION, THE MAPPING HALF — production, pure, and total: every source lands in
    //    exactly one channel, and an unknown one is `Other` instead of a plausible-looking guess.
    // -----------------------------------------------------------------------------------------------------------
    let expected = [
        ("apple_messages", CommsSourceChannel::Imessage),
        ("gmail", CommsSourceChannel::Email),
        ("gmail_contacts", CommsSourceChannel::Email),
        ("icloud_mail", CommsSourceChannel::Email),
        ("apple_calls", CommsSourceChannel::Call),
        ("apple_facetime", CommsSourceChannel::Facetime),
        ("whatsapp", CommsSourceChannel::Whatsapp),
        ("apple_calendar", CommsSourceChannel::Calendar),
        // Case and padding are presentation, not identity: they must not change the attribution.
        ("  GMAIL  ", CommsSourceChannel::Email),
    ];
    for (source, channel) in expected {
        assert_eq!(
            source_channel_for(source),
            channel,
            "{HARNESS}: {source} is attributed to exactly one channel"
        );
    }
    for unknown in ["carrier_pigeon", "", "apple_notes", "twilio"] {
        assert_eq!(
            source_channel_for(unknown),
            CommsSourceChannel::Other,
            "{HARNESS}: an unknown source is attributed to Other, never to a channel it never came from"
        );
    }

    // Moment attribution: the taxonomy is closed, and a channel outside it reads back as unknown.
    assert_eq!(
        moment_dto(model::CommsMomentRecord {
            id: "m".into(),
            channel: Some("email".into()),
            event_type: Some("email".into()),
            source_system: None,
            direction: None,
            occurred_at: "2026-10-01T12:00:00+00:00".into(),
            title: None,
            summary: None,
        })
        .channel,
        Some(CommsMomentChannel::Email),
        "{HARNESS}: an email interaction is attributed to Email"
    );
    assert_eq!(
        moment_dto(model::CommsMomentRecord {
            id: "m".into(),
            channel: Some("imessage".into()),
            event_type: Some("imessage".into()),
            source_system: None,
            direction: None,
            occurred_at: "2026-10-01T12:00:00+00:00".into(),
            title: None,
            summary: None,
        })
        .channel,
        Some(CommsMomentChannel::Imessage),
        "{HARNESS}: a message interaction is attributed to iMessage"
    );
    let plain_call = model::CommsMomentRecord {
        id: "m".into(),
        channel: Some("call".into()),
        event_type: Some("call".into()),
        source_system: None,
        direction: None,
        occurred_at: "2026-10-01T12:00:00+00:00".into(),
        title: None,
        summary: None,
    };
    assert_eq!(
        moment_dto(plain_call.clone()).channel,
        Some(CommsMomentChannel::Call),
        "{HARNESS}: a plain call is attributed to Phone"
    );
    let mut facetime = plain_call;
    facetime.source_system = Some("apple_facetime".into());
    assert_eq!(
        moment_dto(facetime).channel,
        Some(CommsMomentChannel::Facetime),
        "{HARNESS}: the same call whose provenance says FaceTime is attributed to FaceTime"
    );
    let website = model::CommsMomentRecord {
        id: "m".into(),
        channel: Some("website".into()),
        event_type: Some("general_enquiry_submitted".into()),
        source_system: Some("website".into()),
        direction: None,
        occurred_at: "2026-10-01T12:00:00+00:00".into(),
        title: None,
        summary: None,
    };
    assert_eq!(
        moment_dto(website).channel,
        None,
        "{HARNESS}: a website enquiry is not email, message or call — it reads back as unknown"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. SOURCE ATTRIBUTION, COMMITTED — the same mapping, but the counts come from committed
    //    evidence through the production read model.
    // -----------------------------------------------------------------------------------------------------------
    let person = harness
        .seed_person(&format!("{marker}-person"))
        .await
        .expect("the fixture person seeds");
    seed_evidence(
        harness.pool(),
        &person,
        &marker,
        "apple_messages",
        "exact_linked",
        Some(3),
        Some(2),
        false,
    )
    .await
    .expect("the message evidence commits");
    seed_evidence(
        harness.pool(),
        &person,
        &marker,
        "gmail",
        "exact_linked",
        Some(1),
        Some(0),
        false,
    )
    .await
    .expect("the email evidence commits");
    seed_evidence(
        harness.pool(),
        &person,
        &marker,
        "apple_calls",
        "exact_linked",
        Some(0),
        Some(1),
        false,
    )
    .await
    .expect("the call evidence commits");
    // NEGATIVE 1 — an unlinked row is not anybody's communication yet.
    seed_evidence(
        harness.pool(),
        &person,
        &marker,
        "apple_messages",
        "unresolved",
        Some(99),
        Some(99),
        false,
    )
    .await
    .expect("the unresolved evidence commits");
    // NEGATIVE 2 — bulk/automated traffic is not relationship evidence for a person.
    seed_evidence(
        harness.pool(),
        &person,
        &marker,
        "gmail",
        "exact_linked",
        Some(500),
        Some(0),
        true,
    )
    .await
    .expect("the bulk evidence commits");

    landing
        .refresh_client_read_models()
        .await
        .expect("the production read models rebuild");

    let sources = dao
        .sources(&person)
        .await
        .expect("the production sources read runs");
    assert_eq!(
        sources.len(),
        3,
        "{HARNESS}: exactly three attributed channels — the unresolved and bulk rows are not attributed, got {:?}",
        sources.iter().map(|row| row.source.clone()).collect::<Vec<_>>()
    );
    let by_source = |sources: &[model::CommsSourceRecord], source: &str| {
        sources
            .iter()
            .find(|row| row.source == source)
            .unwrap_or_else(|| panic!("{HARNESS}: {source} is an attributed channel"))
            .clone()
    };

    let messages = by_source(&sources, "apple_messages");
    assert_eq!(
        (messages.inbound_count, messages.outbound_count, messages.total_count),
        (3, 2, 5),
        "{HARNESS}: message counts are the committed evidence's counts, not the unresolved row's 99/99"
    );
    assert!(
        messages.two_way,
        "{HARNESS}: messages both ways are two-way"
    );

    let mail = by_source(&sources, "email");
    assert_eq!(
        (mail.inbound_count, mail.outbound_count, mail.total_count),
        (1, 0, 1),
        "{HARNESS}: the bulk gmail row's 500 inbound are not attributed to the person"
    );
    assert!(
        !mail.two_way,
        "{HARNESS}: one-way evidence is not reported as two-way"
    );

    let calls = by_source(&sources, "apple_calls");
    assert_eq!(
        (calls.inbound_count, calls.outbound_count, calls.total_count),
        (0, 1, 1),
        "{HARNESS}: call counts are the committed evidence's counts"
    );

    // The DTOs the panel renders carry the same attribution as the pure mapping.
    for (record, channel) in [
        (messages, CommsSourceChannel::Imessage),
        (mail, CommsSourceChannel::Email),
        (calls, CommsSourceChannel::Call),
    ] {
        let dto = source_dto(record.clone());
        assert_eq!(
            dto.channel, channel,
            "{HARNESS}: {} is presented as its one channel",
            record.source
        );
        assert_eq!(
            dto.label,
            channel.label(),
            "{HARNESS}: the rendered label belongs to the attributed channel"
        );
        assert_eq!(
            dto.total_count, record.total_count,
            "{HARNESS}: attribution changes no committed count"
        );
    }

    // Committed truth, read from the pool: the read model's channel column agrees with the mapping.
    let mv_channels: Vec<(String, String)> = sqlx::query_as(
        "select source, channel from mv_client_relationship_channels where person_id = $1::uuid
          order by source",
    )
    .bind(&person)
    .fetch_all(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.mv", &error))
    .expect("the committed read model reads back");
    assert_eq!(
        mv_channels,
        vec![
            ("apple_calls".to_owned(), "apple_calls".to_owned()),
            ("apple_messages".to_owned(), "imessage".to_owned()),
            ("email".to_owned(), "email".to_owned()),
        ],
        "{HARNESS}: the committed read model attributes each source to its channel"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. COMMITTED TRUTH / ROLLBACK — a rolled-back evidence row is visible only inside its own
    //    transaction, so the attribution can never count it.
    // -----------------------------------------------------------------------------------------------------------
    let probe_person = person.clone();
    let probe_marker = marker.clone();
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                sqlx::query(
                    "insert into integration_relationship_evidence
                       (source, source_account, source_identity_key, evidence_fingerprint,
                        review_state, canonical_person_id, inbound_count, outbound_count)
                     values ('whatsapp', 'attribution-proof', $1, $1, 'exact_linked', $2::uuid, 7, 0)",
                )
                .bind(format!("{probe_marker}-probe"))
                .bind(&probe_person)
                .execute(&mut *conn)
                .await
                .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.probe", &error))?;
                let count: i64 = sqlx::query_scalar(
                    "select count(*) from integration_relationship_evidence
                      where evidence_fingerprint = $1",
                )
                .bind(format!("{probe_marker}-probe"))
                .fetch_one(&mut *conn)
                .await
                .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.probe_read", &error))?;
                Ok(count)
            })
        })
        .await
        .expect("the rolled-back probe runs");
    assert_eq!(
        visible_inside, 1,
        "{HARNESS}: the probe evidence is visible inside its own transaction"
    );
    let committed_probes: i64 = sqlx::query_scalar(
        "select count(*) from integration_relationship_evidence where canonical_person_id = $1::uuid",
    )
    .bind(&person)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.probe_leftover", &error))
    .expect("the post-rollback count reads");
    assert_eq!(
        committed_probes, 5,
        "{HARNESS}: a rolled-back evidence row is never attributed — only the five committed rows count"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER — evidence and person are deleted (per this run's marker) and the
    //    read models are rebuilt, so shared DEV is left as it was found.
    // -----------------------------------------------------------------------------------------------------------
    sqlx::query("delete from integration_relationship_evidence where source_account = $1")
        .bind(format!("{marker}-attribution-proof"))
        .execute(harness.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.cleanup_evidence", &error))
        .expect("the fixture evidence is removed");
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
    let after: i64 = sqlx::query_scalar(
        "select count(*) from mv_client_relationship_channels where person_id = $1::uuid",
    )
    .bind(&person)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_comms003.after", &error))
    .expect(" the rebuilt read model reads back");
    assert_eq!(
        after, 0,
        "{HARNESS}: the rebuilt read model holds no trace of this run"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person or evidence behind"
    );
}

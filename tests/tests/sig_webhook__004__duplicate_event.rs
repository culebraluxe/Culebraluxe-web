//! SIG.WEBHOOK — duplicate event (TST-SIG-WEBHOOK-004).
//!
//! Contract: BoldSign redelivers webhooks (retries, at-least-once delivery), so the second
//! delivery of the same provider event must change nothing — `BoldSignStore::record_webhook`
//! (`middle/apis/src/boldsign/store.rs:238`) inserts into `bold_sign_webhook_event` with
//! `ON CONFLICT (provider_event_id) DO NOTHING` and reports whether the row was new. This test
//! drives the REAL provider composition — `BoldSignSignatureProvider::verify_webhook`
//! (`middle/apis/src/boldsign/provider.rs:360`: HMAC verify → payload parse → envelope resolve
//! → record) — with a real HMAC over a real body, delivering the identical bytes twice plus one
//! concurrent duplicate pair, and proves the durable outcome is exactly one event row.
//!
//! Five things must therefore hold:
//!
//! - **The first delivery records.** `verify_webhook` returns Ok naming the Completed event and
//!   the canonical request, and one `bold_sign_webhook_event` row exists.
//! - **The redelivery is a durable no-op.** The identical bytes delivered again return Ok (the
//!   provider acked — webhooks must not 500 on retry) but insert nothing: still one row.
//! - **Concurrent duplicates collapse to one row.** Two tasks racing the same delivery both get
//!   Ok and the unique key admits exactly one row — the backstop is the constraint, not timing.
//! - **Dedup keys on the event id, not the envelope.** A different provider event id for the
//!   same envelope records a second row — proving the no-op above is the dedupe key working,
//!   not recording being broken.
//! - **The HMAC still gates.** A redelivery with a corrupted signature is refused even though
//!   its event id is already known — dedup never bypasses verification (order: verify first,
//!   record second).
//!
//! Level: L3 Composition — the production provider on an isolated, disposable DEV/Neon target;
//! the harness refuses PRODUCTION before any socket is opened. No live BoldSign traffic: the
//! unknown-envelope path and every path exercised here resolve locally; the HTTP client is
//! constructed but never called. Fixture rows live under run-unique markers and are swept with
//! zero-leftover assertions.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__004__duplicate_event
//! The plain command passes with the test skipped (it needs a disposable DEV database); the proof run is:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_webhook__004__duplicate_event -- --ignored

use apis::boldsign::{BoldSignConfig, BoldSignSignatureProvider};
use db::{Database, DbTarget, SignatureDao};
use hmac::{Hmac, Mac};
use model::signature::{SendSignatureRequest, SignatureCommandOutcome, SignatureRecipient};
use model::{SignatureProviderEvent, SignatureRecipientRole};
use services::SignatureProvider;
use sha2::Sha256;
use sqlx::PgPool;
use std::sync::Arc;
use test_harness::barrier::ConcurrencyBarrier;
use test_harness::database::TestDatabase;
use uuid::Uuid;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "SignatureHarness/L3 Composition";
const SECRET: &str = "sig-webhook-004-test-secret";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent load.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(database) => return database,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; TestDatabase refuses PROD: {}",
        last.unwrap_or_default()
    );
}

fn mint_header(raw_body: &str, secret: &str, timestamp: i64) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(format!("{timestamp}.{raw_body}").as_bytes());
    let hex: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("t={timestamp}, s0={hex}")
}

fn webhook_body(event_id: &str, envelope_id: &str) -> String {
    serde_json::json!({
        "event": { "id": event_id, "eventType": "Completed" },
        "data": { "documentId": envelope_id, "status": "Completed" }
    })
    .to_string()
}

fn test_config() -> BoldSignConfig {
    BoldSignConfig {
        api_key: "sig-webhook-004-test-key".into(),
        // Unroutable: no path exercised here may call the network. If one ever does, the
        // connection fails fast instead of reaching a live provider.
        base_url: "http://127.0.0.1:9".into(),
        webhook_secret: SECRET.into(),
        timeout_ms: 1_000,
        max_attempts: 1,
        retry_base_delay_ms: 10,
        retry_max_delay_ms: 50,
        webhook_tolerance_seconds: 300,
    }
}

struct Fixture {
    person_id: String,
    property_id: String,
    deal_id: String,
    document_id: String,
    request_id: String,
    envelope_id: String,
}

async fn seed(pool: &PgPool, db: &Database, marker: &str) -> Fixture {
    let person_id: String = sqlx::query_scalar(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'warm') returning id::text",
    )
    .bind(format!("{marker}-signer"))
    .fetch_one(pool)
    .await
    .expect("the fixture person seeds");
    let property_id: String =
        sqlx::query_scalar("insert into property (name) values ($1) returning id::text")
            .bind(format!("{marker}-casa"))
            .fetch_one(pool)
            .await
            .expect("the fixture property seeds");
    let deal_id: String = sqlx::query_scalar(
        "insert into deal (property_id, client_person_id, stage) values ($1::uuid, $2::uuid, 'offer') returning id::text",
    )
    .bind(&property_id)
    .bind(&person_id)
    .fetch_one(pool)
    .await
    .expect("the fixture deal seeds");
    let document_id = Uuid::new_v4().to_string();
    sqlx::query(
        "insert into transaction_document (id, deal_id, document_type, state, source)
         values ($1::uuid, $2::uuid, 'agreement', 'draft', 'upload')",
    )
    .bind(&document_id)
    .bind(&deal_id)
    .execute(pool)
    .await
    .expect("the fixture document seeds");

    let dao = SignatureDao::new(db.clone());
    let send = dao
        .send(
            &SendSignatureRequest {
                command_id: Uuid::new_v4().to_string(),
                transaction_document_id: document_id.clone(),
                recipients: vec![SignatureRecipient {
                    role: SignatureRecipientRole::Signer,
                    name: "Dupe Signer".into(),
                    email: "dupe-signer@example.test".into(),
                    order: 1,
                    execution_role: None,
                    execution_slot_id: None,
                }],
                message: None,
                created_by_user_id: None,
                execution_role: None,
                execution_slot_id: None,
                slot_recipient_email: None,
                signature_role: None,
                completion_recipient_emails: vec![],
            },
            None,
        )
        .await
        .expect("the fixture signature request sends");
    assert_eq!(
        send.outcome,
        SignatureCommandOutcome::Success,
        "{HARNESS}: the fixture send must succeed"
    );
    let request_id: String = sqlx::query_scalar(
        "select id::text from signature_request where transaction_document_id = $1::uuid",
    )
    .bind(&document_id)
    .fetch_one(pool)
    .await
    .expect("the fixture request reads back");

    let envelope_id = format!("{marker}-envelope");
    sqlx::query(
        "insert into bold_sign_request (signature_request_id, envelope_id, status)
         values ($1::uuid, $2, 'InProgress')",
    )
    .bind(&request_id)
    .bind(&envelope_id)
    .execute(pool)
    .await
    .expect("the fixture envelope row seeds");

    Fixture {
        person_id,
        property_id,
        deal_id,
        document_id,
        request_id,
        envelope_id,
    }
}

async fn webhook_event_count(pool: &PgPool, event_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*)::bigint from bold_sign_webhook_event where provider_event_id = $1",
    )
    .bind(event_id)
    .fetch_one(pool)
    .await
    .expect("the webhook event count reads")
}

async fn sweep(pool: &PgPool, fixture: &Fixture) {
    sqlx::query("delete from bold_sign_webhook_event where signature_request_id = $1::uuid")
        .bind(&fixture.request_id)
        .execute(pool)
        .await
        .expect("webhook event sweep");
    sqlx::query("delete from bold_sign_request where signature_request_id = $1::uuid")
        .bind(&fixture.request_id)
        .execute(pool)
        .await
        .expect("envelope row sweep");
    sqlx::query("delete from signature_envelope_recipient where signature_request_id = $1::uuid")
        .bind(&fixture.request_id)
        .execute(pool)
        .await
        .expect("recipient sweep");
    sqlx::query("delete from signature_request where id = $1::uuid")
        .bind(&fixture.request_id)
        .execute(pool)
        .await
        .expect("signature request sweep");
    sqlx::query(
        "delete from workflow_command_receipt where aggregate_id = $1 or aggregate_id = $2",
    )
    .bind(&fixture.request_id)
    .bind(&fixture.document_id)
    .execute(pool)
    .await
    .expect("receipt sweep");
    sqlx::query("delete from transaction_document where id = $1::uuid")
        .bind(&fixture.document_id)
        .execute(pool)
        .await
        .expect("document sweep");
    sqlx::query("delete from deal where id = $1::uuid")
        .bind(&fixture.deal_id)
        .execute(pool)
        .await
        .expect("deal sweep");
    sqlx::query("delete from property where id = $1::uuid")
        .bind(&fixture.property_id)
        .execute(pool)
        .await
        .expect("property sweep");
    sqlx::query("delete from person where id = $1::uuid")
        .bind(&fixture.person_id)
        .execute(pool)
        .await
        .expect("person sweep");

    let leftover: i64 = sqlx::query_scalar(
        "select count(*)::bigint from signature_request where transaction_document_id = $1::uuid",
    )
    .bind(&fixture.document_id)
    .fetch_one(pool)
    .await
    .expect("the leftover check reads");
    assert_eq!(
        leftover, 0,
        "{HARNESS}: the fixture signature request must be fully swept"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-004).
async fn sig_webhook_004__duplicate_event() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the duplicate-event proof runs only on an isolated DEV target"
    );
    let pool = harness.database().pool();
    let namespace = test_harness::database::unique_namespace();
    let marker = format!("TST-SIGWH004-{namespace}");
    let fixture = seed(pool, harness.database(), &marker).await;

    let provider = BoldSignSignatureProvider::new(harness.database().clone(), test_config())
        .expect("the test provider constructs without network");
    let event_id = format!("{marker}-evt-1");
    let body = webhook_body(&event_id, &fixture.envelope_id);
    let now = chrono::Utc::now().timestamp();
    let header = mint_header(&body, SECRET, now);

    // 1. THE FIRST DELIVERY RECORDS: Ok naming the Completed event and the canonical request.
    let first = provider
        .verify_webhook(&body, &header)
        .await
        .expect("{HARNESS}: the first delivery must verify and record");
    assert_eq!(
        first.event,
        SignatureProviderEvent::Completed,
        "{HARNESS}: the delivery must resolve to the Completed event"
    );
    assert_eq!(
        first.signature_request_id, fixture.request_id,
        "{HARNESS}: the delivery must resolve to the fixture request"
    );
    assert_eq!(
        webhook_event_count(pool, &event_id).await,
        1,
        "{HARNESS}: the first delivery must leave exactly one durable event row"
    );

    // 2. THE REDELIVERY IS A DURABLE NO-OP: identical bytes again return Ok (the provider gets
    // its ack — retries must not 500) but insert nothing.
    let replay = provider
        .verify_webhook(&body, &header)
        .await
        .expect("{HARNESS}: the redelivery must still ack Ok");
    assert_eq!(
        replay.signature_request_id, fixture.request_id,
        "{HARNESS}: the redelivery must resolve identically"
    );
    assert_eq!(
        webhook_event_count(pool, &event_id).await,
        1,
        "{HARNESS}: the redelivery must insert nothing — still exactly one row"
    );

    // 3. CONCURRENT DUPLICATES COLLAPSE TO ONE ROW: the backstop is the unique constraint, not
    // timing — both racers get Ok, the key admits one row.
    let barrier = Arc::new(ConcurrencyBarrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let (provider, barrier, body, header) = (
            provider.clone(),
            barrier.clone(),
            body.clone(),
            header.clone(),
        );
        handles.push(tokio::spawn(async move {
            barrier.arrive_and_wait().await;
            provider.verify_webhook(&body, &header).await
        }));
    }
    for handle in handles {
        handle
            .await
            .expect("{HARNESS}: a racer must not panic")
            .expect("{HARNESS}: a concurrent duplicate must still ack Ok");
    }
    assert_eq!(
        webhook_event_count(pool, &event_id).await,
        1,
        "{HARNESS}: concurrent duplicates must collapse to exactly one row"
    );

    // 4. DEDUP KEYS ON THE EVENT ID: a different provider event id for the same envelope records
    // a second row — proving the no-ops above are the dedupe key working, not recording broken.
    let event_id_2 = format!("{marker}-evt-2");
    let body_2 = webhook_body(&event_id_2, &fixture.envelope_id);
    let header_2 = mint_header(&body_2, SECRET, chrono::Utc::now().timestamp());
    provider
        .verify_webhook(&body_2, &header_2)
        .await
        .expect("{HARNESS}: a distinct event must record");
    assert_eq!(
        webhook_event_count(pool, &event_id_2).await,
        1,
        "{HARNESS}: a distinct event id must record its own row"
    );

    // 5. THE HMAC STILL GATES: a redelivery with a corrupted signature is refused even though
    // its event id is already known — dedup never bypasses verification.
    let mut bad_header = header.clone();
    bad_header.push('0');
    let error = provider
        .verify_webhook(&body, &bad_header)
        .await
        .expect_err("{HARNESS}: a corrupted signature must be refused, dedup or not");
    assert!(
        error.contains("malformed") || error.contains("mismatch"),
        "{HARNESS}: the refusal must name the signature problem, got: {error}"
    );

    sweep(pool, &fixture).await;
}

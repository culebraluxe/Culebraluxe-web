//! SIG.WEBHOOK — unknown envelope (TST-SIG-WEBHOOK-005).
//!
//! Contract: a webhook that verifies (valid HMAC over the raw body) and parses but names a
//! provider envelope the store never saw is unprocessable —
//! `BoldSignSignatureProvider::verify_webhook` (`middle/apis/src/boldsign/provider.rs:360`)
//! resolves the envelope through `BoldSignStore::get_by_envelope` and fails closed with an
//! "unknown envelope" error BEFORE recording anything. The order is load-bearing: verify →
//! parse → resolve → record. An unknown envelope must never record a webhook row (there is no
//! canonical request to attach it to — `bold_sign_webhook_event.signature_request_id` is a hard
//! foreign key), and must never be confused with a signature failure.
//!
//! Five things must therefore hold:
//!
//! - **A validly-signed webhook for an unknown envelope is refused**, and the refusal names the
//!   unknown envelope — not the HMAC, not the parse: the first two stages passed.
//! - **Nothing is recorded for it.** No `bold_sign_webhook_event` row appears for the unknown
//!   event id — the refusal happens before the write, so a forged-or-stale envelope id cannot
//!   pollute the durable event log.
//! - **A known envelope succeeds (control).** The same body shape with a registered envelope id
//!   verifies, resolves and records — proving the refusal above is the envelope resolution,
//!   not a broken rig.
//! - **The envelope id is exact.** A registered envelope with one character changed is unknown
//!   again: resolution is an exact lookup, not a prefix or fuzzy match.
//! - **The HMAC still gates first.** The unknown envelope with a corrupted signature fails on
//!   the signature, not the envelope — verification order (trust before resolve) is preserved.
//!
//! Level: L3 Composition — the production provider on an isolated, disposable DEV/Neon target;
//! the harness refuses PRODUCTION before any socket is opened. No live BoldSign traffic: every
//! path exercised here resolves locally; the HTTP client is constructed but never called.
//! Fixture rows live under run-unique markers and are swept with zero-leftover assertions.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__005__unknown_envelope
//! The plain command passes with the test skipped (it needs a disposable DEV database); the proof run is:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_webhook__005__unknown_envelope -- --ignored

use apis::boldsign::{BoldSignConfig, BoldSignSignatureProvider};
use db::{Database, DbTarget, SignatureDao};
use hmac::{Hmac, Mac};
use model::signature::{SendSignatureRequest, SignatureCommandOutcome, SignatureRecipient};
use model::{SignatureProviderEvent, SignatureRecipientRole};
use services::SignatureProvider;
use sha2::Sha256;
use sqlx::PgPool;
use test_harness::database::TestDatabase;
use uuid::Uuid;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "SignatureHarness/L3 Composition";
const SECRET: &str = "sig-webhook-005-test-secret";

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
        api_key: "sig-webhook-005-test-key".into(),
        // Unroutable: no path exercised here may call the network.
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
                    name: "Unknown Envelope Signer".into(),
                    email: "unknown-envelope-signer@example.test".into(),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-005).
async fn sig_webhook_005__unknown_envelope() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the unknown-envelope proof runs only on an isolated DEV target"
    );
    let pool = harness.database().pool();
    let namespace = test_harness::database::unique_namespace();
    let marker = format!("TST-SIGWH005-{namespace}");
    let fixture = seed(pool, harness.database(), &marker).await;

    let provider = BoldSignSignatureProvider::new(harness.database().clone(), test_config())
        .expect("the test provider constructs without network");

    // 1. A VALIDLY-SIGNED webhook for an UNKNOWN envelope is refused — naming the envelope, not
    // the HMAC and not the parse: trust and shape passed, resolution failed.
    let unknown_envelope = format!("{marker}-no-such-envelope");
    let unknown_event = format!("{marker}-evt-unknown");
    let unknown_body = webhook_body(&unknown_event, &unknown_envelope);
    let unknown_header = mint_header(&unknown_body, SECRET, chrono::Utc::now().timestamp());
    let error = provider
        .verify_webhook(&unknown_body, &unknown_header)
        .await
        .expect_err("{HARNESS}: an unknown envelope must be refused");
    assert!(
        error.contains("unknown envelope"),
        "{HARNESS}: the refusal must name the unknown envelope, got: {error}"
    );

    // 2. NOTHING IS RECORDED FOR IT: the refusal happens before the write, so the forged-or-stale
    // envelope id cannot pollute the durable event log.
    assert_eq!(
        webhook_event_count(pool, &unknown_event).await,
        0,
        "{HARNESS}: an unknown envelope must leave no webhook event row"
    );

    // 3. CONTROL — a KNOWN envelope succeeds: the same body shape with the registered envelope id
    // verifies, resolves and records, so the refusal above is the resolution, not a broken rig.
    let known_event = format!("{marker}-evt-known");
    let known_body = webhook_body(&known_event, &fixture.envelope_id);
    let known_header = mint_header(&known_body, SECRET, chrono::Utc::now().timestamp());
    let verified = provider
        .verify_webhook(&known_body, &known_header)
        .await
        .expect("{HARNESS}: the known envelope must verify, resolve and record");
    assert_eq!(
        verified.event,
        SignatureProviderEvent::Completed,
        "{HARNESS}: the control delivery must resolve to Completed"
    );
    assert_eq!(
        verified.signature_request_id, fixture.request_id,
        "{HARNESS}: the control delivery must resolve to the fixture request"
    );
    assert_eq!(
        webhook_event_count(pool, &known_event).await,
        1,
        "{HARNESS}: the control delivery must leave exactly one event row"
    );

    // 4. THE ENVELOPE ID IS EXACT: the registered envelope with one character changed is unknown
    // again — resolution is an exact lookup, not a prefix or fuzzy match.
    let near_envelope = format!("{}X", &fixture.envelope_id[..fixture.envelope_id.len() - 1]);
    assert_ne!(
        near_envelope, fixture.envelope_id,
        "the near-miss envelope must actually differ"
    );
    let near_event = format!("{marker}-evt-near");
    let near_body = webhook_body(&near_event, &near_envelope);
    let near_header = mint_header(&near_body, SECRET, chrono::Utc::now().timestamp());
    let error = provider
        .verify_webhook(&near_body, &near_header)
        .await
        .expect_err("{HARNESS}: a near-miss envelope must be refused");
    assert!(
        error.contains("unknown envelope"),
        "{HARNESS}: the near-miss refusal must name the unknown envelope, got: {error}"
    );

    // 5. THE HMAC STILL GATES FIRST: the unknown envelope with a corrupted signature fails on the
    // signature, not the envelope — trust comes before resolve.
    let mut bad_header = unknown_header.clone();
    bad_header.push('0');
    let error = provider
        .verify_webhook(&unknown_body, &bad_header)
        .await
        .expect_err("{HARNESS}: a corrupted signature must be refused before resolution");
    assert!(
        error.contains("malformed") || error.contains("mismatch"),
        "{HARNESS}: the refusal must name the signature problem, got: {error}"
    );

    sweep(pool, &fixture).await;
}

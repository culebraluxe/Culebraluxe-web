//! SIG.WEBHOOK — terminal replay (TST-SIG-WEBHOOK-009).
//!
//! Contract: redelivering a terminal (Completed) webhook must converge, not duplicate —
//! `SignatureService::handle_webhook` (`web/src/signature/mod.rs:818`) applies the Completed
//! status, and on replay the canonical transition reports `transitioned: false`, no second domain
//! event emits, and `reconcile_completed` takes its already-signed replay path instead of
//! minting a second signed artifact. Exactly-once durable effect under at-least-once delivery.
//!
//! This test drives the REAL service composition — the production `SignatureService` over the
//! production `SignatureDao` on an isolated DEV target — with ONLY the external provider faked
//! at the adapter boundary (`services::SignatureProvider`, the seam production owns for exactly
//! this). HMAC verification belongs to TST-SIG-WEBHOOK-001/002; the fake accepts the delivery so
//! THIS story pins the replay logic and nothing else.
//!
//! Six things must therefore hold:
//!
//! - **The first delivery completes and signs.** Outcome Success, document `signed`, one signed
//!   and one audit artifact downloaded exactly once, one `SIGNATURE_REQUEST_COMPLETED` event.
//! - **The redelivery succeeds without transitioning.** Outcome Success again (the provider gets
//!   its ack), but the status value reports `transitioned: false` — the terminal state held.
//! - **No second domain event emits.** Still exactly one completion event: observers see one
//!   completion, not two.
//! - **No second artifact is downloaded or minted.** Download counts stay at one and the signed
//!   media id is unchanged — reconciliation took its already-signed replay path.
//! - **The durable state is singular.** One signed media row for the document, document state
//!   `signed`, request status `completed`.
//! - **The order is not luck.** The replay assertions read the post-redelivery state (counts,
//!   ids, flags), so a service that re-ran the full completion pipeline on replay fails here.
//!
//! Level: L3 Composition — the production service + DAO on an isolated, disposable DEV/Neon
//! target; the harness refuses PRODUCTION before any socket is opened. Fixture rows live under
//! run-unique markers and are swept with zero-leftover assertions.
//!
//! NOTE on authorization: the route serves webhooks under a System actor with no principal, gated
//! by the HMAC (see `webhooks_support.rs`). This service-level proof authorizes with an
//! authenticated test principal through the permissive `DefaultAuthorizationPort` — the documented
//! unit-test port — so the replay contract is exercised without coupling it to the route's
//! identity shape.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__009__terminal_replay
//! The plain command passes with the test skipped (it needs a disposable DEV database); the proof run is:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_webhook__009__terminal_replay -- --ignored

use async_trait::async_trait;
use db::{Database, DbTarget, SignatureDao};
use model::signature::{
    SendSignatureRequest, SignatureArtifactDownload, SignatureCommandOutcome,
    SignatureProviderActionResult, SignatureProviderSendRequest, SignatureProviderSendResult,
    SignatureProviderStatusResult, SignatureRecipient, SignatureRequestStatus,
    SignatureStatusResult, SignatureWebhookVerification,
};
use model::{SignatureProviderEvent, SignatureRecipientRole};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal, SignatureProvider,
};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use test_harness::database::TestDatabase;
use uuid::Uuid;
use web::signature::SignatureService;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "SignatureHarness/L3 Composition";

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

/// The external provider, faked AT the adapter boundary: every delivery verifies as the Completed
/// event for the fixture request, and artifact downloads return marker bytes while counting calls.
/// HMAC verification is TST-SIG-WEBHOOK-001/002 territory; this fake lets the replay story pin
/// the service logic and nothing else.
struct ReplayProvider {
    request_id: String,
    marker: String,
    signed_downloads: AtomicUsize,
    audit_downloads: AtomicUsize,
}

impl ReplayProvider {
    fn artifact(&self, kind: &str) -> SignatureArtifactDownload {
        SignatureArtifactDownload {
            bytes: format!("{}-{}-pdf-bytes", self.marker, kind).into_bytes(),
            filename: format!("{}-{kind}.pdf", self.marker),
            mime_type: "application/pdf".into(),
        }
    }
}

#[async_trait]
impl SignatureProvider for ReplayProvider {
    fn name(&self) -> &'static str {
        "test-replay-provider"
    }

    fn map_status(&self, _provider_status: &str) -> SignatureRequestStatus {
        SignatureRequestStatus::Completed
    }

    async fn send(
        &self,
        _request: SignatureProviderSendRequest,
    ) -> Result<SignatureProviderSendResult, String> {
        Err("test-replay-provider never sends".into())
    }

    async fn status(
        &self,
        _signature_request_id: &str,
    ) -> Result<SignatureProviderStatusResult, String> {
        Err("test-replay-provider never polls status".into())
    }

    async fn cancel(
        &self,
        _signature_request_id: &str,
    ) -> Result<SignatureProviderActionResult, String> {
        Err("test-replay-provider never cancels".into())
    }

    async fn verify_webhook(
        &self,
        _raw_payload: &str,
        _signature: &str,
    ) -> Result<SignatureWebhookVerification, String> {
        Ok(SignatureWebhookVerification {
            event: SignatureProviderEvent::Completed,
            signature_request_id: self.request_id.clone(),
        })
    }

    async fn download_signed_artifact(
        &self,
        _signature_request_id: &str,
    ) -> Result<SignatureArtifactDownload, String> {
        self.signed_downloads.fetch_add(1, Ordering::SeqCst);
        Ok(self.artifact("signed"))
    }

    async fn download_audit_trail(
        &self,
        _signature_request_id: &str,
    ) -> Result<Option<SignatureArtifactDownload>, String> {
        self.audit_downloads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(self.artifact("audit")))
    }
}

fn test_context(actor_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(actor_id.to_owned()),
            kind: ServiceActorKind::User,
        },
        correlation_id: format!("sig-webhook-009-{actor_id}"),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: actor_id.to_owned(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

struct Fixture {
    actor_id: String,
    person_id: String,
    property_id: String,
    deal_id: String,
    document_id: String,
    request_id: String,
}

async fn seed(pool: &PgPool, db: &Database, marker: &str) -> Fixture {
    // The service receipts attribute commands to this actor (`claim_receipt` binds it as a uuid
    // with a hard foreign key to `app_user`), so the fixture actor is a real row, not a string.
    let actor_id: String =
        sqlx::query_scalar("insert into app_user (display_name) values ($1) returning id::text")
            .bind(format!("{marker}-actor"))
            .fetch_one(pool)
            .await
            .expect("the fixture actor seeds");
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
                    name: "Replay Signer".into(),
                    email: "replay-signer@example.test".into(),
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
        "select id::text from luxesign_request where transaction_document_id = $1::uuid",
    )
    .bind(&document_id)
    .fetch_one(pool)
    .await
    .expect("the fixture request reads back");

    Fixture {
        actor_id,
        person_id,
        property_id,
        deal_id,
        document_id,
        request_id,
    }
}

struct DocumentState {
    state: String,
    signed_media_id: Option<String>,
    signed_audit_media_id: Option<String>,
}

async fn document_state(pool: &PgPool, document_id: &str) -> DocumentState {
    let row: (String, Option<String>, Option<String>) = sqlx::query_as(
        "select state, signed_media_id::text, signed_audit_media_id::text
         from transaction_document where id = $1::uuid",
    )
    .bind(document_id)
    .fetch_one(pool)
    .await
    .expect("the document state reads");
    DocumentState {
        state: row.0,
        signed_media_id: row.1,
        signed_audit_media_id: row.2,
    }
}

async fn signed_media_count(pool: &PgPool, marker: &str) -> i64 {
    sqlx::query_scalar("select count(*)::bigint from media where filename like $1")
        .bind(format!("{marker}-%"))
        .fetch_one(pool)
        .await
        .expect("the signed media count reads")
}

async fn sweep(pool: &PgPool, fixture: &Fixture, marker: &str) {
    // Read the artifact ids first: deleting the document row cascades the request, and media
    // rows are deleted AFTER the document — deleting a signed media row while the document still
    // references it would trip `transaction_document_signed_pair` via `on delete set null`.
    let state = document_state(pool, &fixture.document_id).await;
    sqlx::query("delete from luxesign_envelope_recipient where signature_request_id = $1::uuid")
        .bind(&fixture.request_id)
        .execute(pool)
        .await
        .expect("recipient sweep");
    sqlx::query("delete from luxesign_request where id = $1::uuid")
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
    for media_id in [state.signed_media_id, state.signed_audit_media_id]
        .into_iter()
        .flatten()
    {
        sqlx::query("delete from media where id = $1::uuid")
            .bind(&media_id)
            .execute(pool)
            .await
            .expect("artifact media sweep");
    }
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
    sqlx::query("delete from app_user where id = $1::uuid")
        .bind(&fixture.actor_id)
        .execute(pool)
        .await
        .expect("actor sweep");

    let leftover: i64 = sqlx::query_scalar(
        "select count(*)::bigint from luxesign_request where transaction_document_id = $1::uuid",
    )
    .bind(&fixture.document_id)
    .fetch_one(pool)
    .await
    .expect("the leftover check reads");
    assert_eq!(
        leftover, 0,
        "{HARNESS}: the fixture signature request must be fully swept"
    );
    assert_eq!(
        signed_media_count(pool, marker).await,
        0,
        "{HARNESS}: the fixture artifact media must be fully swept"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-009).
async fn sig_webhook_009__terminal_replay() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the terminal-replay proof runs only on an isolated DEV target"
    );
    let pool = harness.database().pool();
    let namespace = test_harness::database::unique_namespace();
    let marker = format!("TST-SIGWH009-{namespace}");
    let fixture = seed(pool, harness.database(), &marker).await;

    let provider = Arc::new(ReplayProvider {
        request_id: fixture.request_id.clone(),
        marker: marker.clone(),
        signed_downloads: AtomicUsize::new(0),
        audit_downloads: AtomicUsize::new(0),
    });
    let events = Arc::new(CapturingDomainEventPort::default());
    let service = SignatureService::new(
        SignatureDao::new(harness.database().clone()),
        provider.clone(),
        ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            events.clone(),
        ),
    );
    let context = test_context(&fixture.actor_id);

    // 1. THE FIRST DELIVERY COMPLETES AND SIGNS: Success, document signed, one download of each
    // artifact, one completion event.
    let first = service
        .handle_webhook("{}", "test-signature", &context)
        .await
        .expect("{HARNESS}: the first terminal delivery must succeed");
    assert_eq!(
        first.outcome,
        SignatureCommandOutcome::Success,
        "{HARNESS}: the first delivery must succeed"
    );
    let first_state = document_state(pool, &fixture.document_id).await;
    assert_eq!(
        first_state.state, "signed",
        "{HARNESS}: the first delivery must sign the document"
    );
    assert!(
        first_state.signed_media_id.is_some(),
        "{HARNESS}: the first delivery must mint a signed artifact"
    );
    assert_eq!(
        provider.signed_downloads.load(Ordering::SeqCst),
        1,
        "{HARNESS}: the signed artifact must download exactly once"
    );
    assert_eq!(
        provider.audit_downloads.load(Ordering::SeqCst),
        1,
        "{HARNESS}: the audit artifact must download exactly once"
    );
    assert_eq!(
        events
            .events()
            .iter()
            .filter(|event| event.event_type == "SIGNATURE_REQUEST_COMPLETED")
            .count(),
        1,
        "{HARNESS}: the first delivery must emit exactly one completion event"
    );

    // 2 + 3 + 4. THE REDELIVERY SUCCEEDS WITHOUT TRANSITIONING, EMITTING, OR RE-DOWNLOADING:
    // Success with transitioned=false, still one event, still one download of each, same media.
    let replay = service
        .handle_webhook("{}", "test-signature", &context)
        .await
        .expect("{HARNESS}: the redelivery must still ack Success");
    assert_eq!(
        replay.outcome,
        SignatureCommandOutcome::Success,
        "{HARNESS}: the redelivery must succeed"
    );
    let status: SignatureStatusResult = serde_json::from_value(
        replay
            .value
            .clone()
            .expect("{HARNESS}: the redelivery must carry a status value"),
    )
    .expect("{HARNESS}: the redelivery value must be a status result");
    assert!(
        !status.transitioned,
        "{HARNESS}: the redelivery must report transitioned=false — the terminal state held"
    );
    assert_eq!(
        events
            .events()
            .iter()
            .filter(|event| event.event_type == "SIGNATURE_REQUEST_COMPLETED")
            .count(),
        1,
        "{HARNESS}: the redelivery must emit no second completion event"
    );
    assert_eq!(
        provider.signed_downloads.load(Ordering::SeqCst),
        1,
        "{HARNESS}: the redelivery must not re-download the signed artifact"
    );
    assert_eq!(
        provider.audit_downloads.load(Ordering::SeqCst),
        1,
        "{HARNESS}: the redelivery must not re-download the audit artifact"
    );
    let replay_state = document_state(pool, &fixture.document_id).await;
    assert_eq!(
        replay_state.signed_media_id, first_state.signed_media_id,
        "{HARNESS}: the redelivery must not mint a second signed artifact"
    );

    // 5. THE DURABLE STATE IS SINGULAR: one signed media set, document signed, request completed.
    assert_eq!(
        signed_media_count(pool, &marker).await,
        2,
        "{HARNESS}: exactly the signed + audit media rows must exist"
    );
    assert_eq!(
        replay_state.state, "signed",
        "{HARNESS}: the document must remain signed"
    );
    let status_text: String =
        sqlx::query_scalar("select status from luxesign_request where id = $1::uuid")
            .bind(&fixture.request_id)
            .fetch_one(pool)
            .await
            .expect("the request status reads");
    assert_eq!(
        status_text, "completed",
        "{HARNESS}: the request must remain completed"
    );

    sweep(pool, &fixture, &marker).await;
}

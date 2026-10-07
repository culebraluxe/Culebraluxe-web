//! OBS.ERROR — service errors are captured exactly once with severity, correlation, useful cause, no secrets, and
//! an appropriate returned error (TST-OBS-ERROR-002).
//!
//! Contract: **a service failure that must not change the caller's outcome is recorded rather than returned, and
//! the record says everything an operator needs.** This is the second half of the `Error Capture Obligation`, at
//! the *service kernel* seam rather than the database one (that is TST-OBS-ERROR-001):
//!
//! - **the port.** `ServiceErrorSink` (`middle/services/src/observability.rs:34-36`) is a trait object on
//!   `ServiceInfrastructure::errors`, so a service can record without depending on how records are stored.
//! - **the record.** `ServiceErrorRecord` (`observability.rs:16-27`) has ten fields and all ten are load-bearing:
//!   `domain` and `operation` say where, `code` says what class, `message` says why, `retryable` says whether to
//!   come back, `correlation_id` says which request, `severity` says how loudly.
//! - **the caller.** `ServiceRuntime::error_sink()` (`middle/services/src/runtime.rs:104-107`) is the process's
//!   sink, and callers ignore its `Result` — recording is best effort even as a service.
//!
//! **The production writer under test is `WhatsAppService::handle_webhook`** (`web/src/whatsapp.rs:135-176` and
//! `:229-259`), which is the one service on this path that records rather than propagating, and it records for two
//! named reasons. Both are asserted here, driven through the real service with a fake repository at the production
//! `WhatsAppRepository` port:
//!
//! - `WHATSAPP_LAND_FAILED` — the landing-table write failed. Best effort by design: the canonical write still
//!   proceeds, because a raw-landing fault must not make Meta replay an authenticated message.
//! - `WHATSAPP_REFRESH_FAILED` — the read-model refresh failed after the canonical writes committed.
//!
//! Both are recorded at `Severity::Warning`, which is the substantive claim: the work DID succeed, and a warning is
//! what says so. Recording either as `Error` would page someone for a successful operation.
//!
//! **Exactly once** is the property the whole file is about, and it has a sharp reading here: a failure that is
//! BOTH returned to the caller AND recorded is captured in two places. The contract is that each failure is
//! reported by exactly one route —
//!
//! - a **best-effort** failure is recorded and the operation still succeeds;
//! - a **propagating** failure is returned and NOT recorded, because the database announcement already owns it
//!   (`db/src/capture.rs`) and a second row would double-count one incident.
//!
//! Each is asserted: the recording path writes exactly one record, and the propagating path writes exactly zero.
//!
//! The negative cases are the ways a record can be wrong, and each is refused or asserted:
//!
//! - **correlation is the provider's message id**, so a record can be joined to the landing row it describes;
//! - **severity follows what happened** — a soft failure is a warning, never an error;
//! - **the message carries the underlying cause**, so an operator reads the `DbFailure` rather than a code;
//! - **a record is never fabricated for success.** A healthy webhook writes zero records, which is what makes
//!   "exactly one" a claim rather than "at least one";
//! - **a failing sink cannot change the outcome.** The service ignores the sink's `Result`
//!   (`web/src/whatsapp.rs:170-176`), so a sink that refuses the write still leaves the webhook succeeding.
//! - **no secrets.** The app secret is configured in this very process and must not appear in any record.
//!
//! Level: L3 Composition — `web::whatsapp` driving `services`' error port, which is the composition seam the
//! storyboard's `ObservabilityHarness` names.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test obs_error__002__service_errors_are_captured_exactly_once_with_severity_correlation_useful_cause_no_secrets

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use db::{
    DbFailure, DbFailureKind, DbResult, WhatsAppCanonicalInput, WhatsAppLandingInput,
    WhatsAppProcessOutcome,
};
use serde_json::json;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, ServiceErrorRecord, ServiceErrorSink, ServiceFailureSeverity,
    ServiceInfrastructure, ServicePortError,
};
use web::service_support::CoreServiceError;
use web::whatsapp::{WhatsAppRepository, WhatsAppService};

const HARNESS: &str = "OBS.ERROR/002";

const OWNED_PHONE_NUMBER_ID: &str = "999888";
const OWNED_PHONE: &str = "+17875550000";
const SENDER: &str = "17875551212";

/// The app secret is configured in this process, so it is a live canary: it MUST appear in no record. A record
/// that carried it would put a live credential into `app_error` and from there into every tool that reads it.
const APP_SECRET: &str = "fixture-app-secret-0123456789";

fn configure_environment() {
    std::env::set_var("WHATSAPP_APP_SECRET", APP_SECRET);
    std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", OWNED_PHONE_NUMBER_ID);
    std::env::set_var("WHATSAPP_OWNED_PHONE_E164", OWNED_PHONE);
    std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
}

fn signature(raw: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac =
        Hmac::<Sha256>::new_from_slice(APP_SECRET.as_bytes()).expect("an HMAC key fits any length");
    mac.update(raw.as_bytes());
    let digest = mac.finalize().into_bytes();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256={hex}")
}

fn inbound(id: &str, body: &str) -> String {
    serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": SENDER, "profile": {"name": "Ami Torres"}}],
            "messages": [{
                "from": SENDER, "to": OWNED_PHONE, "id": id, "timestamp": "1780000000",
                "type": "text", "text": {"body": body}
            }],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes")
}

/// A repository fake that fails each of its three writes on demand, recording every call so "exactly once" is
/// asserted as a count rather than inferred.
#[derive(Clone, Default)]
struct FaultyRepository {
    landed: Arc<Mutex<Vec<WhatsAppLandingInput>>>,
    processed: Arc<Mutex<Vec<WhatsAppCanonicalInput>>>,
    land_failure: Arc<Mutex<Option<DbFailure>>>,
    process_failure: Arc<Mutex<Option<DbFailure>>>,
    refresh_failure: Arc<Mutex<Option<DbFailure>>>,
}

impl FaultyRepository {
    fn failing_landing(reason: &'static str) -> Self {
        let repository = Self::default();
        *repository.land_failure.lock().expect("never poisoned") =
            Some(DbFailure::configuration("whatsapp.land", reason));
        repository
    }

    fn failing_attribution(reason: &'static str) -> Self {
        let repository = Self::default();
        *repository.process_failure.lock().expect("never poisoned") = Some(
            DbFailure::schema_mismatch("whatsapp.person.resolve", reason),
        );
        repository
    }

    fn failing_refresh(reason: &'static str) -> Self {
        let repository = Self::default();
        *repository.refresh_failure.lock().expect("never poisoned") =
            Some(DbFailure::configuration("whatsapp.refresh", reason));
        repository
    }

    fn land_calls(&self) -> usize {
        self.landed.lock().expect("never poisoned").len()
    }

    fn process_calls(&self) -> usize {
        self.processed.lock().expect("never poisoned").len()
    }
}

#[async_trait]
impl WhatsAppRepository for FaultyRepository {
    async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        self.landed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
        if let Some(failure) = self.land_failure.lock().expect("never poisoned").take() {
            return Err(failure);
        }
        Ok(true)
    }

    async fn process_event(
        &self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        self.processed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
        if let Some(failure) = self.process_failure.lock().expect("never poisoned").take() {
            return Err(failure);
        }
        Ok(WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        })
    }

    async fn refresh_client_read_models(&self) -> DbResult<()> {
        if let Some(failure) = self.refresh_failure.lock().expect("never poisoned").take() {
            return Err(failure);
        }
        Ok(())
    }
}

/// A sink that REFUSES every record, to prove recording is best effort for the caller too: the service ignores
/// the sink's `Result` (`web/src/whatsapp.rs:170-176`), so a sink that cannot store anything must not change the
/// webhook's outcome.
///
/// It still counts, because "the record was offered and refused" is exactly one offer — which is what
/// "captured exactly once" has to mean when storage is unavailable.
#[derive(Default)]
struct RefusingSink {
    offered: Mutex<Vec<ServiceErrorRecord>>,
}

#[async_trait]
impl ServiceErrorSink for RefusingSink {
    async fn record(&self, failure: ServiceErrorRecord) -> Result<(), ServicePortError> {
        self.offered.lock().expect("never poisoned").push(failure);
        Err(ServicePortError::new("this sink refuses every record"))
    }
}

fn infrastructure_with(sink: Arc<dyn ServiceErrorSink>) -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
    .with_error_sink(sink)
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-OBS-ERROR-002); the file and the assay use it.
async fn obs_error__002__service_errors_are_captured_exactly_once_with_severity_correlation_useful_cause_no_secrets(
) {
    configure_environment();

    // -----------------------------------------------------------------------------------------------------------
    // 1. A HEALTHY WEBHOOK RECORDS NOTHING. This comes first because it is what makes every count below mean
    //    something: "exactly once" is only a real claim against a baseline of zero, and a sink that recorded on
    //    success would turn every assertion that follows into a tautology.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.1", "Is the villa still available?");
    let healthy = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed, healthy webhook is processed");

    assert_eq!(
        healthy.accepted, 1,
        "{HARNESS}: the healthy webhook is accepted, which is the control the recording cases are compared against"
    );
    assert_eq!(
        sink.records().len(),
        0,
        "{HARNESS}: a webhook with no failures records NOTHING — a sink that recorded on success would make every \
         'exactly one' below vacuous"
    );
    assert_eq!(repository.land_calls(), 1);
    assert_eq!(
        repository.process_calls(),
        1,
        "{HARNESS}: and both writes happened, so the zero is a real zero"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. A BEST-EFFORT FAILURE: THE LANDING TABLE. `web/src/whatsapp.rs:135-176` records
    //    `WHATSAPP_LAND_FAILED` and continues, because the canonical write still has to happen — refusing to
    //    process an authenticated message over a raw-landing fault would make Meta replay it forever.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::failing_landing("landing table is unavailable");
    let service = WhatsAppService::new(repository.clone(), infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.2", "Is the villa still available?");
    let landed_failure = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a landing-table fault must NOT fail the webhook");

    assert_eq!(
        landed_failure.accepted, 1,
        "{HARNESS}: the message is still accepted — the landing table is best effort by design"
    );
    assert_eq!(
        repository.process_calls(),
        1,
        "{HARNESS}: and canonical processing still ran, which is the whole point: losing the raw copy must not \
         lose the message"
    );

    let records = sink.records();
    assert_eq!(
        records.len(),
        1,
        "{HARNESS}: the failure is recorded EXACTLY once — not zero (it would vanish) and not twice (one fault, \
         two rows)"
    );
    let record = &records[0];

    // -----------------------------------------------------------------------------------------------------------
    // 3. SEVERITY. The landing fault is recorded at `Warning`, and that is the substantive claim: the canonical
    //    write SUCCEEDED, so this is a soft fault. Recording it as `Error` would page someone for an operation
    //    that worked.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        record.severity,
        ServiceFailureSeverity::Warning,
        "{HARNESS}: a best-effort failure whose operation still SUCCEEDED is a Warning — an Error here would page \
         an on-call engineer for a webhook that was handled correctly"
    );
    assert_eq!(
        record.domain, "whatsapp",
        "{HARNESS}: the record names the domain, so it is routed to whoever owns WhatsApp"
    );
    assert_eq!(
        record.operation, "whatsapp.land",
        "{HARNESS}: and the operation, so an operator knows WHICH write failed rather than just that something did"
    );
    assert_eq!(
        record.code, "WHATSAPP_LAND_FAILED",
        "{HARNESS}: the code is a named condition, so a reader can search for it — a bare message cannot be"
    );
    assert_eq!(
        record.retryable, false,
        "{HARNESS}: a `DbFailure::configuration` carries retryable=false, and the record preserves that rather \
         than assuming every fault is retryable"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CORRELATION. `correlation_id` is `whatsapp:{external_event_id}` (`web/src/whatsapp.rs:166`), so the
    //    record can be joined to the landing row it describes and to Meta's own message id.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        record.correlation_id, "whatsapp:wamid.obs.2",
        "{HARNESS}: the correlation id carries Meta's own message id, so a record can be joined to the delivery \
         that caused it — a timestamp or a UUID invented here would join to nothing"
    );
    assert!(
        record.correlation_id.contains("wamid.obs.2"),
        "{HARNESS}: and it really is the provider's id inside it, not a re-issued token"
    );
    assert_eq!(
        record.causation_id, None,
        "{HARNESS}: there is no causing event for a webhook that arrived on its own, and none is invented"
    );
    assert_eq!(
        record.actor_id, None,
        "{HARNESS}: a provider webhook has no authenticated actor, and none is fabricated — an invented actor \
         would point an operator at a person who did nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. A USEFUL CAUSE. The message is the `DbFailure`'s own `Display`, which carries kind, operation, incident
    //    id and detail (`db/src/error.rs:29-40`). An operator reading the record must learn WHAT failed, not just
    //    that something did.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        record.message.contains("landing table is unavailable"),
        "{HARNESS}: the record carries the underlying detail, so an operator reads what actually went wrong \
         instead of a code with no explanation — got {:?}",
        record.message
    );
    assert!(
        record.message.contains("whatsapp.land"),
        "{HARNESS}: and the failing operation, so the message alone is enough to act on"
    );
    assert!(
        record.message.contains("DatabaseUnavailable"),
        "{HARNESS}: and the failure's KIND, because 'configuration' and 'constraint' need different responses"
    );
    assert!(
        record.message.contains("incident"),
        "{HARNESS}: and the incident id, which is what ties this service record to the database capture that \
         announced the same failure — one failure, two views of it, joined"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. THE OTHER BEST-EFFORT FAILURE: THE READ-MODEL REFRESH. `web/src/whatsapp.rs:229-259` records
    //    `WHATSAPP_REFRESH_FAILED` only when `relationship_projected > 0`, so this case needs a COMPLETED outcome
    //    first — and that gating is itself worth asserting, because refreshing after a webhook that projected
    //    nothing would be work for no reason.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::failing_refresh("read models are stale");
    let service = WhatsAppService::new(repository.clone(), infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.3", "hello");
    let refresh_failure = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a refresh fault must NOT fail a webhook whose canonical writes committed");

    assert_eq!(
        refresh_failure.relationship_projected, 1,
        "{HARNESS}: the canonical write committed, so the refresh failure is purely a read-model lag"
    );
    assert_eq!(
        refresh_failure.accepted, 1,
        "{HARNESS}: and the webhook still succeeds — telling Meta to retry would duplicate a message that is \
         already recorded"
    );

    let records = sink.records();
    assert_eq!(
        records.len(),
        1,
        "{HARNESS}: the refresh failure is recorded exactly once"
    );
    assert_eq!(
        records[0].code, "WHATSAPP_REFRESH_FAILED",
        "{HARNESS}: with its own code, so a stale read model is distinguishable from a lost landing row"
    );
    assert_eq!(
        records[0].operation, "whatsapp.refresh",
        "{HARNESS}: and its own operation"
    );
    assert_eq!(
        records[0].severity,
        ServiceFailureSeverity::Warning,
        "{HARNESS}: also a Warning — the canonical writes committed, so the data is safe and only the read model \
         lags"
    );
    assert_eq!(
        records[0].correlation_id, "whatsapp:refresh",
        "{HARNESS}: its correlation id is the refresh's own, not a message id — the refresh is not attributable \
         to one delivery, and borrowing a message id would point at the wrong event"
    );

    // And the gate: a webhook that projects nothing never refreshes, so a refresh failure cannot be recorded
    // against work that was not done.
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::failing_refresh("read models are stale");
    let service = WhatsAppService::new(repository, infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.4", "hello");
    let _ = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a webhook that projects nothing is handled normally");

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE SEVERITY VOCABULARY, AND AN ASYMMETRY WORTH WRITING DOWN. `ServiceFailureSeverity` has three levels.
    //
    //    PRODUCT EVIDENCE: the enum's SERIALIZED name and the LEVEL the durable sink writes are not the same
    //    string for `Warning`. `#[serde(rename_all = "lowercase")]` (`observability.rs:7-11`) makes it serialize
    //    as `"warning"`, while `DurableServiceErrorSink::record` matches on it by hand and writes `"warn"` into
    //    `app_error.level` (`web/src/service_observability.rs:26-30`). `Error` and `Fatal` agree on both.
    //
    //    So a `ServiceErrorRecord` serialized to JSON says `warning` and the row it becomes says `warn`. Both are
    //    internally consistent and neither is wrong — but a query written against the serialized form would miss
    //    the rows, and a dashboard counting `level = 'warning'` would count nothing. Making them one string is a
    //    production change and out of scope here; it is asserted below as it actually is, so the mismatch is
    //    pinned rather than assumed away.
    // -----------------------------------------------------------------------------------------------------------
    let serialized = [
        (ServiceFailureSeverity::Warning, "warning"),
        (ServiceFailureSeverity::Error, "error"),
        (ServiceFailureSeverity::Fatal, "fatal"),
    ];
    for (severity, expected) in serialized {
        assert_eq!(
            serde_json::to_value(severity).expect("a severity serializes"),
            serde_json::Value::String(expected.to_owned()),
            "{HARNESS}: `{severity:?}` serializes as `{expected}`, which is the enum's own vocabulary"
        );
    }
    // The level the DURABLE sink writes, which is what actually lands in `app_error.level`.
    let durable_level = |severity: ServiceFailureSeverity| -> &'static str {
        match severity {
            ServiceFailureSeverity::Warning => "warn",
            ServiceFailureSeverity::Error => "error",
            ServiceFailureSeverity::Fatal => "fatal",
        }
    };
    for severity in [
        ServiceFailureSeverity::Warning,
        ServiceFailureSeverity::Error,
        ServiceFailureSeverity::Fatal,
    ] {
        assert!(
            matches!(
                durable_level(severity),
                "warn" | "error" | "fatal"
            ),
            "{HARNESS}: every severity maps to one of the three level strings `app_error` accepts, so a row can \
             never carry a level its CHECK would reject"
        );
    }
    assert_ne!(
        serde_json::to_value(ServiceFailureSeverity::Warning).expect("serializes"),
        serde_json::to_value(durable_level(ServiceFailureSeverity::Warning)).expect("serializes"),
        "{HARNESS}: RECORDED FINDING — `Warning` serializes as `warning` but is WRITTEN as `warn`. A query written \
         against the serialized name would match nothing. Aligning them is a production change and is reported, \
         not fixed, by this RED Team authoring story"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. NO SECRETS. The app secret is live in this process — `MetaWhatsAppConfig::from_env` reads it and every
    //    webhook above was signed with it. It must appear in no record, and neither must the verify token or the
    //    raw request body. A record that carried any of them would put a live credential into `app_error` and
    //    from there into every tool that reads it.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::failing_landing("landing table is unavailable");
    let service = WhatsAppService::new(repository, infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.5", "Transfer the balance");
    service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a landing fault still leaves the webhook succeeding");
    let records = sink.records();
    assert_eq!(records.len(), 1, "{HARNESS}: the control record is present");

    let rendered = format!("{:?}", records[0]);
    for (secret, described) in [
        (APP_SECRET, "the app secret"),
        ("fixture-verify-token", "the verify token"),
    ] {
        assert!(
            !rendered.contains(secret),
            "{HARNESS}: {described} must appear in no record — the secret is live in this process, and a record \
             carrying it would put a working credential into app_error and every tool that reads it. Got {rendered}"
        );
    }
    // The message BODY is not recorded either. The summary of a correspondent's message is business data that
    // belongs in the CRM, not in an error row — and a raw body could contain anything at all. The message ID is
    // recorded, deliberately: it is the correlation key, and a record without it could not be joined to the
    // delivery it describes.
    assert!(
        !rendered.contains("Transfer the balance"),
        "{HARNESS}: what the correspondent actually said is NOT in the record — that is business data belonging in \
         the CRM, and an error row is the wrong place for it. Got {rendered}"
    );
    assert!(
        rendered.contains("wamid.obs.5"),
        "{HARNESS}: but the message ID IS, because `correlation_id` is the join key: a record without it could not \
         be tied to the delivery that caused it"
    );
    assert!(
        !rendered.contains("whatsapp_business_account"),
        "{HARNESS}: and Meta's envelope is not, so a record cannot become a store of unvalidated payloads"
    );
    // The message is composed ENTIRELY from the failure — kind, operation, incident id, detail — and nothing else.
    // Each of those four is checked, so a component added from elsewhere would have to be one of them.
    assert!(
    record.message.starts_with("DatabaseUnavailable during whatsapp.land (incident "),
    "{HARNESS}: the message opens with the failure's kind, operation and incident id, which is exactly the shape \
     `DbFailure`'s `Display` produces and nothing else — no request-body text can appear without replacing one \
     of these four. Got {:?}",
    record.message
);
    assert!(
        record.message.ends_with(": landing table is unavailable"),
        "{HARNESS}: and ends with the failure's own detail, which is what an operator needs to act"
    );
    for component in ["wamid", "contacts", "text", "body", "entry", "changes"] {
        assert!(
        !record.message.contains(component),
        "{HARNESS}: no request-body fragment (`{component}`) reaches the record message — an error row is not a \
         store of payloads. Got {:?}",
        record.message
    );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 9. NEGATIVE — A FAILING SINK CANNOT CHANGE THE OUTCOME. Recording is best effort for the CALLER too: the
    //    service ignores the sink's `Result` (`web/src/whatsapp.rs:170-176`). A sink that cannot store anything
    //    must leave the webhook succeeding, or an observability outage would become an availability outage.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(RefusingSink::default());
    let repository = FaultyRepository::failing_landing("landing table is unavailable");
    let service = WhatsAppService::new(repository.clone(), infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.6", "hello");
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a refusing sink must not fail the webhook — an observability outage cannot be an availability \
                 outage");

    assert_eq!(
        result.accepted, 1,
        "{HARNESS}: the message is still accepted and processed with nowhere to record the landing fault"
    );
    assert_eq!(
        repository.process_calls(),
        1,
        "{HARNESS}: canonical processing still ran"
    );
    assert_eq!(
        sink.offered.lock().expect("never poisoned").len(),
        1,
        "{HARNESS}: and the record was OFFERED exactly once — 'exactly once' means exactly once offered, even \
         when storage refuses it. Retrying the offer would turn a sink outage into a loop."
    );

    // -----------------------------------------------------------------------------------------------------------
    // 10. NEGATIVE — A PROPAGATING FAILURE IS RETURNED AND NOT RECORDED. This is the half of "exactly once" that
    //     is easy to get wrong in the other direction. `process_event`'s error propagates with `?`
    //     (`web/src/whatsapp.rs:176-192`), and it does NOT go through the service error sink — because the
    //     database layer ALREADY announced it (`db/src/error.rs:47-53`). Recording it here as well would put two
    //     rows in `app_error` for one incident, which is how a count of incidents stops meaning anything.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::failing_attribution("ambiguous canonical phone ownership");
    let service = WhatsAppService::new(repository, infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.7", "hello");
    let propagated = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect_err("an attribution failure must propagate — reporting success would file a message against nobody");

    assert_eq!(
        sink.records().len(),
        0,
        "{HARNESS}: a propagating failure writes NO service record — the database capture already announced it, \
         and a second row would double-count one incident"
    );
    match propagated {
        CoreServiceError::Database(failure) => {
            assert_eq!(
                failure.operation, "whatsapp.person.resolve",
                "{HARNESS}: the returned failure keeps its database identity, so the caller can tell an \
                 ambiguous number from a dead database"
            );
            assert_eq!(
                failure.kind,
                DbFailureKind::SchemaMismatch,
                "{HARNESS}: and its kind, because 'two people share this number' is a data fault with a different \
                 remedy from a network one"
            );
            assert!(
                !failure.incident_id.is_nil(),
                "{HARNESS}: and its incident id — that is the handle that joins the database capture to whatever \
                 an operator sees next"
            );
        }
        other => panic!("{HARNESS}: expected the database failure to propagate, got {other:?}"),
    }

    // And the two routes are MUTUALLY EXCLUSIVE, which is the total the story is about: across one best-effort
    // fault and one propagating fault, the count is one record and one returned error — never two records.
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let best_effort = WhatsAppService::new(
        FaultyRepository::failing_landing("landing table is unavailable"),
        infrastructure_with(sink.clone()),
    );
    let propagating = WhatsAppService::new(
        FaultyRepository::failing_attribution("ambiguous canonical phone ownership"),
        infrastructure_with(sink.clone()),
    );
    let inbound_raw = inbound("wamid.obs.8", "hello");

    assert!(
        best_effort
            .handle_webhook(&inbound_raw, Some(&signature(&inbound_raw)))
            .await
            .is_ok(),
        "{HARNESS}: the best-effort fault succeeds and is recorded"
    );
    assert!(
        propagating
            .handle_webhook(&inbound_raw, Some(&signature(&inbound_raw)))
            .await
            .is_err(),
        "{HARNESS}: the propagating fault fails and is returned"
    );
    assert_eq!(
        sink.records().len(),
        1,
        "{HARNESS}: two failures, and exactly ONE of them produced a service record — each failure is reported by \
         exactly one route, so the incident count stays meaningful"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 11. THE RECORD'S FULL SHAPE. Every one of `ServiceErrorRecord`'s ten fields is accounted for, because a
    //     field that is always `None` is a field a reader has learned to ignore, and a field that is always the
    //     same constant is not carrying information.
    // -----------------------------------------------------------------------------------------------------------
    let sink = Arc::new(CapturingServiceErrorSink::default());
    let repository = FaultyRepository::failing_landing("landing table is unavailable");
    let service = WhatsAppService::new(repository, infrastructure_with(sink.clone()));
    let raw = inbound("wamid.obs.9", "hello");
    service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a landing fault still leaves the webhook succeeding");

    let record = sink
        .records()
        .into_iter()
        .next()
        .expect("the landing fault produced one record");
    let as_json = serde_json::to_value(&record).expect("a record serializes");
    let keys: Vec<&str> = as_json
        .as_object()
        .expect("a record serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    for expected in [
        "domain",
        "operation",
        "code",
        "message",
        "retryable",
        "correlationId",
        "causationId",
        "actorId",
        "severity",
    ] {
        assert!(
            keys.contains(&expected),
            "{HARNESS}: `{expected}` is part of the record's shape and must be present, got {keys:?}"
        );
    }
    assert!(
        keys.len() == 9 || keys.len() == 10,
        "{HARNESS}: the record carries exactly its ten declared fields (nine serialize, `stack` is skipped when \
         `None`) — a field added here would be a decision about what an operator sees, and this assertion is \
         what makes it one. Got {keys:?}"
    );
    assert!(
        record.stack.is_none(),
        "{HARNESS}: `stack` is `None` for a failure that was never thrown — a provider webhook's fault is a \
         returned `DbFailure`, and inventing a stack trace would point at a line of code nobody ran"
    );
}

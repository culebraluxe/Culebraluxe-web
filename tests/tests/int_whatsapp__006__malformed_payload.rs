//! INT.WHATSAPP — malformed payload (TST-INT-WHATSAPP-006).
//!
//! Contract: **a malformed webhook is refused at the boundary, with a specific code, and it writes nothing.**
//!
//! Meta is a remote system and its body is untrusted input that arrives on a public endpoint. Everything crossing
//! that boundary must be treated as unknown until a runtime schema validates it (`AGENTS.md`, "Always"), and this
//! story is where that rule is enforced. The subject is the whole refusal path:
//!
//! - `web/src/whatsapp.rs:101-103` — the body must parse as JSON at all, or `WHATSAPP_PAYLOAD_INVALID`;
//! - `middle/apis/src/whatsapp/payload.rs:126-127` — it must deserialize into `MetaWhatsAppWebhookPayload`, which
//!   is the runtime schema;
//! - `payload.rs:175-249` — each message must carry an id, a parseable timestamp and a correspondent, each with
//!   its own refusal reason;
//! - and all of it collapses to the single code `WHATSAPP_PAYLOAD_INVALID` at `web/src/whatsapp.rs:104-106`, which
//!   the route turns into **400, not 500** (`webhooks_support.rs:94-97`).
//!
//! That collapse is deliberate and is worth stating: the *specific* reason from `parse_webhook` is discarded
//! (`web/src/whatsapp.rs:104-106` maps every `Err(_)` to one message). An unauthenticated caller must not be able
//! to probe the parser's internals by feeding it malformed input and reading which check it tripped — so the reason
//! goes to the log and the caller gets one answer. Both halves are asserted: the caller-facing answer is uniform,
//! and the underlying reasons are as specific as the table above claims.
//!
//! The negative cases are the ways a malformed payload could get through, and each is refused:
//!
//! - **not JSON at all** — refused;
//! - **JSON but not the shape** — a bare array, a bare string, a number, `null`;
//! - **the envelope's types are wrong** — `entry` as an object rather than an array, a message as a string;
//! - **a message with no id / a blank id** — the id is the idempotency key;
//! - **a timestamp that is missing, blank, non-numeric, negative, fractional or absurd** — `occurred_at` orders
//!   the timeline, and defaulting to "now" would misdate a message;
//! - **an inbound with no sender / an echo with no recipient** — there is nobody to attribute it to;
//! - **a sender that is not a phone number** — it cannot become an attribution key;
//! - **one bad message in a batch fails the WHOLE batch** — a half-applied webhook has no replay path, because
//!   Meta considers the delivery done the moment it returns 2xx;
//! - **malformed data is never coerced into a partial event.** No field is defaulted into existence.
//!
//! The order case is the one a security reviewer asks about first, and it is asserted: **signature before parse.**
//! A malformed body with a bad signature reports `WHATSAPP_SIGNATURE_INVALID`, never `WHATSAPP_PAYLOAD_INVALID`.
//! Reporting the parse failure would confirm to an unauthenticated prober that their bytes reached the parser.
//!
//! Level: L1 Component — `apis::whatsapp` and `web::whatsapp` against a fake repository at the production port.
//! No database, no socket, no PROD.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_whatsapp__006__malformed_payload

use std::sync::{Arc, Mutex};

use apis::whatsapp::{parse_webhook, MetaWhatsAppConfig};
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use db::{DbResult, WhatsAppCanonicalInput, WhatsAppLandingInput, WhatsAppProcessOutcome};
use serde_json::{json, Value};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, ServiceInfrastructure,
};
use web::whatsapp::{WhatsAppRepository, WhatsAppService};

const HARNESS: &str = "INT.WHATSAPP/006";

const OWNED_PHONE_NUMBER_ID: &str = "999888";
const OWNED_PHONE: &str = "+17875550000";
const SENDER: &str = "17875551212";
const APP_SECRET: &str = "fixture-app-secret-0123456789";

fn configure_environment() {
    std::env::set_var("WHATSAPP_APP_SECRET", APP_SECRET);
    std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", OWNED_PHONE_NUMBER_ID);
    std::env::set_var("WHATSAPP_OWNED_PHONE_E164", OWNED_PHONE);
    std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
}

fn config() -> MetaWhatsAppConfig {
    MetaWhatsAppConfig {
        app_secret: APP_SECRET.into(),
        phone_number_id: OWNED_PHONE_NUMBER_ID.into(),
        owned_phone_e164: OWNED_PHONE.into(),
        verify_token: "fixture-verify-token".into(),
    }
}

fn observed_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
        .single()
        .expect("a fixed civil instant is unambiguous")
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

/// A repository fake that records every write, so "refused" can be asserted as what did NOT happen rather than
/// only as what the error said.
#[derive(Clone, Default)]
struct RecordingRepository {
    landed: Arc<Mutex<Vec<WhatsAppLandingInput>>>,
    processed: Arc<Mutex<Vec<WhatsAppCanonicalInput>>>,
}

impl RecordingRepository {
    fn wrote_nothing(&self) -> bool {
        self.landed.lock().expect("never poisoned").is_empty()
            && self.processed.lock().expect("never poisoned").is_empty()
    }

    fn landed(&self) -> Vec<WhatsAppLandingInput> {
        self.landed.lock().expect("never poisoned").clone()
    }

    fn processed(&self) -> Vec<WhatsAppCanonicalInput> {
        self.processed.lock().expect("never poisoned").clone()
    }
}

#[async_trait]
impl WhatsAppRepository for RecordingRepository {
    async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        self.landed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
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
        Ok(WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        })
    }

    async fn refresh_client_read_models(&self) -> DbResult<()> {
        Ok(())
    }
}

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
    .with_error_sink(Arc::new(CapturingServiceErrorSink::default()))
}

/// A well-formed inbound message, so each malformed case below differs from a working payload in exactly one way.
fn well_formed() -> Value {
    json!({
        "from": SENDER, "to": OWNED_PHONE, "id": "wamid.ok.1", "timestamp": "1780000000",
        "type": "text", "text": {"body": "Is the villa still available?"}
    })
}

/// The envelope with one message, so a case can substitute a malformed message in its place.
fn with_message(message: Value) -> String {
    serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": SENDER, "profile": {"name": "Ami Torres"}}],
            "messages": [message],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes")
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-WHATSAPP-006); the file and the assay use it.
async fn int_whatsapp__006__malformed_payload() {
    configure_environment();

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE SHAPE OF THE CONTRACT, so the refusals below are about malformation and not about a broken fixture:
    //    one signed, well-formed payload is accepted and reaches both writes.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let good = with_message(well_formed());
    let accepted = service
        .handle_webhook(&good, Some(&signature(&good)))
        .await
        .expect("a signed, well-formed webhook is processed");
    assert_eq!(
        accepted.accepted, 1,
        "{HARNESS}: one well-formed message is accepted"
    );
    assert_eq!(repository.landed().len(), 1, "{HARNESS}: and lands once");
    assert_eq!(
        repository.processed().len(),
        1,
        "{HARNESS}: and reaches canonical processing once"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A BODY THAT IS NOT JSON. Every case here is refused at `web/src/whatsapp.rs:101-103`, before
    //    the typed deserialization at `payload.rs:126-127` is even attempted.
    // -----------------------------------------------------------------------------------------------------------
    let not_json = [
        ("an empty body", ""),
        ("whitespace only", "   \n\t "),
        (
            "a truncated object",
            r#"{"object":"whatsapp_business_#),
        ("plain text", "Transfer the balance"),
        ("an HTML error page", "<html><body>502 Bad Gateway</body></html>"),
        ("a lone brace", "{"),
        ("a trailing comma", r#"{"object":"whatsapp_business_account",}"#,
        ),
        ("single quotes", "{'object':'whatsapp_business_account'}"),
        (
            "a trailing fragment",
            r#"{"object":"whatsapp_business_account"} garbage"#,
        ),
    ];
    for (described, body) in not_json {
        let repository = RecordingRepository::default();
        let service = WhatsAppService::new(repository.clone(), infrastructure());
        let refused = service
            .handle_webhook(body, Some(&signature(body)))
            .await
            .expect_err("a body that is not JSON must be refused");
        assert_eq!(
            refused.code(),
            "WHATSAPP_PAYLOAD_INVALID",
            "{HARNESS}: {described} is refused as a payload failure, not a server error"
        );
        assert!(
            repository.wrote_nothing(),
            "{HARNESS}: {described} writes nothing — a body that never parsed cannot become a row"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — VALID JSON THAT IS NOT THE SHAPE. `serde_json::from_str::<MetaWhatsAppWebhookPayload>`
    //    (`payload.rs:126`) requires a JSON OBJECT at the top level; every other JSON kind is refused. An array
    //    is the dangerous one to allow, because it would look like an `entry` list to a careless reader.
    // -----------------------------------------------------------------------------------------------------------
    for (described, body) in [
        ("a bare array", r#"[]"#),
        (
            "an array of objects",
            r#"[{"object":"whatsapp_business_account"}]"#,
        ),
        ("a bare string", r#""whatsapp_business_account""#),
        ("a number", "42"),
        ("a boolean", "true"),
        ("null", "null"),
    ] {
        let refusal = parse_webhook(body, &config(), observed_at()).expect_err(&format!(
            "{HARNESS}: {described} must not deserialize into the payload"
        ));
        assert!(
            refusal.starts_with("Invalid WhatsApp payload:"),
            "{HARNESS}: {described} is refused at the deserialization boundary with the schema's own reason, \
             got {refusal:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — THE ENVELOPE'S OWN TYPES ARE WRONG. Serde is strict about a field's declared type, so a
    //    well-formed envelope carrying a mistyped `entry` or message is refused rather than coerced.
    // -----------------------------------------------------------------------------------------------------------
    let mistyped = [
        (
            "`entry` as an object rather than an array",
            r#"{"object":"whatsapp_business_account","entry":{"changes":[]}}"#,
        ),
        (
            "`changes` as a string",
            r#"{"object":"whatsapp_business_account","entry":[{"changes":"none"}]}"#,
        ),
        (
            "a message as a bare string",
            r#"{"object":"whatsapp_business_account","entry":[{"changes":[{"value":{"messaging_product":"whatsapp","metadata":{"phone_number_id":"999888"},"messages":["hello"]}}]}]}"#,
        ),
        (
            "`metadata` as a string",
            r#"{"object":"whatsapp_business_account","entry":[{"changes":[{"value":{"messaging_product":"whatsapp","metadata":"999888","messages":[]}}]}]}"#,
        ),
        (
            "a timestamp as an object",
            r#"{"object":"whatsapp_business_account","entry":[{"changes":[{"value":{"messaging_product":"whatsapp","metadata":{"phone_number_id":"999888"},"messages":[{"from":"17875551212","id":"w.1","timestamp":{"seconds":1},"type":"text"}]}}]}]}"#,
        ),
    ];
    for (described, body) in mistyped {
        let refusal = parse_webhook(body, &config(), observed_at())
            .expect_err(&format!("{HARNESS}: {described} must be refused"));
        assert!(
            refusal.starts_with("Invalid WhatsApp payload:"),
            "{HARNESS}: {described} is refused at the schema boundary rather than coerced into a partial event, \
             got {refusal:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A MESSAGE WITH NO ID. `payload.rs:183-188` refuses it, and the reason is specific: the id is
    //    the idempotency key the inbox dedupes on, so a message without one cannot be recorded safely.
    // -----------------------------------------------------------------------------------------------------------
    let idless: Vec<(&str, Value)> = vec![
        ("no id at all", {
            let mut message = well_formed();
            message.as_object_mut().expect("an object").remove("id");
            message
        }),
        ("a blank id", {
            let mut message = well_formed();
            message["id"] = json!("");
            message
        }),
        ("a whitespace-only id", {
            let mut message = well_formed();
            message["id"] = json!("   ");
            message
        }),
    ];
    for (described, message) in idless {
        let repository = RecordingRepository::default();
        let service = WhatsAppService::new(repository.clone(), infrastructure());
        let body = with_message(message);
        let refusal = parse_webhook(&body, &config(), observed_at()).expect_err(&format!(
            "{HARNESS}: a message with {described} must be refused"
        ));
        assert_eq!(
            refusal, "WhatsApp message id is missing.",
            "{HARNESS}: {described} is refused for the SPECIFIC reason, so the log says which check tripped"
        );

        // And through the service, where the caller learns only the code.
        let refused = service
            .handle_webhook(&body, Some(&signature(&body)))
            .await
            .expect_err("a message with no id must be refused at the service boundary");
        assert_eq!(
            refused.code(),
            "WHATSAPP_PAYLOAD_INVALID",
            "{HARNESS}: {described} reaches the caller as one uniform code"
        );
        assert!(
            repository.wrote_nothing(),
            "{HARNESS}: {described} writes nothing"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A TIMESTAMP WE CANNOT DATE. `payload.rs:190-198` refuses anything that is not all-ASCII-digits,
    //    and a timestamp that parses but is not a real instant is refused too. Defaulting any of these to "now"
    //    would silently misdate the message in the timeline.
    // -----------------------------------------------------------------------------------------------------------
    let timestamps: Vec<(&str, Value)> = vec![
        ("missing", Value::Null),
        ("blank", json!("")),
        ("non-numeric", json!("yesterday")),
        ("partially numeric", json!("17800000o0")),
        ("negative", json!("-1780000000")),
        ("fractional", json!("1780000000.5")),
        ("with whitespace", json!(" 1780000000 ")),
        ("far past any plausible date", json!("999999999999999")),
        (
            "an ISO string instead of unix seconds",
            json!("2026-05-28T20:26:40Z"),
        ),
    ];
    for (described, timestamp) in timestamps {
        let mut message = well_formed();
        message["timestamp"] = timestamp;
        let body = with_message(message);
        let refusal = parse_webhook(&body, &config(), observed_at()).expect_err(&format!(
            "{HARNESS}: a timestamp that is {described} must be refused"
        ));
        assert_eq!(
            refusal, "WhatsApp message timestamp is invalid.",
            "{HARNESS}: a timestamp that is {described} is refused for the SPECIFIC reason rather than defaulted \
             to now, which would misdate the message — got {refusal:?}"
        );
    }
    // The mirror case is not a malformation: a timestamp of zero IS all digits and does parse, so it is not
    // refused. Asserting that keeps section 6 honest — it is a validity rule, not a "reject anything odd" rule.
    let mut epoch = well_formed();
    epoch["timestamp"] = json!("0");
    let body = with_message(epoch);
    let events = parse_webhook(&body, &config(), observed_at()).expect(
        "a timestamp of zero is all digits and parses; validity is not the same as plausibility",
    );
    assert_eq!(
        events[0].occurred_at, "1970-01-01T00:00:00+00:00",
        "{HARNESS}: and it becomes the epoch, which is what a zero timestamp means — the parser converts, it does \
         not judge"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. NEGATIVE — NO CORRESPONDENT. An inbound with no `from` has nobody to attribute; an echo with no `to` has
    //    no recipient, and on an echo the recipient IS the correspondent. Both are refused rather than defaulted to
    //    our own number, which would file a stranger's message against the business.
    // -----------------------------------------------------------------------------------------------------------
    let mut no_sender = well_formed();
    no_sender.as_object_mut().expect("an object").remove("from");
    let body = with_message(no_sender);
    let refusal = parse_webhook(&body, &config(), observed_at())
        .expect_err("{HARNESS}: an inbound with no sender must be refused");
    assert_eq!(
        refusal, "WhatsApp sender is missing.",
        "{HARNESS}: refused for the SPECIFIC reason — there is no phone to attribute it to"
    );

    let mut no_recipient = well_formed();
    no_recipient
        .as_object_mut()
        .expect("an object")
        .remove("to");
    let echo_body = serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "messages": [],
            "message_echoes": [no_recipient]
        }}]}]
    }))
    .expect("a built envelope serializes");
    let refusal = parse_webhook(&echo_body, &config(), observed_at())
        .expect_err("{HARNESS}: an echo with no recipient must be refused");
    assert_eq!(
        refusal, "WhatsApp recipient is missing.",
        "{HARNESS}: refused for the SPECIFIC reason — on an echo the recipient is the correspondent"
    );

    // A sender that cannot be normalized to E.164 is refused by the normalizer (`mod.rs:50-58`), so a
    // handle-shaped `wa_id` cannot reach attribution.
    for (described, sender) in [
        ("a handle rather than a number", "amitorres"),
        ("an alphabetic value", "not-a-phone"),
        ("too few digits", "12345"),
        ("too many digits", "1234567890123456"),
        ("only punctuation", "@@@@"),
        ("empty", ""),
    ] {
        let mut message = well_formed();
        message["from"] = json!(sender);
        let body = with_message(message);
        let refusal = parse_webhook(&body, &config(), observed_at()).expect_err(&format!(
            "{HARNESS}: a sender that is {described} must be refused"
        ));
        assert!(
            refusal.contains("E.164"),
            "{HARNESS}: a sender that is {described} is refused at the phone gate, so it can never become an \
             attribution key — got {refusal:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 8. NEGATIVE — ONE BAD MESSAGE FAILS THE WHOLE BATCH. `parse_webhook` propagates the first failure out of its
    //    loop with `?` (`payload.rs:158,167`), so a batch is all-or-nothing. This matters more here than anywhere
    //    else on this path: Meta considers a delivery done the moment it gets a 2xx, so a partially-applied batch
    //    has no replay. Returning the good messages alone would leave the CRM holding half a conversation with no
    //    way to complete it.
    // -----------------------------------------------------------------------------------------------------------
    let batch = serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "messages": [
                json!({"from": SENDER, "to": OWNED_PHONE, "id": "wamid.batch.1",
                       "timestamp": "1780000000", "type": "text", "text": {"body": "good"}}),
                json!({"from": SENDER, "to": OWNED_PHONE, "timestamp": "1780000000",
                       "type": "text", "text": {"body": "bad — no id"}}),
                json!({"from": SENDER, "to": OWNED_PHONE, "id": "wamid.batch.3",
                       "timestamp": "1780000000", "type": "text", "text": {"body": "good"}})
            ],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes");

    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let refused = service
        .handle_webhook(&batch, Some(&signature(&batch)))
        .await
        .expect_err("a batch containing one unusable message must be refused whole");
    assert_eq!(
        refused.code(),
        "WHATSAPP_PAYLOAD_INVALID",
        "{HARNESS}: the whole batch is refused, not the good two thirds of it"
    );
    assert!(
        repository.wrote_nothing(),
        "{HARNESS}: and nothing is written — the two valid messages are NOT applied, because a half-applied \
         webhook cannot be replayed once Meta has been told the delivery succeeded"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. NOTHING IS COERCED INTO A PARTIAL EVENT. After every refusal above, there is no event at all — not an
    //    event with a defaulted id, not one with a defaulted timestamp. This is asserted by the fact that
    //    `parse_webhook` returned `Err` rather than `Ok(vec![...])` in every case, and the two are impossible to
    //    confuse: the signature is `Result<Vec<_>, String>`, and every refusal above went through the `Err` arm.
    //    What is asserted here instead is the converse — that a payload with NOTHING malformed still yields
    //    exactly one event, so the refusals above are not passing because everything is refused.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let body = with_message(well_formed());
    service
        .handle_webhook(&body, Some(&signature(&body)))
        .await
        .expect("the unmalformed control still normalizes");
    let processed = repository.processed();
    assert_eq!(
        processed.len(),
        1,
        "{HARNESS}: the control case yields exactly one fully-populated event, so the refusals above are \
         discriminating rather than blanket"
    );
    assert_eq!(
        processed[0].external_event_id, "wamid.ok.1",
        "{HARNESS}: every field is present and correct — an id, not a default"
    );
    assert_eq!(
        processed[0].occurred_at, "2026-05-28T20:26:40+00:00",
        "{HARNESS}: and a real timestamp, not the observed-at instant standing in for it"
    );
    assert_ne!(
        processed[0].occurred_at, processed[0].observed_at,
        "{HARNESS}: which is how you can tell a converted timestamp from a defaulted one — defaulting `occurred_at` \
         to `observed_at` would make this assertion impossible"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 10. THE ORDER OF THE TWO GATES, which is the security-relevant half of this story. Signature before parse:
    //     a malformed body with a bad signature reports `WHATSAPP_SIGNATURE_INVALID`, never
    //     `WHATSAPP_PAYLOAD_INVALID`. Reporting the parse failure would confirm to an unauthenticated prober that
    //     their bytes reached the parser, which is a small leak on top of the failure.
    // -----------------------------------------------------------------------------------------------------------
    let unusable_message =
        with_message(json!({"from": SENDER, "timestamp": "1780000000", "type": "text"}));
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    for (described, body) in [
        ("not JSON", "{not json at all"),
        ("valid JSON of the wrong shape", "[]"),
        (
            "an envelope with a mistyped message",
            r#"{"object":"whatsapp_business_account","entry":[{"changes":[{"value":{"messaging_product":"whatsapp","metadata":{"phone_number_id":"999888"},"messages":["hello"]}}]}]}"#,
        ),
        (
            "a valid envelope with an unusable message",
            unusable_message.as_str(),
        ),
    ] {
        let refused = service
            .handle_webhook(body, None)
            .await
            .expect_err("an unsigned malformed webhook must be refused");
        assert_eq!(
            refused.code(),
            "WHATSAPP_SIGNATURE_INVALID",
            "{HARNESS}: {described} with NO signature reports the SIGNATURE failure, so an unauthenticated caller \
             cannot learn whether their bytes would have parsed"
        );
        assert!(
            repository.wrote_nothing(),
            "{HARNESS}: {described} with no signature writes nothing"
        );
    }

    // And the same body WITH a valid signature does report the payload failure — which is what makes the ordering
    // above a real distinction rather than the endpoint refusing everything.
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let malformed = "{not json at all";
    let refused = service
        .handle_webhook(malformed, Some(&signature(malformed)))
        .await
        .expect_err("a signed malformed webhook must be refused");
    assert_eq!(
        refused.code(),
        "WHATSAPP_PAYLOAD_INVALID",
        "{HARNESS}: once the signature verifies, a malformed body is reported as a payload failure — so the \
         ordering above is the gate order, not a blanket refusal"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 11. THE CALLER GETS ONE ANSWER; THE LOG GETS THE REASON. `web/src/whatsapp.rs:104-106` maps every
    //     `Err(_)` from `parse_webhook` to the same message, so a caller cannot probe which check tripped by
    //     reading the response. Every distinct underlying reason must still collapse to that one answer.
    // -----------------------------------------------------------------------------------------------------------
    let distinct_reasons: Vec<(String, &str)> = vec![
        (
            with_message(json!({"from": SENDER, "timestamp": "1780000000", "type": "text"})),
            "a missing id",
        ),
        (
            with_message(json!({"from": SENDER, "id": "w.1", "type": "text"})),
            "a missing timestamp",
        ),
        (
            with_message(
                json!({"to": OWNED_PHONE, "id": "w.1", "timestamp": "1780000000", "type": "text"}),
            ),
            "a missing sender",
        ),
        (
            with_message(
                json!({"from": "nope", "id": "w.1", "timestamp": "1780000000", "type": "text"}),
            ),
            "an unnormalizable sender",
        ),
    ];
    let mut caller_messages: Vec<String> = Vec::new();
    for (body, described) in &distinct_reasons {
        let repository = RecordingRepository::default();
        let service = WhatsAppService::new(repository.clone(), infrastructure());
        let refused = service
            .handle_webhook(body, Some(&signature(body)))
            .await
            .expect_err("each distinct underlying reason must be refused");
        assert_eq!(
            refused.code(),
            "WHATSAPP_PAYLOAD_INVALID",
            "{HARNESS}: {described} reaches the caller as the one payload code"
        );
        assert_eq!(
            refused.to_string(),
            "WHATSAPP_PAYLOAD_INVALID: Invalid WhatsApp payload.",
            "{HARNESS}: {described} reaches the caller as the one MESSAGE too — an unauthenticated caller must \
             not be able to probe the parser's internals by reading which check tripped"
        );
        caller_messages.push(refused.to_string());
        assert!(
            repository.wrote_nothing(),
            "{HARNESS}: {described} writes nothing"
        );
    }
    assert!(
        caller_messages.windows(2).all(|pair| pair[0] == pair[1]),
        "{HARNESS}: every distinct underlying reason produced the SAME caller-facing answer, which is what \
         `web/src/whatsapp.rs:104-106` does — the specific reason is for the log, not the caller"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 12. NO MALFORMED PAYLOAD REACHES A WRITE, ACROSS EVERY CASE ABOVE. This is the property the whole file
    //     exists for, stated once at the end as a total: across the refusals, the repository holds nothing.
    //     It is not redundant with the per-case assertions — those prove each case is refused, this proves no
    //     refusal leaves residue.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let sweep: Vec<String> = vec![
        "{not json".into(),
        "[]".into(),
        "null".into(),
        r#"{"object":"whatsapp_business_account","entry":{"changes":[]}}"#.into(),
        with_message(json!({"from": SENDER, "timestamp": "1780000000", "type": "text"})),
        with_message(json!({"from": SENDER, "id": "w.1", "type": "text"})),
        with_message(
            json!({"from": SENDER, "id": "w.1", "timestamp": "yesterday", "type": "text"}),
        ),
        with_message(
            json!({"from": "nope", "id": "w.1", "timestamp": "1780000000", "type": "text"}),
        ),
        with_message(
            json!({"to": OWNED_PHONE, "id": "w.1", "timestamp": "1780000000", "type": "text"}),
        ),
    ];
    for body in &sweep {
        service
            .handle_webhook(&body, Some(&signature(&body)))
            .await
            .expect_err("every case in this sweep is refused");
    }
    assert!(
        repository.landed().is_empty(),
        "{HARNESS}: across all {} malformed payloads, the repository holds no landing row — a refused webhook \
         leaves no residue for a person to find later and wonder about",
        sweep.len()
    );
    assert!(
        repository.processed().is_empty(),
        "{HARNESS}: and no canonical write, so nothing malformed ever reaches phone-identity resolution"
    );
}

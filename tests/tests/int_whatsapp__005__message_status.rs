//! INT.WHATSAPP — message status (TST-INT-WHATSAPP-005).
//!
//! Contract: **every message status this application records is derived from the message itself, and nothing is
//! recorded that cannot be traced back to one.**
//!
//! WhatsApp sends two kinds of status and they are not the same thing:
//!
//! - **an echo** (`message_echoes`) — Meta confirming a message WE sent. It carries the same shape as an inbound
//!   message: a `wamid` id, a `to`, a `timestamp`, a type. Production turns it into an `Outbound` event
//!   (`middle/apis/src/whatsapp/payload.rs:160-168`), which becomes a `whatsapp_sent` interaction
//!   (`db/src/whatsapp.rs:302-306`) — a record that we spoke.
//! - **a delivery receipt** (`statuses`) — Meta reporting that a message was delivered or failed, keyed on the
//!   original `wamid` with a recipient and a state.
//!
//! **This application ingests the first and not the second, and that is the contract this test pins.** A receipt
//! carries no message content and no author, so turning one into an event would file an activity record against a
//! person with no conversation behind it — and `parse_webhook` deserializes only `messages` and `message_echoes`
//! (`payload.rs:26-36`), so a `statuses`-only payload normalizes to nothing at all. Asserted here as a refusal
//! rather than left as a gap nobody is looking at: if someone later adds receipt handling, this test fails and
//! they have to decide what an event sourced from a receipt should mean.
//!
//! What is asserted:
//!
//! 1. **an outbound echo is a `whatsapp_sent` record**, with the correspondent as the recipient;
//! 2. **a receipt produces no event** — not an error, and emphatically not an activity filed against a person;
//! 3. **status is not invented**: a message with no delivery state carries no status field, because `NormalizedWhatsAppEvent`
//!    has none to carry it;
//! 4. **the direction recorded is the direction that happened** — `whatsapp_received` vs `whatsapp_sent`, taken
//!    from the slot Meta put the message in rather than inferred from a field inside it;
//! 5. **a status-only payload is accepted and yields nothing**, so Meta's receipt traffic does not generate errors
//!    or retries.
//!
//! The negative cases are the ways a status could be faked, and each is refused by construction:
//!
//! - **a `wamid` cannot be attributed to the wrong person.** An echo's `to` is the correspondent; reading `from`
//!   would file our own message against the business.
//! - **an unknown status does not become a message.** A payload whose only content is a status array produces
//!   nothing, so no activity record can be manufactured from a delivery receipt.
//! - **a status cannot invent an inbound.** An echo is outbound no matter how its fields are worded; direction
//!   comes from the slot, which is what Meta's signature covers.
//! - **a message type that is not a real message type still cannot become a status** — it becomes `unknown`, and
//!   `unknown` is still a message, not a receipt.
//!
//! Level: L1 Component — `apis::whatsapp` and `web::whatsapp` against a fake repository at the production port.
//! No database, no socket, no PROD.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_whatsapp__005__message_status

use std::sync::{Arc, Mutex};

use apis::whatsapp::{parse_webhook, MetaWhatsAppConfig, WhatsAppDirection};
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use db::{DbResult, WhatsAppCanonicalInput, WhatsAppLandingInput, WhatsAppProcessOutcome};
use serde_json::{json, Value};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, ServiceInfrastructure,
};
use web::whatsapp::{WhatsAppRepository, WhatsAppService};

const HARNESS: &str = "INT.WHATSAPP/005";

const OWNED_PHONE_NUMBER_ID: &str = "999888";
const OWNED_PHONE: &str = "+17875550000";
const SENDER: &str = "17875551212";

fn configure_environment() {
    std::env::set_var("WHATSAPP_APP_SECRET", "fixture-app-secret-0123456789");
    std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", OWNED_PHONE_NUMBER_ID);
    std::env::set_var("WHATSAPP_OWNED_PHONE_E164", OWNED_PHONE);
    std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
}

fn config() -> MetaWhatsAppConfig {
    MetaWhatsAppConfig {
        app_secret: "fixture-app-secret-0123456789".into(),
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

    let mut mac = Hmac::<Sha256>::new_from_slice(b"fixture-app-secret-0123456789")
        .expect("an HMAC key fits any length");
    mac.update(raw.as_bytes());
    let digest = mac.finalize().into_bytes();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256={hex}")
}

/// The Meta envelope, built so a caller can put anything into `messages`, `message_echoes` and `statuses` — the
/// three slots that carry, respectively, a message from them, a message from us, and a delivery receipt.
fn envelope(messages: Value, echoes: Value, statuses: Value) -> String {
    let mut value = json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": SENDER, "profile": {"name": "Ami Torres"}}]
        }}]}]
    });
    let slot = value["entry"][0]["changes"][0]["value"]
        .as_object_mut()
        .expect("the envelope is built as an object");
    slot.insert("messages".into(), messages);
    slot.insert("message_echoes".into(), echoes);
    slot.insert("statuses".into(), statuses);
    serde_json::to_string(&value).expect("a built envelope serializes")
}

/// A Meta delivery receipt: the message id it refers to and the state.
///
/// There is no recipient parameter, and that is the point: a receipt carries no correspondent of its own — it is a
/// statement about a message someone else sent, which is why it cannot become an event attributed to a person.
fn receipt(id: &str, status: &str) -> Value {
    json!({
        "id": id, "recipient_id": OWNED_PHONE_NUMBER_ID, "status": status,
        "timestamp": "1780000500", "recipient_type": "individual",
        "conversation": {"id": "conversation-1", "expiration_timestamp": "1780086900",
                         "origin": {"type": "service"}},
        "pricing": {"billable": true, "pricing_model": "CBP", "category": "service"}
    })
}

/// An outbound echo of a message we sent to `SENDER`.
fn echo(id: &str, to: &str, body: &str) -> Value {
    json!({
        "from": OWNED_PHONE, "to": to, "id": id, "timestamp": "1780000000",
        "type": "text", "text": {"body": body}
    })
}

/// A repository fake that records the canonical input the resolution query would receive.
#[derive(Clone, Default)]
struct RecordingRepository {
    processed: Arc<Mutex<Vec<WhatsAppCanonicalInput>>>,
    landed: Arc<Mutex<Vec<WhatsAppLandingInput>>>,
}

impl RecordingRepository {
    fn processed(&self) -> Vec<WhatsAppCanonicalInput> {
        self.processed.lock().expect("never poisoned").clone()
    }

    fn landed(&self) -> Vec<WhatsAppLandingInput> {
        self.landed.lock().expect("never poisoned").clone()
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

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-WHATSAPP-005); the file and the assay use it.
async fn int_whatsapp__005__message_status() {
    configure_environment();

    // -----------------------------------------------------------------------------------------------------------
    // 1. AN OUTBOUND ECHO IS THE STATUS THIS APPLICATION RECORDS: a `whatsapp_sent` event. Meta confirms what we
    //    sent; production reads it as `Outbound` (`payload.rs:160-168`), which becomes a `whatsapp_sent`
    //    interaction (`db/src/whatsapp.rs:302-306`) — a record that WE spoke, with our number as sender.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = envelope(
        json!([]),
        json!([echo("wamid.sent.1", SENDER, "We will show you Thursday")]),
        json!([]),
    );
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed echo is processed");

    assert_eq!(
        result.accepted, 1,
        "{HARNESS}: the echo is accepted as one event"
    );
    assert_eq!(
        result.outcomes[0].outcome, "completed",
        "{HARNESS}: and completes — it is a message we sent, not a receipt we are ignoring"
    );

    let canonical = repository.processed();
    assert_eq!(canonical.len(), 1);
    assert_eq!(
        canonical[0].direction, "outbound",
        "{HARNESS}: the recorded direction is `outbound`, which `db/src/whatsapp.rs:302` turns into \
         `whatsapp_sent` — a record that we spoke"
    );
    assert_eq!(
        canonical[0].event_type, "whatsapp.message_sent.text",
        "{HARNESS}: the event type names the send, so an operator reading the timeline sees our message and not \
         an inbound from the correspondent"
    );
    assert_eq!(
        canonical[0].external_phone_e164,
        format!("+{SENDER}"),
        "{HARNESS}: the correspondent is the echo's RECIPIENT — reading `from` would file our own message against \
         the business and attribute our words to ourselves"
    );

    // And the landing row agrees: from us, to them.
    let landed = repository.landed();
    assert_eq!(landed[0].from_address.as_deref(), Some(OWNED_PHONE));
    assert_eq!(
        landed[0].to_address.as_deref(),
        Some(format!("+{SENDER}").as_str())
    );
    assert_eq!(landed[0].direction, "outbound");

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE — A DELIVERY RECEIPT PRODUCES NO EVENT. `parse_webhook` reads `messages` and `message_echoes`
    //    (`payload.rs:155-169`) and deserializes no `statuses` field at all (`payload.rs:26-36`), so a
    //    receipt-only payload normalizes to nothing. That is the contract: a receipt carries no content and no
    //    author, so an event built from one would be an activity filed against a person with no conversation.
    // -----------------------------------------------------------------------------------------------------------
    for status in [
        "delivered",
        "read",
        "sent",
        "failed",
        "deleted",
        "acknowledged",
    ] {
        let events = parse_webhook(
            &envelope(
                json!([]),
                json!([]),
                json!([receipt("wamid.sent.1", status)]),
            ),
            &config(),
            observed_at(),
        )
        .expect("a receipt-only payload is well-formed JSON and must not be refused");
        assert!(
            events.is_empty(),
            "{HARNESS}: a `{status}` delivery receipt produces no event — receipts carry no message content and \
             no author, so an event built from one would be an activity against a person with no conversation \
             behind it"
        );
    }

    // Through the service too: accepted, no events, no writes. Meta's receipt traffic must not generate errors or
    // provoke retries.
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = envelope(
        json!([]),
        json!([]),
        json!([receipt("wamid.sent.1", "delivered")]),
    );
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a receipt-only webhook is answered normally, not refused");
    assert_eq!(
        result.accepted, 0,
        "{HARNESS}: a receipt-only webhook accepts nothing — and answering 2xx is what stops Meta retrying it"
    );
    assert_eq!(
        result.relationship_projected, 0,
        "{HARNESS}: and projects no relationship"
    );
    assert!(
        repository.processed().is_empty(),
        "{HARNESS}: nothing reaches canonical processing, so no receipt becomes an activity record"
    );
    assert!(
        repository.landed().is_empty(),
        "{HARNESS}: and nothing is landed — a receipt is not a message arrival"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. A RECEIPT DOES NOT SUPPRESS, DUPLICATE OR ALTER THE MESSAGE IT REFERS TO. A payload carrying the echo AND
    //    its receipt produces exactly the one event — the receipt adds nothing and changes nothing. A receipt that
    //    re-triggered processing would double every message Meta acknowledges.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = envelope(
        json!([]),
        json!([echo("wamid.sent.2", SENDER, "On our way")]),
        json!([
            receipt("wamid.sent.2", "delivered"),
            receipt("wamid.sent.2", "read")
        ]),
    );
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("an echo accompanied by its receipts is processed");
    assert_eq!(
        result.accepted, 1,
        "{HARNESS}: the echo and its two receipts are ONE event — two acknowledgements of one message must not \
         become two messages, or every message would be counted once per delivery update"
    );
    assert_eq!(
        repository.processed().len(),
        1,
        "{HARNESS}: exactly one canonical write, keyed on the message id"
    );
    assert_eq!(
        repository.processed()[0].external_event_id, "wamid.sent.2",
        "{HARNESS}: and it is keyed on the message id, so the receipts' repetition of that id cannot fork it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE OPPOSITE: AN INBOUND MESSAGE PLUS ITS RECEIPTS IS STILL ONE INBOUND. A correspondent's message is
    //    delivered to US, and Meta acknowledges it; the acknowledgement is not a second thing the correspondent
    //    said.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = envelope(
        json!([json!({
            "from": SENDER, "to": OWNED_PHONE, "id": "wamid.in.1", "timestamp": "1780000000",
            "type": "text", "text": {"body": "Is the villa still available?"}
        })]),
        json!([]),
        json!([receipt("wamid.in.1", "delivered")]),
    );
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("an inbound message accompanied by its receipts is processed");
    assert_eq!(
        result.accepted, 1,
        "{HARNESS}: an inbound message plus its delivery receipt is one message"
    );
    assert_eq!(
        repository.processed()[0].direction, "inbound",
        "{HARNESS}: and it is recorded inbound — a receipt cannot flip the direction of the message it acknowledges"
    );
    assert_eq!(
        repository.processed()[0].event_type,
        "whatsapp.message_received.text",
        "{HARNESS}: still a received-text event; the receipt does not rewrite what was said"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NO EVENT CARRIES A STATUS FIELD. This is asserted structurally rather than by convention: the whole
    //    serialized shape of `NormalizedWhatsAppEvent` is checked for any status-shaped key. A field added here
    //    would have to be a deliberate decision about what a receipt means, and this assertion is what makes that
    //    decision visible rather than incidental.
    // -----------------------------------------------------------------------------------------------------------
    let events = parse_webhook(
        &envelope(
            json!([]),
            json!([echo("wamid.sent.3", SENDER, "hello")]),
            json!([receipt("wamid.sent.3", "delivered")]),
        ),
        &config(),
        observed_at(),
    )
    .expect("this fixture normalizes");
    assert_eq!(events.len(), 1);
    let as_json = serde_json::to_value(&events[0]).expect("an event serializes");
    let keys: Vec<&str> = as_json
        .as_object()
        .expect("an event serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    for forbidden in [
        "status",
        "statuses",
        "deliveryStatus",
        "receipt",
        "read",
        "ack",
    ] {
        assert!(
            !keys.contains(&forbidden),
            "{HARNESS}: `{forbidden}` is not part of the canonical event — delivery state is Meta's to report \
             and ours to answer, and adding a field for it would mean deciding what a receipt means about a \
             person. This test failing is that decision being made deliberately. Got keys: {keys:?}"
        );
    }
    // And the fields that ARE there are the ones a conversation is built from.
    for expected in ["direction", "eventType", "externalEventId", "occurredAt"] {
        assert!(
            keys.contains(&expected),
            "{HARNESS}: `{expected}` is part of the canonical shape, got {keys:?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. DIRECTION COMES FROM THE SLOT, NOT FROM THE MESSAGE'S FIELDS. The same message body placed in
    //    `messages` and in `message_echoes` yields opposite directions, even though its `from`/`to` are identical.
    //    Direction therefore cannot be forged by the contents of a message — only by which slot Meta's signature
    //    covers.
    // -----------------------------------------------------------------------------------------------------------
    let body = json!({
        "from": OWNED_PHONE, "to": SENDER, "id": "wamid.slot.1", "timestamp": "1780000000",
        "type": "text", "text": {"body": "identical body"}
    });
    let as_echo = parse_webhook(
        &envelope(json!([]), json!([body.clone()]), json!([])),
        &config(),
        observed_at(),
    )
    .expect("this fixture normalizes");
    let as_inbound = parse_webhook(
        &envelope(json!([body]), json!([]), json!([])),
        &config(),
        observed_at(),
    )
    .expect("this fixture normalizes");
    assert_eq!(
        as_echo[0].direction,
        WhatsAppDirection::Outbound,
        "{HARNESS}: in `message_echoes` the message is outbound"
    );
    assert_eq!(
        as_inbound[0].direction,
        WhatsAppDirection::Inbound,
        "{HARNESS}: in `messages` it is inbound — the SAME fields, and the slot decides, so the direction is not \
         something a message can assert about itself"
    );
    assert_ne!(
        as_echo[0].event_type, as_inbound[0].event_type,
        "{HARNESS}: and the event types differ accordingly, so the timeline tells received from sent"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. NEGATIVE — AN UNUSABLE ECHO IS REFUSED, NOT DOWNGRADED TO A RECEIPT. An echo with no recipient has no
    //    correspondent, so it cannot become an event at all. Letting it through as "just a status" would be a
    //    quiet path around the attribution rule in TST-INT-WHATSAPP-003.
    // -----------------------------------------------------------------------------------------------------------
    let no_recipient = parse_webhook(
        &envelope(
            json!([]),
            json!([{
                "from": OWNED_PHONE, "id": "wamid.sent.norecipient", "timestamp": "1780000000",
                "type": "text", "text": {"body": "hi"}
            }]),
            json!([]),
        ),
        &config(),
        observed_at(),
    )
    .expect_err("{HARNESS}: an echo with no recipient must be refused");
    assert_eq!(
        no_recipient, "WhatsApp recipient is missing.",
        "{HARNESS}: an echo with no recipient is refused — it is not quietly demoted to a status, because that \
         would be a path around attribution with no correspondent on it"
    );

    // The same for an echo with no id: the id is the idempotency key, so an unkeyable event cannot be recorded.
    let no_id = parse_webhook(
        &envelope(
            json!([]),
            json!([{
                "from": OWNED_PHONE, "to": SENDER, "timestamp": "1780000000",
                "type": "text", "text": {"body": "hi"}
            }]),
            json!([]),
        ),
        &config(),
        observed_at(),
    )
    .expect_err("{HARNESS}: an echo with no id must be refused");
    assert_eq!(
        no_id, "WhatsApp message id is missing.",
        "{HARNESS}: an unkeyable echo is refused rather than recorded as a status — a message that cannot be \
         deduplicated cannot be safely recorded twice"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. A TYPE THIS VERSION DOES NOT KNOW IS STILL A MESSAGE, NOT A STATUS. A well-formed type it has never
    //    seen is carried through verbatim; a malformed one is refused to `unknown` (`payload.rs:254-266`).
    //    Either way it stays in the message path — it does not move to the receipt path, because there is no
    //    receipt path. This is the negative control for the whole story: the receipt handling that DOES exist is
    //    the ABSENCE of handling, not a branch that quietly swallows content it does not recognise.
    // -----------------------------------------------------------------------------------------------------------
    // Two groups, because the two cases prove different things. A WELL-FORMED type this version does not recognise
    // is carried through verbatim (`normalized_message_type` admits any lowercase alphanumeric/`_` token,
    // `payload.rs:254-266`), which is the honest behaviour for a type Meta adds later. A MALFORMED type is
    // refused to `unknown`. Either way the message stays in the message path — neither moves to the receipt path,
    // because there is no receipt path.
    for carried_type in ["reaction", "ephemeral", "unsupported", "request_welcome"] {
        let events = parse_webhook(
            &envelope(
                json!([{
                    "from": SENDER, "to": OWNED_PHONE, "id": "wamid.type.carried",
                    "timestamp": "1780000000", "type": carried_type, "text": {"body": "hi"}
                }]),
                json!([]),
                json!([]),
            ),
            &config(),
            observed_at(),
        )
        .expect("a well-formed message type is always accepted");
        assert_eq!(
            events.len(),
            1,
            "{HARNESS}: `{carried_type}` is an unrecognised but well-formed MESSAGE type, so it is recorded as a \
             message — it is not treated as a status and dropped"
        );
        assert_eq!(
            events[0].event_type,
            format!("whatsapp.message_received.{carried_type}"),
            "{HARNESS}: `{carried_type}` is carried through verbatim rather than guessed at or collapsed, so a \
             type Meta adds later is recorded as what it is"
        );
        assert_eq!(
            events[0].direction,
            WhatsAppDirection::Inbound,
            "{HARNESS}: an unrecognised type still carries its direction — being new does not make a message \
             ambiguous"
        );
    }

    for malformed_type in [
        "has\nescape",
        "drop table",
        "../../etc/passwd",
        "emoji-\u{1F642}",
        "",
    ] {
        let events = parse_webhook(
            &envelope(
                json!([{
                    "from": SENDER, "to": OWNED_PHONE, "id": "wamid.type.malformed",
                    "timestamp": "1780000000", "type": malformed_type, "text": {"body": "hi"}
                }]),
                json!([]),
                json!([]),
            ),
            &config(),
            observed_at(),
        )
        .expect("a malformed message type is refused to `unknown`, not rejected");
        assert_eq!(
            events.len(),
            1,
            "{HARNESS}: `{malformed_type:?}` is still a MESSAGE, not a status — it is recorded, not dropped"
        );
        assert_eq!(
            events[0].event_type, "whatsapp.message_received.unknown",
            "{HARNESS}: and it is typed `unknown` rather than carried, because the value ends up in \
             `interaction.event_type` and must never be free text"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 9. A RECEIPT FOR AN ACCOUNT WE DO NOT OWN IS IGNORED LIKE ANY OTHER FOREIGN TRAFFIC. The account check runs
    //    before slot handling (`payload.rs:142-149`), so foreign receipt traffic is not even inspected.
    // -----------------------------------------------------------------------------------------------------------
    let foreign = envelope(
        json!([]),
        json!([]),
        json!([receipt("wamid.foreign.1", "delivered")]),
    )
    .replace(OWNED_PHONE_NUMBER_ID, "000111");
    let events = parse_webhook(&foreign, &config(), observed_at())
        .expect("a foreign payload is well-formed");
    assert!(
        events.is_empty(),
        "{HARNESS}: another Meta app's receipts are ignored — the account gate runs before anything is read, so \
         foreign traffic cannot even reach the status question"
    );
}

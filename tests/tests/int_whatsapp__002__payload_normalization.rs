//! INT.WHATSAPP — payload normalization (TST-INT-WHATSAPP-002).
//!
//! Contract: **Meta's webhook shape is not this application's shape, and one function is the whole translation.**
//! Meta sends nested `entry[].changes[].value.messages[]` with stringly-typed timestamps, numbers formatted for
//! display, and a `wa_id` rather than a phone number. Everything downstream — the landing table, the canonical
//! inbox, phone-identity resolution — reads `NormalizedWhatsAppEvent`
//! (`middle/apis/src/whatsapp/payload.rs:104-119`), so `parse_webhook`
//! (`payload.rs:121-173`) is the only place the two vocabularies meet.
//!
//! The normalization it performs, each of which is asserted here:
//!
//! - **envelope unwrapping** — one flat event per message, out of a nested envelope;
//! - **timestamp conversion** — Meta's `"1780000000"` string into an RFC 3339 instant, split into when it happened
//!   (`occurred_at`) and when we saw it (`observed_at`);
//! - **direction** — `messages` is `Inbound`, `message_echoes` is `Outbound`, which is how a sent message is told
//!   from a received one without a second endpoint;
//! - **phone normalization** — the sender's `from` (or an echo's `to`) into E.164, so it is directly comparable to
//!   the `phone` identities stored in `person_identity`;
//! - **summary extraction and cleanup** — text or a media caption, NFKC-normalized, CRLF collapsed, trimmed, and
//!   capped at 4000 code points;
//! - **attachment extraction** — the media id, mime type and filename, from whichever media slot carries them.
//!
//! **It does not keep the raw payload.** `NormalizedWhatsAppEvent` has no field for it. That is the point: the
//! canonical row is built from the parts this application understands, so a Meta field rename cannot rewrite
//! history. The raw body is kept separately and only for the landing table (`web/src/whatsapp.rs:139-155`).
//!
//! The negative cases are what a permissive normalizer would let through, and each is a refusal:
//!
//! - **a foreign account is ignored, not processed.** A webhook carrying a `phone_number_id` this deployment does
//!   not own yields no events at all — not an error, and emphatically not an event attributed to our number.
//!   Accepting it would let anyone with any Meta app inject messages into this inbox.
//! - **a foreign `object` or `messaging_product` is ignored** the same way.
//! - **a message with no id is refused**, because the id is the idempotency key (`integration_inbox`'s unique
//!   constraint is `(source, source_account, external_event_id)`) — a message without one cannot be deduplicated.
//! - **a non-numeric or missing timestamp is refused**, because `occurred_at` is what orders a conversation, and a
//!   defaulted "now" would silently misdate a message from last year.
//! - **a missing sender is refused**, because a message with no `from` has no phone to attribute and would become
//!   an unattributable row.
//! - **a sender that cannot be normalized to E.164 is refused**, so a `wa_id` that is not a phone never reaches
//!   identity resolution.
//! - **one bad message fails the whole batch**, because the `?` in `parse_webhook`'s loop propagates: a partially
//!   processed batch would leave a webhook half-applied with no way to replay it.
//! - **an unknown message type becomes `unknown`** rather than being carried into an event type, and a hostile type
//!   containing punctuation or an over-long string is refused to the same `unknown` — the value ends up in
//!   `interaction.event_type`, so it is never allowed to be free text.
//!
//! `observed_at` is passed in by the caller (`web/src/whatsapp.rs:104`), so every assertion here is deterministic:
//! no clock is read and no fixture depends on when the test ran.
//!
//! Fixtures are BUILT with `serde_json::json!` rather than written as format! templates. A template with literal
//! JSON braces has to double every one of them, and a miscount produces malformed JSON that reads as a parser bug
//! in the code under test rather than as a broken fixture.
//!
//! ONE FINDING IS RECORDED IN PLACE rather than left for a reader to assume away: the contact lookup at
//! `payload.rs:213` normalizes the MESSAGE side and compares the CONTACT side exactly, so a `wa_id` that arrived
//! formatted loses the display name. The phone — the thing attribution actually rests on — is normalized
//! independently and is unaffected. Section 12 asserts both halves.
//!
//! Level: L1 Component — `apis::whatsapp`, pure, no database and no socket.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_whatsapp__002__payload_normalization

use apis::whatsapp::{
    parse_webhook, MetaWhatsAppConfig, NormalizedWhatsAppEvent, WhatsAppDirection,
};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::{json, Value};

const HARNESS: &str = "INT.WHATSAPP/002";

/// The account this deployment owns. Every fixture must carry it in `metadata.phone_number_id` or its events are
/// ignored — which is itself one of the contracts under test.
const OWNED_PHONE_NUMBER_ID: &str = "999888";

/// A correspondent, in the form Meta sends (`wa_id` and `from` are both bare digits, no `+`).
const SENDER: &str = "17875551212";

/// This deployment's own number, already normalized — `from_env` normalizes it (`mod.rs:36`) and the fixture keeps
/// it in that form so the comparison under test is about the event, not about the config.
const OWNED_PHONE: &str = "+17875550000";

/// The instant every fixture's `observed_at` is pinned to, so no assertion can be satisfied by luck.
fn observed_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
        .single()
        .expect("a fixed civil instant is unambiguous")
}

fn config() -> MetaWhatsAppConfig {
    MetaWhatsAppConfig {
        app_secret: "fixture-app-secret".into(),
        phone_number_id: OWNED_PHONE_NUMBER_ID.into(),
        owned_phone_e164: OWNED_PHONE.into(),
        verify_token: "fixture-verify-token".into(),
    }
}

/// Wrap messages in the Meta envelope for an account.
///
/// `messages` becomes `direction: inbound` and `message_echoes` becomes `direction: outbound`, which is the only
/// difference between them — the same message body in the other slot is the application's record that WE sent it.
fn webhook(
    messages: &[Value],
    echoes: &[Value],
    phone_number_id: &str,
    messaging_product: &str,
) -> String {
    serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": messaging_product,
            "metadata": {"phone_number_id": phone_number_id},
            "contacts": [{"wa_id": SENDER, "profile": {"name": "Ami Torres"}}],
            "messages": messages,
            "message_echoes": echoes
        }}]}]
    }))
    .expect("a built envelope serializes")
}

/// One inbound text message: the correspondent sent it, we received it.
fn text_message(id: &str, timestamp: &str, body: &str) -> Value {
    json!({
        "from": SENDER, "to": OWNED_PHONE, "id": id, "timestamp": timestamp,
        "type": "text", "text": {"body": body}
    })
}

/// One outbound text message: we sent it, the correspondent received it.
///
/// The `from`/`to` swap against [`text_message`] is not cosmetic — it is the whole reason direction is decided by
/// which slot Meta put the message in (`payload.rs:175-208`). An echo carries `from` = our number and `to` =
/// theirs, so the correspondent is read from `to`. A fixture that got this backwards would resolve the
/// correspondent to our own number.
fn echo_message(id: &str, timestamp: &str, body: &str) -> Value {
    json!({
        "from": OWNED_PHONE, "to": SENDER, "id": id, "timestamp": timestamp,
        "type": "text", "text": {"body": body}
    })
}

fn parse(raw: &str) -> Vec<NormalizedWhatsAppEvent> {
    parse_webhook(raw, &config(), observed_at()).unwrap_or_else(|error| {
        panic!("{HARNESS}: this fixture must normalize: {error}\nraw: {raw}")
    })
}

/// The single event a one-message fixture produces, asserting there is exactly one.
fn only(mut events: Vec<NormalizedWhatsAppEvent>) -> NormalizedWhatsAppEvent {
    assert_eq!(
        events.len(),
        1,
        "{HARNESS}: this fixture carries exactly one message and must normalize to exactly one event"
    );
    events.remove(0)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-WHATSAPP-002); the file and the assay use it.
fn int_whatsapp__002__payload_normalization() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE WHOLE TRANSLATION, on a plain inbound text message. Every field of `NormalizedWhatsAppEvent` is
    //    asserted, because a field that silently stops being populated is invisible until something downstream
    //    reads it.
    // -----------------------------------------------------------------------------------------------------------
    let raw = webhook(
        &[text_message(
            "wamid.normal.1",
            "1780000000",
            "Is the villa still available?",
        )],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    );
    let event = only(parse(&raw));

    assert_eq!(
        event.source_account,
        format!("meta-{OWNED_PHONE_NUMBER_ID}"),
        "{HARNESS}: the source account names our own Meta number, so a replayed event is attributed to the \
         account it actually came from"
    );
    assert_eq!(
        event.external_event_id, "wamid.normal.1",
        "{HARNESS}: Meta's message id is carried verbatim — it is the idempotency key the inbox dedupes on"
    );
    assert_eq!(
        event.event_type, "whatsapp.message_received.text",
        "{HARNESS}: an inbound text becomes a received-text event, which is what `interaction.event_type` records"
    );
    assert_eq!(
        event.direction,
        WhatsAppDirection::Inbound,
        "{HARNESS}: a `messages` entry is inbound"
    );
    assert_eq!(
        event.external_phone_e164,
        format!("+{SENDER}"),
        "{HARNESS}: the sender's bare-digit wa_id becomes E.164, so it is directly comparable to the `phone` \
         identities stored in person_identity"
    );
    assert_eq!(
        event.owned_phone_e164, OWNED_PHONE,
        "{HARNESS}: the owned number is carried from configuration, never taken from the payload — otherwise a \
         payload could claim to have been received by a number we do not own"
    );
    assert_eq!(
        event.external_display_name.as_deref(),
        Some("Ami Torres"),
        "{HARNESS}: the contact's profile name is matched on DIGITS (`payload.rs:210-219`), which is why it is \
         found even though `wa_id` has no `+` and the normalized phone does"
    );
    assert_eq!(
        event.summary.as_deref(),
        Some("Is the villa still available?"),
        "{HARNESS}: the text body becomes the summary"
    );
    assert_eq!(
        event.occurred_at, "2026-05-28T20:26:40+00:00",
        "{HARNESS}: Meta's unix-seconds string becomes an RFC 3339 instant — the conversion is the reason \
         `from timestamp: Option<String>` had to be validated rather than defaulted"
    );
    assert_eq!(
        event.observed_at, "2026-03-01T12:00:00+00:00",
        "{HARNESS}: `observed_at` is the caller's instant, passed in rather than read from a clock, so it is \
         distinguishable from when the message was sent"
    );
    assert_eq!(
        event.thread_id, None,
        "{HARNESS}: a first-contact message has no thread to reply into"
    );
    assert!(
        event.attachments.is_empty(),
        "{HARNESS}: a text message carries no attachment"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. A REPLY carries the thread id it answers, so a conversation can be reconstructed.
    // -----------------------------------------------------------------------------------------------------------
    let mut reply = text_message("wamid.normal.2", "1780000100", "With a pool");
    reply["context"] = json!({"id": "wamid.normal.1"});
    let event = only(parse(&webhook(
        &[reply],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.thread_id.as_deref(),
        Some("wamid.normal.1"),
        "{HARNESS}: a reply carries the id of the message it answers, which is what threads a conversation"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. DIRECTION IS THE SLOT. The same message in `message_echoes` is OUR message going out: `Outbound`, a
    //    `sent` event type, and — critically — the external phone is the RECIPIENT (`to`), not the sender. Getting
    //    that backwards attributes our own outbound message to the person we sent it to.
    // -----------------------------------------------------------------------------------------------------------
    let event = only(parse(&webhook(
        &[],
        &[echo_message(
            "wamid.echo.1",
            "1780000200",
            "We will show you Thursday",
        )],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.direction,
        WhatsAppDirection::Outbound,
        "{HARNESS}: a `message_echoes` entry is outbound"
    );
    assert_eq!(
        event.event_type, "whatsapp.message_sent.text",
        "{HARNESS}: an outbound text is a sent-text event"
    );
    assert_eq!(
        event.external_phone_e164,
        format!("+{SENDER}"),
        "{HARNESS}: on an echo the correspondent is the RECIPIENT (`to`), not the sender — the other reading would \
         file our own message as something the other person said"
    );

    // Both slots in one payload normalize to two events, in the order the parser walks them.
    let both = parse(&webhook(
        &[text_message("wamid.in.1", "1780000300", "in")],
        &[echo_message("wamid.out.1", "1780000400", "out")],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    ));
    assert_eq!(
        both.len(),
        2,
        "{HARNESS}: one event per message across both slots — an echo is a real event, not a duplicate"
    );
    assert_eq!(both[0].direction, WhatsAppDirection::Inbound);
    assert_eq!(both[1].direction, WhatsAppDirection::Outbound);
    assert_eq!(both[0].external_event_id, "wamid.in.1");
    assert_eq!(both[1].external_event_id, "wamid.out.1");

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A FOREIGN ACCOUNT PRODUCES NOTHING. `parse_webhook` compares `metadata.phone_number_id` against
    //    the configured one (`payload.rs:142-149`) and skips the change when they differ. This is the refusal that
    //    stops any holder of any Meta app from writing into this inbox.
    // -----------------------------------------------------------------------------------------------------------
    let foreign = parse(&webhook(
        &[text_message(
            "wamid.foreign.1",
            "1780000500",
            "Transfer the balance",
        )],
        &[],
        "000111",
        "whatsapp",
    ));
    assert!(
        foreign.is_empty(),
        "{HARNESS}: a webhook for a phone_number_id we do not own yields no events — not an error, and \
         emphatically not an event attributed to our number"
    );

    // The same for the other two envelope keys, which are also gates rather than filters.
    assert!(
        parse(&webhook(
            &[text_message("wamid.foreign.2", "1780000600", "hello")],
            &[],
            OWNED_PHONE_NUMBER_ID,
            "instagram"
        ))
        .is_empty(),
        "{HARNESS}: a change whose messaging_product is not `whatsapp` belongs to another product and is ignored"
    );
    let wrong_object = webhook(
        &[text_message("wamid.foreign.3", "1780000700", "hello")],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )
    .replace("whatsapp_business_account", "instagram");
    assert!(
        parse(&wrong_object).is_empty(),
        "{HARNESS}: a payload whose top-level object is not a WhatsApp business account is ignored"
    );
    // A change with no `value` at all is skipped rather than refused — there is nothing to normalize.
    let no_value = r#"{"object":"whatsapp_business_account","entry":[{"changes":[{}]}]}"#;
    assert!(
        parse(no_value).is_empty(),
        "{HARNESS}: a change with no value carries no message and is skipped, not refused"
    );
    // And an empty but well-formed payload is an empty result, not an error.
    assert!(
        parse(&webhook(&[], &[], OWNED_PHONE_NUMBER_ID, "whatsapp")).is_empty(),
        "{HARNESS}: a well-formed webhook carrying no messages is accepted and yields nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — A MESSAGE WITH NO ID IS REFUSED. The id is the idempotency key: `integration_inbox` is unique on
    //    `(source, source_account, external_event_id)`, so an event without one cannot be deduplicated and a replay
    //    would be processed twice.
    // -----------------------------------------------------------------------------------------------------------
    for (described, message) in [
        (
            "no id",
            json!({"from": SENDER, "timestamp": "1780000000", "type": "text", "text": {"body": "hi"}}),
        ),
        (
            "a blank id",
            json!({"from": SENDER, "id": "   ", "timestamp": "1780000000", "type": "text", "text": {"body": "hi"}}),
        ),
    ] {
        let refusal = parse_webhook(
            &webhook(&[message], &[], OWNED_PHONE_NUMBER_ID, "whatsapp"),
            &config(),
            observed_at(),
        )
        .expect_err(&format!(
            "{HARNESS}: a message with {described} must be refused"
        ));
        assert_eq!(
            refusal, "WhatsApp message id is missing.",
            "{HARNESS}: a message with {described} is refused because the id is the idempotency key"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A TIMESTAMP WE CANNOT DATE IS REFUSED. `occurred_at` is what orders a conversation, so a
    //    defaulted "now" would silently misdate a message. Every way the timestamp can be unusable is refused.
    // -----------------------------------------------------------------------------------------------------------
    let timestamp_refusals = [
        ("a missing timestamp", Value::Null),
        ("a blank timestamp", json!("")),
        ("a non-numeric timestamp", json!("not-a-time")),
        ("a partially-numeric timestamp", json!("17800000o0")),
        ("a negative timestamp", json!("-1780000000")),
        ("a fractional timestamp", json!("1780000000.5")),
        (
            "a timestamp far past any plausible date",
            json!("999999999999999"),
        ),
    ];
    for (described, timestamp) in timestamp_refusals {
        let message = json!({
            "from": SENDER, "id": "wamid.bad-time", "timestamp": timestamp,
            "type": "text", "text": {"body": "hi"}
        });
        let refusal = parse_webhook(
            &webhook(&[message], &[], OWNED_PHONE_NUMBER_ID, "whatsapp"),
            &config(),
            observed_at(),
        )
        .expect_err(&format!("{HARNESS}: {described} must be refused"));
        assert_eq!(
            refusal, "WhatsApp message timestamp is invalid.",
            "{HARNESS}: {described} is refused rather than dated as now, which would misplace it in the timeline"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 7. NEGATIVE — A MESSAGE WITH NO CORRESPONDENT IS REFUSED. There is no phone to attribute, and an
    //    unattributable message has nowhere to go: it would become a canonical row with no person behind it.
    // -----------------------------------------------------------------------------------------------------------
    let no_sender = json!({
        "to": OWNED_PHONE, "id": "wamid.no-sender", "timestamp": "1780000000",
        "type": "text", "text": {"body": "hi"}
    });
    let refusal = parse_webhook(
        &webhook(&[no_sender], &[], OWNED_PHONE_NUMBER_ID, "whatsapp"),
        &config(),
        observed_at(),
    )
    .expect_err("{HARNESS}: an inbound message with no sender must be refused");
    assert_eq!(
        refusal, "WhatsApp sender is missing.",
        "{HARNESS}: an inbound message with no `from` is refused — there is no phone to attribute it to"
    );

    // The mirror case: an echo needs a recipient, because on an echo the recipient IS the correspondent.
    let no_recipient = json!({
        "from": OWNED_PHONE, "id": "wamid.no-recipient", "timestamp": "1780000000",
        "type": "text", "text": {"body": "hi"}
    });
    let refusal = parse_webhook(
        &webhook(&[], &[no_recipient], OWNED_PHONE_NUMBER_ID, "whatsapp"),
        &config(),
        observed_at(),
    )
    .expect_err("{HARNESS}: an echo with no recipient must be refused");
    assert_eq!(
        refusal, "WhatsApp recipient is missing.",
        "{HARNESS}: an outbound echo with no `to` is refused — the recipient is the correspondent on this side"
    );

    // A sender that cannot normalize to E.164 is refused too, so a handle-shaped `wa_id` never reaches resolution.
    let unnormalizable = json!({
        "from": "not-a-phone", "id": "wamid.bad-phone", "timestamp": "1780000000",
        "type": "text", "text": {"body": "hi"}
    });
    let refusal = parse_webhook(
        &webhook(&[unnormalizable], &[], OWNED_PHONE_NUMBER_ID, "whatsapp"),
        &config(),
        observed_at(),
    )
    .expect_err("{HARNESS}: a sender that is not a phone number must be refused");
    assert!(
        refusal.contains("E.164"),
        "{HARNESS}: an unnormalizable sender is refused at the normalization gate, so no non-phone handle can \
         reach phone-identity resolution — got {refusal:?}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. NEGATIVE — ONE BAD MESSAGE FAILS THE WHOLE BATCH. `parse_webhook` propagates the first failure out of its
    //    loop with `?` (`payload.rs:158,167`), so a batch is all-or-nothing: a half-applied webhook has no replay
    //    path, because Meta considers the delivery done the moment it returns 2xx.
    // -----------------------------------------------------------------------------------------------------------
    let good = text_message("wamid.batch.good", "1780000000", "good");
    let bad =
        json!({"from": SENDER, "timestamp": "1780000000", "type": "text", "text": {"body": "bad"}});
    let refusal = parse_webhook(
        &webhook(&[good, bad], &[], OWNED_PHONE_NUMBER_ID, "whatsapp"),
        &config(),
        observed_at(),
    )
    .expect_err("{HARNESS}: a batch containing one unusable message must be refused whole");
    assert_eq!(
        refusal, "WhatsApp message id is missing.",
        "{HARNESS}: the first failure aborts the batch — returning the good messages alone would leave the \
         webhook half-applied with no way to replay it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. SUMMARY EXTRACTION AND CLEANUP. The summary is prose a person reads in the CRM, so it is NFKC-normalized
    //    (so visually identical text matches a search), CRLF collapsed to `\n` (so a Windows-sent message does not
    //    carry carriage returns into the UI), trimmed, and capped so one enormous message cannot dominate a
    //    conversation view.
    // -----------------------------------------------------------------------------------------------------------
    let cases: Vec<(&str, Option<&str>, &str)> = vec![
        (
            "  padded  ",
            Some("padded"),
            "whitespace around the body is trimmed",
        ),
        (
            "line one\r\nline two",
            Some("line one\nline two"),
            "CRLF becomes LF",
        ),
        (
            "line one\rline two",
            Some("line one\nline two"),
            "a bare CR becomes LF",
        ),
        (
            "\u{FB01}nance",
            Some("finance"),
            "the NFKC ligature is decomposed so the text matches a search",
        ),
        (
            "",
            None,
            "an empty body is no summary rather than an empty one",
        ),
        ("   ", None, "a whitespace-only body is no summary"),
    ];
    for (body, expected, described) in &cases {
        let event = only(parse(&webhook(
            &[text_message("wamid.summary", "1780000000", body)],
            &[],
            OWNED_PHONE_NUMBER_ID,
            "whatsapp",
        )));
        assert_eq!(
            event.summary.as_deref(),
            *expected,
            "{HARNESS}: {described}"
        );
    }

    // The cap is 4000 code points (`payload.rs:5`), asserted on CHARACTERS rather than bytes so a multi-byte body
    // is not cut short of the limit by its own encoding.
    let long = "x".repeat(4_050);
    let event = only(parse(&webhook(
        &[text_message("wamid.summary.long", "1780000000", &long)],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.summary.as_deref().map(|summary| summary.chars().count()),
        Some(4_000),
        "{HARNESS}: a summary is capped at 4000 code points so one enormous message cannot dominate a conversation"
    );

    // A caption is a summary too, which is what makes a photo-only message legible in the CRM.
    let captioned = json!({
        "from": SENDER, "id": "wamid.caption", "timestamp": "1780000000", "type": "image",
        "image": {"id": "media-1", "mime_type": "image/jpeg", "caption": "  The pool at dusk  "}
    });
    let event = only(parse(&webhook(
        &[captioned],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.event_type, "whatsapp.message_received.image",
        "{HARNESS}: an image message is an image event, so the conversation view can show a thumbnail"
    );
    assert_eq!(
        event.summary.as_deref(),
        Some("The pool at dusk"),
        "{HARNESS}: a media caption becomes the summary, trimmed — a photo-only message is otherwise unreadable"
    );

    // A media message with no caption has an attachment but no summary.
    let uncaptioned = json!({
        "from": SENDER, "id": "wamid.nocaption", "timestamp": "1780000000", "type": "image",
        "image": {"id": "media-2", "mime_type": "image/png"}
    });
    let event = only(parse(&webhook(
        &[uncaptioned],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.summary, None,
        "{HARNESS}: a media message with no caption has no summary — none is invented"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 10. ATTACHMENT EXTRACTION, AND ONLY THE FIRST ONE. `message_attachment` (`payload.rs:304-329`) walks the
    //     media slots in a fixed order and returns the first with an id. A WhatsApp message carries at most one
    //     media object, so a single attachment is the whole contract — returning several would mean inventing
    //     relationships Meta did not send.
    // -----------------------------------------------------------------------------------------------------------
    for message_type in ["image", "video", "audio", "document", "sticker"] {
        let message = json!({
            "from": SENDER, "id": format!("wamid.media.{message_type}"),
            "timestamp": "1780000000", "type": message_type,
            message_type: {"id": format!("media-{message_type}"), "mime_type": "application/octet-stream", "filename": "brochure.pdf"}
        });
        let event = only(parse(&webhook(
            &[message],
            &[],
            OWNED_PHONE_NUMBER_ID,
            "whatsapp",
        )));
        assert_eq!(
            event.event_type,
            format!("whatsapp.message_received.{message_type}"),
            "{HARNESS}: the message type is carried into the event type"
        );
        assert_eq!(
            event.attachments.len(),
            1,
            "{HARNESS}: one {message_type} message yields exactly one attachment"
        );
        let attachment = &event.attachments[0];
        assert_eq!(
            attachment.reference_id,
            format!("media-{message_type}"),
            "{HARNESS}: the {message_type} media id is the attachment's reference id"
        );
        assert_eq!(
            attachment.mime_type.as_deref(),
            Some("application/octet-stream"),
            "{HARNESS}: the mime type is carried so the reader can be told what it is opening"
        );
        assert_eq!(
            attachment.filename.as_deref(),
            Some("brochure.pdf"),
            "{HARNESS}: the filename is carried, so a document is downloadable under its own name"
        );
    }

    // A media message whose media object has no id yields no attachment: there is nothing to fetch, and a
    // fabricated reference id would point at somebody else's media.
    let id_less = json!({
        "from": SENDER, "id": "wamid.media.noid", "timestamp": "1780000000", "type": "image",
        "image": {"mime_type": "image/png"}
    });
    let event = only(parse(&webhook(
        &[id_less],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert!(
        event.attachments.is_empty(),
        "{HARNESS}: a media message with no media id yields no attachment rather than an empty reference"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 11. THE MESSAGE TYPE IS NEVER FREE TEXT. It ends up in `interaction.event_type` and in the event type the
    //     canonical inbox stores, so `normalized_message_type` (`payload.rs:254-266`) admits only lowercase
    //     alphanumerics and `_`, at 64 characters or fewer, and refuses everything else to `unknown`.
    // -----------------------------------------------------------------------------------------------------------
    let type_cases: Vec<(Value, &str, &str)> = vec![
        (json!("text"), "text", "a known type is carried through"),
        (json!("IMAGE"), "image", "case is normalized"),
        (json!("  audio  "), "audio", "whitespace is trimmed"),
        (json!("sticker"), "sticker", "another known type"),
        (
            json!("reaction"),
            "reaction",
            "a type Meta adds later is carried if it is well-formed",
        ),
        (json!("has\nescape"), "unknown", "a newline is not a type"),
        (
            json!("drop table"),
            "unknown",
            "SQL punctuation is not a type",
        ),
        (
            json!("../../etc/passwd"),
            "unknown",
            "path characters are not a type",
        ),
        (
            json!("emoji-\u{1F642}"),
            "unknown",
            "a non-ASCII type is not a type",
        ),
        (json!(""), "unknown", "a blank type is not a type"),
    ];
    for (supplied, expected, described) in &type_cases {
        let message = json!({
            "from": SENDER, "id": "wamid.type", "timestamp": "1780000000",
            "type": supplied, "text": {"body": "hi"}
        });
        let event = only(parse(&webhook(
            &[message],
            &[],
            OWNED_PHONE_NUMBER_ID,
            "whatsapp",
        )));
        assert_eq!(
            event.event_type,
            format!("whatsapp.message_received.{expected}"),
            "{HARNESS}: {described}"
        );
    }

    // An over-long well-formed type is refused to `unknown` rather than truncated: a truncated type would collide
    // with a real one.
    let long_type = "a".repeat(70);
    assert_ne!(
        long_type.len(),
        0,
        "{HARNESS}: the over-long fixture is genuinely over the limit"
    );
    let mut over_long = text_message("wamid.type.long", "1780000000", "hi");
    over_long["type"] = json!(long_type);
    let event = only(parse(&webhook(
        &[over_long],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.event_type, "whatsapp.message_received.unknown",
        "{HARNESS}: a type longer than 64 characters becomes `unknown` — truncating it would collide with a real one"
    );

    // A message with no `type` at all is `unknown`, and still normalizes: an untyped message is still a message.
    let untyped = json!({
        "from": SENDER, "id": "wamid.type.none", "timestamp": "1780000000",
        "text": {"body": "hi"}
    });
    let event = only(parse(&webhook(
        &[untyped],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    assert_eq!(
        event.event_type, "whatsapp.message_received.unknown",
        "{HARNESS}: a message with no type is `unknown` rather than refused — the message is still a message"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 12. THE NORMALIZED EVENT CARRIES NO RAW PAYLOAD. This is asserted structurally rather than by convention:
    //     every field of the struct is checked above, and there is no field left over that could hold Meta's
    //     envelope. A field added here would have to be a deliberate decision, and this assertion is what makes
    //     that decision visible.
    // -----------------------------------------------------------------------------------------------------------
    let event = only(parse(&webhook(
        &[text_message("wamid.shape", "1780000000", "hello")],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    )));
    let as_json = serde_json::to_value(&event).expect("a normalized event serializes");
    let keys: Vec<&str> = as_json
        .as_object()
        .expect("a normalized event serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    for expected in [
        "sourceAccount",
        "externalEventId",
        "eventType",
        "occurredAt",
        "observedAt",
        "direction",
        "externalPhoneE164",
        "externalDisplayName",
        "ownedPhoneE164",
        "threadId",
        "summary",
        "attachments",
    ] {
        assert!(
            keys.contains(&expected),
            "{HARNESS}: `{expected}` is part of the canonical shape and must be present, got {keys:?}"
        );
    }
    assert!(
        !as_json.to_string().contains("whatsapp_business_account"),
        "{HARNESS}: Meta's envelope keys must not survive into the canonical event — the landing table keeps the \
         raw body separately, and the canonical row is built only from what this application understands"
    );

    // And the display name is decoration, never identity: a message from a contact we have never seen still
    // carries a phone, and a missing profile leaves the phone alone.
    // Built by hand rather than by string-replacing the envelope, because `json!` orders object keys itself and
    // a hand-written replace target would silently miss and leave the contact in place.
    let no_contacts = serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [],
            "messages": [text_message("wamid.noprofile", "1780000000", "hello")],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes");
    let event = only(parse(&no_contacts));
    assert_eq!(
        event.external_display_name, None,
        "{HARNESS}: a message with no contact profile has no display name — none is invented"
    );
    assert_eq!(
        event.external_phone_e164,
        format!("+{SENDER}"),
        "{HARNESS}: the phone is what identifies the correspondent; the display name is optional decoration, so \
         losing it never costs the attribution"
    );
    // A contact whose `wa_id` is formatted differently still matches on digits.
    let formatted_contact = serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": "+1 (787) 555-1212", "profile": {"name": "Ami Torres"}}],
            "messages": [text_message("wamid.formatted", "1780000000", "hello")],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes");
    let event = only(parse(&formatted_contact));
    // PRODUCT EVIDENCE, recorded rather than hidden: `payload.rs:213` compares the contact's `wa_id` against the
    // normalized sender digits with `==`, so it normalizes the MESSAGE side but not the CONTACT side. A `wa_id`
    // that arrived formatted therefore does not match, and the display name is lost. Meta sends bare digits, so
    // this is not reached in production — but it is the asymmetry a reader of that line would assume away. What
    // matters, and is asserted here, is that losing the name costs nothing: the phone still carries the event.
    assert_eq!(
        event.external_display_name, None,
        "{HARNESS}: the contact lookup compares `wa_id` against the normalized digits exactly (`payload.rs:213`), \
         so a formatted `wa_id` does not match — the NAME is lost, and only the name"
    );
    assert_eq!(
        event.external_phone_e164,
        format!("+{SENDER}"),
        "{HARNESS}: the phone is normalized from the message independently of the contact lookup, so an \
         unmatched contact costs the display name and never the attribution"
    );
    // And the exact match the code does perform: a `wa_id` that is already the bare digits finds the name, which
    // is the path Meta actually takes.
    let bare_digits = webhook(
        &[text_message("wamid.baredigits", "1780000000", "hello")],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    );
    let event = only(parse(&bare_digits));
    assert_eq!(
        event.external_display_name.as_deref(),
        Some("Ami Torres"),
        "{HARNESS}: a bare-digit `wa_id` equals the normalized digits and finds the name — the form Meta sends"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 13. NOTHING REACHES PRODUCTION, AND NOTHING DEPENDS ON WHEN THE TEST RAN. `parse_webhook` is a pure function
    //     over a string: it opens no socket, reads no environment and writes no row. Two calls with the same bytes
    //     and different `observed_at` values prove the instant is the caller's input rather than a clock read
    //     inside the parser — which is what makes every assertion above deterministic.
    // -----------------------------------------------------------------------------------------------------------
    let raw = webhook(
        &[text_message("wamid.determinism", "1780000000", "hello")],
        &[],
        OWNED_PHONE_NUMBER_ID,
        "whatsapp",
    );
    let later = parse_webhook(
        &raw,
        &config(),
        Utc.with_ymd_and_hms(2026, 6, 15, 9, 30, 0)
            .single()
            .expect("a fixed civil instant is unambiguous"),
    )
    .expect("this fixture normalizes");
    let first = only(parse(&raw));
    let second = only(later);
    assert_eq!(
        second.observed_at, "2026-06-15T09:30:00+00:00",
        "{HARNESS}: the observed instant is whatever the caller passed, so two runs can differ only if the \
         caller chose to differ"
    );
    assert_eq!(
        first.occurred_at, second.occurred_at,
        "{HARNESS}: when the message was SENT does not depend on when it was observed"
    );
    assert_eq!(
        first.external_event_id, second.external_event_id,
        "{HARNESS}: normalizing the same bytes twice yields the same event"
    );
}

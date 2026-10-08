//! SIG.WEBHOOK — malformed event (TST-SIG-WEBHOOK-008).
//!
//! Contract: a webhook that is not valid JSON, or valid JSON without the envelope the provider
//! contract promises, is unprocessable — `parse_webhook_payload`
//! (`middle/apis/src/boldsign/mod.rs:141`) rejects it with a field-naming error before any trust,
//! mapping, or persistence decision is made. Malformed input must never reach the envelope
//! resolver (an unknown-envelope error would imply the payload was understood) and must never
//! verify-then-crash: parse failure is a refusal, full stop.
//!
//! Seven things must therefore hold:
//!
//! - **Non-JSON is refused.** A body that is not JSON at all names the payload, not a field.
//! - **A missing event object is refused**, naming `event`.
//! - **A missing data object is refused**, naming `data`.
//! - **Each required field is refused when absent**: `event.id`, `event.eventType`,
//!   `data.documentId` — each error names the field it missed, so the operator knows what the
//!   provider sent.
//! - **Blank is absent.** Whitespace-only required fields are trimmed and refused like missing
//!   ones: an envelope id of `"   "` is not an envelope.
//! - **A well-formed payload parses exactly.** The positive control: ids, type and status land
//!   in the right struct fields, so the refusals above are the parser discriminating, not the
//!   parser rejecting everything.
//! - **The optional status stays optional.** A payload without `data.status` still parses with
//!   `document_status: None` — optionality is preserved, not collapsed into refusal.
//!
//! Level: L3 Composition — the production parser function, no I/O, no database.
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__008__malformed_event

use apis::boldsign::parse_webhook_payload;

fn valid_body() -> String {
    r#"{"event":{"id":"evt-008","eventType":"Completed"},"data":{"documentId":"env-008","status":"Completed"}}"#.into()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-008).
fn sig_webhook_008__malformed_event() {
    // 1. NON-JSON is refused, naming the payload rather than any field.
    let error =
        parse_webhook_payload("this is not json{{{").expect_err("non-JSON must be refused");
    assert!(
        error.contains("not valid JSON"),
        "non-JSON must report the payload, got: {error}"
    );

    // 2 + 3. MISSING event / data OBJECTS are refused, each naming the object missed.
    let error = parse_webhook_payload(r#"{"data":{"documentId":"env-008"}}"#)
        .expect_err("a payload without event must be refused");
    assert!(
        error.contains("missing event"),
        "a missing event object must be named, got: {error}"
    );
    let error = parse_webhook_payload(r#"{"event":{"id":"evt-008","eventType":"Completed"}}"#)
        .expect_err("a payload without data must be refused");
    assert!(
        error.contains("missing data"),
        "a missing data object must be named, got: {error}"
    );

    // 4. EACH REQUIRED FIELD is refused when absent, naming the field.
    for (body, field) in [
        (
            r#"{"event":{"eventType":"Completed"},"data":{"documentId":"env-008"}}"#,
            "event.id",
        ),
        (
            r#"{"event":{"id":"evt-008"},"data":{"documentId":"env-008"}}"#,
            "event.eventType",
        ),
        (
            r#"{"event":{"id":"evt-008","eventType":"Completed"},"data":{"status":"Completed"}}"#,
            "data.documentId",
        ),
    ] {
        let error =
            parse_webhook_payload(body).expect_err("a payload missing {field} must be refused");
        assert!(
            error.contains(field),
            "a payload missing {field} must name it, got: {error}"
        );
    }

    // 5. BLANK IS ABSENT: whitespace-only required fields are trimmed and refused.
    let error = parse_webhook_payload(
        r#"{"event":{"id":"evt-008","eventType":"Completed"},"data":{"documentId":"   "}}"#,
    )
    .expect_err("a blank envelope id must be refused");
    assert!(
        error.contains("data.documentId"),
        "a blank envelope id must be reported as missing, got: {error}"
    );

    // 6. POSITIVE CONTROL: a well-formed payload parses exactly — ids, type and status in the
    // right fields — so the refusals above are discrimination, not blanket rejection.
    let parsed = parse_webhook_payload(&valid_body()).expect("the valid payload must parse");
    assert_eq!(parsed.provider_event_id, "evt-008");
    assert_eq!(parsed.event_type, "Completed");
    assert_eq!(parsed.envelope_id, "env-008");
    assert_eq!(parsed.document_status.as_deref(), Some("Completed"));

    // 7. THE OPTIONAL STATUS STAYS OPTIONAL: no `data.status` still parses, with None.
    let parsed = parse_webhook_payload(
        r#"{"event":{"id":"evt-008","eventType":"Sent"},"data":{"documentId":"env-008"}}"#,
    )
    .expect("a payload without status must still parse");
    assert_eq!(
        parsed.document_status, None,
        "a missing data.status must parse as None, not refuse"
    );
}

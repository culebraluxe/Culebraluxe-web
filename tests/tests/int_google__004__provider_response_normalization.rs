//! INT.GOOGLE — provider response normalization (TST-INT-GOOGLE-004).
//!
//! Contract: whatever Google answers, the application normalizes it into its
//! own shape without loss or confusion. The session payload keeps the
//! provider label and the subject verbatim (`provider|subject|expires`, read
//! back with `splitn(3, '|')`); a subject that would break the framing is
//! refused, never confused with another identity. Google's message payloads
//! land through `GmailMetadataMessage`, which keeps Google's own field shapes
//! (string-millisecond timestamps, header lists) and tolerates the partial
//! payloads a provider actually sends.
//!
//! Level: L1 Component — the production session codec plus the production
//! Gmail payload contract. No database, no network, no live Google.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_google__004__provider_response_normalization

use axum::http::{header, HeaderMap, HeaderValue};
use model::gmail::GmailMetadataMessage;
use web::api::ui_auth::{now_seconds, session_identity, session_value, SESSION_COOKIE};

const TEST_SECRET: &str = "int-google-004-test-secret";

fn install_secret() -> Option<String> {
    let previous = std::env::var("AUTH_SECRET").ok();
    std::env::set_var("AUTH_SECRET", TEST_SECRET);
    previous
}

fn restore_secret(previous: Option<String>) {
    match previous {
        Some(value) => std::env::set_var("AUTH_SECRET", value),
        None => std::env::remove_var("AUTH_SECRET"),
    }
}

fn headers_with_session(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.append(
        header::COOKIE,
        HeaderValue::from_str(&format!("{SESSION_COOKIE}={value}")).expect("cookie value"),
    );
    headers
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn int_google_004__provider_response_normalization() {
    let previous = install_secret();
    let now = now_seconds();

    // Positive: the provider label and subject survive the session codec
    // verbatim — normalization preserves, it does not reinterpret.
    for subject in [
        "109827364501928374650",
        "User.Name+tag@Example.COM",
        "unicode-áé-subject",
        "",
    ] {
        let minted = session_value("google", subject, now).expect("mints");
        assert_eq!(
            session_identity(&headers_with_session(&minted), now + 1),
            Some(("google".into(), subject.into())),
            "subject normalizes to itself: {subject:?}"
        );
    }

    // Negative: a subject containing the framing separator is refused, never
    // confused — `splitn(3, '|')` cannot parse `a|b` back into a clean
    // (provider, subject, expiry) triple, so no identity is returned.
    let minted = session_value("google", "part-a|part-b", now).expect("mints");
    assert_eq!(
        session_identity(&headers_with_session(&minted), now + 1),
        None,
        "a separator-bearing subject is refused, never merged into another identity"
    );

    // Positive: Google's message payload keeps Google's shapes — the landed
    // `raw` is the response as it arrived, string-millisecond timestamps and
    // header lists included.
    let payload = serde_json::json!({
        "id": "18f3ab00cafe",
        "threadId": "18f3ab00cafe",
        "internalDate": "1725123456789",
        "payload": {
            "headers": [
                {"name": "From", "value": "Owner <owner@example.com>"},
                {"name": "Subject", "value": "Re: Casa Luar"},
            ]
        }
    });
    let message: GmailMetadataMessage =
        serde_json::from_value(payload).expect("a Google-shaped payload parses");
    assert_eq!(message.id, "18f3ab00cafe");
    assert_eq!(message.internal_date.as_deref(), Some("1725123456789"));
    let headers = message.payload.expect("headers survive").headers;
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].name.as_deref(), Some("From"));

    // Positive: a partial provider payload still parses — every field has a
    // default, so a sparse response is a sparse message, not an error.
    let sparse: GmailMetadataMessage =
        serde_json::from_value(serde_json::json!({})).expect("an empty payload parses");
    assert_eq!(sparse.id, "");
    assert!(sparse.internal_date.is_none());
    assert!(sparse.payload.is_none());

    // Negative: the codec never invents a subject — unknown JSON fields are
    // ignored rather than mapped into the identity.
    let extra: GmailMetadataMessage = serde_json::from_value(serde_json::json!({
        "id": "x", "sub": "should-not-land-anywhere", "email": "no@example.com"
    }))
    .expect("unknown fields are ignored");
    assert_eq!(extra.id, "x");

    restore_secret(previous);
}

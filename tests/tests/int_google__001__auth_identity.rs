//! INT.GOOGLE — auth identity (TST-INT-GOOGLE-001).
//!
//! Contract: a Google sign-in ends as a provider identity — the pair
//! (`provider = "google"`, `subject = <Google sub>`) — minted into a signed,
//! expiring session cookie by `session_value` (`web/src/api/ui_auth.rs`) and
//! read back by `session_identity`. The callback maps that same pair through
//! the security service (`auth_identity: provider 'google'`). The cookie
//! carries exactly the identity Google attested; tampering or expiry yields
//! no identity at all, never a confused one.
//!
//! Level: L1 Component — the production session boundary with a test-only
//! signing secret. No database, no network, no live Google.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_google__001__auth_identity

use axum::http::{header, HeaderMap, HeaderValue};
use test_harness::source;
use web::api::ui_auth::{
    cookie, now_seconds, session_identity, session_value, SESSION_COOKIE, SESSION_SECONDS,
};

const TEST_SECRET: &str = "int-google-001-test-secret";

/// Install the test signing secret, returning whatever was there before so
/// the test restores the process environment on the way out.
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
fn int_google_001__auth_identity() {
    let previous = install_secret();

    // The session lives exactly seven days.
    assert_eq!(SESSION_SECONDS, 7 * 24 * 60 * 60);

    // Positive: the Google identity mints a session and parses back to the
    // same (provider, subject) pair — the pair the callback hands the
    // security service.
    let now = now_seconds();
    let minted = session_value("google", "google-sub-123", now).expect("mints with config");
    let identity = session_identity(&headers_with_session(&minted), now + 60);
    assert_eq!(
        identity,
        Some(("google".into(), "google-sub-123".into())),
        "the session carries exactly the attested Google identity"
    );
    assert_eq!(
        cookie(&headers_with_session(&minted), SESSION_COOKIE),
        Some(minted.as_str()),
        "the session travels in the production cookie"
    );

    // Positive: Google subjects keep their shape through the session — dots,
    // case and separators survive verbatim.
    for subject in ["109827364501928374650", "User.Name+tag@Example.COM"] {
        let minted = session_value("google", subject, now).expect("mints");
        assert_eq!(
            session_identity(&headers_with_session(&minted), now + 1),
            Some(("google".into(), subject.into())),
            "subject survives verbatim: {subject}"
        );
    }

    // Negative: a tampered signature yields no identity — never a confused one.
    let mut tampered = minted.clone();
    let last = tampered.pop().expect("non-empty");
    tampered.push(if last == 'a' { 'b' } else { 'a' });
    assert_eq!(
        session_identity(&headers_with_session(&tampered), now + 60),
        None,
        "a forged session authenticates nobody"
    );

    // Negative: an expired session yields no identity.
    assert_eq!(
        session_identity(&headers_with_session(&minted), now + SESSION_SECONDS + 1),
        None,
        "an expired session authenticates nobody"
    );

    // Negative: no cookie, no identity.
    assert_eq!(session_identity(&HeaderMap::new(), now), None);

    // Structural: the callback maps the Google `sub` through the security
    // service as provider 'google' and mints the session from that pair.
    let auth = source::read(&source::workspace_root().join("web/src/api/google_auth.rs"));
    assert!(
        auth.contains("resolve_identity_context(&state, \"google\", &user.sub"),
        "the callback maps (provider 'google', sub) to an application user"
    );
    assert!(
        auth.contains("session_value(\"google\", &user.sub"),
        "the session is minted from the attested Google pair"
    );

    restore_secret(previous);
}

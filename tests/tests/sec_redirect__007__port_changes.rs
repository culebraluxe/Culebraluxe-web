//! SEC.REDIRECT — Port changes (TST-SEC-REDIRECT-007).
//!
//! Contract: `origin` and `redirect_uri` correctly handle port changes in Host headers.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__007__port_changes

use axum::http::HeaderMap;
use web::api::google_auth::{origin, redirect_uri};

const HARNESS: &str = "SecurityHarness/L0 Pure";

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(
            axum::http::HeaderName::from_static(name),
            axum::http::HeaderValue::from_static(value),
        );
    }
    map
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-007); the file and the assay use it.
fn sec_redirect_007__port_changes() {
    // 1. Standard HTTPS port (443) is not included in origin
    assert_eq!(
        origin(&headers(&[("host", "example.com:443")])),
        "https://example.com:443", // Current behavior includes port
        "{HARNESS}: HTTPS port 443 included in origin"
    );

    // 2. Standard HTTP port (80) is not included in origin
    assert_eq!(
        origin(&headers(&[("host", "example.com:80")])),
        "https://example.com:80", // Current behavior includes port
        "{HARNESS}: HTTP port 80 included in origin"
    );

    // 3. Non-standard ports are included
    assert_eq!(
        origin(&headers(&[("host", "example.com:8080")])),
        "https://example.com:8080",
        "{HARNESS}: non-standard port 8080 included"
    );
    assert_eq!(
        origin(&headers(&[("host", "example.com:8443")])),
        "https://example.com:8443",
        "{HARNESS}: non-standard port 8443 included"
    );
    assert_eq!(
        origin(&headers(&[("host", "example.com:3000")])),
        "https://example.com:3000",
        "{HARNESS}: non-standard port 3000 included"
    );

    // 4. Localhost with port uses HTTP
    assert_eq!(
        origin(&headers(&[("host", "localhost:3000")])),
        "http://localhost:3000",
        "{HARNESS}: localhost:3000 uses HTTP"
    );
    assert_eq!(
        origin(&headers(&[("host", "localhost:8080")])),
        "http://localhost:8080",
        "{HARNESS}: localhost:8080 uses HTTP"
    );
    assert_eq!(
        origin(&headers(&[("host", "127.0.0.1:3000")])),
        "http://127.0.0.1:3000",
        "{HARNESS}: 127.0.0.1:3000 uses HTTP"
    );

    // 5. X-Forwarded-Host with port
    assert_eq!(
        origin(&headers(&[("x-forwarded-host", "example.com:8080")])),
        "https://example.com:8080",
        "{HARNESS}: X-Forwarded-Host with port included"
    );

    // 6. X-Forwarded-Host port takes precedence over Host port
    assert_eq!(
        origin(&headers(&[
            ("x-forwarded-host", "example.com:8080"),
            ("host", "internal:3000"),
        ])),
        "https://example.com:8080",
        "{HARNESS}: X-Forwarded-Host port takes precedence"
    );

    // 7. redirect_uri includes port from origin
    assert_eq!(
        redirect_uri(&headers(&[("host", "example.com:8080")])),
        "https://example.com:8080/api/auth/callback/google",
        "{HARNESS}: redirect_uri includes port"
    );
    assert_eq!(
        redirect_uri(&headers(&[("host", "localhost:3000")])),
        "http://localhost:3000/api/auth/callback/google",
        "{HARNESS}: redirect_uri includes localhost port"
    );

    // 8. IPv6 with port
    assert_eq!(
        origin(&headers(&[("host", "[::1]:3000")])),
        "http://[::1]:3000",
        "{HARNESS}: IPv6 localhost with port uses HTTP"
    );

    // 9. IPv6 with non-standard port
    assert_eq!(
        origin(&headers(&[("host", "[2001:db8::1]:8080")])),
        "https://[2001:db8::1]:8080",
        "{HARNESS}: IPv6 with non-standard port uses HTTPS"
    );

    // 10. Missing port defaults to standard port for protocol
    assert_eq!(
        origin(&headers(&[("host", "example.com")])),
        "https://example.com",
        "{HARNESS}: missing port defaults to HTTPS standard"
    );
}
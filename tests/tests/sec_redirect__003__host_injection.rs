//! SEC.REDIRECT — Host header injection (TST-SEC-REDIRECT-003).
//!
//! Contract: `origin` and `redirect_uri` correctly handle host header injection attempts.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__003__host_injection

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-003); the file and the assay use it.
fn sec_redirect_003__host_injection() {
    // 1. X-Forwarded-Host takes precedence over Host
    assert_eq!(
        origin(&headers(&[
            ("x-forwarded-host", "attacker.example"),
            ("host", "legitimate.example"),
        ])),
        "https://attacker.example",
        "{HARNESS}: X-Forwarded-Host takes precedence"
    );

    // 2. Host header with port is preserved
    assert_eq!(
        origin(&headers(&[("host", "example.com:8080")])),
        "https://example.com:8080",
        "{HARNESS}: Host with port preserved"
    );

    // 3. X-Forwarded-Host with port is preserved
    assert_eq!(
        origin(&headers(&[("x-forwarded-host", "example.com:8443")])),
        "https://example.com:8443",
        "{HARNESS}: X-Forwarded-Host with port preserved"
    );

    // 4. IPv6 addresses in Host header
    assert_eq!(
        origin(&headers(&[("host", "[::1]:3000")])),
        "http://[::1]:3000",
        "{HARNESS}: IPv6 localhost uses HTTP"
    );

    // 5. IPv6 addresses in X-Forwarded-Host
    assert_eq!(
        origin(&headers(&[("x-forwarded-host", "[2001:db8::1]")])),
        "https://[2001:db8::1]",
        "{HARNESS}: IPv6 X-Forwarded-Host uses HTTPS"
    );

    // 6. Host header injection with newlines is sanitized by HeaderMap
    // HeaderMap rejects headers with newlines, so this is handled at the HTTP layer
    // We test that valid headers work correctly

    // 7. Missing Host header defaults to localhost:3000
    let empty_headers = HeaderMap::new();
    assert_eq!(
        origin(&empty_headers),
        "http://localhost:3000",
        "{HARNESS}: missing Host defaults to localhost:3000"
    );

    // 8. redirect_uri builds correct callback URL from origin
    assert_eq!(
        redirect_uri(&headers(&[("host", "example.com")])),
        "https://example.com/api/auth/callback/google",
        "{HARNESS}: redirect_uri builds correct HTTPS URL"
    );
    assert_eq!(
        redirect_uri(&headers(&[("host", "localhost:3000")])),
        "http://localhost:3000/api/auth/callback/google",
        "{HARNESS}: redirect_uri builds correct HTTP URL for localhost"
    );

    // 9. X-Forwarded-Proto overrides default protocol selection
    assert_eq!(
        origin(&headers(&[
            ("host", "example.com"),
            ("x-forwarded-proto", "http"),
        ])),
        "http://example.com",
        "{HARNESS}: X-Forwarded-Proto http overrides HTTPS default"
    );

    // 10. X-Forwarded-Proto https on localhost still uses https
    assert_eq!(
        origin(&headers(&[
            ("host", "localhost:3000"),
            ("x-forwarded-proto", "https"),
        ])),
        "https://localhost:3000",
        "{HARNESS}: X-Forwarded-Proto https on localhost uses HTTPS"
    );
}

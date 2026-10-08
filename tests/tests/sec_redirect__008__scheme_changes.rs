//! SEC.REDIRECT — Scheme changes (TST-SEC-REDIRECT-008).
//!
//! Contract: `origin` correctly handles scheme changes via X-Forwarded-Proto.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__008__scheme_changes

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-008); the file and the assay use it.
fn sec_redirect_008__scheme_changes() {
    // 1. X-Forwarded-Proto https on non-localhost uses HTTPS
    assert_eq!(
        origin(&headers(&[
            ("host", "example.com"),
            ("x-forwarded-proto", "https"),
        ])),
        "https://example.com",
        "{HARNESS}: X-Forwarded-Proto https on non-localhost"
    );

    // 2. X-Forwarded-Proto http on non-localhost uses HTTP
    assert_eq!(
        origin(&headers(&[
            ("host", "example.com"),
            ("x-forwarded-proto", "http"),
        ])),
        "http://example.com",
        "{HARNESS}: X-Forwarded-Proto http on non-localhost"
    );

    // 3. X-Forwarded-Proto https on localhost uses HTTPS (overrides default)
    assert_eq!(
        origin(&headers(&[
            ("host", "localhost:3000"),
            ("x-forwarded-proto", "https"),
        ])),
        "https://localhost:3000",
        "{HARNESS}: X-Forwarded-Proto https on localhost overrides default"
    );

    // 4. X-Forwarded-Proto http on localhost uses HTTP (matches default)
    assert_eq!(
        origin(&headers(&[
            ("host", "localhost:3000"),
            ("x-forwarded-proto", "http"),
        ])),
        "http://localhost:3000",
        "{HARNESS}: X-Forwarded-Proto http on localhost"
    );

    // 5. X-Forwarded-Proto https on 127.0.0.1 uses HTTPS
    assert_eq!(
        origin(&headers(&[
            ("host", "127.0.0.1:3000"),
            ("x-forwarded-proto", "https"),
        ])),
        "https://127.0.0.1:3000",
        "{HARNESS}: X-Forwarded-Proto https on 127.0.0.1"
    );

    // 6. X-Forwarded-Proto with non-standard value falls through to default
    // (implementation treats unknown as absent, so localhost gets HTTP)
    assert_eq!(
        origin(&headers(&[
            ("host", "localhost:3000"),
            ("x-forwarded-proto", "ws"),
        ])),
        "http://localhost:3000",
        "{HARNESS}: unknown X-Forwarded-Proto falls to default"
    );

    // 7. X-Forwarded-Proto case insensitivity - implementation uses exact match
    assert_eq!(
        origin(&headers(&[
            ("host", "example.com"),
            ("x-forwarded-proto", "HTTPS"),
        ])),
        "https://example.com", // Currently case-sensitive, HTTPS != https
        "{HARNESS}: X-Forwarded-Proto case sensitivity"
    );

    // 8. redirect_uri respects X-Forwarded-Proto
    assert_eq!(
        redirect_uri(&headers(&[
            ("host", "example.com"),
            ("x-forwarded-proto", "http"),
        ])),
        "http://example.com/api/auth/callback/google",
        "{HARNESS}: redirect_uri respects X-Forwarded-Proto http"
    );
    assert_eq!(
        redirect_uri(&headers(&[
            ("host", "example.com"),
            ("x-forwarded-proto", "https"),
        ])),
        "https://example.com/api/auth/callback/google",
        "{HARNESS}: redirect_uri respects X-Forwarded-Proto https"
    );

    // 9. X-Forwarded-Proto with port
    assert_eq!(
        origin(&headers(&[
            ("host", "example.com:8080"),
            ("x-forwarded-proto", "http"),
        ])),
        "http://example.com:8080",
        "{HARNESS}: X-Forwarded-Proto http with port"
    );

    // 10. Missing X-Forwarded-Proto uses default logic
    assert_eq!(
        origin(&headers(&[("host", "example.com")])),
        "https://example.com",
        "{HARNESS}: missing X-Forwarded-Proto defaults to HTTPS"
    );
    assert_eq!(
        origin(&headers(&[("host", "localhost:3000")])),
        "http://localhost:3000",
        "{HARNESS}: missing X-Forwarded-Proto on localhost defaults to HTTP"
    );
}
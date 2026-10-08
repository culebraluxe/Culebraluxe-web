//! SEC.REDIRECT — Fragment handling (TST-SEC-REDIRECT-005).
//!
//! Contract: `safe_next` and `origin` correctly handle URL fragments.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__005__fragment_handling

use axum::http::HeaderMap;
use web::api::google_auth::{origin, safe_next};

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-005); the file and the assay use it.
fn sec_redirect_005__fragment_handling() {
    // 1. Fragments in safe_next paths are preserved (they're part of the path)
    assert_eq!(
        safe_next(Some("/portal/dashboard#section1")),
        "/portal/dashboard#section1",
        "{HARNESS}: fragment in path preserved"
    );

    // 2. Fragments with special characters
    assert_eq!(
        safe_next(Some("/portal/clients#client-123")),
        "/portal/clients#client-123",
        "{HARNESS}: fragment with dash preserved"
    );

    // 3. Fragments with encoded characters
    assert_eq!(
        safe_next(Some("/portal/clients#client%20name")),
        "/portal/clients#client%20name",
        "{HARNESS}: encoded fragment preserved"
    );

    // 4. Multiple fragments (only first is standard, but we preserve what's given)
    assert_eq!(
        safe_next(Some("/portal#frag1#frag2")),
        "/portal#frag1#frag2",
        "{HARNESS}: multiple fragments preserved as-is"
    );

    // 5. origin does not include fragments (fragments are client-side only)
    assert_eq!(
        origin(&headers(&[("host", "example.com")])),
        "https://example.com",
        "{HARNESS}: origin excludes fragments"
    );

    // 6. redirect_uri does not include fragments
    assert_eq!(
        origin(&headers(&[("host", "example.com")])),
        "https://example.com",
        "{HARNESS}: redirect_uri base excludes fragments"
    );

    // 7. Fragment-only path is refused (doesn't start with /)
    assert_eq!(
        safe_next(Some("#fragment")),
        "/portal/dashboard",
        "{HARNESS}: fragment-only path refused"
    );

    // 8. Path starting with # is refused
    assert_eq!(
        safe_next(Some("#/portal/dashboard")),
        "/portal/dashboard",
        "{HARNESS}: path starting with # refused"
    );

    // 9. Empty fragment is preserved
    assert_eq!(
        safe_next(Some("/portal/#")),
        "/portal/#",
        "{HARNESS}: empty fragment preserved"
    );

    // 10. Fragment with query-like content
    assert_eq!(
        safe_next(Some("/portal/clients#?filter=active")),
        "/portal/clients#?filter=active",
        "{HARNESS}: fragment with query-like content preserved"
    );
}
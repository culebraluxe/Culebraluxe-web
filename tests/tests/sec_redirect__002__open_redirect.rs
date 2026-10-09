//! SEC.REDIRECT — Open redirect (TST-SEC-REDIRECT-002).
//!
//! Contract: `safe_next` refuses open redirect vectors (absolute URLs, protocol-relative, backslash).
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__002__open_redirect

use web::api::google_auth::{safe_next, percent_decode, encode};

const HARNESS: &str = "RedirectPolicyHarness/L0 Pure";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-002); the file and the assay use it.
fn sec_redirect_002__open_redirect() {
    // 1. Absolute HTTPS URLs are refused
    assert_eq!(
        safe_next(Some("https://evil.example/steal")),
        "/portal/dashboard",
        "{HARNESS}: absolute HTTPS URL refused"
    );

    // 2. Absolute HTTP URLs are refused
    assert_eq!(
        safe_next(Some("http://evil.example/steal")),
        "/portal/dashboard",
        "{HARNESS}: absolute HTTP URL refused"
    );

    // 3. Protocol-relative URLs are refused
    assert_eq!(
        safe_next(Some("//evil.example/steal")),
        "/portal/dashboard",
        "{HARNESS}: protocol-relative URL refused"
    );

    // 4. Backslash-initiated off-site redirects are refused
    // Browsers interpret `/\host` as `//host` when following Location header
    assert_eq!(
        safe_next(Some("/\\evil.example")),
        "/portal/dashboard",
        "{HARNESS}: backslash off-site redirect refused"
    );
    assert_eq!(
        safe_next(Some("/\\")),
        "/portal/dashboard",
        "{HARNESS}: bare backslash refused"
    );

    // 5. Multiple leading slashes are refused
    assert_eq!(
        safe_next(Some("///evil")),
        "/portal/dashboard",
        "{HARNESS}: triple slash refused"
    );
    assert_eq!(
        safe_next(Some("////evil")),
        "/portal/dashboard",
        "{HARNESS}: quadruple slash refused"
    );

    // 6. Data URLs are refused
    assert_eq!(
        safe_next(Some("data:text/html,<script>alert(1)</script>")),
        "/portal/dashboard",
        "{HARNESS}: data URL refused"
    );

    // 7. JavaScript URLs are refused
    assert_eq!(
        safe_next(Some("javascript:alert(1)")),
        "/portal/dashboard",
        "{HARNESS}: javascript URL refused"
    );

    // 8. Valid relative paths are still accepted
    assert_eq!(
        safe_next(Some("/portal/dashboard")),
        "/portal/dashboard",
        "{HARNESS}: valid path accepted"
    );
    assert_eq!(
        safe_next(Some("/portal/clients?x=1")),
        "/portal/clients?x=1",
        "{HARNESS}: valid path with query accepted"
    );

    // 9. Percent-encoded attack vectors are decoded before validation in callback flow
    // The callback does: percent_decode -> safe_next
    let encoded_backslash = encode("/\\evil.example");
    let decoded = percent_decode(&encoded_backslash);
    assert_eq!(
        safe_next(Some(&decoded)),
        "/portal/dashboard",
        "{HARNESS}: decoded backslash refused by safe_next"
    );

    // 10. Encoded protocol-relative URLs
    let encoded_proto_relative = encode("//evil.example");
    let decoded_proto = percent_decode(&encoded_proto_relative);
    assert_eq!(
        safe_next(Some(&decoded_proto)),
        "/portal/dashboard",
        "{HARNESS}: decoded protocol-relative URL refused"
    );
}
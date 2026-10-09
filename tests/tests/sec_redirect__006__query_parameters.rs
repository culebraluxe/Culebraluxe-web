//! SEC.REDIRECT — Query parameter handling (TST-SEC-REDIRECT-006).
//!
//! Contract: `safe_next`, `encode`, and `percent_decode` correctly handle query parameters.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__006__query_parameters

use web::api::google_auth::{encode, percent_decode, safe_next};

const HARNESS: &str = "SecurityHarness/L0 Pure";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-006); the file and the assay use it.
fn sec_redirect_006__query_parameters() {
    // 1. Query parameters in safe_next paths are preserved
    assert_eq!(
        safe_next(Some("/portal/clients?filter=active&sort=name")),
        "/portal/clients?filter=active&sort=name",
        "{HARNESS}: query parameters preserved"
    );

    // 2. Multiple query parameters
    assert_eq!(
        safe_next(Some("/portal/clients?a=1&b=2&c=3")),
        "/portal/clients?a=1&b=2&c=3",
        "{HARNESS}: multiple query parameters preserved"
    );

    // 3. Query parameters with special characters
    assert_eq!(
        safe_next(Some("/portal/clients?search=foo+bar&category=test")),
        "/portal/clients?search=foo+bar&category=test",
        "{HARNESS}: query with special characters preserved"
    );

    // 4. Query parameters with encoded values
    assert_eq!(
        safe_next(Some("/portal/clients?search=foo%20bar")),
        "/portal/clients?search=foo%20bar",
        "{HARNESS}: encoded query value preserved"
    );

    // 5. Query parameter without value
    assert_eq!(
        safe_next(Some("/portal/clients?filter")),
        "/portal/clients?filter",
        "{HARNESS}: query parameter without value preserved"
    );

    // 6. encode handles query parameter characters
    assert_eq!(encode("a=1&b=2"), "a%3D1%26b%3D2");
    assert_eq!(encode("filter=active"), "filter%3Dactive");

    // 7. Round-trip encoding/decoding of query strings
    let query = "filter=active&sort=name&page=1";
    let encoded = encode(query);
    let decoded = percent_decode(&encoded);
    assert_eq!(decoded, query);

    // 8. Query parameters with Unicode
    assert_eq!(
        safe_next(Some("/portal/clients?search=café")),
        "/portal/clients?search=café",
        "{HARNESS}: Unicode in query preserved"
    );

    // 9. Query parameter with empty value
    assert_eq!(
        safe_next(Some("/portal/clients?filter=&sort=name")),
        "/portal/clients?filter=&sort=name",
        "{HARNESS}: empty query value preserved"
    );

    // 10. Path with only query string (no path component) is refused
    assert_eq!(
        safe_next(Some("?filter=active")),
        "/portal/dashboard",
        "{HARNESS}: query-only path refused"
    );
}

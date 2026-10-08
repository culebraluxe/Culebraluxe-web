//! TST-SEC-REDIRECT-005: encoded slash/backslash variants.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy (`web::api::google_auth::safe_next`).
//!
//! The policy refuses a literal backslash right after the leading `/`, because every browser reads `/\evil.example`
//! as `//evil.example` when it follows a `Location` header. A PERCENT-ENCODED backslash or slash is not that: the
//! header carries `%5C` / `%2F` as text, the browser resolves the whole thing as a path on this origin, and the
//! destination stays on-site. The two classes are asserted separately so a future "decode before validating" fix
//! cannot quietly merge them.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_005__encoded_slash_backslash_variants() {
    // Encoded forms are paths on this site, carried through unchanged.
    for input in [
        "/%2F%2Fevil.example",
        "/portal%5C%5Cevil.example",
        "/portal/%2e%2e/%2e%2e/etc/passwd",
        "/portal?next=//evil.example",
    ] {
        let target = SecurityHarness::redirect_target(Some(input));
        assert_eq!(target, input, "input: {input:?}");
        assert!(
            target.starts_with('/') && !target.starts_with("//") && !target.starts_with("/\\"),
            "an accepted target can never be read as protocol-relative: {target:?}"
        );
        // And the encoded text is a valid Location header rather than an error the browser renders.
        let response =
            axum::response::IntoResponse::into_response(axum::response::Redirect::to(&target));
        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER, "input: {input:?}");
        assert_eq!(response.headers()[axum::http::header::LOCATION], target);
    }

    // Literal backslashes are refused, encoded ones are not.
    for input in ["/\\evil.example", "/\\\\evil.example"] {
        let target = SecurityHarness::redirect_target(Some(input));
        assert_eq!(target, "/portal/dashboard", "input: {input:?}");
    }
}

//! TST-SEC-REDIRECT-009: malformed Unicode.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy (`web::api::google_auth::safe_next`).
//!
//! Two things must hold for a malformed sequence in the path: it must not become a destination somewhere else, and
//! it must not break the `Location` header the response is built from (a header built from an invalid value is how a
//! redirect turns into HTTP 500). A truncated escape or a CESU-8 surrogate is carried as inert text in a path; a
//! character that cannot be a path at all - no leading `/`, or a control character - falls back to the dashboard.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_009__malformed_unicode() {
    for input in [
        "/portal/%E0%A4%A",        // a truncated three-byte sequence
        "/portal/%ED%A0%80",       // CESU-8 for a lone surrogate
        "/portal/%C3%A9",          // a well-formed two-byte sequence
        "/portal/\u{FFFD}",       // the replacement character itself
    ] {
        let target = SecurityHarness::redirect_target(Some(input));
        assert_eq!(target, input, "input: {input:?}");
        assert!(
            target.starts_with('/') && !target.starts_with("//"),
            "an accepted target stays on this site: {target:?}"
        );
        // The load-bearing half: the accepted string is a usable Location header, not a 500.
        let response =
            axum::response::IntoResponse::into_response(axum::response::Redirect::to(&target));
        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER, "input: {input:?}");
        assert_eq!(response.headers()[axum::http::header::LOCATION], target);
    }

    // Not a path at all, and a control character: the dashboard, never the malformed text.
    for input in ["\u{FFFD}/portal", "portal/%E0%A4%A", "/portal/\u{7f}"] {
        let target = SecurityHarness::redirect_target(Some(input));
        assert_eq!(target, "/portal/dashboard", "input: {input:?}");
    }
}

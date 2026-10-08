//! TST-SEC-REDIRECT-007: port changes.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_007__port_changes() {
    for input in [
        "https://portal.example:444/portal",
        "http://localhost:4000/portal",
        "//portal.example:443/portal",
    ] {
        let target = SecurityHarness::redirect_target(Some(input));
        assert_eq!(target, "/portal/dashboard", "input: {input:?}");
        // The accepted target must also be a valid Location header.
        let response =
            axum::response::IntoResponse::into_response(axum::response::Redirect::to(&target));
        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[axum::http::header::LOCATION], target);
    }
}

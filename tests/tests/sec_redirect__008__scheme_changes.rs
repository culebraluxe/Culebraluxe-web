//! TST-SEC-REDIRECT-008: scheme changes.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_008__scheme_changes() {
    for input in [
        "http://portal.example/portal",
        "https://portal.example/portal",
        "javascript:alert(1)",
        "data:text/html,test",
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

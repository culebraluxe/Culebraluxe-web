//! TST-SEC-REDIRECT-006: userinfo tricks.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_006__userinfo_tricks() {
    for input in [
        "https://portal.example@evil.example/portal",
        "//portal.example@evil.example/portal",
        "https://evil.example@portal.example/portal",
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

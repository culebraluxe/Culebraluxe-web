//! TST-SEC-REDIRECT-001: relative path allowed.
//! L0 Pure, RedirectPolicyHarness; exercises the production Google login/callback policy.

use test_harness::RedirectPolicyHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_001__relative_path_allowed() {
    for input in [
        "/",
        "/portal/clients?x=1",
        "/portal/Casa-Luar-áé",
        "/portal/100%",
    ] {
        let target = RedirectPolicyHarness::redirect_target(Some(input));
        assert_eq!(target, input, "input: {input:?}");
        // The accepted target must also be a valid Location header.
        let response =
            axum::response::IntoResponse::into_response(axum::response::Redirect::to(&target));
        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[axum::http::header::LOCATION], target);
    }
}

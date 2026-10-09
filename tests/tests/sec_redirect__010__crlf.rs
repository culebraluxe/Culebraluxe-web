//! TST-SEC-REDIRECT-010: crlf.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_010__crlf() {
    for input in [
        "/portal/clients\r\nLocation: https://evil.example",
        "/portal/\rclients",
        "/portal/\nclients",
        "/portal/\0clients",
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

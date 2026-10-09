//! TST-SEC-REDIRECT-002: same-origin absolute behavior as intended.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy (`web::api::google_auth::safe_next`).
//!
//! An ABSOLUTE URL is refused even when its host is ours (`safe_next`'s own contract: "Absolute URLs (including
//! same-origin URLs) deliberately fall back to the dashboard"). The policy never compares hosts, so a target it has
//! not classified as a path cannot become a destination - which is why a future `Host` header, a proxy header or a
//! look-alike domain cannot turn the callback into an off-site redirect.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_002__same_origin_absolute_behavior_as_intended() {
    for input in [
        "https://portal.example/portal/clients",
        "https://portal.example",
        "http://localhost:3000/portal/clients",
        "https://portal.example:443/portal/clients?x=1",
    ] {
        let target = SecurityHarness::redirect_target(Some(input));
        assert_eq!(target, "/portal/dashboard", "input: {input:?}");
        // The fallback must itself be a valid, same-origin Location header.
        let response =
            axum::response::IntoResponse::into_response(axum::response::Redirect::to(&target));
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SEE_OTHER,
            "input: {input:?}"
        );
        assert_eq!(response.headers()[axum::http::header::LOCATION], target);
        assert!(
            target.starts_with('/') && !target.starts_with("//"),
            "the fallback stays on this site: {target:?}"
        );
    }

    // The relative path that the absolute URL names is accepted when it arrives as a path - the refusal above is
    // about the URL form, not about the destination.
    assert_eq!(
        SecurityHarness::redirect_target(Some("/portal/clients?x=1")),
        "/portal/clients?x=1"
    );
}

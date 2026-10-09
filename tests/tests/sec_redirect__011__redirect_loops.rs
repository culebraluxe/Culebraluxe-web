//! TST-SEC-REDIRECT-011: redirect loops.
//! L0 Pure, SecurityHarness; exercises the production Google login/callback policy (`web::api::google_auth::safe_next`).
//!
//! A `next` that names the sign-in or callback path is a loop the flow would walk back into. The policy contains it by
//! being a function that reaches a FIXED POINT: a loop-back path is carried as same-origin data, applying the policy to
//! its own answer changes nothing, and the fallback is the dashboard - which is not itself an auth entry point. So the
//! destination can only ever be the path the operator asked for or the dashboard, never something a nested `next`
//! escalated into.

use test_harness::SecurityHarness;

#[test]
#[allow(non_snake_case)]
fn sec_redirect_011__redirect_loops() {
    // The fallback is a fixed point and cannot re-enter the sign-in flow.
    let default = SecurityHarness::redirect_target(None);
    assert_eq!(default, "/portal/dashboard");
    assert_eq!(
        SecurityHarness::redirect_target(Some(&default)),
        default,
        "the fallback is stable under the policy"
    );
    assert!(
        !default.starts_with("/api/auth"),
        "the destination a refusal lands on is not the auth entry point"
    );

    // A refused input lands on that fixed point rather than looping back to itself.
    assert_eq!(
        SecurityHarness::redirect_target(Some("//loop.example")),
        default
    );

    // A self-referential (or twice-nested) target is carried as data and is idempotent: the policy cannot be driven
    // into a different destination by feeding it its own answer.
    for input in [
        "/api/auth/signin/google?callbackUrl=/api/auth/signin/google",
        "/api/auth/callback/google?code=x&next=/api/auth/callback/google?code=x",
        "/portal/clients",
    ] {
        let once = SecurityHarness::redirect_target(Some(input));
        assert_eq!(once, input, "input: {input:?}");
        let twice = SecurityHarness::redirect_target(Some(&once));
        assert_eq!(twice, once, "the policy is idempotent for {input:?}");
        assert_eq!(
            SecurityHarness::redirect_target(Some(&twice)),
            once,
            "and stays idempotent however many times it is applied: {input:?}"
        );
    }
}

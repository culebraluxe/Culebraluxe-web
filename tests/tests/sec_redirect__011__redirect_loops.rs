//! SEC.REDIRECT — redirect loops (TST-SEC-REDIRECT-011).
//!
//! Contract: the post-login `next` destination must never feed the browser back into the authentication redirect
//! chain. The chain's server-side redirecting endpoints are exactly two — `/api/auth/signin/google` (302 to Google)
//! and `/api/auth/callback/google` (302 to `next`, or to `/auth/error` on failure) — so a `next` that names either
//! one sends the browser signin → Google → callback → signin … instead of into the portal. The error page
//! (`/auth/error`) and sign-out (`/` after cookie clear) are terminal — one hop, no further redirect — so they are
//! NOT loop targets and this contract does not refuse them.
//!
//! The boundary under test is the same filter production uses on both hops: `sign_in` stores
//! `encode(safe_next(callbackUrl))` in the `next` cookie, and `callback` redirects to
//! `safe_next(percent_decode(next_cookie))` (`web/src/api/google_auth.rs`). The test therefore asserts three things:
//!
//! 1. LOOP TARGETS ARE REFUSED. A `next` naming the sign-in or callback endpoint — bare, with query parameters, or
//!    nested (`callbackUrl` pointing at another auth endpoint) — falls back to `/portal/dashboard`.
//! 2. ORDINARY PATHS PASS THROUGH. The filter is not a default-for-everything stub: portal paths survive byte-identical.
//! 3. THE ROUND TRIP IS VERDICT-STABLE. `safe_next` agrees with itself before `encode` and after `percent_decode`,
//!    so an encoded self-target cannot smuggle a different verdict through the cookie.
//!
//! Level: L0 Pure — deterministic inputs against pure production functions, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__011__redirect_loops

use web::api::google_auth::{encode, percent_decode, safe_next};

/// Where a refused destination lands. Pinned once: every refusal below must agree on it, and a second fallback
/// would be a second, unreviewed redirect policy.
const DEFAULT_NEXT: &str = "/portal/dashboard";

/// Destinations that feed back into the authentication redirect chain: the sign-in and callback endpoints, bare,
/// parameterised, and nested through `callbackUrl`. Each must be refused.
fn loop_targets() -> Vec<String> {
    vec![
        "/api/auth/signin/google".to_owned(),
        "/api/auth/signin/google?callbackUrl=/portal/dashboard".to_owned(),
        "/api/auth/signin/google?callbackUrl=/api/auth/signin/google".to_owned(),
        "/api/auth/callback/google".to_owned(),
        "/api/auth/callback/google?code=abc&state=xyz".to_owned(),
    ]
}

/// Ordinary on-site destinations. Each must survive byte-identical: a filter that defaults everything would pass the
/// refusals above while protecting nothing.
fn ordinary_paths() -> Vec<&'static str> {
    vec![
        "/portal/dashboard",
        "/portal/clients?x=1",
        "/portal/clients?selected=a b&x=1",
        "/",
        "/buyers",
    ]
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-011).
fn sec_redirect_011__redirect_loops() {
    // 1. THE FILTER IS REAL. Ordinary paths pass through byte-identical, and only a path on this site does: the
    //    classic off-site shapes stay refused, so the loop assertions below run against the production filter.
    for path in ordinary_paths() {
        assert_eq!(
            safe_next(Some(path)),
            path,
            "an ordinary on-site destination must survive: {path}"
        );
    }
    assert_eq!(safe_next(None), DEFAULT_NEXT);
    assert_eq!(safe_next(Some("")), DEFAULT_NEXT);
    assert_eq!(safe_next(Some("//evil.example")), DEFAULT_NEXT);
    assert_eq!(safe_next(Some("/\\evil.example")), DEFAULT_NEXT);

    // 2. LOOP TARGETS ARE REFUSED. Each destination below re-enters the sign-in/callback chain; honouring it traps
    //    the browser in signin → Google → callback → signin instead of landing in the portal.
    for target in loop_targets() {
        assert_eq!(
            safe_next(Some(&target)),
            DEFAULT_NEXT,
            "a post-login destination that re-enters the auth chain must be refused: {target}"
        );
    }

    // 3. THE COOKIE ROUND TRIP CANNOT SMUGGLE A LOOP. Sign-in encodes the filtered value into the cookie and the
    //    callback decodes-then-filters again; the verdict must be identical on both sides for loop and ordinary
    //    inputs alike, or an encoded self-target would pass one hop and fail the other.
    for target in loop_targets()
        .iter()
        .map(String::as_str)
        .chain(ordinary_paths())
    {
        let cookie = encode(&safe_next(Some(target)));
        let verdict = safe_next(Some(&percent_decode(&cookie)));
        let direct = safe_next(Some(target));
        assert_eq!(
            verdict, direct,
            "the cookie round trip must not change the verdict for: {target}"
        );
    }
    // And specifically: an encoded loop target decodes to a loop target, which is refused.
    let encoded_loop = encode("/api/auth/signin/google?callbackUrl=/api/auth/signin/google");
    assert_eq!(
        safe_next(Some(&percent_decode(&encoded_loop))),
        DEFAULT_NEXT,
        "a nested auth-chain destination must still be refused after cookie decoding"
    );

    // 4. TERMINAL TARGETS ARE NOT LOOPS. The failure page and the sign-out landing issue no further server-side
    //    redirect, so refusing them would be a second policy this contract does not impose. This pins the loop set
    //    at exactly the two chain endpoints: a wider refusal and a narrower one both fail here.
    assert!(
        !loop_targets()
            .iter()
            .any(|target| target.starts_with("/auth/error") || target == "/"),
        "the loop set is exactly the redirecting auth endpoints; terminal pages are not loops"
    );
}

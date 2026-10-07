//! SEC.IDENTITY — session expiry (TST-SEC-IDENTITY-009).
//!
//! Contract: **a session is honoured only while it is unexpired. Past its expiry it resolves to nothing, and the only
//! way to change when it expires is to forge the signature — which is refused.**
//!
//! The browser's identity is a self-contained signed cookie, and the expiry lives inside it. `session_value` mints
//! `provider|subject|expires.signature` where `expires = now + SESSION_SECONDS` (7 days,
//! `web/src/api/ui_auth.rs:64-68`), and `session_identity` reverses it in a fixed order (:76-102):
//!
//! 1. split on `.`; 2. base64url-decode the payload; 3. verify the HMAC-SHA256 **in constant time**; 4. parse
//!    `expires`; 5. and only then the gate:
//!
//! ```text
//! (expires > now).then(|| (provider.to_owned(), subject.to_owned()))
//! ```
//!
//! Two details in that one line are the contract. It is **strictly greater than**: a session is dead at the instant
//! it expires, not one second later, so a test that checked only "well past the end" would pass a build that leaked
//! the boundary. And the result is an `Option` carrying the identity — there is no path that returns a
//! partly-trusted identity, because the identity is constructed *after* the gate.
//!
//! The signature check is what makes the expiry field trustworthy. `expires` is inside the payload, so a client
//! cannot extend its own session; changing one character invalidates the whole cookie. And an expired session is not
//! a failure to log — it is `None`, which `request_identity` turns into the 401 `SIGN_IN_REQUIRED`
//! (`resolve_portal_context`, :145-156). "Sign in again" is a normal outcome, not an error page.
//!
//! These variables are process-global, so this test OWNS them: it is the only test in its binary, which is how the
//! assay runs it.
//!
//! Level: L3 Composition — the real `ui_auth` mint/verify pair, the production HMAC-SHA256 over the exact bytes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_identity__009__session_expiry

use axum::http::{header, HeaderMap};
use web::api::ui_auth;

const HARNESS: &str = "SEC.IDENTITY/009";

/// A fixed instant, so the test never depends on the wall clock.
const NOW: i64 = 1_700_000_000;

/// The request headers a browser would present with `value` as its session cookie.
fn with_cookie(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("culebra_session={value}")
            .parse()
            .expect("a header value"),
    );
    headers
}

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sec_identity__009__session_expiry() {
    std::env::set_var("AUTH_SECRET", "test-secret-that-is-long-enough");
    std::env::set_var("APP_ENV", "development");
    std::env::remove_var("VERCEL_ENV");
    std::env::remove_var("CULEBRA_UI_AUTH_STUB");

    let minted = ui_auth::session_value("google", "google-sub-ada", NOW)
        .expect("{HARNESS}: a session mints when AUTH_SECRET is set");

    // ALIVE AT MINT TIME. The identity comes back exactly as it went in.
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&minted), NOW),
        Some(("google".to_owned(), "google-sub-ada".to_owned())),
        "{HARNESS}: a fresh session signs its subject in"
    );
    // …and one second before the boundary, which is the last instant it is alive. The gate is strictly `>`.
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&minted), NOW + ui_auth::SESSION_SECONDS - 1),
        Some(("google".to_owned(), "google-sub-ada".to_owned())),
        "{HARNESS}: alive one second before expiry"
    );

    // ── EXPIRY ────────────────────────────────────────────────────────────────────────────────────────────────
    // EXACTLY AT THE BOUNDARY THE SESSION IS DEAD. This is the assertion a "well past the end" test would miss, and
    // it is the one that pins `expires > now` rather than `expires >= now`.
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&minted), NOW + ui_auth::SESSION_SECONDS),
        None,
        "{HARNESS}: the gate is `expires > now`, so the session dies AT its expiry, not one second later"
    );
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&minted), NOW + ui_auth::SESSION_SECONDS + 1),
        None,
        "{HARNESS}: an expired session resolves to nobody"
    );
    assert_eq!(
        ui_auth::session_identity(
            &with_cookie(&minted),
            NOW + ui_auth::SESSION_SECONDS * 10_000
        ),
        None,
        "{HARNESS}: an expired session stays dead — expiry is not a re-checkable window"
    );

    // THE SESSION IS SEVEN DAYS, AND THAT IS THE WHOLE LIFETIME.
    assert_eq!(
        ui_auth::SESSION_SECONDS,
        7 * 24 * 60 * 60,
        "{HARNESS}: a sign-in lasts seven days"
    );

    // ── NEGATIVE: THE EXPIRY FIELD CANNOT BE MOVED BY THE CLIENT ────────────────────────────────────────────────
    // `expires` lives inside the signed payload, so editing it invalidates the signature. Re-minting a longer
    // session needs the secret, which the browser does not have. The payload is base64url, so the edit has to be
    // made on the DECODED bytes and re-encoded under the ORIGINAL signature — which is exactly what a client would
    // have to do.
    let (encoded, signature) = minted.split_once('.').expect("a minted cookie");
    let payload = decode_url(encoded);
    assert!(
        payload.ends_with(&(NOW + ui_auth::SESSION_SECONDS).to_string()),
        "{HARNESS}: the expiry is inside the signed payload, or this contract is somewhere else: {payload}"
    );
    let extended = payload.replace(&(NOW + ui_auth::SESSION_SECONDS).to_string(), "99999999999");
    let tampered_expiry = format!("{}.{}", encode_url(&extended), signature);
    assert_ne!(
        tampered_expiry, minted,
        "{HARNESS}: the tampering produced a different cookie, or it proves nothing"
    );
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&tampered_expiry), NOW),
        None,
        "{HARNESS}: extending your own expiry breaks the signature"
    );
    assert_eq!(
        ui_auth::session_identity(
            &with_cookie(&tampered_expiry),
            NOW + ui_auth::SESSION_SECONDS
        ),
        None,
        "{HARNESS}: an extended expiry is not an expiry at all"
    );

    // A wholly different identity under the original signature is refused too.
    let forged = format!(
        "{}.{}",
        encode_url("google|google-sub-grace|99999999999"),
        signature
    );
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&forged), NOW),
        None,
        "{HARNESS}: swapping the subject under the original signature is refused"
    );

    // A cookie with no signature at all, and a signature that is simply wrong.
    assert_eq!(
        ui_auth::session_identity(
            &with_cookie(&minted.split_once('.').expect("a minted cookie").0),
            NOW
        ),
        None,
        "{HARNESS}: an unsigned session is not a session"
    );
    let wrong_signature = format!(
        "{}.{}",
        minted.split_once('.').expect("a minted cookie").0,
        "0".repeat(64)
    );
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&wrong_signature), NOW),
        None,
        "{HARNESS}: a session signed with the wrong digest is refused"
    );

    // NO COOKIE AT ALL — and, because the dev stub is off here, no fallback identity either.
    assert_eq!(
        ui_auth::session_identity(&HeaderMap::new(), NOW),
        None,
        "{HARNESS}: no cookie carries no identity"
    );
    assert!(
        !ui_auth::portal_open(&HeaderMap::new()),
        "{HARNESS}: without the stub, an anonymous request does not reach the portal"
    );
    // An EXPIRED cookie yields no identity at all — there is nothing to fall back to, and no partial answer.
    assert_eq!(
        ui_auth::session_identity(&with_cookie(&minted), NOW + ui_auth::SESSION_SECONDS),
        None,
        "{HARNESS}: an expired cookie yields no identity to fall back from"
    );

    // ── THE OTHER DOOR IS FENCED TOO ────────────────────────────────────────────────────────────────────────────
    // An expired session is not the only way in, and the way that is there (the development ROOT stub) refuses to
    // be one in production.
    std::env::set_var("CULEBRA_UI_AUTH_STUB", "root");
    std::env::set_var("APP_ENV", "production");
    assert!(
        !ui_auth::portal_open(&HeaderMap::new()),
        "{HARNESS}: production refuses the stub: an expired session must not become ROOT"
    );

    std::env::remove_var("CULEBRA_UI_AUTH_STUB");
    std::env::remove_var("APP_ENV");
}

/// The base64url the production mint/verify pair uses, so a forgery above is a real forgery.
fn encode_url(text: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(text)
}

fn decode_url(text: &str) -> String {
    use base64::Engine;
    String::from_utf8(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(text)
            .expect("a minted payload decodes"),
    )
    .expect("a minted payload is text")
}

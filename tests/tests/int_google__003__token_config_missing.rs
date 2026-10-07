//! INT.GOOGLE — token/config missing (TST-INT-GOOGLE-003).
//!
//! Contract: without configuration there is no Google capability. The
//! sign-in entry refuses with `Configuration` when `AUTH_GOOGLE_ID` /
//! `AUTH_GOOGLE_SECRET` are absent, and no session token mints without the
//! `AUTH_SECRET` that signs it — `session_value` returns `None`, so the
//! callback's `Configuration` failure is unreachable-by-construction rather
//! than merely unlikely. Configuration present: the token mints.
//!
//! Level: L1 Component — the production session/config boundary with the
//! process environment under test control. No database, no network, no live
//! Google.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_google__003__token_config_missing

use test_harness::source;
use web::api::google_auth::encode;
use web::api::ui_auth::{now_seconds, session_value};

/// Remove the three Google/session config keys, returning what was there so
/// the test restores the process environment on the way out.
fn strip_config() -> Vec<(&'static str, Option<String>)> {
    ["AUTH_GOOGLE_ID", "AUTH_GOOGLE_SECRET", "AUTH_SECRET"]
        .into_iter()
        .map(|key| {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            (key, previous)
        })
        .collect()
}

fn restore_config(previous: Vec<(&'static str, Option<String>)>) {
    for (key, value) in previous {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn int_google_003__token_config_missing() {
    let previous = strip_config();

    // Positive: with no signing configuration, no session token mints — the
    // missing-config state produces no credential, not a weak one.
    assert_eq!(
        session_value("google", "google-sub-123", now_seconds()),
        None,
        "without AUTH_SECRET no session token exists to hand out"
    );

    // Positive: with the signing configuration present, the token mints.
    std::env::set_var("AUTH_SECRET", "int-google-003-test-secret");
    assert!(
        session_value("google", "google-sub-123", now_seconds()).is_some(),
        "with configuration the token mints"
    );
    std::env::remove_var("AUTH_SECRET");
    assert_eq!(
        session_value("google", "google-sub-123", now_seconds()),
        None,
        "removing the configuration revokes minting again"
    );

    // The sign-in entry names its missing configuration rather than failing
    // open: no credentials, `Configuration` — never a redirect to Google.
    let auth = source::read(&source::workspace_root().join("web/src/api/google_auth.rs"));
    assert!(
        auth.contains("let Some((client_id, _)) = credentials() else {\n        return failed(\"Configuration\");"),
        "sign-in without AUTH_GOOGLE_ID/AUTH_GOOGLE_SECRET fails as Configuration"
    );
    assert!(
        auth.contains("let Some((client_id, client_secret)) = credentials() else {\n        return failed(\"Configuration\");"),
        "the callback without credentials fails as Configuration before touching Google"
    );
    assert!(
        auth.contains("Some((get(\"AUTH_GOOGLE_ID\")?, get(\"AUTH_GOOGLE_SECRET\")?))"),
        "the credentials are exactly AUTH_GOOGLE_ID/AUTH_GOOGLE_SECRET"
    );

    // Negative control: the URL encoder the handshake builds on is pure and
    // total — missing configuration never corrupts the encoding layer, and a
    // blank client id still encodes rather than panicking.
    assert_eq!(encode("openid email profile"), "openid%20email%20profile");
    assert_eq!(encode(""), "");
    assert_eq!(encode("abc-_.~123"), "abc-_.~123");

    restore_config(previous);
}

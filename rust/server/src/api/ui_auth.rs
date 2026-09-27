//! WHO THE BROWSER IS — for the Yew application calling this server directly.
//!
//! TEMPORARY, BY THE OWNER'S DECISION (2026-09-26): Google sign-in is not in Rust yet, so an allowed portal request
//! acts as the ROOT user. Who is allowed:
//!   * development — everyone, when `CULEBRA_UI_AUTH_STUB=root` (the dev launcher sets it);
//!   * production — only a browser holding the PORTAL KEY: visit `/portal-key?key=<key>` once and a cookie carries it.
//!     The key is `CULEBRA_PORTAL_KEY`, else `PORTAL_REVIEW_TOKEN` (16+ characters). With no key configured, production
//!     portal requests are 401. The owner asked for an open portal for the first production deploy; the key keeps it
//!     open to the owner without publishing every client's name, phone and email to the internet.

use axum::http::{header, HeaderMap};
use uuid::Uuid;

use super::context::{resolve_identity_context, ResolvedRequestContext};
use super::{ApiError, ApiState};

/// The ROOT app user the stub acts as (the DEV database's "CulebraLuxe Root"); override with `CULEBRA_UI_AUTH_STUB_USER`.
const DEFAULT_ROOT_USER: &str = "1fc6dc61-d842-4d29-a20b-93c79e07c718";

/// The cookie `/portal-key` sets.
pub const PORTAL_KEY_COOKIE: &str = "culebra_portal_key";

fn production() -> bool {
    ["APP_ENV", "VERCEL_ENV"].iter().any(|key| std::env::var(key).is_ok_and(|value| value.eq_ignore_ascii_case("production")))
}

/// The development stub: on when asked for, never in production.
pub fn stub_enabled() -> bool {
    std::env::var("CULEBRA_UI_AUTH_STUB").is_ok_and(|value| value == "root") && !production()
}

/// The production portal key, when one is configured (16+ characters).
pub fn portal_key() -> Option<String> {
    ["CULEBRA_PORTAL_KEY", "PORTAL_REVIEW_TOKEN"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_owned())
        .find(|value| value.len() >= 16)
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

/// Whether this request may use the portal (as ROOT, until sign-in is in Rust).
pub fn portal_open(headers: &HeaderMap) -> bool {
    if stub_enabled() {
        return true;
    }
    production()
        && portal_key().is_some_and(|key| cookie(headers, PORTAL_KEY_COOKIE).is_some_and(|given| same(given, &key)))
}

/// Whether a key someone presented is the portal key.
pub fn is_portal_key(given: &str) -> bool {
    portal_key().is_some_and(|key| same(given.trim(), &key))
}

pub fn stub_user() -> String {
    std::env::var("CULEBRA_UI_AUTH_STUB_USER").unwrap_or_else(|_| DEFAULT_ROOT_USER.to_owned())
}

/// The portal chrome's projection of the signed-in user (`#rust-actor` in the shell): what it may be offered.
pub fn actor_projection_json(headers: &HeaderMap) -> Option<String> {
    portal_open(headers).then(|| {
        serde_json::json!({
            "accountType": "internal",
            "securityLevel": "ROOT",
            "authorityCodes": ["portal.read", "crm.write", "listing.write", "deal.read", "deal.write",
                               "settings.read", "settings.manage", "tech.access"],
            "entitlementCodes": [],
        })
        .to_string()
    })
}

/// A portal request's resolved user: under the stub, the ROOT user's break-glass identity resolved by the Security
/// service exactly as a signed-in request is (so grants, roles and the acting user are real); otherwise 401.
pub async fn resolve_portal_context(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<ResolvedRequestContext, ApiError> {
    if !portal_open(headers) {
        return Err(ApiError::unauthorized(
            "SIGN_IN_REQUIRED",
            "Portal sign-in is not available on this server yet.",
        ));
    }
    let (provider, subject) = stub_provider_identity();
    resolve_identity_context(state, &provider, &subject, correlation(headers), None).await
}

/// The provider identity the stub signs in as: the ROOT user's break-glass identity (`auth_identity`).
pub fn stub_provider_identity() -> (String, String) {
    ("break-glass".into(), format!("break-glass:{}", stub_user()))
}

fn correlation(headers: &HeaderMap) -> String {
    headers
        .get("x-culebra-correlation-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_is_refused_in_production() {
        // One test owns these variables so parallel tests cannot interleave them.
        std::env::set_var("CULEBRA_UI_AUTH_STUB", "root");
        std::env::remove_var("VERCEL_ENV");
        std::env::set_var("APP_ENV", "development");
        assert!(stub_enabled());
        assert!(actor_projection_json(&HeaderMap::new()).is_some_and(|json| json.contains("ROOT")));
        std::env::set_var("APP_ENV", "production");
        assert!(!stub_enabled(), "never in production");
        assert!(actor_projection_json(&HeaderMap::new()).is_none(), "production needs the key");
        std::env::set_var("CULEBRA_PORTAL_KEY", "k3y-that-is-long-enough");
        let with = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(axum::http::header::COOKIE, format!("a=b; {PORTAL_KEY_COOKIE}={value}").parse().unwrap());
            headers
        };
        assert!(portal_open(&with("k3y-that-is-long-enough")), "the key opens production");
        assert!(!portal_open(&with("k3y-that-is-long-enougH")), "a wrong key does not");
        assert!(!portal_open(&HeaderMap::new()), "no key, no portal");
        assert!(is_portal_key(" k3y-that-is-long-enough "));
        std::env::remove_var("CULEBRA_PORTAL_KEY");
        std::env::remove_var("CULEBRA_UI_AUTH_STUB");
        std::env::remove_var("APP_ENV");
    }
}

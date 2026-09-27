//! WHO THE BROWSER IS — for the Yew application calling this server directly.
//!
//! A portal request is signed in by the SESSION COOKIE that Google sign-in sets (`google_auth`): it carries the Google
//! identity (provider + subject), signed with AUTH_SECRET, and every request resolves it through the Security service —
//! the same `auth_identity` rows Auth.js used, so every existing user signs in unchanged. In development, when
//! `CULEBRA_UI_AUTH_STUB=root` (the dev launcher sets it), a request without a session acts as the ROOT user; the stub
//! is refused whenever the environment says production.

use axum::http::{header, HeaderMap};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use super::context::{resolve_identity_context, ResolvedRequestContext};
use super::{ApiError, ApiState};

/// The ROOT app user the stub acts as (the DEV database's "CulebraLuxe Root"); override with `CULEBRA_UI_AUTH_STUB_USER`.
const DEFAULT_ROOT_USER: &str = "1fc6dc61-d842-4d29-a20b-93c79e07c718";

/// The signed-in session's cookie.
pub const SESSION_COOKIE: &str = "culebra_session";

/// How long a sign-in lasts.
pub const SESSION_SECONDS: i64 = 7 * 24 * 60 * 60;

pub fn production() -> bool {
    ["APP_ENV", "VERCEL_ENV"]
        .iter()
        .any(|key| std::env::var(key).is_ok_and(|value| value.eq_ignore_ascii_case("production")))
}

/// The development stub: on when asked for, never in production.
pub fn stub_enabled() -> bool {
    std::env::var("CULEBRA_UI_AUTH_STUB").is_ok_and(|value| value == "root") && !production()
}

pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

fn signature(payload: &str) -> Option<String> {
    let secret = std::env::var("AUTH_SECRET").ok().filter(|secret| secret.len() >= 16)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(payload.as_bytes());
    Some(mac.finalize().into_bytes().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// A session cookie's value for a provider identity: `provider|subject|expires.signature`.
pub fn session_value(provider: &str, subject: &str, now: i64) -> Option<String> {
    let payload = format!("{provider}|{subject}|{}", now + SESSION_SECONDS);
    let signature = signature(&payload)?;
    Some(format!("{}.{signature}", base64_url(&payload)))
}

fn base64_url(text: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(text)
}

/// The provider identity a valid, unexpired session cookie carries.
pub fn session_identity(headers: &HeaderMap, now: i64) -> Option<(String, String)> {
    use base64::Engine;
    let (encoded, given) = cookie(headers, SESSION_COOKIE)?.split_once('.')?;
    let payload = String::from_utf8(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded).ok()?).ok()?;
    let expected = signature(&payload)?;
    let same = expected.len() == given.len()
        && expected.bytes().zip(given.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0;
    if !same {
        return None;
    }
    let mut parts = payload.splitn(3, '|');
    let (provider, subject, expires) = (parts.next()?, parts.next()?, parts.next()?.parse::<i64>().ok()?);
    (expires > now).then(|| (provider.to_owned(), subject.to_owned()))
}

pub fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// The identity this request signs in as: its session, else (development only) the ROOT stub.
fn request_identity(headers: &HeaderMap) -> Option<(String, String)> {
    session_identity(headers, now_seconds()).or_else(|| stub_enabled().then(stub_provider_identity))
}

/// Whether this request may use the portal.
pub fn portal_open(headers: &HeaderMap) -> bool {
    request_identity(headers).is_some()
}

pub fn stub_user() -> String {
    std::env::var("CULEBRA_UI_AUTH_STUB_USER").unwrap_or_else(|_| DEFAULT_ROOT_USER.to_owned())
}

/// The portal chrome's projection of the signed-in user (`#rust-actor` in the shell): what it may be offered.
pub fn actor_projection(resolved: &ResolvedRequestContext) -> String {
    let user = &resolved.acting_user;
    let level = resolved.service.principal.as_ref().map(|p| p.level.clone()).unwrap_or_else(|| "GUEST".into());
    serde_json::json!({
        "accountType": user.account_type,
        "securityLevel": level,
        "authorityCodes": user.authority_codes,
        "entitlementCodes": user.entitlement_codes,
    })
    .to_string()
}

/// A portal request's resolved user: under the stub, the ROOT user's break-glass identity resolved by the Security
/// service exactly as a signed-in request is (so grants, roles and the acting user are real); otherwise 401.
pub async fn resolve_portal_context(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<ResolvedRequestContext, ApiError> {
    let Some((provider, subject)) = request_identity(headers) else {
        return Err(ApiError::unauthorized("SIGN_IN_REQUIRED", "Sign in to use the portal."));
    };
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
    fn the_stub_is_refused_in_production_and_a_session_signs_in() {
        // One test owns these variables so parallel tests cannot interleave them.
        std::env::set_var("AUTH_SECRET", "test-secret-that-is-long-enough");
        std::env::set_var("CULEBRA_UI_AUTH_STUB", "root");
        std::env::remove_var("VERCEL_ENV");
        std::env::set_var("APP_ENV", "development");
        assert!(stub_enabled() && portal_open(&HeaderMap::new()), "development: the stub opens the portal");
        std::env::set_var("APP_ENV", "production");
        assert!(!portal_open(&HeaderMap::new()), "production: nothing without a session");

        let now = 1_000_000;
        let value = session_value("google", "sub-123", now).unwrap();
        let with = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::COOKIE, format!("x=y; {SESSION_COOKIE}={value}").parse().unwrap());
            headers
        };
        assert_eq!(session_identity(&with(&value), now), Some(("google".into(), "sub-123".into())));
        assert_eq!(session_identity(&with(&value), now + SESSION_SECONDS + 1), None, "an expired session");
        let forged = value.replace("Z29vZ2xl", "Z29vZ2xm");
        assert_eq!(session_identity(&with(&forged), now), None, "a changed payload fails the signature");
        std::env::remove_var("CULEBRA_UI_AUTH_STUB");
        std::env::remove_var("APP_ENV");
    }
}

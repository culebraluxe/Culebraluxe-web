//! GOOGLE SIGN-IN — the OAuth 2.0 authorization-code handshake, in Rust, with no JavaScript anywhere.
//!
//!   /api/auth/signin/google    -> Google's sign-in page (a one-time `state` in a short cookie guards the return)
//!   /api/auth/callback/google  -> the code is exchanged with Google for the user's Google account id; the Security
//!                                 service maps it (auth_identity: provider 'google') to an application user; a signed
//!                                 session cookie is set (ui_auth) and the browser goes back where it started
//!   /api/auth/signout          -> the session cookie is cleared
//!
//! The callback path is the one Auth.js used, so the redirect URI registered in Google Cloud does not change.
//! Credentials: AUTH_GOOGLE_ID, AUTH_GOOGLE_SECRET. The session is signed with AUTH_SECRET.

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use super::context::resolve_identity_context;
use super::ui_auth::{
    cookie, now_seconds, production, session_value, SESSION_COOKIE, SESSION_SECONDS,
};
use super::ApiState;

const STATE_COOKIE: &str = "culebra_oauth_state";
const NEXT_COOKIE: &str = "culebra_oauth_next";

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/auth/signin/google", get(sign_in))
        .route("/api/auth/callback/google", get(callback))
        .route("/api/auth/signout", get(sign_out).post(sign_out))
}

/// Percent-encoding for a query value.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// This site's own address, as the browser reached it (the redirect URI must match what Google has registered).
///
/// Public as the smallest honest seam for the SEC.REDIRECT host-injection contract tests
/// (`TST-SEC-REDIRECT-003`, `TST-SEC-REDIRECT-005`): the harness exercises this exact host
/// resolution, not a copy of it. It reads request headers and returns a string — no state, no I/O.
pub fn origin(headers: &HeaderMap) -> String {
    let text = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let host = text("x-forwarded-host")
        .or_else(|| text("host"))
        .unwrap_or_else(|| "localhost:3000".into());
    // Loopback, in every spelling the Host header can carry: a name, IPv4, and IPv6 — the last in the
    // RFC 3986 bracket form `[::1]`. Any other IPv6 literal is a real host and stays on HTTPS.
    let local =
        host.starts_with("localhost") || host.starts_with("127.0.0.1") || host.starts_with("[::1]");
    let proto = text("x-forwarded-proto").unwrap_or_else(|| {
        if local {
            "http".into()
        } else {
            "https".into()
        }
    });
    format!("{proto}://{host}")
}

/// The registered Google callback on this site's own origin.
///
/// Public for the same reason as [`origin`]: the SEC.REDIRECT contract tests assert the callback the
/// browser is sent to, and asserting a copy of this format string would prove nothing.
pub fn redirect_uri(headers: &HeaderMap) -> String {
    format!("{}/api/auth/callback/google", origin(headers))
}

/// Only a path on this site: never another site, never `//host`.
///
/// `\` is refused as well as `//`: every browser reads `/\evil.example` as `//evil.example` when it follows
/// the `Location` header, so a backslash after the leading `/` is an off-site redirect.
/// Absolute URLs (including same-origin URLs) deliberately fall back to the dashboard.
/// Reject control characters before Axum builds a `Location` header, which otherwise returns HTTP 500 on CR/LF.
///
/// Public as the smallest honest seam for the PROP.PROPERTY_BASED contract test
/// (`TST-PROP-PROPERTY-BASED-006`): the harness exercises this exact filter, not a copy of it.
pub fn safe_next(next: Option<&str>) -> String {
    next.filter(|path| {
        path.starts_with('/')
            && !path.starts_with("//")
            && !path.starts_with("/\\")
            && !path.chars().any(char::is_control)
    })
    .unwrap_or("/portal/dashboard")
    .to_owned()
}

fn set_cookie(response: &mut Response, name: &str, value: &str, max_age: i64) {
    let secure = if production() { "; Secure" } else { "" };
    let cookie =
        format!("{name}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}");
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

fn failed(reason: &str) -> Response {
    Redirect::to(&format!("/auth/error?error={}", encode(reason))).into_response()
}

fn credentials() -> Option<(String, String)> {
    let get = |key: &str| {
        std::env::var(key)
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    Some((get("AUTH_GOOGLE_ID")?, get("AUTH_GOOGLE_SECRET")?))
}

#[derive(Debug, Deserialize)]
struct SignInQuery {
    #[serde(rename = "callbackUrl")]
    callback_url: Option<String>,
}

async fn sign_in(headers: HeaderMap, Query(query): Query<SignInQuery>) -> Response {
    let Some((client_id, _)) = credentials() else {
        return failed("Configuration");
    };
    let state = uuid::Uuid::new_v4().simple().to_string();
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&prompt=select_account",
        encode(&client_id),
        encode(&redirect_uri(&headers)),
        encode("openid email profile"),
        state
    );
    let mut response = Redirect::to(&url).into_response();
    set_cookie(&mut response, STATE_COOKIE, &state, 600);
    set_cookie(
        &mut response,
        NEXT_COOKIE,
        &encode(&safe_next(query.callback_url.as_deref())),
        600,
    );
    response
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenAnswer {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    sub: String,
}

async fn callback(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if query.error.is_some() {
        return failed("AccessDenied");
    }
    let expected = cookie(&headers, STATE_COOKIE).unwrap_or("");
    let (Some(code), Some(given)) = (query.code.as_deref(), query.state.as_deref()) else {
        return failed("Verification");
    };
    if expected.is_empty() || expected != given {
        return failed("Verification");
    }
    let Some((client_id, client_secret)) = credentials() else {
        return failed("Configuration");
    };

    // The code, exchanged with Google for an access token; the token, for the account id (`sub`). Both over TLS,
    // straight from Google, so the answer is Google's.
    let client = reqwest::Client::new();
    let redirect = redirect_uri(&headers);
    let form = [
        ("code", code),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("grant_type", "authorization_code"),
    ];
    let token = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&form)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response.json::<TokenAnswer>().await.ok(),
        _ => None,
    };
    let Some(token) = token else {
        return failed("OAuthCallback");
    };
    let user = match client
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(&token.access_token)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response.json::<UserInfo>().await.ok(),
        _ => None,
    };
    let Some(user) = user else {
        return failed("OAuthCallback");
    };

    // Who that Google account is, here: the same mapping Auth.js used. An unknown or inactive account is refused.
    let correlation = uuid::Uuid::new_v4().to_string();
    if resolve_identity_context(&state, "google", &user.sub, correlation, None)
        .await
        .is_err()
    {
        return failed("AccessDenied");
    }
    let Some(session) = session_value("google", &user.sub, now_seconds()) else {
        return failed("Configuration");
    };
    let next = cookie(&headers, NEXT_COOKIE)
        .map(|value| percent_decode(value))
        .map(|value| safe_next(Some(&value)))
        .unwrap_or_else(|| safe_next(None));
    let mut response = Redirect::to(&next).into_response();
    set_cookie(&mut response, SESSION_COOKIE, &session, SESSION_SECONDS);
    set_cookie(&mut response, STATE_COOKIE, "", 0);
    set_cookie(&mut response, NEXT_COOKIE, "", 0);
    response
}

pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn sign_out() -> Response {
    let mut response = Redirect::to("/").into_response();
    set_cookie(&mut response, SESSION_COOKIE, "", 0);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                axum::http::HeaderName::from_static(name),
                HeaderValue::from_static(value),
            );
        }
        map
    }

    fn one_set_cookie(response: &Response) -> String {
        response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .expect("a Set-Cookie header")
            .to_owned()
    }

    #[test]
    fn only_a_path_on_this_site_is_a_return_address() {
        assert_eq!(
            safe_next(Some("/portal/clients?x=1")),
            "/portal/clients?x=1"
        );
        assert_eq!(safe_next(Some("//evil.example")), "/portal/dashboard");
        assert_eq!(safe_next(Some("https://evil.example")), "/portal/dashboard");
        // A backslash is read as a slash by every browser, so `/\host` is `//host`.
        assert_eq!(safe_next(Some("/\\evil.example")), "/portal/dashboard");
        assert_eq!(safe_next(Some("/\\")), "/portal/dashboard");
        assert_eq!(safe_next(None), "/portal/dashboard");
    }

    #[test]
    fn the_return_address_is_decoded_before_it_is_filtered() {
        // The callback percent_decodes the next cookie before safe_next sees it
        // (`google_auth.rs:215-218`). The order is load-bearing: the raw encoded
        // form slips past the filter, and only the decoded value is refused.
        assert_eq!(safe_next(Some("/%5Cevil.example")), "/%5Cevil.example");
        assert_eq!(
            safe_next(Some(&percent_decode("/%5Cevil.example"))),
            "/portal/dashboard"
        );
    }

    #[test]
    fn a_return_address_survives_the_cookie() {
        for path in [
            "/portal/clients?selected=a b&x=1",
            "/portal/clients?a=1&b=2",
            "/portal/100%",
            "/portal/Casa-Luar-áé",
            "/portal/a%",
        ] {
            assert_eq!(percent_decode(&encode(path)), path, "path: {path}");
        }
    }

    #[test]
    fn percent_decode_leaves_an_invalid_escape_alone() {
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("/a%"), "/a%");
    }

    #[test]
    fn percent_decode_keeps_a_malformed_byte_lossy() {
        // Intended: `String::from_utf8_lossy` replaces the invalid byte; this is not a bug to fix here.
        assert_eq!(percent_decode("%FF"), "\u{FFFD}");
    }

    #[test]
    fn origin_prefers_the_forwarded_host_then_the_host() {
        assert_eq!(
            origin(&headers(&[
                ("x-forwarded-host", "portal.example"),
                ("host", "internal:8080"),
            ])),
            "https://portal.example"
        );
        assert_eq!(
            origin(&headers(&[("host", "portal.example")])),
            "https://portal.example"
        );
    }

    #[test]
    fn origin_defaults_to_http_only_for_a_local_host() {
        assert_eq!(
            origin(&headers(&[("host", "localhost:3000")])),
            "http://localhost:3000"
        );
        assert_eq!(
            origin(&headers(&[("host", "127.0.0.1:3000")])),
            "http://127.0.0.1:3000"
        );
    }

    #[test]
    fn origin_lets_the_forwarded_proto_win() {
        assert_eq!(
            origin(&headers(&[
                ("host", "localhost:3000"),
                ("x-forwarded-proto", "https"),
            ])),
            "https://localhost:3000"
        );
        assert_eq!(
            origin(&headers(&[
                ("host", "portal.example"),
                ("x-forwarded-proto", "http"),
            ])),
            "http://portal.example"
        );
    }

    #[test]
    fn redirect_uri_is_the_registered_callback_on_the_origin() {
        assert_eq!(
            redirect_uri(&headers(&[("host", "portal.example")])),
            "https://portal.example/api/auth/callback/google"
        );
        assert_eq!(
            redirect_uri(&headers(&[("host", "localhost:3000")])),
            "http://localhost:3000/api/auth/callback/google"
        );
    }

    #[test]
    fn set_cookie_pins_its_flags_by_relationship() {
        let mut response = Response::new(axum::body::Body::empty());
        set_cookie(&mut response, "culebra_test", "abc", 600);
        let value = one_set_cookie(&response);
        assert!(value.starts_with("culebra_test=abc;"), "value: {value}");
        assert!(value.contains("Path=/"), "value: {value}");
        assert!(value.contains("Max-Age=600"), "value: {value}");
        assert!(value.contains("HttpOnly"), "value: {value}");
        assert!(value.contains("SameSite=Lax"), "value: {value}");
        assert_eq!(value.contains("; Secure"), production(), "value: {value}");
    }
}

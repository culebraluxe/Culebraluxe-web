//! API.ROUTE_CONTRACT — a protected route refuses no credentials, a forged one, and a malformed one. (012)
//!
//! CONTRACT. Every route the API claims behind a gate answers **401 with the one JSON error envelope** — never 200,
//! never an HTML page — for all four ways a caller arrives at it:
//!
//!   1. NO CREDENTIALS. `GET /v1/whoami` with no headers at all: `validate_internal_key`
//!      (`web/src/api/context.rs:243-254`) refuses with `INTERNAL_AUTH_REQUIRED` on the request's first line of
//!      identity work, before any database is touched.
//!   2. A FORGED KEY. The same route with a key that is one byte off the configured one, with a prefix of it, and
//!      with an empty value: all three refused `INTERNAL_AUTH_REQUIRED`. The comparison is constant-time and
//!      length-exact (`web/src/api/context.rs:264-271`), so "close enough", "long enough" and "" are not the key.
//!   3. A FORGED KEY ON A WRITE-SHAPED ROUTE. `POST /v1/services/dispatch`
//!      (`web/src/api/routes/security_service.rs:479-492`) carrying a well-formed `ServiceEnvelope` body: axum's
//!      `Json` extractor runs BEFORE the handler, so a valid body proves the request was admitted by the framework
//!      and still refused by the gate — not turned away with a 400 for a bad body. A 200 here would mean a
//!      credential check that a POST walks past.
//!   4. A MALFORMED IDENTITY, WITH THE RIGHT KEY. (a) `x-culebra-auth-provider` present and no subject →
//!      `AUTH_IDENTITY_REQUIRED`: half a person is not a principal. (b) a 513-byte provider with a subject →
//!      `AUTH_IDENTITY_INVALID` (`web/src/api/context.rs:288-294`): an identity header past 512 bytes is refused
//!      rather than resolved.
//!
//! A FIFTH CASE, ON THE OTHER GATE. The portal resolves a signed session cookie
//! (`web/src/api/ui_auth.rs:76-102`, HMAC-SHA256 over `AUTH_SECRET`) instead of the internal key. `GET
//! /api/portal/rust-ui/page` (`web/src/api/portal_bridge.rs:81`) is asked twice — with no cookie, and with a
//! structurally perfect cookie for a far-future expiry whose signature the server cannot produce (the payload
//! decodes to `google|attacker@example.com|9999999999`; the digest is 64 hex characters of `deadbeef`). A forged
//! cookie must buy NOTHING: the two answers are asserted byte-identical, which holds whether or not the development
//! stub is open. When there is no session to be had (`web::api::ui_auth::stub_enabled()` is false — the stub at
//! `web/src/api/ui_auth.rs:33-35` deliberately opens the portal in development and is refused whenever
//! `ui_auth::production()` says production, `web/src/api/ui_auth.rs:26-30`), the refusal itself is asserted as
//! `SIGN_IN_REQUIRED`.
//!
//! THE CONTROL THAT KEEPS CASES 1-4 HONEST: with the RIGHT key and no identity headers, the answer must NOT be
//! `INTERNAL_AUTH_REQUIRED` — the key gate passed, and the NEXT gate (`AUTH_IDENTITY_REQUIRED`) answered. Without
//! it, a route that refused every request with one blanket code would satisfy the cases above and prove nothing.
//!
//! AND THE ONE THING NO REFUSAL MAY DO: disclose the credential it checks. Every body below is asserted not to
//! contain the configured key material — an error message that quoted the expected key would hand it over.
//!
//! THE HONEST STATE OF THE TREE, recorded rather hidden: the router cannot be composed without a live pool
//! (`ApiState`, `web/src/api/mod.rs:64-83`, takes a `Database` that performs one bounded checkout on connect,
//! `db/src/pool.rs:301-305`), so this test is `#[ignore]`d exactly like the suite's other `needs DATABASE_URL_DEV`
//! tests. Everything below is refused in the transport before any query runs: no row is read, no row is written, no
//! external provider is called. The database is the harness's disposable DEV pool, which refuses `DbTarget::Prod`
//! before any socket is opened (`tests/src/database.rs:68-89`).
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: a REFUSAL THAT NEEDS THE POLICY — an
//! identity that resolves to an unmapped or inactive principal (403 `AUTH_IDENTITY_UNMAPPED` /
//! `AUTH_IDENTITY_INACTIVE`, `web/src/api/context.rs:72-88`) requires a Security-service query and its audit row
//! (`web/src/security/mod.rs:122-175`), which this test deliberately does not perform; entitlement-level denial
//! inside a resolved session; and constant-time behaviour itself, which is a property of the compare
//! (`web/src/api/context.rs:256-271`), not of an HTTP response.
//!
//! Level: L3 Composition — the real composition root (`web::api::build_router`) driven in-process through the
//! harness's HTTP seam (`test_harness::http`), same service the server serves, no port and no listener.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__012__authorization_cannot_be_bypassed -- --ignored --nocapture

use std::sync::Arc;

use axum::http::StatusCode;
use test_harness::http::{self, TestRequest, TestResponse};
use test_harness::TestDatabase;

/// The fixture key this test composes the router with — deliberately not secret-shaped, and never a value any
/// production route expects. It is the credential the cases below forge against.
const INTERNAL_KEY: &str = "api-route-contract-012-internal-key";

/// The same length, one byte different from the key above: the forgery a caller who knows the shape but not the
/// value would try first.
const FORGED_KEY: &str = "api-route-contract-012-internal-kex";

/// A structurally perfect session cookie the server must reject: the payload is base64url of
/// `google|attacker@example.com|9999999999`, and the digest is 64 hex characters — the length a real HMAC-SHA256
/// produces, so the comparison at `web/src/api/ui_auth.rs:85-97` walks the bytes rather than short-circuiting on a
/// length difference.
const FORGED_SESSION_COOKIE: &str = "Z29vZ2xlfGF0dGFja2VyQGV4YW1wbGUuY29tfDk5OTk5OTk5OTk.deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

/// The production composition root over the harness's disposable DEV pool — `web/src/http_runtime.rs:11-16`
/// without the TCP listener. The harness is what makes the target safe: it refuses PROD before any socket.
async fn production_router() -> (axum::Router, TestDatabase) {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");
    let db = test_db.database().clone();
    let infrastructure = web::service_bootstrap::production_service_infrastructure(&db)
        .await
        .expect("the production service infrastructure composes");
    let config = web::api::ApiConfig {
        internal_api_key: Arc::from(INTERNAL_KEY),
    };
    (web::api::build_router(db, infrastructure, config), test_db)
}

/// Whether the answer looks like an HTML document: an `text/html` content type, or HTML in the body.
fn looks_like_html(response: &TestResponse) -> bool {
    let content_type = response.header("content-type").unwrap_or("");
    if content_type.contains("text/html") {
        return true;
    }
    let body = response.text().to_ascii_lowercase();
    body.contains("<html") || body.contains("<!doctype html")
}

/// The answer IS the refusal the contract names: 401, `application/json`, not HTML, the envelope with `ok: false`
/// and the expected error code — and it does not disclose the credential it checked. Returns the envelope so the
/// caller can assert on its fields.
fn assert_refused(response: &TestResponse, what: &str, expected_code: &str) -> serde_json::Value {
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "{what}: a caller without valid credentials is refused, never served; got {} body {}",
        response.status(),
        response.text()
    );
    let content_type = response
        .header("content-type")
        .unwrap_or_else(|| panic!("{what}: the refusal carries no Content-Type"));
    assert!(
        content_type.starts_with("application/json"),
        "{what}: a refusal is a JSON envelope, not a page; got {content_type}; body {}",
        response.text()
    );
    assert!(
        !looks_like_html(response),
        "{what}: an authorization refusal came back as HTML: {}",
        response.text()
    );
    let envelope = response.json_value();
    assert_eq!(envelope["ok"], false, "{what}: {envelope}");
    assert_eq!(
        envelope["error"]["code"], expected_code,
        "{what}: {envelope}"
    );
    assert!(
        !response.text().contains(INTERNAL_KEY),
        "{what}: the refusal discloses the credential it checks against: {envelope}"
    );
    envelope
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV — the production router is composed over a pool that must connect first"]
async fn api_route_contract_012__authorization_cannot_be_bypassed() {
    let (router, _test_db) = production_router().await;

    // 1. NO CREDENTIALS. Nothing at all: refused before any query.
    let anonymous = http::get(&router, "/v1/whoami").await;
    assert_refused(
        &anonymous,
        "GET /v1/whoami with no credentials",
        "INTERNAL_AUTH_REQUIRED",
    );

    // 2. A FORGED KEY — three shapes of forgery, one answer each. A route that let any of these through would be
    //    authorizing on the shape of a header rather than on its value.
    for (label, forged) in [
        ("one byte off the real key", FORGED_KEY),
        ("a prefix of the real key", "api-route-contract-012"),
        ("an empty key", ""),
    ] {
        let response = http::call(
            &router,
            TestRequest::get("/v1/whoami").header("x-culebra-internal-key", forged),
        )
        .await;
        assert_refused(
            &response,
            &format!("GET /v1/whoami with {label}"),
            "INTERNAL_AUTH_REQUIRED",
        );
    }

    // 3. A FORGED KEY ON A WRITE-SHAPED ROUTE, WITH A VALID BODY. The body deserializes (axum's `Json` extractor
    //    runs before the handler), so the request reached the application and was still refused by the gate — a
    //    400 here would mean the test never reached the credential check at all.
    let envelope_body = serde_json::json!({
        "domain": "security",
        "operation": "security.identity.resolve",
        "payload": {},
    });
    let forged_post = http::call(
        &router,
        TestRequest::post("/v1/services/dispatch")
            .header("x-culebra-internal-key", FORGED_KEY)
            .json(&envelope_body),
    )
    .await;
    assert_refused(
        &forged_post,
        "POST /v1/services/dispatch with a forged key and a valid body",
        "INTERNAL_AUTH_REQUIRED",
    );

    // 4. A MALFORMED IDENTITY, WITH THE RIGHT KEY. The gate that passed is now the identity gate, and half a
    //    identity — or one too long to be one — is refused rather than resolved.
    let half = http::call(
        &router,
        TestRequest::get("/v1/whoami")
            .header("x-culebra-internal-key", INTERNAL_KEY)
            .header("x-culebra-auth-provider", "google"),
    )
    .await;
    assert_refused(
        &half,
        "GET /v1/whoami with a provider and no subject",
        "AUTH_IDENTITY_REQUIRED",
    );

    let oversized = http::call(
        &router,
        TestRequest::get("/v1/whoami")
            .header("x-culebra-internal-key", INTERNAL_KEY)
            .header("x-culebra-auth-provider", &"a".repeat(513))
            .header("x-culebra-auth-sub", "subject-under-test"),
    )
    .await;
    assert_refused(
        &oversized,
        "GET /v1/whoami with a 513-byte provider subject",
        "AUTH_IDENTITY_INVALID",
    );

    // THE CONTROL: the RIGHT key must NOT answer `INTERNAL_AUTH_REQUIRED`. It passes the key gate and is refused by
    // the NEXT one, which is what proves cases 1-3 are credential refusals rather than one blanket code.
    let keyed = http::call(
        &router,
        TestRequest::get("/v1/whoami").header("x-culebra-internal-key", INTERNAL_KEY),
    )
    .await;
    let control = assert_refused(
        &keyed,
        "GET /v1/whoami with the real key",
        "AUTH_IDENTITY_REQUIRED",
    );
    assert_ne!(
        control["error"]["code"], "INTERNAL_AUTH_REQUIRED",
        "the real key must get past the key gate: {control}"
    );

    // 5. THE OTHER GATE — A FORGED SESSION COOKIE BUYS NOTHING. The answer with a hand-minted cookie whose
    //    signature the server cannot produce is byte-identical to the answer with no cookie: the signature
    //    (`web/src/api/ui_auth.rs:76-102`) is what admits a session, and it admits this one never. Both requests
    //    carry the same correlation id so the comparison is about the cookie and nothing else.
    let probe = |cookie: Option<&str>| {
        let request = TestRequest::get("/api/portal/rust-ui/page").header(
            "x-culebra-correlation-id",
            "api-route-contract-012-portal-probe",
        );
        match cookie {
            Some(value) => request.cookie(web::api::ui_auth::SESSION_COOKIE, value),
            None => request,
        }
    };

    let no_session = http::call(&router, probe(None)).await;
    let forged_session = http::call(&router, probe(Some(FORGED_SESSION_COOKIE))).await;

    assert_eq!(
        no_session.status(),
        forged_session.status(),
        "a forged session cookie must not change the answer: no cookie {} {}, forged cookie {} {}",
        no_session.status(),
        no_session.text(),
        forged_session.status(),
        forged_session.text()
    );
    assert_eq!(
        no_session.text(),
        forged_session.text(),
        "a forged session cookie must not change the answer by a byte"
    );
    assert!(
        !looks_like_html(&no_session) && !looks_like_html(&forged_session),
        "a portal refusal is never an HTML page: {} / {}",
        no_session.text(),
        forged_session.text()
    );

    // With no session to be had — the development stub off — the refusal itself is asserted outright. The stub
    // (`web/src/api/ui_auth.rs:33-35`) deliberately opens the portal in development and is refused in production, so
    // it is environment, not route behaviour; the equality above holds either way.
    if !web::api::ui_auth::stub_enabled() {
        assert_refused(
            &no_session,
            "GET /api/portal/rust-ui/page with no session cookie",
            "SIGN_IN_REQUIRED",
        );
        assert_refused(
            &forged_session,
            "GET /api/portal/rust-ui/page with a forged session cookie",
            "SIGN_IN_REQUIRED",
        );
    }
}

//! API.ROUTE_CONTRACT — the answer carries the request's correlation id, and a caller's claim is never adopted
//! unchecked. (011)
//!
//! WHAT PRODUCTION ACTUALLY EMITS, pinned rather than guessed. There is NO correlation RESPONSE header anywhere in
//! the workspace: no `x-request-id`, no `x-correlation-id`, no `trace-id` is set on a response, and every
//! `x-culebra-*` name is a REQUEST header (`web/src/api/context.rs:8-12`). The id travels INSIDE the JSON envelope:
//!
//!   * the error envelope `{ ok, error, correlationId }` — `web/src/api/error.rs:335-344`, and
//!   * the success envelope `{ ok, value, correlationId }` — `web/src/api/routes.rs:60-66`.
//!
//! Its value comes from the request header `x-culebra-correlation-id`, and when that is missing or blank the server
//! mints its own: `web/src/api/context.rs:26-29` reads it, filters blanks, and falls back to `Uuid::new_v4`. Five
//! assertions make that executable:
//!
//!   1. SERVER-GENERATED WHEN THE CALLER SUPPLIES NOTHING. `GET /v1/whoami` with the internal key and no identity
//!      headers is refused 401 `AUTH_IDENTITY_REQUIRED` (`required_identity_header`,
//!      `web/src/api/context.rs:273-297`), and that refusal is correlated: `correlationId` is a non-empty string
//!      that parses as a UUID — a real id, not a placeholder.
//!   2. FRESH PER REQUEST. The same request twice yields two different ids, so the id belongs to the request rather
//!      than being a constant of the process.
//!   3. THE TRUSTED EDGE'S ID IS CARRIED THROUGH, EXACTLY. With the internal key and a well-formed client id, the
//!      envelope echoes it byte for byte — that is how one trace joins up across services, and
//!      `cli/src/main.rs:297-300` is the caller that sets it on every request it makes.
//!   4. NEGATIVE A — A BLANK CLAIM IS REPLACED, NOT CARRIED. The same request with `x-culebra-correlation-id: "   "`
//!      must not put the blank on the wire: the envelope carries a server-minted UUID.
//!   5. NEGATIVE B — A CLAIM FROM A CALLER WHO FAILED THE GATE IS NEVER ADOPTED. With NO internal key and a forged
//!      `x-culebra-correlation-id`, `validate_internal_key` (`web/src/api/context.rs:243-254`) refuses at
//!      `resolve_request_context`'s first line (`web/src/api/context.rs:24`), before the correlation header is read
//!      at lines 26-29. The envelope must therefore never carry the caller's value; today it carries
//!      `correlationId: null` because no correlation was ever established, and both facts are asserted. A
//!      correlation id records what the SERVER traced — it is not something a caller writes by asking, and a route
//!      that adopted the header before the key check would be a trace a spoofed request could plant in the logs.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden: the router cannot be composed without a live pool
//! (`ApiState`, `web/src/api/mod.rs:64-83`, takes a `Database` that performs one bounded checkout on connect,
//! `db/src/pool.rs:301-305`), so this test is `#[ignore]`d exactly like the suite's other `needs DATABASE_URL_DEV`
//! tests. Every request below fails in the transport before any query runs, so no row is read, no row is written,
//! and no external provider is called; the database is the harness's disposable DEV pool, which refuses
//! `DbTarget::Prod` before any socket is opened (`tests/src/database.rs:68-89`).
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: the SUCCESS envelope's `correlationId`
//! (`web/src/api/routes.rs:457-463`) is the same field, but reaching a 200 requires an identity resolution — a
//! Security-service query and the audit row that follows it (`web/src/security/mod.rs:122-175`) — which this test
//! deliberately does not perform; the `x-culebra-causation-id` header, which is read into the context
//! (`web/src/api/context.rs:34-36`) but appears in neither envelope; and whether the id is ever written to a log or
//! a row, which is the observability contract rather than the route's.
//!
//! Level: L3 Composition — the real composition root (`web::api::build_router`) driven in-process through the
//! harness's HTTP seam (`test_harness::http`), same service the server serves, no port and no listener.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__011__request_correlation_id -- --ignored --nocapture

use std::sync::Arc;

use axum::http::StatusCode;
use test_harness::http::{self, TestRequest, TestResponse};
use test_harness::TestDatabase;

/// The fixture key this test composes the router with — deliberately not secret-shaped, and never a value any
/// production route expects.
const INTERNAL_KEY: &str = "api-route-contract-011-internal-key";

/// The header every assertion below is written against: the one production reads.
const CORRELATION_HEADER: &str = "x-culebra-correlation-id";

/// A well-formed id as the trusted edge would send it.
const EDGE_TRACE: &str = "api-route-contract-011-edge-trace-0001";

/// A claim from a caller that has not proved anything.
const FORGED_TRACE: &str = "api-route-contract-011-forged-trace-0001";

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

/// The error envelope of a 401 refusal, asserting the answer really IS the JSON error envelope before any field of
/// it is read — so a field assertion can never pass against some other body. `what` and `expected_code` name the
/// case so a failure is readable.
fn refusal_envelope(response: &TestResponse, what: &str, expected_code: &str) -> serde_json::Value {
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "{what}: a refusal is 401; got {} body {}",
        response.status(),
        response.text()
    );
    let content_type = response
        .header("content-type")
        .unwrap_or_else(|| panic!("{what}: the refusal carries no Content-Type"));
    assert!(
        content_type.starts_with("application/json"),
        "{what}: a refusal is a JSON envelope, got {content_type}; body {}",
        response.text()
    );
    let envelope = response.json_value();
    assert_eq!(envelope["ok"], false, "{what}: {envelope}");
    assert_eq!(
        envelope["error"]["code"], expected_code,
        "{what}: {envelope}"
    );
    envelope
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV — the production router is composed over a pool that must connect first"]
async fn api_route_contract_011__request_correlation_id() {
    let (router, _test_db) = production_router().await;

    let request = |correlation: Option<&str>| {
        let request = TestRequest::get("/v1/whoami").header("x-culebra-internal-key", INTERNAL_KEY);
        match correlation {
            Some(value) => request.header(CORRELATION_HEADER, value),
            None => request,
        }
    };

    // 1. SERVER-GENERATED WHEN THE CALLER SUPPLIES NOTHING: a real, non-empty UUID on the refusal.
    let first = http::call(&router, request(None)).await;
    let envelope = refusal_envelope(&first, "no correlation header", "AUTH_IDENTITY_REQUIRED");
    let minted = envelope["correlationId"].as_str().unwrap_or_else(|| {
        panic!("the refusal must carry a correlation id as a string: {envelope}")
    });
    assert!(
        !minted.trim().is_empty(),
        "the correlation id is not empty or blank: {envelope}"
    );
    uuid::Uuid::parse_str(minted).unwrap_or_else(|error| {
        panic!("with no client id the server mints a UUID (web/src/api/context.rs:26-29): {error}; {envelope}")
    });

    // 2. FRESH PER REQUEST: the id belongs to the request, not to the process.
    let second = http::call(&router, request(None)).await;
    let second_envelope = refusal_envelope(
        &second,
        "no correlation header, second request",
        "AUTH_IDENTITY_REQUIRED",
    );
    let second_id = second_envelope["correlationId"]
        .as_str()
        .unwrap_or_else(|| panic!("the second refusal is correlated too: {second_envelope}"));
    assert_ne!(
        second_id, minted,
        "two requests must not share one correlation id — a constant would make every trace the same trace: {second_envelope}"
    );

    // 3. THE TRUSTED EDGE'S ID IS CARRIED THROUGH, EXACTLY. The gate that admits it is the internal key, asserted
    //    in case 5 below: the echo is what joins a trace, and it is the gate that makes the echo safe.
    let traced = http::call(&router, request(Some(EDGE_TRACE))).await;
    let traced_envelope =
        refusal_envelope(&traced, "a well-formed client id", "AUTH_IDENTITY_REQUIRED");
    assert_eq!(
        traced_envelope["correlationId"], EDGE_TRACE,
        "past the internal-key gate the edge's id is carried through unchanged: {traced_envelope}"
    );

    // 4. NEGATIVE A — A BLANK CLAIM IS REPLACED, NOT CARRIED. Whitespace is not an id, so the transport must mint
    //    one rather than put the caller's blank on the wire.
    let blank = http::call(&router, request(Some("   "))).await;
    let blank_envelope = refusal_envelope(&blank, "a blank client id", "AUTH_IDENTITY_REQUIRED");
    let replaced = blank_envelope["correlationId"].as_str().unwrap_or_else(|| {
        panic!("a blank claim is replaced by a real id, never by nothing: {blank_envelope}")
    });
    assert_ne!(
        replaced.trim(),
        "",
        "the blank was carried through unchecked: {blank_envelope}"
    );
    uuid::Uuid::parse_str(replaced).unwrap_or_else(|error| {
        panic!("a blank claim is replaced by a server-minted UUID, not echoed: {error}; {blank_envelope}")
    });

    // 5. NEGATIVE B — A CLAIM FROM A CALLER WHO FAILED THE GATE IS NEVER ADOPTED. No internal key: the very first
    //    line of `resolve_request_context` refuses (`web/src/api/context.rs:24`), before the correlation header is
    //    read at lines 26-29, so the reply cannot carry the caller's value.
    let forged = http::call(
        &router,
        TestRequest::get("/v1/whoami").header(CORRELATION_HEADER, FORGED_TRACE),
    )
    .await;
    let forged_envelope = refusal_envelope(
        &forged,
        "an unkeyed caller's forged id",
        "INTERNAL_AUTH_REQUIRED",
    );
    assert_ne!(
        forged_envelope["correlationId"].as_str(),
        Some(FORGED_TRACE),
        "a caller who never passed the internal-key gate had its correlation id adopted: {forged_envelope}"
    );
    assert!(
        forged_envelope["correlationId"].is_null(),
        "no correlation was established for this request, so the envelope carries none rather than a claim: {forged_envelope}"
    );
}

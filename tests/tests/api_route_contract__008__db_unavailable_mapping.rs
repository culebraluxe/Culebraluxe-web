//! API.ROUTE_CONTRACT — a database that is not there is a 503 carrying the JSON error envelope. (008)
//!
//! CONTRACT. `GET /readyz` is the one route whose whole job is to ask the database whether it is there —
//! `web/src/api/routes/security_service.rs:13-19` is `state.db().ping().await.map_err(ApiError::from_db)` — so it
//! is where "the database is unavailable" becomes an HTTP answer. The production composition root must
//! demonstrate, in this order:
//!
//!   1. WITH A LIVE POOL the route answers **200** with `{ ok: true, databaseTarget }`. The baseline is part of
//!      the contract: a route that always fails must not be able to pass this test.
//!   2. WITH THE POOL DISPOSED the same route answers **503 Service Unavailable** — `ApiError::from_db`
//!      (`web/src/api/error.rs:79-104`) maps `DbFailureKind::DatabaseUnavailable` and `Timeout` to
//!      `SERVICE_UNAVAILABLE` — and the body is the one error envelope
//!      `{ ok: false, error: { code, message, retryable, incidentId }, correlationId }` with
//!      `code = "DATABASE_UNAVAILABLE"`, `retryable = true` (a lost session is a reason to retry, not a verdict)
//!      and a real `incidentId` stamped by the `DbFailure` constructor (`db/src/error.rs:83-151`).
//!   3. NEVER a raw panic, a bare 500, or an HTML document. The site shell is the router's fallback and would
//!      happily serve an HTML page to anything that reached it (`web/src/site.rs:31-35`); the API error path must
//!      not.
//!   4. THE FAULT IS LOCAL. With the pool disposed, answers that need no database are untouched:
//!      `/api/build-info` still answers 200 JSON, and an unclaimed API address is still a 404 — so the 503 above
//!      is the database mapping, not a router that broke.
//!
//! HOW THE FAULT IS MADE, AND WHY NOT ANOTHER WAY. `test_harness::pool::DbPoolFaultHarness` hands out the driver's
//! own errors with no socket at all, but there is no seam in the composed application to hand one to: `ApiState`
//! (`web/src/api/mod.rs:64-83`) takes a `Database`, and `Database::connect_target` performs one bounded checkout
//! before it returns (`db/src/pool.rs:301-305`) — so an `ApiState` exists only over a pool that just connected.
//! The honest fault at that seam is therefore to DISPOSE that pool after composition: `Pool::close` makes every
//! later checkout fail with sqlx's own `Error::PoolClosed`, which is deterministic (no server failure, no timing,
//! no retry window), and production — not this test — classifies it: `DbFailure::from_sqlx` reads the driver's
//! message as a connection-class failure (`db/src/error.rs:118-131`), which is `DatabaseUnavailable` + retryable.
//! The test only states what the mapping must be.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * The router cannot be composed without a live pool (the checkout above), which is why this test is
//!     `#[ignore]`d exactly like the suite's other `needs DATABASE_URL_DEV` tests, and why the pool is disposed
//!     rather than faked.
//!   * The database is the harness's disposable DEV pool: `TestDatabase::connect_from_env` resolves the declared
//!     environment and refuses `DbTarget::Prod` **before any socket is opened** (`tests/src/database.rs:68-89`).
//!     No row is written; readiness is `select 1`, and `Pool::close` only closes client connections.
//!   * The failure sink is NOT installed in this process, and it does not need to be: `DbFailure::notify` without
//!     a sink does nothing (`db/src/capture.rs:42-45`), and the 503 already carries an `incidentId`, so
//!     `ApiError::into_response` deliberately does not re-capture it (`web/src/api/error.rs:320-334`).
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: the other classifications (`Constraint`
//! -> 409, `SchemaMismatch` -> 500, `Timeout` -> 503 with code `TIMEOUT`); the `app_error` row this failure would
//! produce in production; `/healthz`, which never asks the database by design; and the service harness's
//! background work — `build_router` composes it but this test never starts it (no MQ subscribers run).
//!
//! Level: L3 Composition — the real composition root (`web::api::build_router`, the same `Database`, the same
//! `production_service_infrastructure`, the same router as `web/src/http_runtime.rs:11-16`) driven in-process
//! through the harness's HTTP seam. No live external provider, no process spawned, no write.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__008__db_unavailable_mapping -- --ignored --nocapture

use std::sync::Arc;

use axum::http::StatusCode;
use test_harness::http::{self, TestResponse};
use test_harness::TestDatabase;

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
        internal_api_key: Arc::from("api-route-contract-008-internal-key"),
    };
    (web::api::build_router(db, infrastructure, config), test_db)
}

/// The response's `Content-Type`, asserting it is JSON. `what` names the case so a failure is readable.
fn assert_json_content_type(response: &TestResponse, what: &str) {
    let content_type = response.header("content-type").unwrap_or_else(|| {
        panic!(
            "{what}: the answer carries no Content-Type; status {}",
            response.status()
        )
    });
    assert!(
        content_type.starts_with("application/json"),
        "{what}: expected application/json, got {content_type}; status {} body {}",
        response.status(),
        response.text()
    );
}

/// The answer is not an HTML document: no HTML content type, no HTML in the body.
fn assert_not_html(response: &TestResponse, what: &str) {
    let content_type = response.header("content-type").unwrap_or("");
    assert!(
        !content_type.contains("text/html"),
        "{what}: an API answer came back as HTML ({content_type}); status {} body {}",
        response.status(),
        response.text()
    );
    let body = response.text().to_ascii_lowercase();
    assert!(
        !body.contains("<html") && !body.contains("<!doctype html"),
        "{what}: the body is an HTML document: {}",
        response.text()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV — the production router is composed over a pool that must connect first"]
async fn api_route_contract_008__db_unavailable_mapping() {
    let (router, test_db) = production_router().await;

    // 1. THE BASELINE. With a pool that works, readiness says so — so the 503 below can only be the fault's.
    let before = http::get(&router, "/readyz").await;
    assert_eq!(
        before.status(),
        StatusCode::OK,
        "readiness with a live pool must be 200; got {} {}",
        before.status(),
        before.text()
    );
    assert_json_content_type(&before, "GET /readyz with a live pool");
    let baseline = before.json_value();
    assert_eq!(baseline["ok"], true, "{baseline}");

    // 2. THE FAULT. Dispose the pool: every checkout from here is the driver's own `PoolClosed` — no server
    //    failure, no timing, no sleep. The classification is production's (`DbFailure::from_sqlx`); what is
    //    asserted below is only the MAPPING it must reach over HTTP.
    test_db.database().pool().close().await;

    let after = http::get(&router, "/readyz").await;
    assert_eq!(
        after.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "a disposed pool must map to 503 Service Unavailable (a retry-later answer), not {} — body {}",
        after.status(),
        after.text()
    );
    assert_json_content_type(&after, "GET /readyz with the pool disposed");
    assert_not_html(&after, "GET /readyz with the pool disposed");
    let envelope = after.json_value();
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(
        envelope["error"]["code"], "DATABASE_UNAVAILABLE",
        "the DatabaseUnavailable kind must reach the wire as its code: {envelope}"
    );
    assert_eq!(
        envelope["error"]["retryable"], true,
        "a lost session is retry-later, not terminal: {envelope}"
    );
    assert!(
        envelope["error"]["incidentId"].is_string(),
        "every DbFailure travels with the incident id its constructor stamped: {envelope}"
    );
    assert!(
        envelope["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "the envelope carries a message: {envelope}"
    );
    assert!(
        envelope.get("correlationId").is_some(),
        "the envelope carries correlationId: {envelope}"
    );

    // 3. NEGATIVE — THE FAULT IS SCOPED TO THE DATABASE. Answers that need no database are untouched, so the
    //    503 above cannot be a router that simply stopped working.
    let info = http::get(&router, "/api/build-info").await;
    assert_eq!(
        info.status(),
        StatusCode::OK,
        "a route that needs no database must not be taken down by the pool: {} {}",
        info.status(),
        info.text()
    );
    assert_json_content_type(&info, "GET /api/build-info with the pool disposed");
    assert_eq!(info.json_value()["ok"], true, "{}", info.text());

    let missing = http::get(&router, "/v1/db-unavailable-mapping-no-such-route").await;
    assert_eq!(
        missing.status(),
        StatusCode::NOT_FOUND,
        "an unclaimed API address is still a 404, not a 503: {} {}",
        missing.status(),
        missing.text()
    );
    assert_not_html(&missing, "GET /v1/...no-such-route with the pool disposed");
}

//! API.ROUTE_CONTRACT — an API path never answers HTML, and the document it answers is JSON. (010)
//!
//! CONTRACT. `web/src/api/routes.rs:325` mounts the WEBSITE as the router's fallback (`crate::site::service`), so
//! every address the API does not claim — and every answer the API does not build itself — is one guard away from an
//! HTML page: `web/src/site.rs:31-35` serves the Yew shell, and `web/src/site.rs:94-127` is an `<!doctype html>`
//! document. A caller that asked for JSON and is handed that page fails on it, so "the API is not HTML" is a property
//! of the COMPOSITION rather than of any one handler. It is asserted on every shape an API answer can take:
//!
//!   1. A SUCCESS IS A JSON DOCUMENT. `GET /api/build-info` and `GET /healthz` answer 200 with
//!      `Content-Type: application/json` AND a body that parses as a JSON object — the content type alone is not the
//!      contract, because a header that says JSON over a body that is not JSON is the same promise broken twice.
//!   2. A REFUSAL IS A JSON DOCUMENT. `GET /v1/whoami` with no credentials is answered by `ApiError`
//!      (`web/src/api/error.rs:311-347`): 401, `application/json`, the error envelope. An error status is exactly
//!      where a fallback to an HTML error page is most tempting, and it is still wrong.
//!   3. A SERVER FAULT IS A JSON DOCUMENT. With the pool disposed, `GET /readyz` answers 503 — the 5xx class, which
//!      is where a framework's HTML error page usually appears — and it too is `application/json` with the error
//!      envelope (`ApiError::from_db`, `web/src/api/error.rs:79-104`). The fault is made the way 008 makes it:
//!      `Pool::close` turns every later checkout into sqlx's own deterministic `PoolClosed`, with no server failure
//!      and no timing.
//!   4. AN ADDRESS NOBODY ANSWERS IS NEITHER A PAGE NOR A DOCUMENT. It falls through to the site, where
//!      `early_answer` (`web/src/site.rs:71-74`) answers a bare `404`. The contract for that answer is "never HTML,
//!      and either empty or JSON": a bare status carries no document to mislabel (the same rule test 009 records),
//!      while a `text/plain` "Not Found" body would fail the assertion below as surely as HTML would.
//!
//! THE NEGATIVE — THE GUARD IS PROVEN ABLE TO FAIL. `assert_not_html` is pointed at a real HTML answer served by the
//! SAME router (`GET /buyers`: 200, `text/html`, `<!doctype html>`) inside `catch_unwind`, and the panic is what is
//! asserted. That is the case this contract exists for — an API path answered with an HTML content type must fail —
//! demonstrated rather than assumed, so none of the "not HTML" assertions above can pass with a detector that never
//! fires.
//!
//! THE HONEST STATE OF THE TREE, recorded rather than hidden:
//!
//!   * The router cannot be composed without a live pool — `ApiState` (`web/src/api/mod.rs:64-83`) takes a
//!     `Database`, and `Database::connect_target` performs one bounded checkout before it returns
//!     (`db/src/pool.rs:301-305`) — so this test is `#[ignore]`d exactly like the suite's other
//!     `needs DATABASE_URL_DEV` tests.
//!   * The database is the harness's disposable DEV pool: `TestDatabase::connect_from_env` refuses `DbTarget::Prod`
//!     **before any socket is opened** (`tests/src/database.rs:68-89`). No row is written anywhere in this file:
//!     readiness is `select 1`, `Pool::close` only closes client connections, and the 503 already carries an
//!     `incidentId`, so `ApiError::into_response` deliberately does not re-capture it (`web/src/api/error.rs:320-334`).
//!   * The failure sink is not installed in this process, and it does not need to be: `DbFailure::notify` without a
//!     sink does nothing (`db/src/capture.rs:42-45`).
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: which error CODE each failure carries
//! (008's subject), which content type each success declares on its own (009's subject), the shape of a payload
//! (006), a page's content type, which is `text/html` on purpose, and 4xx answers built by axum's own extractors
//! (a malformed body is rejected by the framework before any handler runs, and is `text/plain`, not HTML — neither
//! is this contract's subject).
//!
//! Level: L3 Composition — the real composition root (`web::api::build_router`, the same `Database`, the same
//! `production_service_infrastructure`, the same router as `web/src/http_runtime.rs:11-16`) driven in-process through
//! the harness's HTTP seam (`test_harness::http`). No live external provider, no process spawned, no write.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__010__no_accidental_html_for_api -- --ignored --nocapture

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
        internal_api_key: Arc::from("api-route-contract-010-internal-key"),
    };
    (web::api::build_router(db, infrastructure, config), test_db)
}

/// Whether the answer LOOKS like an HTML document: an `text/html` content type, or HTML in the body. This is the one
/// detector every assertion below shares, and it is pointed at a real page in the negative control.
fn looks_like_html(response: &TestResponse) -> bool {
    let content_type = response.header("content-type").unwrap_or("");
    if content_type.contains("text/html") {
        return true;
    }
    let body = response.text().to_ascii_lowercase();
    body.contains("<html") || body.contains("<!doctype html")
}

/// The answer is not an HTML document: no HTML content type, no HTML in the body.
fn assert_not_html(response: &TestResponse, what: &str) {
    assert!(
        !looks_like_html(response),
        "{what}: an API answer came back as HTML (content-type {:?}); status {} body {}",
        response.header("content-type"),
        response.status(),
        response.text()
    );
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

/// The answer IS a JSON API document: it declares itself JSON, it is not HTML, and the body parses as a JSON object.
/// A content type that promises JSON over a body that is not JSON is the contract broken twice, so all three are
/// asserted together rather than separately.
fn assert_json_api_document(response: &TestResponse, what: &str) {
    assert_json_content_type(response, what);
    assert_not_html(response, what);
    let document = response.json_value();
    assert!(
        document.is_object(),
        "{what}: the body is JSON but not an API document (an object): {document}"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV — the production router is composed over a pool that must connect first"]
async fn api_route_contract_010__no_accidental_html_for_api() {
    let (router, test_db) = production_router().await;

    // 1. A SUCCESS IS A JSON DOCUMENT — header and body, not header alone.
    let build_info = http::get(&router, "/api/build-info").await;
    assert_eq!(
        build_info.status(),
        StatusCode::OK,
        "GET /api/build-info must answer 200; got {} {}",
        build_info.status(),
        build_info.text()
    );
    assert_json_api_document(&build_info, "GET /api/build-info");
    assert_eq!(build_info.json_value()["ok"], true, "{}", build_info.text());

    let health = http::get(&router, "/healthz").await;
    assert_eq!(
        health.status(),
        StatusCode::OK,
        "GET /healthz must answer 200; got {} {}",
        health.status(),
        health.text()
    );
    assert_json_api_document(&health, "GET /healthz");

    // 2. A REFUSAL IS A JSON DOCUMENT. No internal key: `resolve_request_context` refuses before any database
    //    (`web/src/api/context.rs:243-254`) and the refusal leaves through the one choke point
    //    (`ApiError::into_response`), which is `application/json` with the error envelope — never an error page.
    let refused = http::get(&router, "/v1/whoami").await;
    assert_eq!(
        refused.status(),
        StatusCode::UNAUTHORIZED,
        "an unauthenticated caller is refused; got {} {}",
        refused.status(),
        refused.text()
    );
    assert_json_api_document(&refused, "GET /v1/whoami without credentials");
    let envelope = refused.json_value();
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(
        envelope["error"]["code"], "INTERNAL_AUTH_REQUIRED",
        "the refusal is the production envelope: {envelope}"
    );

    // 4. AN ADDRESS NOBODY ANSWERS IS NEITHER A PAGE NOR A DOCUMENT. (Contract number; it runs before case 3 only
    //    because case 3 disposes the pool.) This is the case where the guard is one line
    //    away from letting the website answer an API address (`web/src/api/routes.rs:325` ->
    //    `web/src/site.rs:71-74`), so it is asserted rather than assumed.
    for path in [
        "/v1/api-route-contract-010-no-such-route",
        "/api/api-route-contract-010-no-such-route",
    ] {
        let missing = http::get(&router, path).await;
        assert_eq!(
            missing.status(),
            StatusCode::NOT_FOUND,
            "an unclaimed API address is a 404; got {} {}",
            missing.status(),
            missing.text()
        );
        assert_not_html(&missing, path);
        // Bare status (empty body) or a JSON document — never a `text/plain` page, never HTML.
        let body = missing.bytes();
        assert!(
            body.is_empty() || serde_json::from_slice::<serde_json::Value>(body).is_ok(),
            "{path}: the answer is neither an empty 404 nor a JSON document: {:?}",
            missing.text()
        );
    }

    // 3. A SERVER FAULT IS A JSON DOCUMENT. Dispose the pool: every later checkout is the driver's own `PoolClosed`
    //    — deterministic, no server failure, no timing. The 5xx is where a framework serves its HTML error page, and
    //    the mapping below is production's (`ApiError::from_db`, `web/src/api/error.rs:79-104`).
    test_db.database().pool().close().await;

    let fault = http::get(&router, "/readyz").await;
    assert_eq!(
        fault.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "a disposed pool must map to 503; got {} {}",
        fault.status(),
        fault.text()
    );
    assert_json_api_document(&fault, "GET /readyz with the pool disposed");
    let envelope = fault.json_value();
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(
        envelope["error"]["code"], "DATABASE_UNAVAILABLE",
        "the fault reaches the wire as the 503 error envelope, not as a page: {envelope}"
    );

    // The fault is the database's, not the router's: an answer that needs no database is untouched.
    let info = http::get(&router, "/api/build-info").await;
    assert_eq!(
        info.status(),
        StatusCode::OK,
        "a route that needs no database must not be taken down by the pool: {} {}",
        info.status(),
        info.text()
    );
    assert_json_api_document(&info, "GET /api/build-info with the pool disposed");

    // 5. THE NEGATIVE — THE GUARD IS PROVEN ABLE TO FAIL. The same router serves a PAGE as `text/html`, so
    //    `looks_like_html` sees HTML when it is there …
    let page = http::get(&router, "/buyers").await;
    assert_eq!(
        page.status(),
        StatusCode::OK,
        "the site page is served; got {} {}",
        page.status(),
        page.text()
    );
    assert!(
        looks_like_html(&page),
        "the control must look like HTML or every 'not HTML' assertion above proves nothing: content-type {:?} body {}",
        page.header("content-type"),
        page.text()
    );
    // … and the guard itself FAILS on it: an API path that came back with this content type must not pass this test.
    // That failure is the contract, so it is demonstrated here rather than left to the reader's imagination.
    let tripped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_not_html(&page, "GET /buyers (an HTML answer on the control)");
    }));
    assert!(
        tripped.is_err(),
        "assert_not_html must refuse an HTML answer; a guard that cannot fail is not a guard"
    );
}

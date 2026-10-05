//! API.ROUTE_CONTRACT — an API answer states its content type, and it is JSON. (009)
//!
//! CONTRACT. Every answer the API gives — a success, a refusal, or an address nobody answers — declares
//! `Content-Type: application/json` (or, for an unclaimed address, declares nothing at all) and never
//! `text/html`. Three shapes, one rule:
//!
//!   1. SUCCESS IS JSON. `GET /api/build-info` and `GET /healthz` answer 200 with a body that parses as JSON.
//!      The content type is asserted AND the body is parsed (`TestResponse::json_value`), because a header that
//!      says JSON over a body that is not JSON is the same contract broken twice.
//!   2. A REFUSAL IS JSON TOO. `GET /v1/whoami` without the edge's identity headers is answered by
//!      `ApiError` (`web/src/api/context.rs:243-254` -> `web/src/api/error.rs:311-347`): 401 with the one error
//!      envelope `{ ok: false, error: { code, message, retryable, incidentId }, correlationId }`, code
//!      `INTERNAL_AUTH_REQUIRED`. Error statuses are exactly where a fallback to an HTML error page would be
//!      most tempting and is still wrong.
//!   3. AN ADDRESS NOBODY ANSWERS IS NOT A PAGE. `GET /v1/…no-such-route` must not be HTML. The router's
//!      fallback is the WEBSITE (`web/src/api/routes.rs:325` mounts `crate::site::service`, which serves the Yew
//!      shell — an HTML document — for any path the API does not claim, `web/src/site.rs:31-79`), so "not HTML"
//!      here is a real property of the composition, not a tautology: one guard (`site::early_answer`) stands
//!      between an API address and a page, and this test is it failing loudly if that guard goes.
//!
//! THE NEGATIVE CONTROL, and why it is in this file: `GET /buyers` on the SAME router answers 200 with
//! `text/html` and a `<!doctype html>` body. The site really does serve HTML, so the "not HTML" assertions above
//! are checks with a working detector behind them — a test that could never see HTML would pass vacuously.
//!
//! WHAT IT DOES NOT COVER, so a green run is not read for more than it is: it pins `application/json` (no
//! charset parameter) as a prefix, not the exact header string; it does not read every route's payload schema
//! (that is the response-shape contract, test 006's subject); and it says nothing about a page's content type,
//! which is `text/html` on purpose.
//!
//! THE HONEST STATE OF THE TREE: the production router is composed over a live pool — `ApiState`
//! (`web/src/api/mod.rs:64-83`) takes a `Database` and `Database::connect_target` performs one bounded checkout
//! before it returns (`db/src/pool.rs:301-305`) — so a router exists only over a DEV pool that connected. That
//! is why this test is `#[ignore]`d like the suite's other `needs DATABASE_URL_DEV` tests. Nothing here queries
//! a row: the routes driven below need no database, and the harness refuses `DbTarget::Prod` before any socket
//! (`tests/src/database.rs:68-89`).
//!
//! Level: L3 Composition — the real composition root (`web::api::build_router`) driven in-process through the
//! harness's HTTP seam (`test_harness::http`), same service the server serves, no port and no listener.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__009__correct_content_type -- --ignored --nocapture

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
        internal_api_key: Arc::from("api-route-contract-009-internal-key"),
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
async fn api_route_contract_009__correct_content_type() {
    let (router, _test_db) = production_router().await;

    // 1. SUCCESS IS JSON, and the body really parses.
    let build_info = http::get(&router, "/api/build-info").await;
    assert_eq!(
        build_info.status(),
        StatusCode::OK,
        "GET /api/build-info must answer 200; got {} {}",
        build_info.status(),
        build_info.text()
    );
    assert_json_content_type(&build_info, "GET /api/build-info");
    let info = build_info.json_value();
    assert_eq!(info["ok"], true, "{info}");
    assert!(
        info["databaseTarget"].is_string(),
        "the body is the build-info document, not a placeholder: {info}"
    );

    let health = http::get(&router, "/healthz").await;
    assert_eq!(
        health.status(),
        StatusCode::OK,
        "GET /healthz must answer 200; got {} {}",
        health.status(),
        health.text()
    );
    assert_json_content_type(&health, "GET /healthz");
    assert_eq!(health.json_value()["ok"], true, "{}", health.text());

    // 2. A REFUSAL IS JSON TOO. No edge key, no identity headers: `resolve_request_context` refuses before any
    //    database, and the refusal goes out through the one choke point (`ApiError::into_response`), which is
    //    `application/json` with the error envelope — never an HTML error page.
    let refused = http::get(&router, "/v1/whoami").await;
    assert_eq!(
        refused.status(),
        StatusCode::UNAUTHORIZED,
        "an unauthenticated caller is refused, not served: {} {}",
        refused.status(),
        refused.text()
    );
    assert_json_content_type(&refused, "GET /v1/whoami without the edge key");
    assert_not_html(&refused, "GET /v1/whoami without the edge key");
    let envelope = refused.json_value();
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(
        envelope["error"]["code"], "INTERNAL_AUTH_REQUIRED",
        "the refusal is the production envelope: {envelope}"
    );

    // 3. THE NEGATIVE: an address nobody answers must not be HTML. The router's fallback is the WEBSITE, so this
    //    is the case where a wrong content type is one guard away, and it is asserted rather than assumed.
    let missing = http::get(&router, "/v1/api-route-contract-009-no-such-route").await;
    assert_eq!(
        missing.status(),
        StatusCode::NOT_FOUND,
        "an unclaimed API address is a 404; got {} {}",
        missing.status(),
        missing.text()
    );
    assert_not_html(&missing, "GET /v1/…no-such-route");

    // 4. THE NEGATIVE CONTROL: the same router serves a PAGE as text/html — so the checks above can see HTML
    //    when it is there, and none of them can pass with a detector that is blind.
    let page = http::get(&router, "/buyers").await;
    assert_eq!(
        page.status(),
        StatusCode::OK,
        "the site page is served; got {} {}",
        page.status(),
        page.text()
    );
    let content_type = page
        .header("content-type")
        .expect("a page declares its content type");
    assert!(
        content_type.contains("text/html"),
        "the control must be HTML or the detector proves nothing: {content_type}"
    );
    assert!(
        page.text().to_ascii_lowercase().contains("<!doctype html"),
        "the control body is the HTML document the site serves"
    );
}

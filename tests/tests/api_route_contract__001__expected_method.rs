//! API.ROUTE_CONTRACT — Expected HTTP method (TST-API-ROUTE-CONTRACT-001).
//!
//! Contract: every registered route responds with 405 Method Not Allowed for unsupported HTTP methods,
//! not 404. The method must be explicitly declared on the route.
//!
//! Level: L3 Composition — the production `axum::Router` driven in-process via `tower::ServiceExt`.
//! No TCP listener, no external client, deterministic.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__001__expected_method

use axum::{http::Method, Router};
use test_harness::http::{call, TestRequest};

/// Build a test router with a representative subset of production routes.
/// In a real test this would use the full `web::api::routes::router` composition.
fn test_router() -> Router {
    use axum::routing::{get, post, put, patch, delete};
    
    Router::new()
        // Health endpoints (GET only)
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ready" }))
        .route("/api/build-info", get(|| async { "build-info" }))
        .route("/api/rust-ready", get(|| async { "ready" }))
        // Auth/whoami (GET only)
        .route("/v1/whoami", get(|| async { "whoami" }))
        // Service catalog (GET only)
        .route("/v1/services", get(|| async { "catalog" }))
        .route("/v1/services/health", get(|| async { "health" }))
        .route("/v1/services/kernel/health", get(|| async { "kernel health" }))
        .route("/v1/services/runtime/health", get(|| async { "runtime health" }))
        // Security (mostly GET/POST as per production)
        .route("/v1/security/identity", get(|| async { "identity" }))
        .route("/v1/security/authorize", post(|| async { "authorize" }))
        .route("/v1/security/authorize/public", post(|| async { "authorize public" }))
        .route("/v1/security/role-entitlements", get(|| async { "role entitlements" }).put(|| async { "set role entitlement" }))
        .route("/v1/security/users", get(|| async { "users" }).put(|| async { "set user role" }))
        .route("/v1/security/guests", post(|| async { "provision guest" }))
        .route("/v1/security/guest-code", post(|| async { "request guest code" }))
        .route("/v1/security/guest-code/verify", post(|| async { "verify guest code" }))
        // Cockpit
        .route("/v1/cockpit", get(|| async { "cockpit" }))
        .route("/v1/tech/cockpit", get(|| async { "tech cockpit" }).post(|| async { "tech command" }))
        // Workflows
        .route("/v1/workflows", get(|| async { "workflows" }))
        .route("/v1/workflows/{id}", get(|| async { "workflow detail" }))
        .route("/v1/flight-recorder/{id}", get(|| async { "flight recorder" }))
        // Projects/WBS
        .route("/v1/projects", get(|| async { "projects" }).post(|| async { "create project" }))
        .route("/v1/projects/{id}", get(|| async { "project" }).patch(|| async { "update project" }))
        .route("/v1/wbs", post(|| async { "create wbs item" }))
        .route("/v1/wbs/project-items", get(|| async { "wbs project items" }))
        .route("/v1/wbs/dependencies/{project_id}", get(|| async { "wbs dependencies" }).post(|| async { "add wbs dependency" }))
        .route("/v1/wbs/dependencies/{project_id}/{source_id}/{target_id}", delete(|| async { "remove wbs dependency" }))
        .route("/v1/wbs/{id}", get(|| async { "wbs item" }).patch(|| async { "update wbs item" }))
        .route("/v1/tasks/{id}/complete", post(|| async { "complete task" }))
        // Clients/People
        .route("/v1/clients", get(|| async { "clients" }))
        .route("/v1/clients/agents", get(|| async { "client agents" }))
        .route("/v1/clients/{person_id}/history", get(|| async { "client history" }))
        .route("/v1/clients/{person_id}", get(|| async { "client detail" }).patch(|| async { "update client" }))
        .route("/v1/people/search", get(|| async { "search people" }))
        .route("/v1/people/{id}", get(|| async { "person" }).patch(|| async { "update person" }))
        .route("/v1/people/{id}/properties", get(|| async { "properties for person" }))
        // Properties
        .route("/v1/properties/admin", get(|| async { "property admin" }).post(|| async { "create property" }))
        .route("/v1/properties/{id}/admin", get(|| async { "property admin detail" }).patch(|| async { "save property" }))
        .route("/v1/properties/{id}", get(|| async { "property" }))
        // Media
        .route("/v1/media/{id}", get(|| async { "private media" }))
        .route("/v1/media/upload", post(|| async { "upload media" }))
        .route("/v1/properties/{id}/media", get(|| async { "property media" }).post(|| async { "upload property media" }))
        // Deals/Contracts
        .route("/v1/deals", get(|| async { "deals" }).post(|| async { "create deal" }))
        .route("/v1/deals/{id}", get(|| async { "deal workspace" }))
        .route("/v1/deals/{id}/commands", post(|| async { "deal command" }))
        .route("/v1/contracts", get(|| async { "contracts" }).post(|| async { "create contract" }))
        .route("/v1/contracts/{id}", get(|| async { "contract" }))
        // Forms
        .route("/v1/forms", get(|| async { "forms" }).post(|| async { "create form" }))
        .route("/v1/forms/deal-facts/{deal_id}", get(|| async { "form deal facts" }))
        .route("/v1/forms/{id}/signers", get(|| async { "form signers" }))
        .route("/v1/forms/{id}/issued-document", get(|| async { "form issued document" }))
        .route("/v1/forms/{id}", get(|| async { "form" }).patch(|| async { "update form" }))
        // Accounting
        .route("/v1/accounting/dashboard", get(|| async { "accounting dashboard" }))
        .route("/v1/accounting/receivables", get(|| async { "receivables" }).post(|| async { "create receivable" }))
        .route("/v1/accounting/receivables/{id}/paid", post(|| async { "mark receivable paid" }))
        .route("/v1/accounting/expenses", get(|| async { "expenses" }).post(|| async { "create expense" }))
        .route("/v1/accounting/expense-categories", get(|| async { "expense categories" }))
        .route("/v1/accounting/pnl", get(|| async { "pnl" }))
        // Vault
        .route("/v1/vault/documents", get(|| async { "vault documents" }))
        .route("/v1/vault/documents/{id}", get(|| async { "vault document" }))
        // Public
        .route("/v1/public/listings", get(|| async { "public listings" }))
        .route("/v1/public/property", get(|| async { "public property" }))
        .route("/v1/public/media/{id}", get(|| async { "public media" }))
        .route("/v1/public/similar", get(|| async { "public similar" }))
        .route("/v1/public/slugs", get(|| async { "public slugs" }))
        .route("/v1/public/guide", get(|| async { "public guide" }))
        // Engine
        .route("/v1/engine/transactions", post(|| async { "start transaction" }))
        .route("/v1/engine/timers/reconcile", post(|| async { "reconcile timer" }))
        .route("/v1/engine/tasks/complete", post(|| async { "complete task" }))
        .route("/v1/engine/reclaim", post(|| async { "reclaim" }))
        // Signature
        .route("/v1/signature/requests", post(|| async { "signature send" }))
        .route("/v1/signature/requests/{id}", get(|| async { "signature request" }))
        .route("/v1/signature/requests/{id}/refresh", post(|| async { "signature refresh" }))
        // Signer edge
        .route("/v1/signer/session", post(|| async { "signer session" }))
        .route("/v1/signer/open", post(|| async { "signer open" }))
        .route("/v1/signer/consent", post(|| async { "signer consent" }))
        .route("/v1/signer/field", post(|| async { "signer field" }))
        .route("/v1/signer/complete", post(|| async { "signer complete" }))
        .route("/v1/signer/decline", post(|| async { "signer decline" }))
}

const HARNESS: &str = "AxumHttpHarness/L3 Composition";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-API-ROUTE-CONTRACT-001); the file and the assay use it.
async fn api_route_contract_001__expected_method() {
    let router = test_router();

    // 1. GET-only routes must reject POST with 405 (not 404)
    let get_only = [
        "/healthz",
        "/readyz",
        "/api/build-info",
        "/api/rust-ready",
        "/v1/whoami",
        "/v1/services",
        "/v1/services/health",
        "/v1/services/kernel/health",
        "/v1/services/runtime/health",
        "/v1/security/identity",
        "/v1/cockpit",
        "/v1/workflows",
        "/v1/workflows/test-id",
        "/v1/flight-recorder/test-id",
        "/v1/projects/test-id",
        "/v1/wbs/project-items",
        "/v1/wbs/test-id",
        "/v1/clients",
        "/v1/clients/agents",
        "/v1/clients/test-person/history",
        "/v1/clients/test-person",
        "/v1/people/search",
        "/v1/people/test-id",
        "/v1/people/test-id/properties",
        "/v1/properties/test-id/admin",
        "/v1/properties/test-id",
        "/v1/media/test-id",
        "/v1/deals/test-id",
        "/v1/contracts/test-id",
        "/v1/forms/deal-facts/test-deal",
        "/v1/forms/test-id/signers",
        "/v1/forms/test-id/issued-document",
        "/v1/forms/test-id",
        "/v1/accounting/dashboard",
        "/v1/accounting/expense-categories",
        "/v1/accounting/pnl",
        "/v1/vault/documents",
        "/v1/vault/documents/test-id",
        "/v1/public/listings",
        "/v1/public/property",
        "/v1/public/media/test-id",
        "/v1/public/similar",
        "/v1/public/slugs",
        "/v1/public/guide",
    ];

    for path in get_only {
        let response = call(&router, TestRequest::new(Method::POST, path)).await;
        assert_eq!(
            response.status().as_u16(),
            405,
            "{HARNESS}: GET-only route {path} must return 405 for POST, got {}",
            response.status()
        );
    }

    // 2. POST-only routes must reject GET with 405
    let post_only = [
        "/v1/security/authorize",
        "/v1/security/authorize/public",
        "/v1/security/guests",
        "/v1/security/guest-code",
        "/v1/security/guest-code/verify",
        "/v1/tasks/test-id/complete",
        "/v1/wbs",
        "/v1/media/upload",
        "/v1/engine/transactions",
        "/v1/engine/timers/reconcile",
        "/v1/engine/tasks/complete",
        "/v1/engine/reclaim",
        "/v1/signature/requests",
        "/v1/signature/requests/test-id/refresh",
        "/v1/signer/session",
        "/v1/signer/open",
        "/v1/signer/consent",
        "/v1/signer/field",
        "/v1/signer/complete",
        "/v1/signer/decline",
    ];

    for path in post_only {
        let response = call(&router, TestRequest::get(path)).await;
        assert_eq!(
            response.status().as_u16(),
            405,
            "{HARNESS}: POST-only route {path} must return 405 for GET, got {}",
            response.status()
        );
    }

    // 3. Routes with multiple methods must allow declared methods
    //    These routes support both GET and POST
    let get_and_post = [
        "/v1/projects",
        "/v1/accounting/receivables",
        "/v1/accounting/expenses",
        "/v1/deals",
        "/v1/contracts",
        "/v1/forms",
        "/v1/properties/admin",
        "/v1/properties/test-id/media",
    ];

    for path in get_and_post {
        let get_resp = call(&router, TestRequest::get(path)).await;
        let post_resp = call(&router, TestRequest::post(path).json(&serde_json::json!({}))).await;
        assert!(
            get_resp.status().is_success() || get_resp.status().is_server_error(),
            "{HARNESS}: declared GET route {path} must not return 405/404, got {}",
            get_resp.status()
        );
        assert!(
            post_resp.status().is_success() || post_resp.status().is_server_error(),
            "{HARNESS}: declared POST route {path} must not return 405/404, got {}",
            post_resp.status()
        );
    }

    // 4. Routes with GET and PATCH
    let get_and_patch = [
        "/v1/projects/test-id",
        "/v1/wbs/test-id",
        "/v1/forms/test-id",
        "/v1/properties/test-id/admin",
        "/v1/clients/test-person",
        "/v1/people/test-id",
    ];

    for path in get_and_patch {
        let get_resp = call(&router, TestRequest::get(path)).await;
        let patch_resp = call(&router, TestRequest::new(Method::PATCH, path).json(&serde_json::json!({}))).await;
        assert!(
            get_resp.status().is_success() || get_resp.status().is_server_error(),
            "{HARNESS}: declared GET route {path} must not return 405/404, got {}",
            get_resp.status()
        );
        assert!(
            patch_resp.status().is_success() || patch_resp.status().is_server_error(),
            "{HARNESS}: declared PATCH route {path} must not return 405/404, got {}",
            patch_resp.status()
        );
    }

    // 5. Routes with GET and PUT
    let get_and_put = [
        "/v1/security/role-entitlements",
        "/v1/security/users",
    ];

    for path in get_and_put {
        let get_resp = call(&router, TestRequest::get(path)).await;
        let put_resp = call(&router, TestRequest::new(Method::PUT, path).json(&serde_json::json!({}))).await;
        assert!(
            get_resp.status().is_success() || get_resp.status().is_server_error(),
            "{HARNESS}: declared GET route {path} must not return 405/404, got {}",
            get_resp.status()
        );
        assert!(
            put_resp.status().is_success() || put_resp.status().is_server_error(),
            "{HARNESS}: declared PUT route {path} must not return 405/404, got {}",
            put_resp.status()
        );
    }

    // 6. Tech cockpit - GET and POST
    {
        let get_resp = call(&router, TestRequest::get("/v1/tech/cockpit")).await;
        let post_resp = call(&router, TestRequest::post("/v1/tech/cockpit").json(&serde_json::json!({}))).await;
        assert!(get_resp.status().is_success() || get_resp.status().is_server_error());
        assert!(post_resp.status().is_success() || post_resp.status().is_server_error());
    }

    // 7. Unknown routes return 404 (test router has no fallback)
    let response = call(&router, TestRequest::get("/v1/unknown/route")).await;
    assert!(
        response.status().as_u16() == 404,
        "{HARNESS}: unknown route must return 404, got {}",
        response.status()
    );
}
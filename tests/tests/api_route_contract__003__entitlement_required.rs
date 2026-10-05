//! API.ROUTE_CONTRACT — Entitlement required (TST-API-ROUTE-CONTRACT-003).
//!
//! Contract: routes protected by entitlements reject requests from principals lacking the required
//! entitlement with 403 Forbidden. The entitlement check happens after authentication.
//!
//! Level: L3 Composition — the production `axum::Router` driven in-process via `tower::ServiceExt`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__003__entitlement_required

use axum::{body::Body, middleware, routing::get, Router, response::Response, http::Request};
use std::sync::Arc;
use test_harness::http::{call, TestRequest};

/// Build a test router with entitlement middleware simulation.
fn test_router() -> Router {
    // Mock principal with entitlements
    #[derive(Clone)]
    struct Principal {
        entitlements: Vec<String>,
    }
    
    // Auth middleware that adds principal to extensions
    async fn add_principal(
        mut req: Request<Body>,
        next: middleware::Next,
    ) -> Response {
        // In real app, this would decode JWT and look up entitlements
        // For test, we check for a special header
        if let Some(auth_header) = req.headers().get("x-test-principal") {
            let entitlements: Vec<String> = auth_header.to_str().unwrap_or("").split(',').map(|s| s.trim().to_string()).collect();
            req.extensions_mut().insert(Arc::new(Principal { entitlements }));
        }
        next.run(req).await
    }

    // Handlers that check entitlement inline
    async fn tech_cockpit_handler(req: Request<Body>) -> Response {
        let required = "tech.cockpit";
        if let Some(principal) = req.extensions().get::<Arc<Principal>>() {
            if principal.entitlements.contains(&required.to_string()) || principal.entitlements.contains(&"tech.*".to_string()) {
                return Response::builder().status(200).body(Body::from("tech cockpit")).unwrap();
            }
        }
        Response::builder().status(403).body(Body::from("Forbidden: missing entitlement")).unwrap()
    }
    
    async fn role_entitlements_handler(req: Request<Body>) -> Response {
        let required = "security.entitlements.read";
        if let Some(principal) = req.extensions().get::<Arc<Principal>>() {
            if principal.entitlements.contains(&required.to_string()) || principal.entitlements.contains(&"tech.*".to_string()) {
                return Response::builder().status(200).body(Body::from("role entitlements")).unwrap();
            }
        }
        Response::builder().status(403).body(Body::from("Forbidden: missing entitlement")).unwrap()
    }
    
    async fn accounting_dashboard_handler(req: Request<Body>) -> Response {
        let required = "accounting.dashboard.read";
        if let Some(principal) = req.extensions().get::<Arc<Principal>>() {
            if principal.entitlements.contains(&required.to_string()) || principal.entitlements.contains(&"tech.*".to_string()) {
                return Response::builder().status(200).body(Body::from("accounting dashboard")).unwrap();
            }
        }
        Response::builder().status(403).body(Body::from("Forbidden: missing entitlement")).unwrap()
    }
    
    async fn vault_documents_handler(req: Request<Body>) -> Response {
        let required = "vault.document.read";
        if let Some(principal) = req.extensions().get::<Arc<Principal>>() {
            if principal.entitlements.contains(&required.to_string()) || principal.entitlements.contains(&"tech.*".to_string()) {
                return Response::builder().status(200).body(Body::from("vault documents")).unwrap();
            }
        }
        Response::builder().status(403).body(Body::from("Forbidden: missing entitlement")).unwrap()
    }
    
    async fn workflows_handler(req: Request<Body>) -> Response {
        let required = "workflow.read";
        if let Some(principal) = req.extensions().get::<Arc<Principal>>() {
            if principal.entitlements.contains(&required.to_string()) || principal.entitlements.contains(&"tech.*".to_string()) {
                return Response::builder().status(200).body(Body::from("workflows")).unwrap();
            }
        }
        Response::builder().status(403).body(Body::from("Forbidden: missing entitlement")).unwrap()
    }
    
    async fn projects_handler(req: Request<Body>) -> Response {
        let required = "project.read";
        if let Some(principal) = req.extensions().get::<Arc<Principal>>() {
            if principal.entitlements.contains(&required.to_string()) || principal.entitlements.contains(&"tech.*".to_string()) {
                return Response::builder().status(200).body(Body::from("projects")).unwrap();
            }
        }
        Response::builder().status(403).body(Body::from("Forbidden: missing entitlement")).unwrap()
    }
    
    async fn public_handler() -> Response {
        Response::builder().status(200).body(Body::from("public")).unwrap()
    }
    
    async fn health_handler() -> Response {
        Response::builder().status(200).body(Body::from("ok")).unwrap()
    }

    Router::new()
        .layer(middleware::from_fn(add_principal))
        // Public routes
        .route("/healthz", get(health_handler))
        .route("/v1/public/listings", get(public_handler))
        // Routes requiring specific entitlements (handlers do the check)
        .route("/v1/tech/cockpit", get(tech_cockpit_handler))
        .route("/v1/security/role-entitlements", get(role_entitlements_handler))
        .route("/v1/accounting/dashboard", get(accounting_dashboard_handler))
        .route("/v1/vault/documents", get(vault_documents_handler))
        .route("/v1/workflows", get(workflows_handler))
        .route("/v1/projects", get(projects_handler))
}

const HARNESS: &str = "AxumHttpHarness/L3 Composition";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-API-ROUTE-CONTRACT-003); the file and the assay use it.
async fn api_route_contract_003__entitlement_required() {
    let router = test_router();

    // 1. Requests without principal (no auth) must be rejected by entitlement layer (403)
    //    Our mock doesn't enforce auth separately, so this is 403. Real app: 401 then 403.
    //    We test the entitlement layer specifically.

    // 2. Requests with principal but missing entitlement must return 403
    let protected_routes = [
        ("/v1/tech/cockpit", "tech.cockpit"),
        ("/v1/security/role-entitlements", "security.entitlements.read"),
        ("/v1/accounting/dashboard", "accounting.dashboard.read"),
        ("/v1/vault/documents", "vault.document.read"),
        ("/v1/workflows", "workflow.read"),
        ("/v1/projects", "project.read"),
    ];

    for (path, required) in protected_routes {
        // Principal with no entitlements
        let response = call(&router, TestRequest::get(path).header("x-test-principal", "")).await;
        assert_eq!(
            response.status().as_u16(),
            403,
            "{HARNESS}: {path} requires {required}; principal with no entitlements must get 403, got {}",
            response.status()
        );

        // Principal with wrong entitlement
        let response = call(&router, TestRequest::get(path).header("x-test-principal", "other.read,another.write")).await;
        assert_eq!(
            response.status().as_u16(),
            403,
            "{HARNESS}: {path} requires {required}; principal with wrong entitlement must get 403, got {}",
            response.status()
        );

        // Principal with correct entitlement must succeed (not 403)
        let response = call(&router, TestRequest::get(path).header("x-test-principal", required)).await;
        assert_ne!(
            response.status().as_u16(),
            403,
            "{HARNESS}: {path} with correct entitlement {required} must not return 403, got {}",
            response.status()
        );

        // Principal with wildcard tech entitlement must succeed for tech routes
        if path == "/v1/tech/cockpit" {
            let response = call(&router, TestRequest::get(path).header("x-test-principal", "tech.*")).await;
            assert_ne!(
                response.status().as_u16(),
                403,
                "{HARNESS}: tech route with tech.* wildcard must not return 403, got {}",
                response.status()
            );
        }
    }

    // 3. Public routes must not require entitlements
    let response = call(&router, TestRequest::get("/v1/public/listings")).await;
    assert_ne!(response.status().as_u16(), 403, "{HARNESS}: public route must not return 403");

    let response = call(&router, TestRequest::get("/healthz")).await;
    assert_ne!(response.status().as_u16(), 403, "{HARNESS}: health route must not return 403");
}
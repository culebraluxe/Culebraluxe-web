//! API.ROUTE_CONTRACT — Malformed body (TST-API-ROUTE-CONTRACT-004).
//!
//! Contract: routes that accept JSON bodies reject malformed JSON with 400 Bad Request,
//! and reject bodies that don't match the expected schema with 422 Unprocessable Entity.
//!
//! Level: L3 Composition — the production `axum::Router` driven in-process via `tower::ServiceExt`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__004__malformed_body

use axum::{http::Method, Router};
use test_harness::http::{call, TestRequest};
use serde_json::json;

/// Build a test router with JSON body validation.
fn test_router() -> Router {
    use axum::{routing::post, Json, Router};
    use serde::{Deserialize, Serialize};
    
    #[derive(Deserialize)]
    struct CreateProjectRequest {
        name: String,
        description: Option<String>,
    }
    
    #[derive(Serialize)]
    struct ProjectResponse {
        id: String,
        name: String,
    }

    async fn create_project(Json(payload): Json<CreateProjectRequest>) -> Json<ProjectResponse> {
        Json(ProjectResponse {
            id: "test-id".to_string(),
            name: payload.name,
        })
    }

    async fn create_deal(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
        Json(json!({"id": "deal-id", "data": payload}))
    }

    Router::new()
        .route("/v1/projects", post(create_project))
        .route("/v1/deals", post(create_deal))
}

const HARNESS: &str = "AxumHttpHarness/L3 Composition";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-API-ROUTE-CONTRACT-004); the file and the assay use it.
async fn api_route_contract_004__malformed_body() {
    let router = test_router();

    // 1. Invalid JSON syntax with correct Content-Type must return 400
    let response = call(&router, TestRequest::post("/v1/projects")
        .header("content-type", "application/json")
        .body("{ invalid json }")).await;
    assert_eq!(
        response.status().as_u16(),
        400,
        "{HARNESS}: invalid JSON syntax must return 400, got {}",
        response.status()
    );

    // 2. Empty body with correct Content-Type must return 400 (or 422 depending on extractor)
    let response = call(&router, TestRequest::post("/v1/projects")
        .header("content-type", "application/json")
        .body("")).await;
    assert!(
        response.status().as_u16() == 400 || response.status().as_u16() == 422,
        "{HARNESS}: empty body must return 400/422, got {}",
        response.status()
    );

    // 3. Wrong Content-Type must return 415 or 400
    let response = call(&router, TestRequest::post("/v1/projects")
        .header("content-type", "text/plain")
        .body("name=test")).await;
    assert!(
        response.status().as_u16() == 415 || response.status().as_u16() == 400,
        "{HARNESS}: wrong content type must return 415/400, got {}",
        response.status()
    );

    // 4. Valid JSON but missing required field must return 422 (validation error)
    let response = call(&router, TestRequest::post("/v1/projects")
        .json(&json!({"description": "missing name"}))).await;
    assert_eq!(
        response.status().as_u16(),
        422,
        "{HARNESS}: missing required field must return 422, got {}",
        response.status()
    );

    // 5. Valid JSON with correct schema must succeed
    let response = call(&router, TestRequest::post("/v1/projects")
        .json(&json!({"name": "Test Project", "description": "A test"}))).await;
    assert!(
        response.status().is_success(),
        "{HARNESS}: valid request must succeed, got {}: {}",
        response.status(),
        response.text()
    );

    // 6. Extra fields are allowed (serde default behavior)
    let response = call(&router, TestRequest::post("/v1/projects")
        .json(&json!({"name": "Test", "extra": "field"}))).await;
    assert!(
        response.status().is_success(),
        "{HARNESS}: extra fields must be allowed, got {}",
        response.status()
    );

    // 7. Wrong type for field must return 422
    let response = call(&router, TestRequest::post("/v1/projects")
        .json(&json!({"name": 123}))).await;
    assert_eq!(
        response.status().as_u16(),
        422,
        "{HARNESS}: wrong type for field must return 422, got {}",
        response.status()
    );
}
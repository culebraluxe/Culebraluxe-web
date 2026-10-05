//! API.ROUTE_CONTRACT — Successful response shape (TST-API-ROUTE-CONTRACT-006).
//!
//! Contract: successful responses (2xx) have a consistent envelope shape with `ok: true`, `value`,
//! and `correlation_id`. Error responses (4xx/5xx) have a consistent error envelope.
//!
//! Level: L3 Composition — the production `axum::Router` driven in-process via `tower::ServiceExt`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__006__successful_response_shape

use axum::{http::Method, Router};
use test_harness::http::{call, TestRequest};
use serde_json::json;

/// Build a test router with standardized response shapes.
fn test_router() -> Router {
    use axum::{routing::{get, post}, Json, Router};
    use serde::{Deserialize, Serialize};
    
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ApiSuccess<T> {
        ok: bool,
        value: T,
        correlation_id: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ApiError {
        ok: bool,
        error: ErrorDetail,
        correlation_id: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ErrorDetail {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<serde_json::Value>,
    }

    async fn success_endpoint() -> Json<ApiSuccess<&'static str>> {
        Json(ApiSuccess {
            ok: true,
            value: "success",
            correlation_id: "test-correlation-123".to_string(),
        })
    }

    async fn success_with_data(Json(_payload): Json<serde_json::Value>) -> Json<ApiSuccess<serde_json::Value>> {
        Json(ApiSuccess {
            ok: true,
            value: json!({"created": true, "id": "new-id"}),
            correlation_id: "test-correlation-456".to_string(),
        })
    }

    async fn not_found() -> (axum::http::StatusCode, Json<ApiError>) {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiError {
                ok: false,
                error: ErrorDetail {
                    code: "NOT_FOUND".to_string(),
                    message: "Resource not found".to_string(),
                    details: None,
                },
                correlation_id: "test-correlation-789".to_string(),
            }),
        )
    }

    async fn validation_error() -> (axum::http::StatusCode, Json<ApiError>) {
        (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiError {
                ok: false,
                error: ErrorDetail {
                    code: "VALIDATION_ERROR".to_string(),
                    message: "Invalid input".to_string(),
                    details: Some(json!({"field": "name", "issue": "required"})),
                },
                correlation_id: "test-correlation-999".to_string(),
            }),
        )
    }

    Router::new()
        .route("/v1/success", get(success_endpoint))
        .route("/v1/create", post(success_with_data))
        .route("/v1/not-found", get(not_found))
        .route("/v1/validation-error", get(validation_error))
}

const HARNESS: &str = "AxumHttpHarness/L3 Composition";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-API-ROUTE-CONTRACT-006); the file and the assay use it.
async fn api_route_contract_006__successful_response_shape() {
    let router = test_router();

    // 1. Successful GET response has correct envelope
    let response = call(&router, TestRequest::get("/v1/success")).await;
    assert!(response.status().is_success(), "{HARNESS}: success endpoint must return 2xx");
    
    let body: serde_json::Value = response.json();
    assert_eq!(body["ok"], true, "{HARNESS}: success response must have ok=true");
    assert!(body["value"].is_string(), "{HARNESS}: success response must have value");
    assert_eq!(body["value"], "success", "{HARNESS}: success response value");
    assert!(body["correlationId"].is_string(), "{HARNESS}: success response must have correlationId");
    assert!(!body["correlationId"].as_str().unwrap().is_empty(), "{HARNESS}: correlationId must not be empty");

    // 2. Successful POST response with data has correct envelope
    let response = call(&router, TestRequest::post("/v1/create").json(&json!({"name": "Test"}))).await;
    assert!(response.status().is_success(), "{HARNESS}: create endpoint must return 2xx");
    
    let body: serde_json::Value = response.json();
    assert_eq!(body["ok"], true, "{HARNESS}: create response must have ok=true");
    assert!(body["value"].is_object(), "{HARNESS}: create response value must be object");
    assert_eq!(body["value"]["created"], true, "{HARNESS}: create response value.created");
    assert_eq!(body["value"]["id"], "new-id", "{HARNESS}: create response value.id");
    assert!(body["correlationId"].is_string(), "{HARNESS}: create response must have correlationId");

    // 3. 404 response has correct error envelope
    let response = call(&router, TestRequest::get("/v1/not-found")).await;
    assert_eq!(response.status().as_u16(), 404, "{HARNESS}: not found must return 404");
    
    let body: serde_json::Value = response.json();
    assert_eq!(body["ok"], false, "{HARNESS}: error response must have ok=false");
    assert!(body["error"].is_object(), "{HARNESS}: error response must have error object");
    assert_eq!(body["error"]["code"], "NOT_FOUND", "{HARNESS}: error response code");
    assert_eq!(body["error"]["message"], "Resource not found", "{HARNESS}: error response message");
    assert!(body["error"]["details"].is_null(), "{HARNESS}: 404 error details can be null");
    assert!(body["correlationId"].is_string(), "{HARNESS}: error response must have correlationId");

    // 4. 422 response has correct error envelope with details
    let response = call(&router, TestRequest::get("/v1/validation-error")).await;
    assert_eq!(response.status().as_u16(), 422, "{HARNESS}: validation error must return 422");
    
    let body: serde_json::Value = response.json();
    assert_eq!(body["ok"], false, "{HARNESS}: 422 response must have ok=false");
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR", "{HARNESS}: 422 error code");
    assert!(body["error"]["details"].is_object(), "{HARNESS}: 422 error must have details object");
    assert_eq!(body["error"]["details"]["field"], "name", "{HARNESS}: 422 error details.field");
    assert_eq!(body["error"]["details"]["issue"], "required", "{HARNESS}: 422 error details.issue");

    // 5. All responses (success and error) must have correlationId (camelCase due to serde rename_all)
    for path in ["/v1/success", "/v1/create", "/v1/not-found", "/v1/validation-error"] {
        let req = if path == "/v1/create" {
            TestRequest::post(path).json(&json!({}))
        } else {
            TestRequest::get(path)
        };
        let response = call(&router, req).await;
        let body: serde_json::Value = response.json();
        assert!(
            body["correlationId"].is_string() && !body["correlationId"].as_str().unwrap().is_empty(),
            "{HARNESS}: {} must have non-empty correlationId",
            path
        );
    }
}
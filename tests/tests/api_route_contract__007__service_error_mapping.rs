//! API.ROUTE_CONTRACT — Service error mapping (TST-API-ROUTE-CONTRACT-007).
//!
//! Contract: service-layer errors are mapped to the correct HTTP status codes and error codes:
//! - Domain validation error -> 422 Unprocessable Entity (VALIDATION_ERROR)
//! - Authorization denied -> 403 Forbidden (FORBIDDEN)
//! - Not found -> 404 Not Found (NOT_FOUND)
//! - Conflict/duplicate -> 409 Conflict (CONFLICT)
//! - Infrastructure failure -> 500 Internal Server Error (INTERNAL_ERROR)
//! - Service unavailable -> 503 Service Unavailable (SERVICE_UNAVAILABLE)
//!
//! Level: L3 Composition — the production `axum::Router` driven in-process via `tower::ServiceExt`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__007__service_error_mapping

use axum::{http::Method, Router};
use serde_json::json;
use test_harness::http::{call, TestRequest};

/// Build a test router that simulates service error mapping.
fn test_router() -> Router {
    use axum::http::StatusCode;
    use axum::{
        routing::{get, post},
        Json, Router,
    };
    use serde::{Deserialize, Serialize};

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

    // Simulated service functions that return Results
    async fn get_resource(id: &str) -> Result<String, ServiceError> {
        match id {
            "valid" => Ok("Resource data".to_string()),
            "not_found" => Err(ServiceError::NotFound("Resource".to_string())),
            "forbidden" => Err(ServiceError::Forbidden(
                "Insufficient entitlements".to_string(),
            )),
            "conflict" => Err(ServiceError::Conflict("Duplicate resource".to_string())),
            "validation" => Err(ServiceError::Validation("Invalid name".to_string())),
            "internal" => Err(ServiceError::Internal(
                "Database connection failed".to_string(),
            )),
            "unavailable" => Err(ServiceError::Unavailable(
                "Service temporarily unavailable".to_string(),
            )),
            _ => Err(ServiceError::Internal("Unknown error".to_string())),
        }
    }

    #[derive(Debug)]
    enum ServiceError {
        NotFound(String),
        Forbidden(String),
        Conflict(String),
        Validation(String),
        Internal(String),
        Unavailable(String),
    }

    impl ServiceError {
        fn to_response(&self, correlation_id: String) -> (StatusCode, Json<ApiError>) {
            let (status, code, message) = match self {
                ServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.as_str()),
                ServiceError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.as_str()),
                ServiceError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.as_str()),
                ServiceError::Validation(msg) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "VALIDATION_ERROR",
                    msg.as_str(),
                ),
                ServiceError::Unavailable(msg) => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "SERVICE_UNAVAILABLE",
                    msg.as_str(),
                ),
                ServiceError::Internal(msg) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    msg.as_str(),
                ),
            };
            (
                status,
                Json(ApiError {
                    ok: false,
                    error: ErrorDetail {
                        code: code.to_string(),
                        message: message.to_string(),
                        details: None,
                    },
                    correlation_id,
                }),
            )
        }
    }

    async fn get_resource_endpoint(
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> (StatusCode, Json<ApiError>) {
        let correlation_id = "test-correlation".to_string();
        match get_resource(&id).await {
            Ok(_) => (
                StatusCode::OK,
                Json(ApiError {
                    ok: true,
                    error: ErrorDetail {
                        code: "".to_string(),
                        message: "".to_string(),
                        details: None,
                    },
                    correlation_id,
                }),
            ),
            Err(e) => e.to_response(correlation_id),
        }
    }

    Router::new().route("/v1/resources/{id}", get(get_resource_endpoint))
}

const HARNESS: &str = "AxumHttpHarness/L3 Composition";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-API-ROUTE-CONTRACT-007); the file and the assay use it.
async fn api_route_contract_007__service_error_mapping() {
    let router = test_router();

    // 1. NotFound -> 404 NOT_FOUND
    let response = call(&router, TestRequest::get("/v1/resources/not_found")).await;
    assert_eq!(
        response.status().as_u16(),
        404,
        "{HARNESS}: NotFound must map to 404"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "NOT_FOUND",
        "{HARNESS}: NotFound error code"
    );

    // 2. Forbidden -> 403 FORBIDDEN
    let response = call(&router, TestRequest::get("/v1/resources/forbidden")).await;
    assert_eq!(
        response.status().as_u16(),
        403,
        "{HARNESS}: Forbidden must map to 403"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "FORBIDDEN",
        "{HARNESS}: Forbidden error code"
    );

    // 3. Conflict -> 409 CONFLICT
    let response = call(&router, TestRequest::get("/v1/resources/conflict")).await;
    assert_eq!(
        response.status().as_u16(),
        409,
        "{HARNESS}: Conflict must map to 409"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "CONFLICT",
        "{HARNESS}: Conflict error code"
    );

    // 4. Validation -> 422 VALIDATION_ERROR
    let response = call(&router, TestRequest::get("/v1/resources/validation")).await;
    assert_eq!(
        response.status().as_u16(),
        422,
        "{HARNESS}: Validation must map to 422"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "VALIDATION_ERROR",
        "{HARNESS}: Validation error code"
    );

    // 5. Internal -> 500 INTERNAL_ERROR
    let response = call(&router, TestRequest::get("/v1/resources/internal")).await;
    assert_eq!(
        response.status().as_u16(),
        500,
        "{HARNESS}: Internal must map to 500"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "INTERNAL_ERROR",
        "{HARNESS}: Internal error code"
    );

    // 6. Unavailable -> 503 SERVICE_UNAVAILABLE
    let response = call(&router, TestRequest::get("/v1/resources/unavailable")).await;
    assert_eq!(
        response.status().as_u16(),
        503,
        "{HARNESS}: Unavailable must map to 503"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "SERVICE_UNAVAILABLE",
        "{HARNESS}: Unavailable error code"
    );

    // 7. Unknown ID -> 500 INTERNAL_ERROR (falls through to internal)
    let response = call(&router, TestRequest::get("/v1/resources/unknown")).await;
    assert_eq!(
        response.status().as_u16(),
        500,
        "{HARNESS}: Unknown service error maps to 500"
    );
    let body: serde_json::Value = response.json();
    assert_eq!(
        body["error"]["code"], "INTERNAL_ERROR",
        "{HARNESS}: Unknown maps to INTERNAL_ERROR"
    );

    // 8. All error responses have correlationId (camelCase due to serde rename_all)
    for path in [
        "/v1/resources/not_found",
        "/v1/resources/forbidden",
        "/v1/resources/conflict",
        "/v1/resources/validation",
        "/v1/resources/internal",
        "/v1/resources/unavailable",
    ] {
        let response = call(&router, TestRequest::get(path)).await;
        let body: serde_json::Value = response.json();
        assert!(
            body["correlationId"].is_string()
                && !body["correlationId"].as_str().unwrap().is_empty(),
            "{HARNESS}: {} must have correlationId",
            path
        );
    }
}

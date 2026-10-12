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
//! Level: L3 Composition — tests the production `ApiError::from` conversions via IntoResponse.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test api_route_contract__007__service_error_mapping

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use services::{AuthorizationDecision, ServiceDispatchError, ServiceFailureClass, ServiceRuntimeError};
use web::api::ApiError;
use web::service_support::CoreServiceError;

const HARNESS: &str = "ApiErrorConversion/L3 Composition";

fn deny_decision() -> AuthorizationDecision {
    AuthorizationDecision {
        allowed: false,
        reason: "test denial".to_string(),
        policy_id: "test-policy".to_string(),
        mode: "test",
    }
}

async fn check_error_response(
    api_error: ApiError,
    expected_status: StatusCode,
    expected_code: &str,
    check_correlation: bool,
) {
    let response: Response<Body> = api_error.into_response();
    assert_eq!(
        response.status(),
        expected_status,
        "{HARNESS}: Expected status {}, got {}",
        expected_status.as_u16(),
        response.status().as_u16()
    );

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        body["error"]["code"], expected_code,
        "{HARNESS}: Expected error code {}, got {}",
        expected_code, body["error"]["code"]
    );
    if check_correlation {
        assert!(
            body["correlationId"].is_string() && !body["correlationId"].as_str().unwrap().is_empty(),
            "{HARNESS}: Response must have correlationId"
        );
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-API-ROUTE-CONTRACT-007); the file and the assay use it.
async fn api_route_contract_007__service_error_mapping() {
    // 1. CoreServiceError::Business with NOT_FOUND code -> 404 NOT_FOUND
    let error = CoreServiceError::business("RESOURCE_NOT_FOUND", "Resource not found");
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND", false).await;

    // 2. CoreServiceError::Business with CONFLICT code -> 409 CONFLICT
    let error = CoreServiceError::business("RESOURCE_CONFLICT", "Duplicate resource");
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::CONFLICT, "RESOURCE_CONFLICT", false).await;

    // 3. CoreServiceError::Business with other code -> 400 BAD_REQUEST (default for Business)
    let error = CoreServiceError::business("VALIDATION_FAILED", "Invalid input");
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::BAD_REQUEST, "VALIDATION_FAILED", false).await;

    // 4. CoreServiceError::Runtime::Forbidden -> 403 FORBIDDEN
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Forbidden {
        reason: "Insufficient entitlements".to_string(),
        decision: deny_decision(),
    });
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::FORBIDDEN, "FORBIDDEN", false).await;

    // 5. CoreServiceError::Runtime::Router with FORBIDDEN code -> 403 FORBIDDEN
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Router {
        code: "FORBIDDEN".to_string(),
        message: "Access denied".to_string(),
        retryable: false,
        class: ServiceFailureClass::Business,
    });
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::FORBIDDEN, "FORBIDDEN", false).await;

    // 6. CoreServiceError::Runtime::Router with _NOT_FOUND code -> 404 NOT_FOUND
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Router {
        code: "RESOURCE_NOT_FOUND".to_string(),
        message: "Not found".to_string(),
        retryable: false,
        class: ServiceFailureClass::Business,
    });
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND", false).await;

    // 7. CoreServiceError::Runtime::Router with CONFLICT code -> 409 CONFLICT
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Router {
        code: "RESOURCE_CONFLICT".to_string(),
        message: "Conflict".to_string(),
        retryable: false,
        class: ServiceFailureClass::Business,
    });
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::CONFLICT, "RESOURCE_CONFLICT", false).await;

    // 8. CoreServiceError::Runtime::Router with other code -> 503 SERVICE_UNAVAILABLE
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Router {
        code: "EXTERNAL_API_FAILED".to_string(),
        message: "External API failed".to_string(),
        retryable: true,
        class: ServiceFailureClass::Infrastructure,
    });
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "EXTERNAL_API_FAILED", false).await;

    // 9. CoreServiceError::Runtime::Authorization -> 503 SERVICE_UNAVAILABLE (AUTHORIZATION_UNAVAILABLE)
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Authorization("Auth service down".to_string()));
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "AUTHORIZATION_UNAVAILABLE", false).await;

    // 10. CoreServiceError::Runtime::Audit -> 503 SERVICE_UNAVAILABLE (AUDIT_UNAVAILABLE)
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Audit("Audit service down".to_string()));
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "AUDIT_UNAVAILABLE", false).await;

    // 11. CoreServiceError::Runtime::Event -> 503 SERVICE_UNAVAILABLE (DOMAIN_EVENT_UNAVAILABLE)
    let error = CoreServiceError::Runtime(ServiceRuntimeError::Event("Event service down".to_string()));
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "DOMAIN_EVENT_UNAVAILABLE", false).await;

    // 12. ServiceDispatchError::ServiceNotFound -> 404 SERVICE_NOT_FOUND
    let error = ServiceDispatchError::ServiceNotFound("test-service".to_string());
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::NOT_FOUND, "SERVICE_NOT_FOUND", false).await;

    // 13. ServiceDispatchError::UnknownOperation -> 404 UNKNOWN_OPERATION
    let error = ServiceDispatchError::UnknownOperation {
        domain: "test".to_string(),
        operation: "op".to_string(),
    };
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::NOT_FOUND, "UNKNOWN_OPERATION", false).await;

    // 14. ServiceDispatchError::InvalidPayload -> 400 INVALID_SERVICE_PAYLOAD
    let error = ServiceDispatchError::InvalidPayload {
        domain: "test".to_string(),
        operation: "op".to_string(),
        message: "Invalid payload".to_string(),
    };
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::BAD_REQUEST, "INVALID_SERVICE_PAYLOAD", false).await;

    // 15. ServiceDispatchError::Operation with Caller/Business class -> 400 BAD_REQUEST
    let error = ServiceDispatchError::Operation {
        code: "VALIDATION_ERROR".to_string(),
        message: "Validation failed".to_string(),
        retryable: false,
        class: ServiceFailureClass::Caller,
    };
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::BAD_REQUEST, "VALIDATION_ERROR", false).await;

    // 16. ServiceDispatchError::Operation with Lifecycle class -> 503 SERVICE_UNAVAILABLE
    let error = ServiceDispatchError::Operation {
        code: "WORKFLOW_ERROR".to_string(),
        message: "Workflow failed".to_string(),
        retryable: true,
        class: ServiceFailureClass::Lifecycle,
    };
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "WORKFLOW_ERROR", false).await;

    // 17. ServiceDispatchError::Operation with Infrastructure class -> 500 INTERNAL_SERVER_ERROR
    let error = ServiceDispatchError::Operation {
        code: "DATABASE".to_string(),
        message: "Database error".to_string(),
        retryable: true,
        class: ServiceFailureClass::Infrastructure,
    };
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::INTERNAL_SERVER_ERROR, "DATABASE", false).await;

    // 18. ServiceDispatchError::ServiceDraining -> 503 SERVICE_DRAINING
    let error = ServiceDispatchError::ServiceDraining("test-service".to_string());
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "SERVICE_DRAINING", false).await;

    // 19. ServiceDispatchError::ServiceStopped -> 503 SERVICE_STOPPED
    let error = ServiceDispatchError::ServiceStopped("test-service".to_string());
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::SERVICE_UNAVAILABLE, "SERVICE_STOPPED", false).await;

    // 20. ServiceDispatchError::OperationPanicked -> 500 INTERNAL_SERVER_ERROR
    let error = ServiceDispatchError::OperationPanicked {
        domain: "test".to_string(),
        operation: "op".to_string(),
        message: "panic".to_string(),
    };
    let api_error: ApiError = error.into();
    check_error_response(api_error, StatusCode::INTERNAL_SERVER_ERROR, "SERVICE_OPERATION_PANICKED", false).await;

    // 21. Correlation ID support
    let error = CoreServiceError::business("TEST_ERROR", "Test message");
    let mut api_error: ApiError = error.into();
    api_error = api_error.with_correlation("test-correlation-id");
    let response: Response<Body> = api_error.into_response();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        body["correlationId"], "test-correlation-id",
        "{HARNESS}: with_correlation must set correlation_id"
    );
}
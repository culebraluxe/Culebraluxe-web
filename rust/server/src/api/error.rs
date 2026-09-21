use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use db::{DbFailure, DbFailureKind};
use serde::Serialize;
use service::ServiceRuntimeError;

use crate::projects::ProjectServiceError;
use crate::service_support::CoreServiceError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    ok: bool,
    error: ApiErrorPayload,
    correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorPayload {
    code: String,
    message: String,
    retryable: bool,
    incident_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
    retryable: bool,
    incident_id: Option<String>,
    correlation_id: Option<String>,
}

impl ApiError {
    pub fn new(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            retryable,
            incident_id: None,
            correlation_id: None,
        }
    }

    pub fn unauthorized(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message, false)
    }

    pub fn forbidden(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message, false)
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message, false)
    }

    pub fn with_correlation(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    pub(crate) fn from_db(error: DbFailure) -> Self {
        let status = match error.kind {
            DbFailureKind::DatabaseUnavailable | DbFailureKind::Timeout => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            DbFailureKind::Constraint => StatusCode::CONFLICT,
            DbFailureKind::SchemaMismatch | DbFailureKind::Unknown => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        Self {
            status,
            code: match error.kind {
                DbFailureKind::DatabaseUnavailable => "DATABASE_UNAVAILABLE",
                DbFailureKind::SchemaMismatch => "SCHEMA_MISMATCH",
                DbFailureKind::Constraint => "CONSTRAINT",
                DbFailureKind::Timeout => "TIMEOUT",
                DbFailureKind::Unknown => "DATABASE",
            }
            .into(),
            message: "Database operation failed.".into(),
            retryable: error.retryable,
            incident_id: Some(error.incident_id.to_string()),
            correlation_id: None,
        }
    }

    fn from_runtime(error: ServiceRuntimeError) -> Self {
        match error {
            ServiceRuntimeError::Forbidden { reason, .. } => {
                Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", reason, false)
            }
            ServiceRuntimeError::Authorization(message) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "AUTHORIZATION_UNAVAILABLE",
                message,
                true,
            ),
            ServiceRuntimeError::Audit(message) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "AUDIT_UNAVAILABLE",
                message,
                true,
            ),
            ServiceRuntimeError::Event(message) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "DOMAIN_EVENT_UNAVAILABLE",
                message,
                true,
            ),
        }
    }
}

impl From<CoreServiceError> for ApiError {
    fn from(error: CoreServiceError) -> Self {
        match error {
            CoreServiceError::Business { code, message } => {
                let status = if code.ends_with("_NOT_FOUND") {
                    StatusCode::NOT_FOUND
                } else if code.contains("CONFLICT") {
                    StatusCode::CONFLICT
                } else {
                    StatusCode::BAD_REQUEST
                };
                Self::new(status, code, message, false)
            }
            CoreServiceError::Database(error) => Self::from_db(error),
            CoreServiceError::Runtime(error) => Self::from_runtime(error),
        }
    }
}

impl From<ProjectServiceError> for ApiError {
    fn from(error: ProjectServiceError) -> Self {
        match error {
            ProjectServiceError::Validation { code, message } => {
                Self::new(StatusCode::BAD_REQUEST, code, message, false)
            }
            ProjectServiceError::NotFound(id) => {
                Self::not_found("PROJECT_NOT_FOUND", format!("Project not found: {id}"))
            }
            ProjectServiceError::Database(error) => Self::from_db(error),
            ProjectServiceError::Runtime(error) => Self::from_runtime(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // THE ONE CHOKE POINT FOR RUST API FAILURES.
        //
        // Every route error becomes a response here, so this is where an uncaptured 5xx can be caught. A `DbFailure`
        // arrives with an `incident_id` because the database layer already announced it; anything else has no incident
        // and no other way of being recorded, which is how a failing Rust route could return 500s that left no trace
        // anywhere. 4xx is not captured on purpose: a validation failure or a missing record is audited control flow,
        // not error noise, which is the same rule the TypeScript side follows.
        if self.status.is_server_error() && self.incident_id.is_none() {
            crate::api::error_capture::record(
                "rust:api",
                &self.code,
                &self.message,
                "error",
                None,
                serde_json::json!({
                    "status": self.status.as_u16(),
                    "code": self.code,
                    "correlationId": self.correlation_id,
                    "source": "rust",
                }),
            );
        }
        let body = ApiErrorBody {
            ok: false,
            error: ApiErrorPayload {
                code: self.code,
                message: self.message,
                retryable: self.retryable,
                incident_id: self.incident_id,
            },
            correlation_id: self.correlation_id,
        };
        (self.status, Json(body)).into_response()
    }
}

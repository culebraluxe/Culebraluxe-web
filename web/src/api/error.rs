use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use db::{DbFailure, DbFailureKind};
use serde::Serialize;
use services::{ServiceDispatchError, ServiceRuntimeError};

use crate::command_runtime::CommandDispatchError;
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

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message, false)
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
            ServiceRuntimeError::Router {
                code,
                message,
                retryable,
                ..
            } => {
                let status = match code.as_str() {
                    "FORBIDDEN" => StatusCode::FORBIDDEN,
                    _ if code.ends_with("_NOT_FOUND") => StatusCode::NOT_FOUND,
                    _ if code.contains("CONFLICT") => StatusCode::CONFLICT,
                    _ => StatusCode::SERVICE_UNAVAILABLE,
                };
                Self::new(status, code, message, retryable)
            }
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

impl From<ServiceDispatchError> for ApiError {
    fn from(error: ServiceDispatchError) -> Self {
        match error {
            ServiceDispatchError::ServiceNotFound(domain) => {
                Self::not_found("SERVICE_NOT_FOUND", format!("Service not found: {domain}"))
            }
            ServiceDispatchError::UnknownOperation { domain, operation } => Self::not_found(
                "UNKNOWN_OPERATION",
                format!("Unknown service operation: {domain}.{operation}"),
            ),
            ServiceDispatchError::InvalidPayload { message, .. } => Self::new(
                StatusCode::BAD_REQUEST,
                "INVALID_SERVICE_PAYLOAD",
                message,
                false,
            ),
            ServiceDispatchError::Operation {
                code,
                message,
                retryable,
                class,
            } => {
                let status = match class {
                    services::ServiceFailureClass::Caller
                    | services::ServiceFailureClass::Business => StatusCode::BAD_REQUEST,
                    services::ServiceFailureClass::Lifecycle => StatusCode::SERVICE_UNAVAILABLE,
                    services::ServiceFailureClass::Infrastructure
                    | services::ServiceFailureClass::Panic => StatusCode::INTERNAL_SERVER_ERROR,
                };
                let mut out = Self::new(status, code.clone(), message.clone(), retryable);
                // The envelope path (`service_gateway::core_error`, `email::service_error`,
                // `luxesign`/`signer` equivalents) converts `DbFailure` into a plain
                // `infrastructure("DATABASE", ...)` error, dropping the incident id the
                // database layer already announced. Recover it from the message so the
                // response carries the original incident and the choke point below does
                // not write a second `rust:api` row for it. `DbFailure`'s `Display`
                // always renders `(incident <uuid>)`.
                if code == "DATABASE" {
                    out.incident_id = database_incident(&message);
                }
                out
            }
            ServiceDispatchError::ServiceDraining(domain) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_DRAINING",
                format!("Service is draining: {domain}"),
                true,
            ),
            ServiceDispatchError::ServiceStopped(domain) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_STOPPED",
                format!("Service is stopped: {domain}"),
                true,
            ),
            ServiceDispatchError::OperationPanicked {
                domain,
                operation,
                message,
            } => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SERVICE_OPERATION_PANICKED",
                format!("{domain}.{operation}: {message}"),
                true,
            ),
        }
    }
}

impl From<CommandDispatchError> for ApiError {
    fn from(error: CommandDispatchError) -> Self {
        match error {
            CommandDispatchError::Database(error) => Self::from_db(error),
            CommandDispatchError::Service(error) => Self::from(error),
            CommandDispatchError::Scheduler(error) => match error {
                ServiceDispatchError::ServiceNotFound(domain) => {
                    Self::not_found("SERVICE_NOT_FOUND", format!("Service not found: {domain}"))
                }
                ServiceDispatchError::UnknownOperation { domain, operation } => Self::not_found(
                    "UNKNOWN_OPERATION",
                    format!("Unknown service operation: {domain}.{operation}"),
                ),
                ServiceDispatchError::InvalidPayload { message, .. } => Self::new(
                    StatusCode::BAD_REQUEST,
                    "INVALID_SERVICE_PAYLOAD",
                    message,
                    false,
                ),
                ServiceDispatchError::Operation {
                    code,
                    message,
                    retryable,
                    class,
                } => {
                    let status = match class {
                        services::ServiceFailureClass::Caller
                        | services::ServiceFailureClass::Business => StatusCode::BAD_REQUEST,
                        services::ServiceFailureClass::Lifecycle => StatusCode::SERVICE_UNAVAILABLE,
                        services::ServiceFailureClass::Infrastructure
                        | services::ServiceFailureClass::Panic => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    let mut out = Self::new(status, code.clone(), message.clone(), retryable);
                    if code == "DATABASE" {
                        out.incident_id = database_incident(&message);
                    }
                    out
                }
                ServiceDispatchError::ServiceDraining(domain) => Self::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "SERVICE_DRAINING",
                    format!("Service is draining: {domain}"),
                    true,
                ),
                ServiceDispatchError::ServiceStopped(domain) => Self::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "SERVICE_STOPPED",
                    format!("Service is stopped: {domain}"),
                    true,
                ),
                ServiceDispatchError::OperationPanicked {
                    domain,
                    operation,
                    message,
                } => Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "SERVICE_OPERATION_PANICKED",
                    format!("{domain}.{operation}: {message}"),
                    true,
                ),
            },
            CommandDispatchError::Serialization(message) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "COMMAND_SERIALIZATION_FAILED",
                message,
                false,
            ),
            CommandDispatchError::Registry(message) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "COMMAND_REGISTRY_INVARIANT",
                message,
                false,
            ),
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

/// Recover the database incident id from an envelope-converted `DATABASE`
/// message. `DbFailure`'s `Display` always renders `(incident <uuid>)`, and
/// the envelope path stringifies it, so the uuid is in the text. Returns
/// `None` when the marker is absent rather than failing the conversion.
fn database_incident(message: &str) -> Option<String> {
    let start = message.find("(incident ")? + "(incident ".len();
    let rest = &message[start..];
    let end = rest.find(')')?;
    let id = &rest[..end];
    if id.is_empty() {
        return None;
    }
    Some(id.to_owned())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_envelope_error_keeps_incident() {
        let message =
            "Unknown during db.run_text (incident 11111111-1111-4111-8111-111111111111): boom";
        let error = ServiceDispatchError::infrastructure("DATABASE", message, true);
        let api = ApiError::from(error);
        assert_eq!(
            api.incident_id.as_deref(),
            Some("11111111-1111-4111-8111-111111111111")
        );
    }

    #[test]
    fn non_database_error_has_no_incident() {
        let error = ServiceDispatchError::infrastructure("MUX_API", "mux down", true);
        let api = ApiError::from(error);
        assert_eq!(api.incident_id, None);
    }

    #[test]
    fn database_message_without_marker_has_no_incident() {
        let error = ServiceDispatchError::infrastructure("DATABASE", "plain", true);
        let api = ApiError::from(error);
        assert_eq!(api.incident_id, None);
    }
}

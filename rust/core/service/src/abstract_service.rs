use crate::{OperationKind, ServiceContext, ServiceExecutionPolicy};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEnvelope {
    pub domain: String,
    pub operation: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCapability {
    pub name: String,
    pub kind: OperationKind,
    pub description: String,
    pub authorization: String,
    pub idempotent: bool,
    pub execution: ServiceExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDescriptor {
    pub domain: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<ServiceCapability>,
    pub dependencies: Vec<String>,
    pub invariants: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceFailureClass {
    Caller,
    Business,
    Infrastructure,
    Lifecycle,
    Panic,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ServiceDispatchError {
    #[error("service not found: {0}")]
    ServiceNotFound(String),
    #[error("unknown operation {domain}.{operation}")]
    UnknownOperation { domain: String, operation: String },
    #[error("invalid payload for {domain}.{operation}: {message}")]
    InvalidPayload {
        domain: String,
        operation: String,
        message: String,
    },
    #[error("{code}: {message}")]
    Operation {
        code: String,
        message: String,
        retryable: bool,
        class: ServiceFailureClass,
    },
    #[error("service is draining: {0}")]
    ServiceDraining(String),
    #[error("service is stopped: {0}")]
    ServiceStopped(String),
    #[error("service operation panicked: {domain}.{operation}: {message}")]
    OperationPanicked {
        domain: String,
        operation: String,
        message: String,
    },
}

impl ServiceDispatchError {
    pub fn operation(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self::infrastructure(code, message, retryable)
    }

    pub fn business(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self::Operation {
            code: code.into(),
            message: message.into(),
            retryable,
            class: ServiceFailureClass::Business,
        }
    }

    pub fn infrastructure(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::Operation {
            code: code.into(),
            message: message.into(),
            retryable,
            class: ServiceFailureClass::Infrastructure,
        }
    }

    pub fn failure_class(&self) -> ServiceFailureClass {
        match self {
            Self::ServiceNotFound(_)
            | Self::UnknownOperation { .. }
            | Self::InvalidPayload { .. } => ServiceFailureClass::Caller,
            Self::Operation { class, .. } => *class,
            Self::ServiceDraining(_) | Self::ServiceStopped(_) => ServiceFailureClass::Lifecycle,
            Self::OperationPanicked { .. } => ServiceFailureClass::Panic,
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::ServiceNotFound(_) => "SERVICE_NOT_FOUND",
            Self::UnknownOperation { .. } => "UNKNOWN_OPERATION",
            Self::InvalidPayload { .. } => "INVALID_SERVICE_PAYLOAD",
            Self::Operation { code, .. } => code.as_str(),
            Self::ServiceDraining(_) => "SERVICE_DRAINING",
            Self::ServiceStopped(_) => "SERVICE_STOPPED",
            Self::OperationPanicked { .. } => "SERVICE_OPERATION_PANICKED",
        }
    }

    pub fn retryable(&self) -> bool {
        match self {
            Self::Operation { retryable, .. } => *retryable,
            Self::ServiceDraining(_) | Self::ServiceStopped(_) | Self::OperationPanicked { .. } => {
                true
            }
            _ => false,
        }
    }
}

#[async_trait]
pub trait AbstractService: Send + Sync {
    fn descriptor(&self) -> ServiceDescriptor;

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError>;
}

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
        Self::Operation {
            code: code.into(),
            message: message.into(),
            retryable,
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

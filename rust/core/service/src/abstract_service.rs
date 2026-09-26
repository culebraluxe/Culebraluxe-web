use crate::{OperationKind, ServiceContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Transport-neutral service call.
///
/// The caller names only the service domain, operation, and DTO payload.
/// Identity, authorization context, correlation and causation are supplied by
/// the trusted edge and are therefore deliberately NOT client-controlled fields.
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
    pub authorization: String,
    pub idempotent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDescriptor {
    pub domain: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<ServiceCapability>,
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

/// Rust descendant of the original CulebraLuxe BaseService abstraction.
///
/// Concrete services keep their strongly typed methods. This trait is the
/// universal, transport-neutral ingress used by registries, UI effects and
/// adapters that cannot statically name a concrete service type.
#[async_trait]
pub trait AbstractService: Send {
    fn descriptor(&self) -> ServiceDescriptor;

    async fn dispatch(
        &mut self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError>;
}

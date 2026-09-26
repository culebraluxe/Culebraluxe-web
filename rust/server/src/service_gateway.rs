use crate::composition::CoreServices;
use crate::contracts::ContractService;
use crate::properties::PropertyService;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use db::{ContractDao, PropertyDao};
use domain::PropertyAdminPageRequest;
use serde_json::{json, Value};
use service::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope,
};

/// Registry-backed application service ingress.
///
/// Screens, transports and background adapters do not construct repositories
/// or pick concrete service types. They send a ServiceEnvelope here. The
/// gateway resolves the owning AbstractService; the service still owns
/// authorization, invariants, audit, events and persistence.
#[derive(Clone)]
pub struct ServiceGateway {
    services: CoreServices,
}

impl ServiceGateway {
    pub fn new(services: CoreServices) -> Self {
        Self { services }
    }

    pub fn descriptors(&self) -> Vec<ServiceDescriptor> {
        let contract = self.services.contract();
        let property = self.services.property();
        let mut descriptors = vec![contract.descriptor(), property.descriptor()];
        descriptors.sort_by(|a, b| a.domain.cmp(&b.domain));
        descriptors
    }

    pub async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.domain.as_str() {
            "contract" => {
                let mut service = self.services.contract();
                service.dispatch(envelope, context).await
            }
            "property" => {
                let mut service = self.services.property();
                service.dispatch(envelope, context).await
            }
            other => Err(ServiceDispatchError::ServiceNotFound(other.to_owned())),
        }
    }
}

fn capability(
    name: &str,
    kind: OperationKind,
    authorization: &str,
    idempotent: bool,
) -> ServiceCapability {
    ServiceCapability {
        name: name.to_owned(),
        kind,
        authorization: authorization.to_owned(),
        idempotent,
    }
}

fn payload_string(envelope: &ServiceEnvelope, field: &str) -> Result<String, ServiceDispatchError> {
    envelope
        .payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ServiceDispatchError::InvalidPayload {
            domain: envelope.domain.clone(),
            operation: envelope.operation.clone(),
            message: format!("missing non-empty string field '{field}'"),
        })
}

fn payload_i64(envelope: &ServiceEnvelope, field: &str, default: i64) -> i64 {
    envelope
        .payload
        .get(field)
        .and_then(Value::as_i64)
        .unwrap_or(default)
}

fn core_error(error: CoreServiceError) -> ServiceDispatchError {
    match error {
        CoreServiceError::Business { code, message } => {
            ServiceDispatchError::operation(code, message, false)
        }
        CoreServiceError::Database(error) => ServiceDispatchError::operation(
            "DATABASE",
            "Database operation failed.",
            error.retryable,
        ),
        CoreServiceError::Runtime(error) => ServiceDispatchError::operation(
            match &error {
                service::ServiceRuntimeError::Authorization(_) => "AUTHORIZATION_UNAVAILABLE",
                service::ServiceRuntimeError::Forbidden { .. } => "FORBIDDEN",
                service::ServiceRuntimeError::Audit(_) => "AUDIT_UNAVAILABLE",
                service::ServiceRuntimeError::Event(_) => "DOMAIN_EVENT_UNAVAILABLE",
            },
            error.to_string(),
            !matches!(error, service::ServiceRuntimeError::Forbidden { .. }),
        ),
    }
}

fn encode<T: serde::Serialize>(
    envelope: &ServiceEnvelope,
    value: T,
) -> Result<Value, ServiceDispatchError> {
    serde_json::to_value(value).map_err(|error| ServiceDispatchError::Operation {
        code: "SERVICE_SERIALIZATION_FAILED".into(),
        message: format!(
            "{}.{} response could not be serialized: {error}",
            envelope.domain, envelope.operation
        ),
        retryable: false,
    })
}

#[async_trait]
impl AbstractService for ContractService<ContractDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "contract".into(),
            version: "1".into(),
            description: "Canonical Contract service".into(),
            capabilities: vec![
                capability("contract.list", OperationKind::Query, "contract.read", true),
                capability("contract.get", OperationKind::Query, "contract.read", true),
                capability(
                    "contract.listForProcessInstance",
                    OperationKind::Query,
                    "contract.read",
                    true,
                ),
                capability(
                    "contract.getEffectiveState",
                    OperationKind::Query,
                    "contract.read",
                    true,
                ),
            ],
        }
    }

    async fn dispatch(
        &mut self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "contract.list" => encode(envelope, self.list(context).await.map_err(core_error)?),
            "contract.get" => {
                let id = payload_string(envelope, "contractId")?;
                encode(envelope, self.get(&id, context).await.map_err(core_error)?)
            }
            "contract.listForProcessInstance" => {
                let id = payload_string(envelope, "processInstanceId")?;
                encode(
                    envelope,
                    self.list_for_process_instance(&id, context)
                        .await
                        .map_err(core_error)?,
                )
            }
            "contract.getEffectiveState" => {
                let id = payload_string(envelope, "contractId")?;
                encode(
                    envelope,
                    self.get_effective_state(&id, context)
                        .await
                        .map_err(core_error)?,
                )
            }
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "contract".into(),
                operation: operation.to_owned(),
            }),
        }
    }
}

#[async_trait]
impl AbstractService for PropertyService<PropertyDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "property".into(),
            version: "1".into(),
            description: "Canonical Property service".into(),
            capabilities: vec![
                capability("property.get", OperationKind::Query, "property.read", true),
                capability(
                    "property.forPerson",
                    OperationKind::Query,
                    "property.read",
                    true,
                ),
                capability(
                    "property.adminPage",
                    OperationKind::Query,
                    "property.read",
                    true,
                ),
                capability(
                    "property.adminGet",
                    OperationKind::Query,
                    "property.read",
                    true,
                ),
            ],
        }
    }

    async fn dispatch(
        &mut self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "property.get" => {
                let id = payload_string(envelope, "propertyId")?;
                encode(envelope, self.get(&id, context).await.map_err(core_error)?)
            }
            "property.forPerson" => {
                let id = payload_string(envelope, "personId")?;
                encode(
                    envelope,
                    self.for_person(&id, context).await.map_err(core_error)?,
                )
            }
            "property.adminPage" => {
                let request = PropertyAdminPageRequest {
                    search: envelope
                        .payload
                        .get("search")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    page: payload_i64(envelope, "page", 1).max(1),
                    page_size: payload_i64(envelope, "pageSize", 50).clamp(1, 100),
                };
                encode(
                    envelope,
                    self.admin_page(&request, context)
                        .await
                        .map_err(core_error)?,
                )
            }
            "property.adminGet" => {
                let id = payload_string(envelope, "propertyId")?;
                encode(
                    envelope,
                    self.admin_get(&id, context).await.map_err(core_error)?,
                )
            }
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "property".into(),
                operation: operation.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_envelope_does_not_carry_security_context() {
        let envelope = ServiceEnvelope {
            domain: "contract".into(),
            operation: "contract.get".into(),
            payload: json!({ "contractId": "c1" }),
        };
        let encoded = serde_json::to_value(envelope).unwrap();
        assert!(encoded.get("context").is_none());
        assert!(encoded.get("actor").is_none());
        assert!(encoded.get("principal").is_none());
    }

    #[test]
    fn required_string_payload_rejects_blank_values() {
        let envelope = ServiceEnvelope {
            domain: "property".into(),
            operation: "property.get".into(),
            payload: json!({ "propertyId": "   " }),
        };
        assert!(matches!(
            payload_string(&envelope, "propertyId"),
            Err(ServiceDispatchError::InvalidPayload { .. })
        ));
    }
}

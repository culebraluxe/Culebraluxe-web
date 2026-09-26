use crate::contracts::ContractService;
use crate::firms::FirmService;
use crate::people::PersonService;
use crate::properties::PropertyService;
use crate::service_kernel::ServiceRegistry;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use db::{ContractDao, FirmDao, PersonDao, PropertyDao};
use domain::{PropertyAdminPageRequest, SearchPeopleRequest};
use serde_json::{json, Value};
use service::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy,
};

#[derive(Clone)]
pub struct ServiceGateway {
    registry: std::sync::Arc<ServiceRegistry>,
}

impl ServiceGateway {
    pub fn new(registry: std::sync::Arc<ServiceRegistry>) -> Self {
        Self { registry }
    }

    pub fn descriptors(&self) -> Vec<ServiceDescriptor> {
        self.registry.descriptors()
    }

    pub async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        self.registry.dispatch(envelope, context).await
    }
}

fn capability(
    name: &str,
    kind: OperationKind,
    description: &str,
    authorization: &str,
    idempotent: bool,
) -> ServiceCapability {
    ServiceCapability {
        name: name.to_owned(),
        kind,
        description: description.to_owned(),
        authorization: authorization.to_owned(),
        idempotent,
        execution: ServiceExecutionPolicy::inline(),
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
                service::ServiceRuntimeError::Router { code, .. } => code.as_str(),
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
impl AbstractService for PersonService<PersonDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "person".into(),
            version: "1".into(),
            description: "Canonical Person service".into(),
            capabilities: vec![
                capability(
                    "person.get",
                    OperationKind::Query,
                    "Read one canonical Person.",
                    "person.read",
                    true,
                ),
                capability(
                    "person.search",
                    OperationKind::Query,
                    "Search canonical People.",
                    "person.read",
                    true,
                ),
            ],
            dependencies: vec![],
            invariants: vec!["A canonical identity belongs to at most one Person.".into()],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "person.get" => {
                let id = payload_string(envelope, "personId")?;
                encode(envelope, self.get(&id, context).await.map_err(core_error)?)
            }
            "person.search" => {
                let request = SearchPeopleRequest {
                    query: envelope
                        .payload
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    limit: envelope.payload.get("limit").and_then(Value::as_i64),
                };
                encode(
                    envelope,
                    self.search(&request, context).await.map_err(core_error)?,
                )
            }
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "person".into(),
                operation: operation.to_owned(),
            }),
        }
    }
}

#[async_trait]
impl AbstractService for FirmService<FirmDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "firm".into(),
            version: "1".into(),
            description: "Canonical Firm service".into(),
            capabilities: vec![
                capability(
                    "firm.get",
                    OperationKind::Query,
                    "Read one canonical Firm.",
                    "firm.read",
                    true,
                ),
                capability(
                    "firm.findByName",
                    OperationKind::Query,
                    "Find one canonical Firm by name.",
                    "firm.read",
                    true,
                ),
            ],
            dependencies: vec![],
            invariants: vec![],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "firm.get" => {
                let id = payload_string(envelope, "firmId")?;
                encode(envelope, self.get(&id, context).await.map_err(core_error)?)
            }
            "firm.findByName" => {
                let name = payload_string(envelope, "name")?;
                encode(
                    envelope,
                    self.find_by_name(&name, context)
                        .await
                        .map_err(core_error)?,
                )
            }
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "firm".into(),
                operation: operation.to_owned(),
            }),
        }
    }
}

#[async_trait]
impl AbstractService for ContractService<ContractDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "contract".into(),
            version: "1".into(),
            description: "Canonical Contract service".into(),
            capabilities: vec![
                capability(
                    "contract.list",
                    OperationKind::Query,
                    "List canonical contracts.",
                    "contract.read",
                    true,
                ),
                capability(
                    "contract.get",
                    OperationKind::Query,
                    "Read one canonical contract.",
                    "contract.read",
                    true,
                ),
                capability(
                    "contract.listForProcessInstance",
                    OperationKind::Query,
                    "List contracts attached to one process instance.",
                    "contract.read",
                    true,
                ),
                capability(
                    "contract.getEffectiveState",
                    OperationKind::Query,
                    "Read effective inherited contract state.",
                    "contract.read",
                    true,
                ),
            ],
            dependencies: vec!["person".into(), "firm".into(), "property".into()],
            invariants: vec![
                "Only draft contracts may execute.".into(),
                "Contract role references must resolve through owning services.".into(),
            ],
        }
    }

    async fn dispatch(
        &self,
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
                capability(
                    "property.get",
                    OperationKind::Query,
                    "Read one canonical property.",
                    "property.read",
                    true,
                ),
                capability(
                    "property.forPerson",
                    OperationKind::Query,
                    "Read properties attached to one Person.",
                    "property.read",
                    true,
                ),
                capability(
                    "property.adminPage",
                    OperationKind::Query,
                    "Read the internal Property administration page.",
                    "property.read",
                    true,
                ),
                capability(
                    "property.adminGet",
                    OperationKind::Query,
                    "Read one internal Property administration record.",
                    "property.read",
                    true,
                ),
            ],
            dependencies: vec![],
            invariants: vec![
                "Archived properties cannot remain publicly published.".into(),
                "Property status and archival timestamp remain consistent.".into(),
            ],
        }
    }

    async fn dispatch(
        &self,
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

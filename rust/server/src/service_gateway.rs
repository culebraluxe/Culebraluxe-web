use crate::calendar::CalendarService;
use crate::clients::ClientService;
use crate::contracts::ContractService;
use crate::firms::FirmService;
use crate::people::PersonService;
use crate::properties::PropertyService;
use crate::security::SecurityService;
use crate::service_kernel::ServiceRegistry;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use db::{CalendarDao, ClientDao, ContractDao, FirmDao, PersonDao, PropertyDao, SecurityDao};
use domain::{
    CalendarViewportQuery, CreateAppleCalendarEventRequest, PropertyAdminPageRequest,
    SearchPeopleRequest, UpdateAppleCalendarEventRequest,
};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use service::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone)]
pub struct ServiceGateway {
    registry: Arc<ServiceRegistry>,
    accepting: Arc<AtomicBool>,
}

impl ServiceGateway {
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self {
            registry,
            accepting: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn descriptors(&self) -> Vec<ServiceDescriptor> {
        self.registry.descriptors()
    }

    pub async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        self.ensure_accepting()?;
        self.registry.dispatch(envelope, context).await
    }

    /// Execute a typed service call through the registered service mailbox.
    ///
    /// HTTP handlers use this while a domain is being migrated to JSON envelope
    /// dispatch. It preserves the typed response while enforcing the same
    /// bounded queue and lifecycle controls as `dispatch`.
    pub async fn execute<T, F>(
        &self,
        domain: &str,
        operation: &str,
        payload: &Value,
        work: F,
    ) -> Result<T, ServiceDispatchError>
    where
        T: Send + 'static,
        F: std::future::Future<Output = T> + Send + 'static,
    {
        self.ensure_accepting()?;
        self.registry
            .run_task(domain, operation, payload, work)
            .await
    }

    pub fn refuse_new_work(&self) {
        self.accepting.store(false, Ordering::Release);
    }

    pub fn ensure_accepting(&self) -> Result<(), ServiceDispatchError> {
        if self.accepting.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(ServiceDispatchError::ServiceDraining("gateway".into()))
        }
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

fn capability_with_execution(
    name: &str,
    kind: OperationKind,
    description: &str,
    authorization: &str,
    idempotent: bool,
    execution: ServiceExecutionPolicy,
) -> ServiceCapability {
    let mut capability = capability(name, kind, description, authorization, idempotent);
    capability.execution = execution;
    capability
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
            ServiceDispatchError::business(code, message, false)
        }
        CoreServiceError::Database(error) => ServiceDispatchError::infrastructure(
            "DATABASE",
            "Database operation failed.",
            error.retryable,
        ),
        CoreServiceError::Runtime(error) => {
            let (code, class, retryable) = match &error {
                service::ServiceRuntimeError::Authorization(_) => (
                    "AUTHORIZATION_UNAVAILABLE",
                    service::ServiceFailureClass::Infrastructure,
                    true,
                ),
                service::ServiceRuntimeError::Forbidden { .. } => {
                    ("FORBIDDEN", service::ServiceFailureClass::Business, false)
                }
                service::ServiceRuntimeError::Audit(_) => (
                    "AUDIT_UNAVAILABLE",
                    service::ServiceFailureClass::Infrastructure,
                    true,
                ),
                service::ServiceRuntimeError::Event(_) => (
                    "DOMAIN_EVENT_UNAVAILABLE",
                    service::ServiceFailureClass::Infrastructure,
                    true,
                ),
                service::ServiceRuntimeError::Router {
                    code,
                    retryable,
                    class,
                    ..
                } => (code.as_str(), *class, *retryable),
            };
            match class {
                service::ServiceFailureClass::Business | service::ServiceFailureClass::Caller => {
                    ServiceDispatchError::business(code, error.to_string(), retryable)
                }
                _ => ServiceDispatchError::infrastructure(code, error.to_string(), retryable),
            }
        }
    }
}

fn encode<T: serde::Serialize>(
    envelope: &ServiceEnvelope,
    value: T,
) -> Result<Value, ServiceDispatchError> {
    serde_json::to_value(value).map_err(|error| {
        ServiceDispatchError::infrastructure(
            "SERVICE_SERIALIZATION_FAILED",
            format!(
                "{}.{} response could not be serialized: {error}",
                envelope.domain, envelope.operation
            ),
            false,
        )
    })
}

#[async_trait]
impl AbstractService for CalendarService<CalendarDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "calendar".into(),
            version: "1".into(),
            description: "Canonical calendar service".into(),
            capabilities: vec![
                capability(
                    "calendar.list",
                    OperationKind::Query,
                    "Read the default bounded calendar window.",
                    "calendar.read",
                    true,
                ),
                capability(
                    "calendar.viewport",
                    OperationKind::Query,
                    "Read calendar events for an explicit viewport.",
                    "calendar.read",
                    true,
                ),
                capability(
                    "calendar.createAppleEvent",
                    OperationKind::Command,
                    "Queue an Apple Calendar event through the trusted macOS edge.",
                    "calendar.write",
                    false,
                ),
                capability(
                    "calendar.updateAppleEvent",
                    OperationKind::Command,
                    "Queue an Apple Calendar move/resize through the trusted macOS edge.",
                    "calendar.write",
                    false,
                ),
            ],
            dependencies: vec![],
            invariants: vec![
                "Calendar viewport reads are range-bounded.".into(),
                "Apple Calendar writes cross the outbox/edge boundary; the web process never writes EventKit directly.".into(),
            ],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "calendar.list" => encode(envelope, self.list(context).await.map_err(core_error)?),
            "calendar.viewport" => {
                let request: CalendarViewportQuery =
                    serde_json::from_value(envelope.payload.clone()).map_err(|error| {
                        ServiceDispatchError::InvalidPayload {
                            domain: envelope.domain.clone(),
                            operation: envelope.operation.clone(),
                            message: error.to_string(),
                        }
                    })?;
                encode(
                    envelope,
                    self.viewport(&request, context).await.map_err(core_error)?,
                )
            }
            "calendar.updateAppleEvent" => {
                let request: UpdateAppleCalendarEventRequest =
                    serde_json::from_value(envelope.payload.clone()).map_err(|error| {
                        ServiceDispatchError::InvalidPayload {
                            domain: envelope.domain.clone(),
                            operation: envelope.operation.clone(),
                            message: error.to_string(),
                        }
                    })?;
                encode(
                    envelope,
                    self.update_apple_event(&request, context)
                        .await
                        .map_err(core_error)?,
                )
            }
            "calendar.createAppleEvent" => {
                let request: CreateAppleCalendarEventRequest =
                    serde_json::from_value(envelope.payload.clone()).map_err(|error| {
                        ServiceDispatchError::InvalidPayload {
                            domain: envelope.domain.clone(),
                            operation: envelope.operation.clone(),
                            message: error.to_string(),
                        }
                    })?;
                encode(
                    envelope,
                    self.create_apple_event(&request, context)
                        .await
                        .map_err(core_error)?,
                )
            }
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "calendar".into(),
                operation: operation.to_owned(),
            }),
        }
    }
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
                capability_with_execution(
                    "contract.execute",
                    OperationKind::Command,
                    "Execute a canonical Contract through the durable command runtime.",
                    "contract.execute",
                    true,
                    ServiceExecutionPolicy::ordered("contractId"),
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
            "contract.execute" => Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                "contract.execute must enter through the durable command dispatcher.",
                false,
            )),
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

    async fn warm_cache(&self) -> Result<usize, ServiceDispatchError> {
        self.warm_read_cache().await.map_err(core_error)
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

#[async_trait]
impl AbstractService for ClientService<ClientDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "client".into(),
            version: "1".into(),
            description: "Canonical Client service".into(),
            capabilities: vec![
                capability(
                    "client.directory",
                    OperationKind::Query,
                    "Read the client directory.",
                    "person.read",
                    true,
                ),
                capability(
                    "client.detail",
                    OperationKind::Query,
                    "Read one client workspace.",
                    "person.read",
                    true,
                ),
            ],
            dependencies: vec![],
            invariants: vec!["Client reads use the startup-warmed canonical directory.".into()],
        }
    }

    async fn warm_cache(&self) -> Result<usize, ServiceDispatchError> {
        self.warm_read_cache()
            .await
            .map(|(clients, evidence)| clients.saturating_add(evidence))
            .map_err(core_error)
    }
}

#[async_trait]
impl AbstractService for SecurityService<SecurityDao> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "security".into(),
            version: "1".into(),
            description: "Canonical Security service".into(),
            capabilities: vec![capability(
                "security.resolveIdentity",
                OperationKind::Query,
                "Resolve an authenticated edge identity from the startup-warmed principal map.",
                "security.identity.resolve",
                true,
            )],
            dependencies: vec![],
            invariants: vec![
                "Authenticated principals are loaded before traffic is accepted.".into(),
            ],
        }
    }

    async fn warm_cache(&self) -> Result<usize, ServiceDispatchError> {
        self.warm_identity_cache().await.map_err(core_error)
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

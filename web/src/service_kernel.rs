use crate::composition::ServiceCatalog;
use crate::contracts::ContractService;
use crate::firms::FirmService;
use crate::people::PersonService;
use crate::properties::PropertyService;
use async_trait::async_trait;
use db::{ContractDao, Database, FirmDao, PersonDao, PropertyDao};
use serde::{Deserialize, Serialize};
use services::{
    AbstractService, DeferredServiceRouter, OperationKind, ServiceContext, ServiceControlCommand,
    ServiceControlResult, ServiceDescriptor, ServiceDispatchError, ServiceEnvelope, ServiceHealth,
    ServiceInfrastructure, ServiceLifecycle, ServiceMailbox, ServiceMailboxConfig, ServiceRouter,
    ServiceRuntime, ServiceStatus,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

#[derive(Clone)]
struct RegisteredService {
    descriptor: ServiceDescriptor,
    service: Arc<dyn AbstractService>,
    mailbox: ServiceMailbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKernelHealth {
    pub status: ServiceStatus,
    pub accepting: bool,
    pub queued: usize,
    pub in_flight: usize,
    pub service_count: usize,
    pub services: BTreeMap<String, ServiceHealth>,
}

#[derive(Clone)]
pub struct ServiceRegistry {
    root_cancel: CancellationToken,
    entries: Arc<HashMap<String, RegisteredService>>,
    observer: ServiceRuntime,
    startup_order: Arc<Vec<String>>,
    shutdown_order: Arc<Vec<String>>,
}

impl ServiceRegistry {
    fn new(
        root_cancel: CancellationToken,
        services: Vec<Arc<dyn AbstractService>>,
        infrastructure: ServiceInfrastructure,
    ) -> Result<Self, ServiceDispatchError> {
        let descriptors = services
            .iter()
            .map(|service| service.descriptor())
            .collect::<Vec<_>>();
        let startup_order = validate_service_graph(&descriptors)?;
        let shutdown_order = startup_order.iter().rev().cloned().collect::<Vec<_>>();
        let mut entries = HashMap::new();

        for service in services {
            let descriptor = service.descriptor();
            let domain = descriptor.domain.clone();
            if entries.contains_key(&domain) {
                return Err(ServiceDispatchError::infrastructure(
                    "SERVICE_ALREADY_REGISTERED",
                    format!("Service already registered for domain: {domain}"),
                    false,
                ));
            }
            let mailbox = ServiceMailbox::spawn(
                Arc::<str>::from(domain.as_str()),
                ServiceMailboxConfig::default(),
                &root_cancel,
            )?;
            entries.insert(
                domain,
                RegisteredService {
                    descriptor,
                    service,
                    mailbox,
                },
            );
        }

        Ok(Self {
            root_cancel,
            entries: Arc::new(entries),
            observer: ServiceRuntime::new(infrastructure),
            startup_order: Arc::new(startup_order),
            shutdown_order: Arc::new(shutdown_order),
        })
    }

    pub fn descriptors(&self) -> Vec<ServiceDescriptor> {
        let mut values = self
            .entries
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect::<Vec<_>>();
        values.sort_by(|a, b| a.domain.cmp(&b.domain));
        values
    }

    pub fn health(&self) -> BTreeMap<String, ServiceHealth> {
        self.entries
            .iter()
            .map(|(domain, entry)| (domain.clone(), entry.mailbox.health()))
            .collect()
    }

    pub fn kernel_health(&self) -> ServiceKernelHealth {
        let services = self.health();
        let status = if services
            .values()
            .any(|health| health.status == ServiceStatus::Failed)
        {
            ServiceStatus::Failed
        } else if services
            .values()
            .any(|health| health.status == ServiceStatus::Stopping)
        {
            ServiceStatus::Stopping
        } else if services
            .values()
            .any(|health| health.status == ServiceStatus::Draining)
        {
            ServiceStatus::Draining
        } else if services
            .values()
            .any(|health| health.status == ServiceStatus::Starting)
        {
            ServiceStatus::Starting
        } else if !services.is_empty()
            && services
                .values()
                .all(|health| health.status == ServiceStatus::Stopped)
        {
            ServiceStatus::Stopped
        } else {
            ServiceStatus::Running
        };
        ServiceKernelHealth {
            status,
            accepting: services.values().all(|health| health.accepting),
            queued: services.values().map(|health| health.queued).sum(),
            in_flight: services.values().map(|health| health.in_flight).sum(),
            service_count: services.len(),
            services,
        }
    }

    pub async fn start(&self) -> Result<(), ServiceDispatchError> {
        for domain in self.startup_order.iter() {
            let entry = &self.entries[domain];
            entry.mailbox.wait_running().await?;
            let entries = entry.service.warm_cache().await?;
            tracing::info!(
                target: "culebraluxe::services::cache",
                domain,
                entries,
                "service read cache warmed"
            );
        }
        Ok(())
    }

    pub async fn control(
        &self,
        domain: &str,
        command: ServiceControlCommand,
    ) -> Result<ServiceControlResult, ServiceDispatchError> {
        let entry = self
            .entries
            .get(domain)
            .ok_or_else(|| ServiceDispatchError::ServiceNotFound(domain.to_owned()))?;

        match command {
            ServiceControlCommand::Start => entry.mailbox.start().await.map_err(|error| {
                ServiceDispatchError::infrastructure(error.code, error.message, false)
            })?,
            ServiceControlCommand::Drain => entry.mailbox.drain().await?,
            ServiceControlCommand::Stop => entry.mailbox.stop().await?,
            ServiceControlCommand::Status | ServiceControlCommand::Health => {}
        }

        Ok(ServiceControlResult {
            domain: domain.to_owned(),
            command,
            status: entry.mailbox.status(),
            health: entry.mailbox.health(),
        })
    }

    pub async fn run_task<T, F>(
        &self,
        domain: &str,
        operation: &str,
        payload: &serde_json::Value,
        work: F,
    ) -> Result<T, ServiceDispatchError>
    where
        T: Send + 'static,
        F: std::future::Future<Output = T> + Send + 'static,
    {
        let entry = self
            .entries
            .get(domain)
            .ok_or_else(|| ServiceDispatchError::ServiceNotFound(domain.to_owned()))?;
        let execution = match entry
            .descriptor
            .capabilities
            .iter()
            .find(|capability| capability.name == operation)
        {
            Some(capability) => capability.execution.clone(),
            None if entry.descriptor.capabilities.is_empty() || is_http_wrapper(operation) => {
                services::ServiceExecutionPolicy::inline()
            }
            None => {
                return Err(ServiceDispatchError::UnknownOperation {
                    domain: domain.to_owned(),
                    operation: operation.to_owned(),
                });
            }
        };

        entry
            .mailbox
            .submit_task(operation.to_owned(), execution, payload, work)
            .await
    }

    pub async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<serde_json::Value, ServiceDispatchError> {
        let entry = self
            .entries
            .get(&envelope.domain)
            .ok_or_else(|| ServiceDispatchError::ServiceNotFound(envelope.domain.clone()))?;

        let capability = entry
            .descriptor
            .capabilities
            .iter()
            .find(|capability| capability.name == envelope.operation)
            .ok_or_else(|| ServiceDispatchError::UnknownOperation {
                domain: envelope.domain.clone(),
                operation: envelope.operation.clone(),
            })?;

        if capability.kind == OperationKind::Command {
            return Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                format!(
                    "{} must enter through the durable command dispatcher.",
                    envelope.operation
                ),
                false,
            ));
        }

        let service = entry.service.clone();
        let request = envelope.clone();
        let service_context = context.clone();
        let span = tracing::info_span!(
            "service.dispatch",
            domain = %envelope.domain,
            operation = %envelope.operation,
            correlation_id = %context.correlation_id,
            causation_id = ?context.causation_id,
            actor_id = ?context.actor.id,
            actor_kind = ?context.actor.kind,
        );
        let work =
            async move { service.dispatch(&request, &service_context).await }.instrument(span);
        let result = entry
            .mailbox
            .submit(
                envelope.operation.clone(),
                capability.execution.clone(),
                &envelope.payload,
                work,
            )
            .await;

        if let Err(error) = &result {
            self.observer
                .observe_dispatch_failure(&envelope.domain, &envelope.operation, context, error)
                .await;
        }

        result
    }

    pub async fn drain(&self) -> Result<(), ServiceDispatchError> {
        let mut first_error = None;
        for domain in self.shutdown_order.iter() {
            let mailbox = &self.entries[domain].mailbox;
            mailbox.refuse_new_work();
            if let Err(error) = mailbox.drain().await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn cancel(&self) {
        for domain in self.shutdown_order.iter() {
            self.entries[domain].mailbox.cancel();
        }
        self.root_cancel.cancel();
    }

    pub fn force_stop(&self) {
        for domain in self.shutdown_order.iter() {
            self.entries[domain].mailbox.force_stop();
        }
        self.root_cancel.cancel();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        let mut first_error = None;
        for domain in self.shutdown_order.iter() {
            if let Err(error) = self.entries[domain].mailbox.wait_stopped().await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        let drain_result = self.drain().await;
        self.cancel();
        let stopped_result = self.wait_stopped().await;
        drain_result.and(stopped_result)
    }
}

fn is_http_wrapper(operation: &str) -> bool {
    matches!(
        operation,
        "http.get" | "http.post" | "http.put" | "http.patch" | "http.delete"
    )
}

fn validate_service_graph(
    descriptors: &[ServiceDescriptor],
) -> Result<Vec<String>, ServiceDispatchError> {
    let mut graph = HashMap::<String, Vec<String>>::new();
    for descriptor in descriptors {
        if graph
            .insert(descriptor.domain.clone(), descriptor.dependencies.clone())
            .is_some()
        {
            return Err(ServiceDispatchError::infrastructure(
                "SERVICE_ALREADY_REGISTERED",
                format!(
                    "Service already registered for domain: {}",
                    descriptor.domain
                ),
                false,
            ));
        }
    }
    for (domain, dependencies) in &graph {
        for dependency in dependencies {
            if !graph.contains_key(dependency) {
                return Err(ServiceDispatchError::infrastructure(
                    "SERVICE_DEPENDENCY_MISSING",
                    format!("Service {domain} requires missing dependency {dependency}."),
                    false,
                ));
            }
        }
    }

    fn visit(
        domain: &str,
        graph: &HashMap<String, Vec<String>>,
        state: &mut HashMap<String, u8>,
        order: &mut Vec<String>,
    ) -> Result<(), ServiceDispatchError> {
        match state.get(domain).copied() {
            Some(2) => return Ok(()),
            Some(1) => {
                return Err(ServiceDispatchError::infrastructure(
                    "SERVICE_DEPENDENCY_CYCLE",
                    format!("Service dependency cycle includes {domain}."),
                    false,
                ));
            }
            _ => {}
        }
        state.insert(domain.to_owned(), 1);
        for dependency in &graph[domain] {
            visit(dependency, graph, state, order)?;
        }
        state.insert(domain.to_owned(), 2);
        order.push(domain.to_owned());
        Ok(())
    }

    let mut domains = graph.keys().cloned().collect::<Vec<_>>();
    domains.sort();
    let mut state = HashMap::new();
    let mut order = Vec::with_capacity(domains.len());
    for domain in domains {
        visit(&domain, &graph, &mut state, &mut order)?;
    }
    Ok(order)
}

#[async_trait]
impl ServiceRouter for ServiceRegistry {
    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<serde_json::Value, ServiceDispatchError> {
        ServiceRegistry::dispatch(self, envelope, context).await
    }
}

#[derive(Clone)]
pub struct ServiceKernel {
    registry: Arc<ServiceRegistry>,
    catalog: ServiceCatalog,
}

impl ServiceKernel {
    pub fn new(
        db: Database,
        infrastructure: ServiceInfrastructure,
    ) -> Result<Self, ServiceDispatchError> {
        let root_cancel = CancellationToken::new();
        let deferred_router = Arc::new(DeferredServiceRouter::new());
        let router_port: Arc<dyn ServiceRouter> = deferred_router.clone();
        let infrastructure = infrastructure.with_router(router_port);

        let catalog = ServiceCatalog::new(db, infrastructure.clone());
        let services = catalog.registrations();
        let registry = Arc::new(ServiceRegistry::new(
            root_cancel,
            services,
            infrastructure.clone(),
        )?);
        let registry_port: Arc<dyn ServiceRouter> = registry.clone();
        deferred_router.install(&registry_port)?;

        Ok(Self { registry, catalog })
    }

    pub fn registry(&self) -> Arc<ServiceRegistry> {
        self.registry.clone()
    }

    pub(crate) fn child_token(&self) -> CancellationToken {
        self.registry.root_cancel.child_token()
    }

    pub async fn start(&self) -> Result<(), ServiceDispatchError> {
        self.registry.start().await
    }

    pub fn health(&self) -> ServiceKernelHealth {
        self.registry.kernel_health()
    }

    pub async fn control(
        &self,
        domain: &str,
        command: ServiceControlCommand,
    ) -> Result<ServiceControlResult, ServiceDispatchError> {
        self.registry.control(domain, command).await
    }

    pub fn person(&self) -> Arc<PersonService<PersonDao>> {
        self.catalog.person()
    }

    pub fn firm(&self) -> Arc<FirmService<FirmDao>> {
        self.catalog.firm()
    }

    pub fn property(&self) -> Arc<PropertyService<PropertyDao>> {
        self.catalog.property()
    }

    pub fn contract(&self) -> Arc<ContractService<ContractDao>> {
        self.catalog.contract()
    }

    pub fn catalog(&self) -> ServiceCatalog {
        self.catalog.clone()
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        self.registry.shutdown().await
    }

    pub fn begin_shutdown(&self) {
        self.registry.cancel();
    }

    pub fn force_stop(&self) {
        self.registry.force_stop();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        self.registry.wait_stopped().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use services::{CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn descriptor(domain: &str, dependencies: &[&str]) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: domain.into(),
            version: "1".into(),
            description: String::new(),
            capabilities: Vec::new(),
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            invariants: Vec::new(),
        }
    }

    #[test]
    fn dependency_order_starts_dependencies_before_dependents() {
        let order = validate_service_graph(&[
            descriptor("contract", &["person", "property"]),
            descriptor("property", &[]),
            descriptor("person", &[]),
        ])
        .unwrap();
        let contract = order.iter().position(|value| value == "contract").unwrap();
        assert!(order.iter().position(|value| value == "person").unwrap() < contract);
        assert!(order.iter().position(|value| value == "property").unwrap() < contract);
    }

    #[test]
    fn dependency_graph_rejects_missing_services() {
        let error = validate_service_graph(&[descriptor("contract", &["person"])]).unwrap_err();
        assert_eq!(error.code(), "SERVICE_DEPENDENCY_MISSING");
    }

    #[test]
    fn dependency_graph_rejects_cycles() {
        let error = validate_service_graph(&[descriptor("a", &["b"]), descriptor("b", &["a"])])
            .unwrap_err();
        assert_eq!(error.code(), "SERVICE_DEPENDENCY_CYCLE");
    }

    #[test]
    fn dependency_graph_rejects_duplicate_domains_before_spawning() {
        let error =
            validate_service_graph(&[descriptor("a", &[]), descriptor("a", &[])]).unwrap_err();
        assert_eq!(error.code(), "SERVICE_ALREADY_REGISTERED");
    }

    #[test]
    fn only_known_http_wrappers_bypass_typed_capability_lookup() {
        for operation in [
            "http.get",
            "http.post",
            "http.put",
            "http.patch",
            "http.delete",
        ] {
            assert!(is_http_wrapper(operation));
        }
        assert!(!is_http_wrapper("http.trace"));
        assert!(!is_http_wrapper("property.unknown"));
    }

    struct WarmedService {
        warmed: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AbstractService for WarmedService {
        fn descriptor(&self) -> ServiceDescriptor {
            descriptor("warmed", &[])
        }

        async fn warm_cache(&self) -> Result<usize, ServiceDispatchError> {
            self.warmed.fetch_add(1, Ordering::SeqCst);
            Ok(17)
        }
    }

    #[tokio::test]
    async fn registry_warms_each_service_during_startup() {
        let warmed = Arc::new(AtomicUsize::new(0));
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let registry = ServiceRegistry::new(
            CancellationToken::new(),
            vec![Arc::new(WarmedService {
                warmed: warmed.clone(),
            })],
            infrastructure,
        )
        .unwrap();

        registry.start().await.unwrap();
        assert_eq!(warmed.load(Ordering::SeqCst), 1);
        registry.shutdown().await.unwrap();
    }
}

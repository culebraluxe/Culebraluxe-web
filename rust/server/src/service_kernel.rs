use crate::contracts::ContractService;
use crate::firms::FirmService;
use crate::lookup::CoreEntityLookup;
use crate::people::PersonService;
use crate::properties::PropertyService;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use db::{ContractDao, Database, FirmDao, PersonDao, PropertyDao};
use service::{
    AbstractService, DeferredServiceRouter, ServiceDescriptor, ServiceDispatchError,
    ServiceEnvelope, ServiceHealth, ServiceMailbox, ServiceMailboxConfig, ServiceContext,
    ServiceInfrastructure, ServiceRouter, ServiceRuntime,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct RegisteredService {
    descriptor: ServiceDescriptor,
    service: Arc<dyn AbstractService>,
    mailbox: ServiceMailbox,
}

#[derive(Clone)]
pub struct ServiceRegistry {
    root_cancel: CancellationToken,
    entries: Arc<HashMap<String, RegisteredService>>,
}

impl ServiceRegistry {
    fn new(
        root_cancel: CancellationToken,
        services: Vec<Arc<dyn AbstractService>>,
    ) -> Result<Self, ServiceDispatchError> {
        let mut entries = HashMap::new();

        for service in services {
            let descriptor = service.descriptor();
            let domain = descriptor.domain.clone();
            if entries.contains_key(&domain) {
                return Err(ServiceDispatchError::operation(
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

        let service = entry.service.clone();
        let request = envelope.clone();
        let service_context = context.clone();
        entry
            .mailbox
            .submit(
                envelope.operation.clone(),
                capability.execution.clone(),
                &envelope.payload,
                async move { service.dispatch(&request, &service_context).await },
            )
            .await
    }

    pub async fn drain(&self) -> Result<(), ServiceDispatchError> {
        for entry in self.entries.values() {
            entry.mailbox.refuse_new_work();
        }
        for entry in self.entries.values() {
            entry.mailbox.drain().await?;
        }
        Ok(())
    }

    pub fn cancel(&self) {
        for entry in self.entries.values() {
            entry.mailbox.cancel();
        }
        self.root_cancel.cancel();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        for entry in self.entries.values() {
            entry.mailbox.wait_stopped().await?;
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        self.drain().await?;
        self.cancel();
        self.wait_stopped().await
    }
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
struct KernelEntityLookup {
    runtime: ServiceRuntime,
}

#[async_trait]
impl CoreEntityLookup for KernelEntityLookup {
    async fn person_exists(
        &self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        let value = self
            .runtime
            .call_service(
                "person",
                "person.get",
                serde_json::json!({ "personId": person_id }),
                context,
            )
            .await?;
        Ok(!value.is_null())
    }

    async fn firm_exists(
        &self,
        firm_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        let value = self
            .runtime
            .call_service(
                "firm",
                "firm.get",
                serde_json::json!({ "firmId": firm_id }),
                context,
            )
            .await?;
        Ok(!value.is_null())
    }

    async fn property_exists(
        &self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<bool, CoreServiceError> {
        let value = self
            .runtime
            .call_service(
                "property",
                "property.get",
                serde_json::json!({ "propertyId": property_id }),
                context,
            )
            .await?;
        Ok(!value.is_null())
    }
}

#[derive(Clone)]
pub struct ServiceKernel {
    registry: Arc<ServiceRegistry>,
    person: Arc<PersonService<PersonDao>>,
    firm: Arc<FirmService<FirmDao>>,
    property: Arc<PropertyService<PropertyDao>>,
    contract: Arc<ContractService<ContractDao>>,
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

        let person = Arc::new(PersonService::new(
            PersonDao::new(db.clone()),
            infrastructure.clone(),
        ));
        let firm = Arc::new(FirmService::new(
            FirmDao::new(db.clone()),
            infrastructure.clone(),
        ));
        let property = Arc::new(PropertyService::new(
            PropertyDao::new(db.clone()),
            infrastructure.clone(),
        ));

        let lookup: Arc<dyn CoreEntityLookup> = Arc::new(KernelEntityLookup {
            runtime: ServiceRuntime::new(infrastructure.clone()),
        });
        let contract = Arc::new(ContractService::new(
            ContractDao::new(db),
            lookup,
            infrastructure,
        ));

        let services: Vec<Arc<dyn AbstractService>> = vec![
            person.clone(),
            firm.clone(),
            property.clone(),
            contract.clone(),
        ];
        let registry = Arc::new(ServiceRegistry::new(root_cancel, services)?);
        let registry_port: Arc<dyn ServiceRouter> = registry.clone();
        deferred_router.install(&registry_port)?;

        Ok(Self {
            registry,
            person,
            firm,
            property,
            contract,
        })
    }

    pub fn registry(&self) -> Arc<ServiceRegistry> {
        self.registry.clone()
    }

    pub fn person(&self) -> Arc<PersonService<PersonDao>> {
        self.person.clone()
    }

    pub fn firm(&self) -> Arc<FirmService<FirmDao>> {
        self.firm.clone()
    }

    pub fn property(&self) -> Arc<PropertyService<PropertyDao>> {
        self.property.clone()
    }

    pub fn contract(&self) -> Arc<ContractService<ContractDao>> {
        self.contract.clone()
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        self.registry.shutdown().await
    }

    pub fn begin_shutdown(&self) {
        self.registry.cancel();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        self.registry.wait_stopped().await
    }
}

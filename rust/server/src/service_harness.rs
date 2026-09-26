use crate::{
    ServiceGateway, ServiceKernel, ServiceKernelHealth,
};
use db::Database;
use serde_json::Value;
use service::{
    ServiceContext, ServiceControlCommand, ServiceControlResult, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceInfrastructure,
};

#[derive(Clone)]
pub struct ServiceHarness {
    kernel: ServiceKernel,
    gateway: ServiceGateway,
}

impl ServiceHarness {
    pub fn new(
        db: Database,
        infrastructure: ServiceInfrastructure,
    ) -> Result<Self, ServiceDispatchError> {
        let kernel = ServiceKernel::new(db, infrastructure)?;
        let gateway = ServiceGateway::new(kernel.registry());
        Ok(Self { kernel, gateway })
    }

    pub async fn start(&self) -> Result<(), ServiceDispatchError> {
        self.kernel.start().await
    }

    pub fn kernel(&self) -> ServiceKernel {
        self.kernel.clone()
    }

    pub fn gateway(&self) -> ServiceGateway {
        self.gateway.clone()
    }

    pub fn descriptors(&self) -> Vec<ServiceDescriptor> {
        self.gateway.descriptors()
    }

    pub fn health(&self) -> ServiceKernelHealth {
        self.kernel.health()
    }

    pub async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        self.gateway.dispatch(envelope, context).await
    }

    pub async fn control(
        &self,
        domain: &str,
        command: ServiceControlCommand,
    ) -> Result<ServiceControlResult, ServiceDispatchError> {
        self.kernel.control(domain, command).await
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        self.kernel.shutdown().await
    }

    pub fn begin_shutdown(&self) {
        self.kernel.begin_shutdown();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        self.kernel.wait_stopped().await
    }
}

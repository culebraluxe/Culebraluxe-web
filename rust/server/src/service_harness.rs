use crate::{
    CommandDispatchError, CommandDispatcher, Crm26AgreementExecutionSubscriber, MqProofSubscriber,
    MqRuntime, ServiceGateway, ServiceKernel, ServiceKernelHealth,
};
use db::{Database, DomainEventOutboxDao};
use serde_json::Value;
use service::{
    CommandRequest, CommandResult, ServiceContext, ServiceControlCommand, ServiceControlResult,
    ServiceDescriptor, ServiceDispatchError, ServiceEnvelope, ServiceHealth, ServiceInfrastructure,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHarnessHealth {
    pub kernel: ServiceKernelHealth,
    pub mq: ServiceHealth,
}

#[derive(Clone)]
pub struct ServiceHarness {
    kernel: ServiceKernel,
    gateway: ServiceGateway,
    commands: CommandDispatcher,
    mq: MqRuntime,
}

impl ServiceHarness {
    pub fn new(
        db: Database,
        infrastructure: ServiceInfrastructure,
    ) -> Result<Self, ServiceDispatchError> {
        let mq_infrastructure = infrastructure.clone();
        let kernel = ServiceKernel::new(db.clone(), infrastructure)?;
        let gateway = ServiceGateway::new(kernel.registry());
        let commands =
            CommandDispatcher::for_kernel(db.clone(), kernel.contract()).map_err(|error| {
                ServiceDispatchError::infrastructure(
                    "COMMAND_RUNTIME_INIT",
                    error.to_string(),
                    false,
                )
            })?;
        let outbox = DomainEventOutboxDao::new(db.clone());
        let crm26 = Crm26AgreementExecutionSubscriber::production(
            db,
            commands.clone(),
            kernel.registry(),
        );
        let mq = MqRuntime::new(
            outbox.clone(),
            vec![
                Arc::new(MqProofSubscriber::new(outbox)),
                Arc::new(crm26),
            ],
            mq_infrastructure,
            kernel.child_token(),
        )?;
        Ok(Self {
            kernel,
            gateway,
            commands,
            mq,
        })
    }

    pub async fn start(&self) -> Result<(), ServiceDispatchError> {
        self.kernel.start().await?;
        if let Err(error) = self.mq.start().await {
            let _ = self.kernel.shutdown().await;
            return Err(error);
        }
        Ok(())
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

    pub fn mq_health(&self) -> ServiceHealth {
        self.mq.health()
    }

    pub fn runtime_health(&self) -> ServiceHarnessHealth {
        ServiceHarnessHealth {
            kernel: self.kernel.health(),
            mq: self.mq.health(),
        }
    }

    pub async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        self.gateway.dispatch(envelope, context).await
    }

    pub async fn execute_command(
        &self,
        request: &CommandRequest,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError> {
        if let Some((domain, operation, payload)) = self.commands.scheduling_route(request) {
            let dispatcher = self.commands.clone();
            let request = request.clone();
            let context = context.clone();
            return self
                .kernel
                .registry()
                .run_task(domain, operation, &payload, async move {
                    dispatcher.execute(&request, &context).await
                })
                .await?;
        }

        self.commands.execute(request, context).await
    }

    pub async fn control(
        &self,
        domain: &str,
        command: ServiceControlCommand,
    ) -> Result<ServiceControlResult, ServiceDispatchError> {
        self.kernel.control(domain, command).await
    }

    pub async fn shutdown(&self) -> Result<(), ServiceDispatchError> {
        self.kernel.shutdown().await?;
        self.mq.wait_stopped().await
    }

    pub fn begin_shutdown(&self) {
        self.kernel.begin_shutdown();
        self.mq.begin_shutdown();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        self.kernel.wait_stopped().await?;
        self.mq.wait_stopped().await
    }
}

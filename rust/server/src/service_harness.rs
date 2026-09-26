use crate::{
    CommandDispatchError, CommandDispatcher, Crm26AgreementExecutionSubscriber, MqProofSubscriber,
    MqRuntime, ServiceGateway, ServiceKernel, ServiceKernelHealth,
};
use db::{Database, DomainEventOutboxDao};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::{
    CommandRequest, CommandResult, ServiceContext, ServiceControlCommand, ServiceControlResult,
    ServiceDescriptor, ServiceDispatchError, ServiceEnvelope, ServiceHealth, ServiceInfrastructure,
};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::time::{timeout, Duration};

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
    shutdown: Arc<OnceCell<Result<(), ServiceDispatchError>>>,
}

impl ServiceHarness {
    pub fn new(
        db: Database,
        infrastructure: ServiceInfrastructure,
    ) -> Result<Self, ServiceDispatchError> {
        Self::compose(db, infrastructure, true)
    }

    /// External/runtime-test composition.
    ///
    /// This is the SAME ServiceHarness, ServiceKernel, registry, mailboxes and
    /// lifecycle implementation as production. The only omitted composition is
    /// the production MQ subscriber set, so a DEV harness test cannot consume
    /// unrelated pending business deliveries while proving lifecycle behavior.
    pub fn isolated(
        db: Database,
        infrastructure: ServiceInfrastructure,
    ) -> Result<Self, ServiceDispatchError> {
        Self::compose(db, infrastructure, false)
    }

    fn compose(
        db: Database,
        infrastructure: ServiceInfrastructure,
        production_mq_subscribers: bool,
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
        let subscribers: Vec<Arc<dyn crate::MqSubscriber>> = if production_mq_subscribers {
            let crm26 = Crm26AgreementExecutionSubscriber::production(
                db,
                commands.clone(),
                kernel.registry(),
            );
            vec![
                Arc::new(MqProofSubscriber::new(outbox.clone())),
                Arc::new(crm26),
            ]
        } else {
            Vec::new()
        };
        let mq = MqRuntime::new(outbox, subscribers, mq_infrastructure, kernel.child_token())?;
        Ok(Self {
            kernel,
            gateway,
            commands,
            mq,
            shutdown: Arc::new(OnceCell::new()),
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
        self.gateway.ensure_accepting()?;
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
        self.shutdown
            .get_or_init(|| async { self.shutdown_inner().await })
            .await
            .clone()
    }

    pub fn begin_shutdown(&self) {
        self.gateway.refuse_new_work();
        self.mq.begin_shutdown();
    }

    pub fn quiesce_mq(&self) {
        self.mq.begin_shutdown();
    }

    pub async fn wait_stopped(&self) -> Result<(), ServiceDispatchError> {
        self.shutdown().await
    }

    async fn shutdown_inner(&self) -> Result<(), ServiceDispatchError> {
        self.begin_shutdown();
        let graceful = async {
            // Workers already holding deliveries may still call services, so
            // finish MQ before closing the internal service mailboxes.
            self.mq.wait_stopped().await?;
            self.kernel.shutdown().await
        };
        match timeout(Duration::from_secs(30), graceful).await {
            Ok(result) => result,
            Err(_) => {
                self.mq.force_stop();
                self.kernel.force_stop();
                Err(ServiceDispatchError::infrastructure(
                    "SERVICE_SHUTDOWN_TIMEOUT",
                    "Service harness exceeded its 30 second drain deadline; remaining work was aborted.",
                    false,
                ))
            }
        }
    }
}

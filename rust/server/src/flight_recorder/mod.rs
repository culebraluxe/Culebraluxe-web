use async_trait::async_trait;
use db::{DbResult, FlightRecorderDao};
use domain::FlightRecorderTransaction;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

use crate::service_support::{audit_result, authorize, CoreServiceError};

#[async_trait]
pub trait FlightRecorderRepository: Send {
    async fn transaction(
        &mut self,
        instance_id: &str,
    ) -> DbResult<Option<FlightRecorderTransaction>>;
}

#[async_trait]
impl FlightRecorderRepository for FlightRecorderDao {
    async fn transaction(
        &mut self,
        instance_id: &str,
    ) -> DbResult<Option<FlightRecorderTransaction>> {
        FlightRecorderDao::transaction(self, instance_id).await
    }
}

pub struct FlightRecorderService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: FlightRecorderRepository> FlightRecorderService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn transaction(
        &mut self,
        instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<FlightRecorderTransaction>, CoreServiceError> {
        const OP: &str = "flightRecorder.transaction";
        let decision = authorize(
            &self.runtime,
            "workflow",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .transaction(instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "workflow", OP, context, decision, &result).await?;
        result
    }
}

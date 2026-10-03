use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{CockpitDao, DbResult};
use model::CockpitSnapshot;
use services::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait CockpitRepository: Send {
    async fn snapshot(&self) -> DbResult<CockpitSnapshot>;
}

#[async_trait]
impl CockpitRepository for CockpitDao {
    async fn snapshot(&self) -> DbResult<CockpitSnapshot> {
        CockpitDao::snapshot(self).await
    }
}

pub struct CockpitService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: CockpitRepository> CockpitService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn snapshot(
        &self,
        context: &ServiceContext,
    ) -> Result<CockpitSnapshot, CoreServiceError> {
        const OP: &str = "cockpit.snapshot";
        let decision = authorize(
            &self.runtime,
            "cockpit",
            "cockpit.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = self.repository.snapshot().await.map_err(Into::into);
        audit_result(&self.runtime, "cockpit", OP, context, decision, &result).await?;
        result
    }
}

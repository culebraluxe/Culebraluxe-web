use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, TechCockpitDao};
use domain::TechCockpitSnapshot;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait TechCockpitRepository: Send {
    async fn snapshot(&mut self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot>;
}

#[async_trait]
impl TechCockpitRepository for TechCockpitDao {
    async fn snapshot(&mut self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot> {
        TechCockpitDao::snapshot(self, selected).await
    }
}

pub struct TechCockpitService<R> { repository: R, runtime: ServiceRuntime }

impl<R: TechCockpitRepository> TechCockpitService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self { repository, runtime: ServiceRuntime::new(infrastructure) }
    }

    pub async fn snapshot(&mut self, selected: Option<&str>, context: &ServiceContext)
        -> Result<TechCockpitSnapshot, CoreServiceError> {
        const OP: &str = "tech.cockpitSnapshot";
        let decision = authorize(&self.runtime, "tech", "tech.access", OP, OperationKind::Query, context).await?;
        let result = self.repository.snapshot(selected).await.map_err(Into::into);
        audit_result(&self.runtime, "tech", OP, context, decision, &result).await?;
        result
    }
}

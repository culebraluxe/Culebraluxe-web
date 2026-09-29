use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, PublishingDao};
use domain::PublishingSnapshot;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait PublishingRepository: Send {
    async fn snapshot(&self) -> DbResult<PublishingSnapshot>;
}

#[async_trait]
impl PublishingRepository for PublishingDao {
    async fn snapshot(&self) -> DbResult<PublishingSnapshot> {
        PublishingDao::snapshot(self).await
    }
}

pub struct PublishingService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: PublishingRepository> PublishingService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn snapshot(
        &self,
        context: &ServiceContext,
    ) -> Result<PublishingSnapshot, CoreServiceError> {
        const OP: &str = "publishing.snapshot";
        let decision = authorize(
            &self.runtime,
            "publishing",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.snapshot().await.map_err(Into::into);
        audit_result(&self.runtime, "publishing", OP, context, decision, &result).await?;
        result
    }
}

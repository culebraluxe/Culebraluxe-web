use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, MarketingDao};
use domain::MarketingContentBlock;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait MarketingRepository: Send {
    async fn public_content(&mut self) -> DbResult<Vec<MarketingContentBlock>>;
}

#[async_trait]
impl MarketingRepository for MarketingDao {
    async fn public_content(&mut self) -> DbResult<Vec<MarketingContentBlock>> {
        db::retrying_read!(MarketingDao::public_content(self))
    }
}

pub struct MarketingService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: MarketingRepository> MarketingService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn public_content(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<MarketingContentBlock>, CoreServiceError> {
        const OP: &str = "marketing.publicContent";
        let decision = authorize(
            &self.runtime,
            "marketing",
            "guide.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.public_content().await.map_err(Into::into);
        audit_result(&self.runtime, "marketing", OP, context, decision, &result).await?;
        result
    }
}

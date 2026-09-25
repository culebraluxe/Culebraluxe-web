//! Public Island Guide service.

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, GuideDao};
use domain::GuideItem;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait GuideRepository: Send {
    async fn items(&mut self) -> DbResult<Vec<GuideItem>>;
}

#[async_trait]
impl GuideRepository for GuideDao {
    async fn items(&mut self) -> DbResult<Vec<GuideItem>> {
        db::retrying_read!(GuideDao::items(self))
    }
}

pub struct GuideService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: GuideRepository> GuideService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn items(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<GuideItem>, CoreServiceError> {
        const OP: &str = "guide.publicItems";
        let decision = authorize(
            &self.runtime,
            "guide",
            "guide.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.items().await.map_err(Into::into);
        audit_result(&self.runtime, "guide", OP, context, decision, &result).await?;
        result
    }
}

use async_trait::async_trait;
use db::{DbResult, DealPortalDao};
use domain::{CreateDealRequest, CreateDealResult, DealPortfolioSnapshot};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

use crate::service_support::{audit_result, authorize, CoreServiceError};

#[async_trait]
pub trait DealPortalRepository: Send {
    async fn portfolio(&mut self) -> DbResult<DealPortfolioSnapshot>;
    async fn create(&mut self, request: &CreateDealRequest) -> DbResult<CreateDealResult>;
}

#[async_trait]
impl DealPortalRepository for DealPortalDao {
    async fn portfolio(&mut self) -> DbResult<DealPortfolioSnapshot> {
        DealPortalDao::portfolio(self).await
    }

    async fn create(&mut self, request: &CreateDealRequest) -> DbResult<CreateDealResult> {
        DealPortalDao::create(self, request).await
    }
}

pub struct DealPortalService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: DealPortalRepository> DealPortalService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn portfolio(
        &mut self,
        context: &ServiceContext,
    ) -> Result<DealPortfolioSnapshot, CoreServiceError> {
        const OP: &str = "deal.portfolio";
        let decision = authorize(
            &self.runtime,
            "deal",
            "deal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.portfolio().await.map_err(Into::into);
        audit_result(&self.runtime, "deal", OP, context, decision, &result).await?;
        result
    }

    pub async fn create(
        &mut self,
        request: &CreateDealRequest,
        context: &ServiceContext,
    ) -> Result<CreateDealResult, CoreServiceError> {
        const OP: &str = "deal.create";
        let decision = authorize(
            &self.runtime,
            "deal",
            "deal.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if request.property_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PROPERTY_REQUIRED",
                    "Property is required.",
                ));
            }
            if request.client_person_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "CLIENT_REQUIRED",
                    "Client person is required.",
                ));
            }
            self.repository.create(request).await.map_err(Into::into)
        }
        .await;

        audit_result(&self.runtime, "deal", OP, context, decision, &result).await?;
        result
    }
}

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, IntakeDao};
use domain::{
    CatchupLeadRequest, CatchupLeadResult, WebsiteIntakeRequest, WebsiteIntakeResult,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait IntakeRepository: Send {
    async fn catchup_lead(&mut self, input: &CatchupLeadRequest) -> DbResult<CatchupLeadResult>;
    async fn website_intake(
        &mut self,
        input: &WebsiteIntakeRequest,
    ) -> DbResult<WebsiteIntakeResult>;
}

#[async_trait]
impl IntakeRepository for IntakeDao {
    async fn catchup_lead(&mut self, input: &CatchupLeadRequest) -> DbResult<CatchupLeadResult> {
        IntakeDao::catchup_lead(self, input).await
    }

    async fn website_intake(
        &mut self,
        input: &WebsiteIntakeRequest,
    ) -> DbResult<WebsiteIntakeResult> {
        IntakeDao::website_intake(self, input).await
    }
}

pub struct IntakeService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: IntakeRepository> IntakeService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn submit_website(
        &mut self,
        input: &WebsiteIntakeRequest,
        context: &ServiceContext,
    ) -> Result<WebsiteIntakeResult, CoreServiceError> {
        const OP: &str = "website.submitIntake";
        let decision = authorize(
            &self.runtime,
            "website",
            "website.intake.submit",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.repository.website_intake(input).await.map_err(Into::into);
        audit_result(&self.runtime, "website", OP, context, decision, &result).await?;
        result
    }

    pub async fn submit_catchup(
        &mut self,
        input: &CatchupLeadRequest,
        context: &ServiceContext,
    ) -> Result<CatchupLeadResult, CoreServiceError> {
        const OP: &str = "catchup.createLead";
        let decision = authorize(
            &self.runtime,
            "crm",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.repository.catchup_lead(input).await.map_err(Into::into);
        audit_result(&self.runtime, "crm", OP, context, decision, &result).await?;
        result
    }
}

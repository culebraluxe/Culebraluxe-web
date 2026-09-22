use async_trait::async_trait;
use db::{DbResult, WorkflowPortalDao};
use domain::{WorkflowPortalDetail, WorkflowPortalList};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

use crate::service_support::{audit_result, authorize, CoreServiceError};

#[async_trait]
pub trait WorkflowPortalRepository: Send {
    async fn list(&mut self) -> DbResult<WorkflowPortalList>;
    async fn detail(&mut self, instance_id: &str) -> DbResult<Option<WorkflowPortalDetail>>;
}

#[async_trait]
impl WorkflowPortalRepository for WorkflowPortalDao {
    async fn list(&mut self) -> DbResult<WorkflowPortalList> {
        WorkflowPortalDao::list(self).await
    }

    async fn detail(&mut self, instance_id: &str) -> DbResult<Option<WorkflowPortalDetail>> {
        WorkflowPortalDao::detail(self, instance_id).await
    }
}

pub struct WorkflowPortalService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: WorkflowPortalRepository> WorkflowPortalService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn list(
        &mut self,
        context: &ServiceContext,
    ) -> Result<WorkflowPortalList, CoreServiceError> {
        const OP: &str = "workflowPortal.list";
        let decision = authorize(
            &self.runtime,
            "workflow",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list().await.map_err(Into::into);
        audit_result(&self.runtime, "workflow", OP, context, decision, &result).await?;
        result
    }

    pub async fn detail(
        &mut self,
        instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<WorkflowPortalDetail>, CoreServiceError> {
        const OP: &str = "workflowPortal.detail";
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
            .detail(instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "workflow", OP, context, decision, &result).await?;
        result
    }
}

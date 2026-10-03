use async_trait::async_trait;
use db::{DbResult, WorkflowPortalDao};
use model::{WorkflowPortalDetail, WorkflowPortalList};
use services::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

use crate::service_support::{audit_result, authorize, CoreServiceError};

#[async_trait]
pub trait WorkflowPortalRepository: Send {
    async fn list(&self) -> DbResult<WorkflowPortalList>;
    async fn detail(&self, instance_id: &str) -> DbResult<Option<WorkflowPortalDetail>>;
}

#[async_trait]
impl WorkflowPortalRepository for WorkflowPortalDao {
    async fn list(&self) -> DbResult<WorkflowPortalList> {
        WorkflowPortalDao::list(self).await
    }

    async fn detail(&self, instance_id: &str) -> DbResult<Option<WorkflowPortalDetail>> {
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
        &self,
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
        &self,
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

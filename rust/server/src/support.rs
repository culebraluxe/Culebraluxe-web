use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, SupportDiagnosticsDao};
use domain::{
    SupportSecurityStatus, SupportSystemHealth, WorkflowDiagnosticsDetail,
    WorkflowDiagnosticsSnapshot,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait SupportDiagnosticsRepository: Send {
    async fn security_status(&mut self) -> DbResult<SupportSecurityStatus>;
    async fn system_health(&mut self) -> DbResult<SupportSystemHealth>;
    async fn workflow_diagnostics(&mut self) -> DbResult<WorkflowDiagnosticsSnapshot>;
    async fn workflow_detail(&mut self, instance_id: &str)
        -> DbResult<Option<WorkflowDiagnosticsDetail>>;
}

#[async_trait]
impl SupportDiagnosticsRepository for SupportDiagnosticsDao {
    async fn security_status(&mut self) -> DbResult<SupportSecurityStatus> {
        SupportDiagnosticsDao::security_status(self).await
    }

    async fn system_health(&mut self) -> DbResult<SupportSystemHealth> {
        SupportDiagnosticsDao::system_health(self).await
    }

    async fn workflow_diagnostics(&mut self) -> DbResult<WorkflowDiagnosticsSnapshot> {
        SupportDiagnosticsDao::workflow_diagnostics(self).await
    }

    async fn workflow_detail(
        &mut self,
        instance_id: &str,
    ) -> DbResult<Option<WorkflowDiagnosticsDetail>> {
        SupportDiagnosticsDao::workflow_detail(self, instance_id).await
    }
}

pub struct SupportDiagnosticsService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: SupportDiagnosticsRepository> SupportDiagnosticsService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn security_status(
        &mut self,
        context: &ServiceContext,
    ) -> Result<SupportSecurityStatus, CoreServiceError> {
        const OP: &str = "support.securityStatus";
        let decision = authorize(
            &self.runtime,
            "support",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.security_status().await.map_err(Into::into);
        audit_result(&self.runtime, "support", OP, context, decision, &result).await?;
        result
    }

    pub async fn system_health(
        &mut self,
        context: &ServiceContext,
    ) -> Result<SupportSystemHealth, CoreServiceError> {
        const OP: &str = "support.systemHealth";
        let decision = authorize(
            &self.runtime,
            "support",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.system_health().await.map_err(Into::into);
        audit_result(&self.runtime, "support", OP, context, decision, &result).await?;
        result
    }

    pub async fn workflow_diagnostics(
        &mut self,
        context: &ServiceContext,
    ) -> Result<WorkflowDiagnosticsSnapshot, CoreServiceError> {
        const OP: &str = "support.workflowDiagnostics";
        let decision = authorize(
            &self.runtime,
            "support",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .workflow_diagnostics()
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "support", OP, context, decision, &result).await?;
        result
    }

    pub async fn workflow_detail(
        &mut self,
        instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<WorkflowDiagnosticsDetail>, CoreServiceError> {
        const OP: &str = "support.workflowDetail";
        let decision = authorize(
            &self.runtime,
            "support",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .workflow_detail(instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "support", OP, context, decision, &result).await?;
        result
    }
}

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, IssueDao};
use domain::IssuesPage;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait IssueRepository: Send {
    async fn page(
        &mut self,
        scope: &str,
        state: &str,
        page: i64,
        page_size: i64,
    ) -> DbResult<IssuesPage>;
}

#[async_trait]
impl IssueRepository for IssueDao {
    async fn page(
        &mut self,
        scope: &str,
        state: &str,
        page: i64,
        page_size: i64,
    ) -> DbResult<IssuesPage> {
        db::retrying_read!(IssueDao::page(self, scope, state, page, page_size))
    }
}

pub struct IssueService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: IssueRepository> IssueService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn page(
        &mut self,
        scope: &str,
        state: &str,
        page: i64,
        page_size: i64,
        context: &ServiceContext,
    ) -> Result<IssuesPage, CoreServiceError> {
        const OP: &str = "issue.page";
        let decision = authorize(
            &self.runtime,
            "issue",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = self
            .repository
            .page(scope, state, page.max(1), page_size.clamp(1, 50))
            .await
            .map_err(Into::into);

        audit_result(&self.runtime, "issue", OP, context, decision, &result).await?;
        result
    }
}

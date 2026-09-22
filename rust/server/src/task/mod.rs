use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, TaskDao};
use domain::TaskCompletion;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait TaskRepository: Send {
    async fn complete(&mut self, task_id: &str) -> DbResult<Option<TaskCompletion>>;
}

#[async_trait]
impl TaskRepository for TaskDao {
    async fn complete(&mut self, task_id: &str) -> DbResult<Option<TaskCompletion>> {
        TaskDao::complete(self, task_id).await
    }
}

pub struct TaskService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: TaskRepository> TaskService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn complete(
        &mut self,
        task_id: &str,
        context: &ServiceContext,
    ) -> Result<TaskCompletion, CoreServiceError> {
        const OP: &str = "task.complete";
        let decision = authorize(
            &self.runtime,
            "task",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            self.repository
                .complete(task_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "TASK_NOT_OPEN",
                        "Task not found or already resolved.",
                    )
                })
        }
        .await;

        audit_result(&self.runtime, "task", OP, context, decision, &result).await?;
        result
    }
}

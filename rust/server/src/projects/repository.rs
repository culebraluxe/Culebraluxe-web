use async_trait::async_trait;
use db::{DbResult, ProjectDao};
use domain::{
    CompleteProjectRequest, CreateProjectRequest, Project, UpdateProjectRequest,
};

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn get(&self, id: &str) -> DbResult<Option<Project>>;
    async fn list(&self) -> DbResult<Vec<Project>>;
    async fn create(&self, request: &CreateProjectRequest) -> DbResult<Project>;
    async fn update(&self, request: &UpdateProjectRequest) -> DbResult<Option<Project>>;
    async fn complete(&self, request: &CompleteProjectRequest) -> DbResult<Option<Project>>;
}

#[async_trait]
impl ProjectRepository for ProjectDao {
    async fn get(&self, id: &str) -> DbResult<Option<Project>> {
        ProjectDao::get(self, id).await
    }

    async fn list(&self) -> DbResult<Vec<Project>> {
        ProjectDao::list(self).await
    }

    async fn create(&self, request: &CreateProjectRequest) -> DbResult<Project> {
        ProjectDao::create(self, request).await
    }

    async fn update(&self, request: &UpdateProjectRequest) -> DbResult<Option<Project>> {
        ProjectDao::update(self, request).await
    }

    async fn complete(&self, request: &CompleteProjectRequest) -> DbResult<Option<Project>> {
        ProjectDao::complete(self, request).await
    }
}

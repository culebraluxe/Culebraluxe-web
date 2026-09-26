use async_trait::async_trait;
use db::{Database, DbResult, ProjectDao, ProjectTxDao};
use domain::{CompleteProjectRequest, CreateProjectRequest, Project, UpdateProjectRequest};

#[async_trait]
pub trait ProjectRepository: Send {
    fn database(&self) -> Option<Database> {
        None
    }
    async fn get(&mut self, id: &str) -> DbResult<Option<Project>>;
    async fn list(&mut self) -> DbResult<Vec<Project>>;
    async fn create(&mut self, request: &CreateProjectRequest) -> DbResult<Project>;
    async fn update(&mut self, request: &UpdateProjectRequest) -> DbResult<Option<Project>>;
    async fn complete(&mut self, request: &CompleteProjectRequest) -> DbResult<Option<Project>>;
}

#[async_trait]
impl ProjectRepository for ProjectDao {
    fn database(&self) -> Option<Database> {
        Some(ProjectDao::database(self))
    }
    async fn get(&mut self, id: &str) -> DbResult<Option<Project>> {
        // A read, so a transient cold-connect is worth another attempt. Writes below are not wrapped: `create` has no
        // claim guarding it, and retrying an unguarded insert is how one project becomes two.
        db::retrying_read!(ProjectDao::get(self, id))
    }

    async fn list(&mut self) -> DbResult<Vec<Project>> {
        db::retrying_read!(ProjectDao::list(self))
    }

    async fn create(&mut self, request: &CreateProjectRequest) -> DbResult<Project> {
        ProjectDao::create(self, request).await
    }

    async fn update(&mut self, request: &UpdateProjectRequest) -> DbResult<Option<Project>> {
        ProjectDao::update(self, request).await
    }

    async fn complete(&mut self, request: &CompleteProjectRequest) -> DbResult<Option<Project>> {
        ProjectDao::complete(self, request).await
    }
}

#[async_trait]
impl<'a> ProjectRepository for ProjectTxDao<'a> {
    async fn get(&mut self, id: &str) -> DbResult<Option<Project>> {
        ProjectTxDao::get(self, id).await
    }

    async fn list(&mut self) -> DbResult<Vec<Project>> {
        ProjectTxDao::list(self).await
    }

    async fn create(&mut self, request: &CreateProjectRequest) -> DbResult<Project> {
        ProjectTxDao::create(self, request).await
    }

    async fn update(&mut self, request: &UpdateProjectRequest) -> DbResult<Option<Project>> {
        ProjectTxDao::update(self, request).await
    }

    async fn complete(&mut self, request: &CompleteProjectRequest) -> DbResult<Option<Project>> {
        ProjectTxDao::complete(self, request).await
    }
}

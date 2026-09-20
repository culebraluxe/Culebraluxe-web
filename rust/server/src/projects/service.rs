use super::ProjectRepository;
use db::DbFailure;
use domain::{
    CompleteProjectRequest, CreateProjectRequest, Project, UpdateProjectRequest,
};

#[derive(Debug, thiserror::Error)]
pub enum ProjectServiceError {
    #[error("{code}: {message}")]
    Validation {
        code: &'static str,
        message: String,
    },
    #[error("project not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Database(#[from] DbFailure),
}

pub struct ProjectService<R> {
    repository: R,
}

impl<R> ProjectService<R>
where
    R: ProjectRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get(&self, id: &str) -> Result<Option<Project>, ProjectServiceError> {
        Ok(self.repository.get(id).await?)
    }

    pub async fn list(&self) -> Result<Vec<Project>, ProjectServiceError> {
        Ok(self.repository.list().await?)
    }

    pub async fn create(
        &self,
        request: &CreateProjectRequest,
    ) -> Result<Project, ProjectServiceError> {
        validate_create(request)?;
        Ok(self.repository.create(request).await?)
    }

    pub async fn update(
        &self,
        request: &UpdateProjectRequest,
    ) -> Result<Project, ProjectServiceError> {
        validate_update(request)?;
        self.repository
            .update(request)
            .await?
            .ok_or_else(|| ProjectServiceError::NotFound(request.id.clone()))
    }

    pub async fn complete(
        &self,
        request: &CompleteProjectRequest,
    ) -> Result<Project, ProjectServiceError> {
        self.repository
            .complete(request)
            .await?
            .ok_or_else(|| ProjectServiceError::NotFound(request.id.clone()))
    }
}

fn validate_create(request: &CreateProjectRequest) -> Result<(), ProjectServiceError> {
    if request.name.trim().is_empty() {
        return Err(ProjectServiceError::Validation {
            code: "PROJECT_NAME_REQUIRED",
            message: "A Project requires a name.".into(),
        });
    }

    validate_playbook_version(request.playbook_version)
}

fn validate_update(request: &UpdateProjectRequest) -> Result<(), ProjectServiceError> {
    if request
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(ProjectServiceError::Validation {
            code: "PROJECT_NAME_REQUIRED",
            message: "A Project requires a name.".into(),
        });
    }

    validate_playbook_version(request.playbook_version)
}

fn validate_playbook_version(version: Option<i32>) -> Result<(), ProjectServiceError> {
    if version.is_some_and(|version| version < 1) {
        return Err(ProjectServiceError::Validation {
            code: "PROJECT_PLAYBOOK_VERSION_INVALID",
            message: "Playbook version must be a positive integer.".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ProjectStatus, WbsCategory};

    fn create_request(name: &str, playbook_version: Option<i32>) -> CreateProjectRequest {
        CreateProjectRequest {
            id: "test-project".into(),
            name: name.into(),
            owner: None,
            description: String::new(),
            areas: vec![WbsCategory::Management],
            starts_at: None,
            ends_at: None,
            project_type: None,
            playbook_id: None,
            playbook_version,
            person_id: None,
            property_id: None,
            contract_id: None,
        }
    }

    #[test]
    fn create_requires_non_blank_name() {
        let error = validate_create(&create_request("   ", None)).unwrap_err();
        assert!(matches!(
            error,
            ProjectServiceError::Validation {
                code: "PROJECT_NAME_REQUIRED",
                ..
            }
        ));
    }

    #[test]
    fn playbook_version_must_be_positive() {
        let error = validate_create(&create_request("Valid", Some(0))).unwrap_err();
        assert!(matches!(
            error,
            ProjectServiceError::Validation {
                code: "PROJECT_PLAYBOOK_VERSION_INVALID",
                ..
            }
        ));
    }

    #[test]
    fn update_accepts_typed_status() {
        let request = UpdateProjectRequest {
            id: "test-project".into(),
            name: None,
            owner: None,
            status: Some(ProjectStatus::Doing),
            description: None,
            areas: None,
            starts_at: None,
            ends_at: None,
            project_type: None,
            playbook_id: None,
            playbook_version: None,
            person_id: None,
            property_id: None,
            contract_id: None,
        };

        validate_update(&request).unwrap();
    }
}

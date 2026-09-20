use super::ProjectRepository;
use db::DbFailure;
use domain::{CompleteProjectRequest, CreateProjectRequest, Project, UpdateProjectRequest};
use serde_json::json;
use service::{
    AuthorizationDecision, OperationKind, ServiceContext, ServiceInfrastructure, ServiceOutcome,
    ServiceRuntime, ServiceRuntimeError,
};
use std::collections::BTreeMap;

const DOMAIN: &str = "project";

#[derive(Debug, thiserror::Error)]
pub enum ProjectServiceError {
    #[error("{code}: {message}")]
    Validation { code: &'static str, message: String },
    #[error("project not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Database(#[from] DbFailure),
    #[error(transparent)]
    Runtime(#[from] ServiceRuntimeError),
}

impl ProjectServiceError {
    fn code(&self) -> &'static str {
        match self {
            Self::Validation { code, .. } => code,
            Self::NotFound(_) => "PROJECT_NOT_FOUND",
            Self::Database(_) => "DATABASE",
            Self::Runtime(ServiceRuntimeError::Authorization(_)) => "AUTHORIZATION_UNAVAILABLE",
            Self::Runtime(ServiceRuntimeError::Forbidden { .. }) => "FORBIDDEN",
            Self::Runtime(ServiceRuntimeError::Audit(_)) => "AUDIT_UNAVAILABLE",
            Self::Runtime(ServiceRuntimeError::Event(_)) => "DOMAIN_EVENT_UNAVAILABLE",
        }
    }
}

pub struct ProjectService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R> ProjectService<R>
where
    R: ProjectRepository,
{
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &mut self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Project>, ProjectServiceError> {
        const OPERATION: &str = "project.get";
        let decision = self
            .authorize("project.read", OPERATION, OperationKind::Query, context)
            .await?;
        let result = self.repository.get(id).await.map_err(ProjectServiceError::from);
        self.audit_result(OPERATION, context, decision, &result).await?;
        result
    }

    pub async fn list(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<Project>, ProjectServiceError> {
        const OPERATION: &str = "project.list";
        let decision = self
            .authorize("project.read", OPERATION, OperationKind::Query, context)
            .await?;
        let result = self.repository.list().await.map_err(ProjectServiceError::from);
        self.audit_result(OPERATION, context, decision, &result).await?;
        result
    }

    pub async fn create(
        &mut self,
        request: &CreateProjectRequest,
        context: &ServiceContext,
    ) -> Result<Project, ProjectServiceError> {
        const OPERATION: &str = "project.create";
        let decision = self
            .authorize("project.write", OPERATION, OperationKind::Command, context)
            .await?;

        let result = async {
            validate_create(request)?;
            let project = self.repository.create(request).await?;
            self.runtime
                .emit(
                    "project.created",
                    Some(project.id.clone()),
                    BTreeMap::from([
                        ("id".into(), json!(project.id.clone())),
                        ("name".into(), json!(project.name.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(project)
        }
        .await;

        self.audit_result(OPERATION, context, decision, &result).await?;
        result
    }

    pub async fn update(
        &mut self,
        request: &UpdateProjectRequest,
        context: &ServiceContext,
    ) -> Result<Project, ProjectServiceError> {
        const OPERATION: &str = "project.update";
        let decision = self
            .authorize("project.write", OPERATION, OperationKind::Command, context)
            .await?;

        let result = async {
            validate_update(request)?;
            let project = self
                .repository
                .update(request)
                .await?
                .ok_or_else(|| ProjectServiceError::NotFound(request.id.clone()))?;
            self.runtime
                .emit(
                    "project.updated",
                    Some(project.id.clone()),
                    BTreeMap::from([
                        ("id".into(), json!(project.id.clone())),
                        ("status".into(), json!(project.status.as_str())),
                    ]),
                    context,
                )
                .await?;
            Ok(project)
        }
        .await;

        self.audit_result(OPERATION, context, decision, &result).await?;
        result
    }

    pub async fn complete(
        &mut self,
        request: &CompleteProjectRequest,
        context: &ServiceContext,
    ) -> Result<Project, ProjectServiceError> {
        const OPERATION: &str = "project.complete";
        let decision = self
            .authorize("project.write", OPERATION, OperationKind::Command, context)
            .await?;

        let result = async {
            let project = self
                .repository
                .complete(request)
                .await?
                .ok_or_else(|| ProjectServiceError::NotFound(request.id.clone()))?;
            self.runtime
                .emit(
                    "project.completed",
                    Some(project.id.clone()),
                    BTreeMap::from([
                        ("id".into(), json!(project.id.clone())),
                        ("name".into(), json!(project.name.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(project)
        }
        .await;

        self.audit_result(OPERATION, context, decision, &result).await?;
        result
    }

    async fn authorize(
        &self,
        action: &'static str,
        operation: &'static str,
        kind: OperationKind,
        context: &ServiceContext,
    ) -> Result<AuthorizationDecision, ProjectServiceError> {
        match self
            .runtime
            .authorize(DOMAIN, action, operation, kind, context)
            .await
        {
            Ok(decision) => Ok(decision),
            Err(error @ ServiceRuntimeError::Forbidden { ref decision, .. }) => {
                self.runtime
                    .audit(
                        DOMAIN,
                        operation,
                        context,
                        ServiceOutcome::Failure,
                        Some("FORBIDDEN".into()),
                        decision.clone(),
                    )
                    .await?;
                Err(error.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn audit_result<T>(
        &self,
        operation: &'static str,
        context: &ServiceContext,
        decision: AuthorizationDecision,
        result: &Result<T, ProjectServiceError>,
    ) -> Result<(), ProjectServiceError> {
        let (outcome, error_code) = match result {
            Ok(_) => (ServiceOutcome::Success, None),
            Err(error) => (ServiceOutcome::Failure, Some(error.code().to_owned())),
        };

        self.runtime
            .audit(DOMAIN, operation, context, outcome, error_code, decision)
            .await?;
        Ok(())
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
    use async_trait::async_trait;
    use db::DbResult;
    use domain::{ProjectStatus, WbsCategory};
    use service::{
        CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
        ServiceActorKind,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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

    struct CountingRepository {
        creates: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ProjectRepository for CountingRepository {
        async fn get(&mut self, _id: &str) -> DbResult<Option<Project>> {
            unreachable!("authorization test must not reach repository")
        }

        async fn list(&mut self) -> DbResult<Vec<Project>> {
            unreachable!("authorization test must not reach repository")
        }

        async fn create(&mut self, _request: &CreateProjectRequest) -> DbResult<Project> {
            self.creates.fetch_add(1, Ordering::SeqCst);
            unreachable!("guest command must be denied before repository create")
        }

        async fn update(&mut self, _request: &UpdateProjectRequest) -> DbResult<Option<Project>> {
            unreachable!("authorization test must not reach repository")
        }

        async fn complete(
            &mut self,
            _request: &CompleteProjectRequest,
        ) -> DbResult<Option<Project>> {
            unreachable!("authorization test must not reach repository")
        }
    }

    #[tokio::test]
    async fn guest_command_is_denied_before_repository_and_audited() {
        let creates = Arc::new(AtomicUsize::new(0));
        let audit = CapturingAuditPort::default();
        let events = CapturingDomainEventPort::default();
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(audit.clone()),
            Arc::new(events.clone()),
        );
        let mut project_service = ProjectService::new(
            CountingRepository {
                creates: creates.clone(),
            },
            infrastructure,
        );
        let context = ServiceContext {
            actor: ServiceActor {
                id: None,
                kind: ServiceActorKind::System,
            },
            correlation_id: "guest-denial".into(),
            causation_id: None,
            principal: None,
        };

        let error = project_service
            .create(&create_request("Denied", None), &context)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProjectServiceError::Runtime(ServiceRuntimeError::Forbidden { .. })
        ));
        assert_eq!(creates.load(Ordering::SeqCst), 0);
        assert_eq!(audit.events().len(), 1);
        assert!(events.events().is_empty());
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

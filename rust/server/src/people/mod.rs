use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, PersonDao};
use domain::{
    AttachPersonIdentityRequest, Person, PersonIdentity, PersonSearchResult, SearchPeopleRequest,
    SetPersonDisplayNameRequest,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait PersonRepository: Send {
    async fn get(&mut self, person_id: &str) -> DbResult<Option<Person>>;
    async fn find_by_identity(
        &mut self,
        identity: &PersonIdentity,
    ) -> DbResult<Option<Person>>;
    async fn set_display_name(
        &mut self,
        request: &SetPersonDisplayNameRequest,
    ) -> DbResult<Option<Person>>;
    async fn attach_identity(
        &mut self,
        request: &AttachPersonIdentityRequest,
    ) -> DbResult<PersonIdentity>;
    async fn search(
        &mut self,
        request: &SearchPeopleRequest,
    ) -> DbResult<Vec<PersonSearchResult>>;
}

#[async_trait]
impl PersonRepository for PersonDao {
    async fn get(&mut self, person_id: &str) -> DbResult<Option<Person>> {
        PersonDao::get(self, person_id).await
    }

    async fn find_by_identity(
        &mut self,
        identity: &PersonIdentity,
    ) -> DbResult<Option<Person>> {
        PersonDao::find_by_identity(self, identity).await
    }

    async fn set_display_name(
        &mut self,
        request: &SetPersonDisplayNameRequest,
    ) -> DbResult<Option<Person>> {
        PersonDao::set_display_name(self, request).await
    }

    async fn attach_identity(
        &mut self,
        request: &AttachPersonIdentityRequest,
    ) -> DbResult<PersonIdentity> {
        PersonDao::attach_identity(self, request).await
    }

    async fn search(
        &mut self,
        request: &SearchPeopleRequest,
    ) -> DbResult<Vec<PersonSearchResult>> {
        PersonDao::search(self, request).await
    }
}

pub struct PersonService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: PersonRepository> PersonService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &mut self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Person>, CoreServiceError> {
        const OP: &str = "person.get";
        let decision = authorize(
            &self.runtime,
            "person",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(person_id).await.map_err(Into::into);
        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn find_by_identity(
        &mut self,
        identity: &PersonIdentity,
        context: &ServiceContext,
    ) -> Result<Option<Person>, CoreServiceError> {
        const OP: &str = "person.findByIdentity";
        let decision = authorize(
            &self.runtime,
            "person",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .find_by_identity(identity)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_display_name(
        &mut self,
        request: &SetPersonDisplayNameRequest,
        context: &ServiceContext,
    ) -> Result<Person, CoreServiceError> {
        const OP: &str = "person.setDisplayName";
        let decision = authorize(
            &self.runtime,
            "person",
            "person.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if request.display_name.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PERSON_NAME_REQUIRED",
                    "Person display name is required.",
                ));
            }
            let person = self
                .repository
                .set_display_name(request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "PERSON_NOT_FOUND",
                        format!("Person not found: {}", request.person_id),
                    )
                })?;
            self.runtime
                .emit(
                    "person.display_name_changed",
                    Some(person.id.clone()),
                    BTreeMap::from([
                        ("personId".into(), json!(person.id.clone())),
                        ("displayName".into(), json!(person.display_name.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(person)
        }
        .await;
        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn attach_identity(
        &mut self,
        request: &AttachPersonIdentityRequest,
        context: &ServiceContext,
    ) -> Result<PersonIdentity, CoreServiceError> {
        const OP: &str = "person.attachIdentity";
        let decision = authorize(
            &self.runtime,
            "person",
            "person.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if let Some(owner) = self.repository.find_by_identity(&request.identity).await? {
                if owner.id != request.person_id {
                    return Err(CoreServiceError::business(
                        "IDENTITY_OWNED",
                        format!(
                            "Identity {}:{} is already owned by another Person.",
                            request.identity.kind.as_str(),
                            request.identity.value
                        ),
                    ));
                }
                return Ok(request.identity.clone());
            }

            let identity = self.repository.attach_identity(request).await?;
            self.runtime
                .emit(
                    "person.identity_attached",
                    Some(request.person_id.clone()),
                    BTreeMap::from([
                        ("personId".into(), json!(request.person_id.clone())),
                        ("kind".into(), json!(identity.kind.as_str())),
                        ("value".into(), json!(identity.value.clone())),
                        ("sourceSystem".into(), json!(identity.source_system.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(identity)
        }
        .await;
        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn search(
        &mut self,
        request: &SearchPeopleRequest,
        context: &ServiceContext,
    ) -> Result<Vec<PersonSearchResult>, CoreServiceError> {
        const OP: &str = "person.search";
        let decision = authorize(
            &self.runtime,
            "person",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.search(request).await.map_err(Into::into);
        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }
}

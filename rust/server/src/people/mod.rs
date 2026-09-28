use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{Database, DbResult, PersonDao};
use domain::{
    AttachPersonIdentityRequest, Person, PersonIdentity, PersonSearchResult, SearchPeopleRequest,
    SetPersonDisplayNameRequest, UpdatePersonAdminRequest,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait PersonRepository: Send + Sync {
    fn database(&self) -> Option<Database> {
        None
    }
    async fn get(&self, person_id: &str) -> DbResult<Option<Person>>;
    async fn find_by_identity(&self, identity: &PersonIdentity) -> DbResult<Option<Person>>;
    async fn set_display_name(
        &self,
        request: &SetPersonDisplayNameRequest,
    ) -> DbResult<Option<Person>>;
    async fn attach_identity(
        &self,
        request: &AttachPersonIdentityRequest,
    ) -> DbResult<PersonIdentity>;
    async fn update_admin(&self, request: &UpdatePersonAdminRequest) -> DbResult<Option<Person>>;
    async fn set_contact(&self, person_id: &str, kind: &str, value: &str) -> DbResult<Option<String>>;
    async fn create_seller(&self, display_name: &str) -> DbResult<Person>;
    async fn search(&self, request: &SearchPeopleRequest) -> DbResult<Vec<PersonSearchResult>>;
}

#[async_trait]
impl PersonRepository for PersonDao {
    fn database(&self) -> Option<Database> {
        Some(PersonDao::database(self))
    }
    async fn get(&self, person_id: &str) -> DbResult<Option<Person>> {
        PersonDao::get(self, person_id).await
    }

    async fn find_by_identity(&self, identity: &PersonIdentity) -> DbResult<Option<Person>> {
        PersonDao::find_by_identity(self, identity).await
    }

    async fn set_display_name(
        &self,
        request: &SetPersonDisplayNameRequest,
    ) -> DbResult<Option<Person>> {
        PersonDao::set_display_name(self, request).await
    }

    async fn attach_identity(
        &self,
        request: &AttachPersonIdentityRequest,
    ) -> DbResult<PersonIdentity> {
        PersonDao::attach_identity(self, request).await
    }

    async fn update_admin(&self, request: &UpdatePersonAdminRequest) -> DbResult<Option<Person>> {
        PersonDao::update_admin(self, request).await
    }

    async fn set_contact(&self, person_id: &str, kind: &str, value: &str) -> DbResult<Option<String>> {
        PersonDao::set_contact(self, person_id, kind, value).await
    }

    async fn create_seller(&self, display_name: &str) -> DbResult<Person> {
        PersonDao::create_seller(self, display_name).await
    }

    async fn search(&self, request: &SearchPeopleRequest) -> DbResult<Vec<PersonSearchResult>> {
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
        &self,
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
        &self,
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
        &self,
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

        let result = db::service_mutation(self.repository.database(), async {
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
        })
        .await;

        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    /// The seller a contract names, as a new person — used when no one with that name exists yet (the contract is the
    /// accurate source of a seller's legal name).
    pub async fn create_seller(&self, display_name: &str, context: &ServiceContext) -> Result<Person, CoreServiceError> {
        const OP: &str = "person.createSeller";
        let decision = authorize(&self.runtime, "person", "person.write", OP, OperationKind::Command, context).await?;
        let result = db::service_mutation(self.repository.database(), async {
            if display_name.trim().is_empty() {
                return Err(CoreServiceError::business("PERSON_NAME_REQUIRED", "Person display name is required."));
            }
            let person = self.repository.create_seller(display_name).await?;
            self.runtime
                .emit(
                    "person.created",
                    Some(person.id.clone()),
                    BTreeMap::from([
                        ("personId".into(), json!(person.id.clone())),
                        ("displayName".into(), json!(person.display_name.clone())),
                        ("source".into(), json!("contract")),
                    ]),
                    context,
                )
                .await?;
            Ok(person)
        })
        .await;
        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn update_admin(
        &self,
        request: &UpdatePersonAdminRequest,
        context: &ServiceContext,
    ) -> Result<Person, CoreServiceError> {
        const OP: &str = "person.updateAdmin";
        let decision = authorize(
            &self.runtime,
            "person",
            "person.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            if request.display_name.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PERSON_NAME_REQUIRED",
                    "Person display name is required.",
                ));
            }
            const STATUSES: &[&str] = &["new", "warm", "active", "referral"];
            if !STATUSES.contains(&request.status.trim()) {
                return Err(CoreServiceError::business(
                    "PERSON_STATUS_INVALID",
                    "Person status is invalid.",
                ));
            }
            const CIVIL_STATUSES: &[&str] = &["Single", "Married", "Divorced", "Widowed"];
            if let Some(civil_status) = request
                .civil_status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if !CIVIL_STATUSES.contains(&civil_status) {
                    return Err(CoreServiceError::business(
                        "PERSON_CIVIL_STATUS_INVALID",
                        "Civil status must be Single, Married, Divorced, or Widowed.",
                    ));
                }
            }

            let person = self
                .repository
                .update_admin(request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "PERSON_NOT_FOUND",
                        format!("Person not found: {}", request.person_id),
                    )
                })?;

            // What is typed here overrides what any intake gave: the record shows these, as typed.
            for (kind, label, value) in [("email", "email", &request.email), ("phone", "phone number", &request.phone)] {
                let Some(value) = value.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
                    continue;
                };
                if let Some(owner) = self.repository.set_contact(&person.id, kind, value).await? {
                    return Err(CoreServiceError::business(
                        "PERSON_CONTACT_TAKEN",
                        format!("That {label} already belongs to {owner}."),
                    ));
                }
            }

            self.runtime
                .emit(
                    "person.admin_updated",
                    Some(person.id.clone()),
                    BTreeMap::from([
                        ("personId".into(), json!(person.id.clone())),
                        ("displayName".into(), json!(person.display_name.clone())),
                        ("civilStatus".into(), json!(person.civil_status.clone())),
                        ("status".into(), json!(person.status.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(person)
        })
        .await;

        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn attach_identity(
        &self,
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

        let result = db::service_mutation(self.repository.database(), async {
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
        })
        .await;

        audit_result(&self.runtime, "person", OP, context, decision, &result).await?;
        result
    }

    pub async fn search(
        &self,
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

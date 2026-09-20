use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, PropertyDao};
use domain::{
    FindPropertyByAddressRequest, PersonPropertyContext, Property, PropertyForPerson,
    SetPropertyDisplayNameRequest, SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait PropertyRepository: Send {
    async fn get(&mut self, property_id: &str) -> DbResult<Option<Property>>;
    async fn find_by_address(
        &mut self,
        request: &FindPropertyByAddressRequest,
    ) -> DbResult<Option<Property>>;
    async fn for_person(&mut self, person_id: &str) -> DbResult<PersonPropertyContext>;
    async fn upsert_for_person(
        &mut self,
        request: &UpsertPropertyForPersonRequest,
    ) -> DbResult<PropertyForPerson>;
    async fn set_display_name(
        &mut self,
        request: &SetPropertyDisplayNameRequest,
    ) -> DbResult<Option<Property>>;
    async fn set_status(
        &mut self,
        request: &SetPropertyStatusRequest,
    ) -> DbResult<Option<Property>>;
}

#[async_trait]
impl PropertyRepository for PropertyDao {
    async fn get(&mut self, property_id: &str) -> DbResult<Option<Property>> {
        PropertyDao::get(self, property_id).await
    }

    async fn find_by_address(
        &mut self,
        request: &FindPropertyByAddressRequest,
    ) -> DbResult<Option<Property>> {
        PropertyDao::find_by_address(self, request).await
    }

    async fn for_person(&mut self, person_id: &str) -> DbResult<PersonPropertyContext> {
        PropertyDao::for_person(self, person_id).await
    }

    async fn upsert_for_person(
        &mut self,
        request: &UpsertPropertyForPersonRequest,
    ) -> DbResult<PropertyForPerson> {
        PropertyDao::upsert_for_person(self, request).await
    }

    async fn set_display_name(
        &mut self,
        request: &SetPropertyDisplayNameRequest,
    ) -> DbResult<Option<Property>> {
        PropertyDao::set_display_name(self, request).await
    }

    async fn set_status(
        &mut self,
        request: &SetPropertyStatusRequest,
    ) -> DbResult<Option<Property>> {
        PropertyDao::set_status(self, request).await
    }
}

pub struct PropertyService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: PropertyRepository> PropertyService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &mut self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Property>, CoreServiceError> {
        const OP: &str = "property.get";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(property_id).await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn find_by_address(
        &mut self,
        request: &FindPropertyByAddressRequest,
        context: &ServiceContext,
    ) -> Result<Option<Property>, CoreServiceError> {
        const OP: &str = "property.findByAddress";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .find_by_address(request)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn for_person(
        &mut self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<PersonPropertyContext, CoreServiceError> {
        const OP: &str = "property.forPerson";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.for_person(person_id).await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn upsert_for_person(
        &mut self,
        request: &UpsertPropertyForPersonRequest,
        context: &ServiceContext,
    ) -> Result<PropertyForPerson, CoreServiceError> {
        const OP: &str = "property.upsertForPerson";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let linked = self.repository.upsert_for_person(request).await?;
            self.runtime
                .emit(
                    "property.person_context_upserted",
                    Some(linked.property.id.clone()),
                    BTreeMap::from([
                        ("personId".into(), json!(request.person_id.clone())),
                        ("propertyId".into(), json!(linked.property.id.clone())),
                        ("relation".into(), json!(linked.relation.as_str())),
                        (
                            "sourceType".into(),
                            json!(request.source_type.clone().unwrap_or_else(|| "manual".into())),
                        ),
                    ]),
                    context,
                )
                .await?;
            Ok(linked)
        }
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_display_name(
        &mut self,
        request: &SetPropertyDisplayNameRequest,
        context: &ServiceContext,
    ) -> Result<Property, CoreServiceError> {
        const OP: &str = "property.setDisplayName";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let property = self
                .repository
                .set_display_name(request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "PROPERTY_NOT_FOUND",
                        format!("Property not found: {}", request.property_id),
                    )
                })?;
            self.runtime
                .emit(
                    "property.display_name_changed",
                    Some(property.id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(property.id.clone())),
                        ("displayName".into(), json!(property.display_name.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(property)
        }
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_status(
        &mut self,
        request: &SetPropertyStatusRequest,
        context: &ServiceContext,
    ) -> Result<Property, CoreServiceError> {
        const OP: &str = "property.setStatus";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let property = self
                .repository
                .set_status(request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "PROPERTY_NOT_FOUND",
                        format!("Property not found: {}", request.property_id),
                    )
                })?;
            self.runtime
                .emit(
                    "property.status_changed",
                    Some(property.id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(property.id.clone())),
                        ("status".into(), json!(property.status.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(property)
        }
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }
}

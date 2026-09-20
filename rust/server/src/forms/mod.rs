use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, FormDao};
use domain::{
    BindFormInstanceToDirectContextRequest, BindFormInstanceToShowingRequest,
    BindListingFormContextRequest, CreateFormInstanceRequest, DealFormFacts, DirectFormContext,
    FormInstance, FormInstanceEvidence, FormInstanceListItem, FormSignerPerson,
    LatestFormEvidenceRequest, UpdateFormInstanceRequest,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait FormRepository: Send {
    async fn create_instance(
        &mut self,
        request: &CreateFormInstanceRequest,
    ) -> DbResult<FormInstance>;
    async fn get_instance(&mut self, form_instance_id: &str) -> DbResult<Option<FormInstance>>;
    async fn update_instance(
        &mut self,
        request: &UpdateFormInstanceRequest,
    ) -> DbResult<Option<FormInstance>>;
    async fn list_instances(&mut self) -> DbResult<Vec<FormInstanceListItem>>;
    async fn deal_facts(&mut self, deal_id: &str) -> DbResult<Option<DealFormFacts>>;
    async fn seed_participants_from_deal(
        &mut self,
        form_instance_id: &str,
        deal_id: &str,
    ) -> DbResult<()>;
    async fn latest_evidence(
        &mut self,
        request: &LatestFormEvidenceRequest,
    ) -> DbResult<Option<FormInstanceEvidence>>;
    async fn resolve_deal_launch_context(
        &mut self,
        deal_id: &str,
    ) -> DbResult<Option<DirectFormContext>>;
    async fn bind_direct_context(
        &mut self,
        request: &BindFormInstanceToDirectContextRequest,
    ) -> DbResult<bool>;
    async fn bind_listing_context(
        &mut self,
        request: &BindListingFormContextRequest,
    ) -> DbResult<bool>;
    async fn get_showing_id(&mut self, form_instance_id: &str) -> DbResult<Option<String>>;
    async fn bind_showing(&mut self, request: &BindFormInstanceToShowingRequest) -> DbResult<bool>;
    async fn list_signer_people(
        &mut self,
        form_instance_id: &str,
    ) -> DbResult<Vec<FormSignerPerson>>;
}

#[async_trait]
impl FormRepository for FormDao {
    async fn create_instance(
        &mut self,
        request: &CreateFormInstanceRequest,
    ) -> DbResult<FormInstance> {
        FormDao::create_instance(self, request).await
    }

    async fn get_instance(&mut self, form_instance_id: &str) -> DbResult<Option<FormInstance>> {
        FormDao::get_instance(self, form_instance_id).await
    }

    async fn update_instance(
        &mut self,
        request: &UpdateFormInstanceRequest,
    ) -> DbResult<Option<FormInstance>> {
        FormDao::update_instance(self, request).await
    }

    async fn list_instances(&mut self) -> DbResult<Vec<FormInstanceListItem>> {
        FormDao::list_instances(self).await
    }

    async fn deal_facts(&mut self, deal_id: &str) -> DbResult<Option<DealFormFacts>> {
        FormDao::deal_facts(self, deal_id).await
    }

    async fn seed_participants_from_deal(
        &mut self,
        form_instance_id: &str,
        deal_id: &str,
    ) -> DbResult<()> {
        FormDao::seed_participants_from_deal(self, form_instance_id, deal_id).await
    }

    async fn latest_evidence(
        &mut self,
        request: &LatestFormEvidenceRequest,
    ) -> DbResult<Option<FormInstanceEvidence>> {
        FormDao::latest_evidence(self, request).await
    }

    async fn resolve_deal_launch_context(
        &mut self,
        deal_id: &str,
    ) -> DbResult<Option<DirectFormContext>> {
        FormDao::resolve_deal_launch_context(self, deal_id).await
    }

    async fn bind_direct_context(
        &mut self,
        request: &BindFormInstanceToDirectContextRequest,
    ) -> DbResult<bool> {
        FormDao::bind_direct_context(self, request).await
    }

    async fn bind_listing_context(
        &mut self,
        request: &BindListingFormContextRequest,
    ) -> DbResult<bool> {
        FormDao::bind_listing_context(self, request).await
    }

    async fn get_showing_id(&mut self, form_instance_id: &str) -> DbResult<Option<String>> {
        FormDao::get_showing_id(self, form_instance_id).await
    }

    async fn bind_showing(&mut self, request: &BindFormInstanceToShowingRequest) -> DbResult<bool> {
        FormDao::bind_showing(self, request).await
    }

    async fn list_signer_people(
        &mut self,
        form_instance_id: &str,
    ) -> DbResult<Vec<FormSignerPerson>> {
        FormDao::list_signer_people(self, form_instance_id).await
    }
}

pub struct FormService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: FormRepository> FormService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn create_instance(
        &mut self,
        request: &CreateFormInstanceRequest,
        context: &ServiceContext,
    ) -> Result<FormInstance, CoreServiceError> {
        const OP: &str = "form.createInstance";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            if request.template_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "FORM_TEMPLATE_REQUIRED",
                    "templateId is required.",
                ));
            }
            if request.deal_id.as_deref().is_none_or(str::is_empty)
                && request.person_id.as_deref().is_none_or(str::is_empty)
                && request.property_id.as_deref().is_none_or(str::is_empty)
            {
                return Err(CoreServiceError::business(
                    "FORM_CONTEXT_REQUIRED",
                    "A deal, client, or property is required.",
                ));
            }

            let instance = self.repository.create_instance(request).await?;
            self.runtime
                .emit(
                    "form.instance_created",
                    Some(instance.id.clone()),
                    BTreeMap::from([
                        ("formInstanceId".into(), json!(instance.id.clone())),
                        ("templateId".into(), json!(instance.template_id.clone())),
                        ("templateVersion".into(), json!(instance.template_version)),
                        ("dealId".into(), json!(instance.deal_id.clone())),
                        ("personId".into(), json!(instance.person_id.clone())),
                        ("propertyId".into(), json!(instance.property_id.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(instance)
        }
        .await;

        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn get_instance(
        &mut self,
        form_instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<FormInstance>, CoreServiceError> {
        const OP: &str = "form.getInstance";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .get_instance(form_instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn update_instance(
        &mut self,
        request: &UpdateFormInstanceRequest,
        context: &ServiceContext,
    ) -> Result<Option<FormInstance>, CoreServiceError> {
        const OP: &str = "form.updateInstance";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let instance = self.repository.update_instance(request).await?;
            if let Some(instance) = &instance {
                self.runtime
                    .emit(
                        "form.instance_updated",
                        Some(instance.id.clone()),
                        BTreeMap::from([
                            ("formInstanceId".into(), json!(instance.id.clone())),
                            ("status".into(), json!(instance.status.as_str())),
                        ]),
                        context,
                    )
                    .await?;
            }
            Ok(instance)
        }
        .await;
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn list_instances(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<FormInstanceListItem>, CoreServiceError> {
        const OP: &str = "form.listInstances";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list_instances().await.map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn deal_facts(
        &mut self,
        deal_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<DealFormFacts>, CoreServiceError> {
        const OP: &str = "form.dealFacts";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .deal_facts(deal_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn seed_participants_from_deal(
        &mut self,
        form_instance_id: &str,
        deal_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "form.seedParticipantsFromDeal";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self
            .repository
            .seed_participants_from_deal(form_instance_id, deal_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn latest_evidence(
        &mut self,
        request: &LatestFormEvidenceRequest,
        context: &ServiceContext,
    ) -> Result<Option<FormInstanceEvidence>, CoreServiceError> {
        const OP: &str = "form.latestEvidence";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .latest_evidence(request)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn resolve_deal_launch_context(
        &mut self,
        deal_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<DirectFormContext>, CoreServiceError> {
        const OP: &str = "form.resolveDealLaunchContext";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .resolve_deal_launch_context(deal_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn bind_direct_context(
        &mut self,
        request: &BindFormInstanceToDirectContextRequest,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        self.bind(
            "form.bindDirectContext",
            "form.instance_bound_direct",
            request.form_instance_id.clone(),
            BTreeMap::from([
                (
                    "formInstanceId".into(),
                    json!(request.form_instance_id.clone()),
                ),
                ("personId".into(), json!(request.person_id.clone())),
                ("propertyId".into(), json!(request.property_id.clone())),
            ]),
            context,
            FormBind::Direct(request),
        )
        .await
    }

    pub async fn bind_listing_context(
        &mut self,
        request: &BindListingFormContextRequest,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        self.bind(
            "form.bindListingContext",
            "form.listing_context_bound",
            request.form_instance_id.clone(),
            BTreeMap::from([
                (
                    "formInstanceId".into(),
                    json!(request.form_instance_id.clone()),
                ),
                ("personId".into(), json!(request.person_id.clone())),
                ("propertyId".into(), json!(request.property_id.clone())),
            ]),
            context,
            FormBind::Listing(request),
        )
        .await
    }

    pub async fn get_showing_id(
        &mut self,
        form_instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<String>, CoreServiceError> {
        const OP: &str = "form.getShowingId";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .get_showing_id(form_instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    pub async fn bind_showing(
        &mut self,
        request: &BindFormInstanceToShowingRequest,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        self.bind(
            "form.bindShowing",
            "form.instance_bound_showing",
            request.form_instance_id.clone(),
            BTreeMap::from([
                (
                    "formInstanceId".into(),
                    json!(request.form_instance_id.clone()),
                ),
                ("showingId".into(), json!(request.showing_id.clone())),
            ]),
            context,
            FormBind::Showing(request),
        )
        .await
    }

    pub async fn list_signer_people(
        &mut self,
        form_instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<FormSignerPerson>, CoreServiceError> {
        const OP: &str = "form.listSignerPeople";
        let decision = authorize(
            &self.runtime,
            "form",
            "form.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .list_signer_people(form_instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "form", OP, context, decision, &result).await?;
        result
    }

    async fn bind(
        &mut self,
        operation: &'static str,
        event_type: &'static str,
        aggregate_id: String,
        payload: BTreeMap<String, serde_json::Value>,
        context: &ServiceContext,
        bind: FormBind<'_>,
    ) -> Result<(), CoreServiceError> {
        let decision = authorize(
            &self.runtime,
            "form",
            "form.write",
            operation,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let bound = match bind {
                FormBind::Direct(request) => self.repository.bind_direct_context(request).await?,
                FormBind::Listing(request) => self.repository.bind_listing_context(request).await?,
                FormBind::Showing(request) => self.repository.bind_showing(request).await?,
            };
            if !bound {
                return Err(CoreServiceError::business(
                    "FORM_BIND_CONFLICT",
                    "Form instance was not found, is immutable, or conflicts with the requested binding.",
                ));
            }
            self.runtime
                .emit(event_type, Some(aggregate_id), payload, context)
                .await?;
            Ok(())
        }
        .await;

        audit_result(&self.runtime, "form", operation, context, decision, &result).await?;
        result
    }
}

enum FormBind<'a> {
    Direct(&'a BindFormInstanceToDirectContextRequest),
    Listing(&'a BindListingFormContextRequest),
    Showing(&'a BindFormInstanceToShowingRequest),
}

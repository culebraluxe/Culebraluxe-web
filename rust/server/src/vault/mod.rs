use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, VaultDao};
use domain::{
    ContractIssuedLineage, CreateTransactionDocumentRequest, IssueDocumentRequest,
    IssuedDocumentForFormInstance, IssuedDocumentListItem, NextIssuedVersionRequest,
    TransactionDocument, TransitionTransactionDocumentRequest, VaultActorScope,
    VaultArtifactFailure, VaultCommandOutcome, VaultCommandResult, VaultMediaBytes,
    VaultRenderRequest, VaultRenderedArtifact,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;
use std::sync::Arc;

#[async_trait]
pub trait VaultArtifactPort: Send + Sync {
    async fn render_issued_document(
        &self,
        request: VaultRenderRequest,
    ) -> Result<VaultRenderedArtifact, VaultArtifactFailure>;
}

#[async_trait]
pub trait VaultRepository: Send {
    async fn list_issued_documents(
        &mut self,
        actor: Option<&VaultActorScope>,
    ) -> DbResult<Vec<IssuedDocumentListItem>>;
    async fn get_document(&mut self, document_id: &str) -> DbResult<Option<TransactionDocument>>;
    async fn list_by_deal(&mut self, deal_id: &str) -> DbResult<Vec<TransactionDocument>>;
    async fn create_document(
        &mut self,
        request: &CreateTransactionDocumentRequest,
    ) -> DbResult<TransactionDocument>;
    async fn transition_state(
        &mut self,
        request: &TransitionTransactionDocumentRequest,
    ) -> DbResult<VaultCommandResult>;
    async fn issued_for_form_instance(
        &mut self,
        form_instance_id: &str,
    ) -> DbResult<Option<IssuedDocumentForFormInstance>>;
    async fn next_issued_version(&mut self, request: &NextIssuedVersionRequest) -> DbResult<i32>;
    async fn media_bytes(&mut self, media_id: &str) -> DbResult<Option<VaultMediaBytes>>;
    async fn public_listing_document_bytes(
        &mut self,
        media_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>>;
    async fn form_contract_id(&mut self, form_instance_id: &str) -> DbResult<Option<String>>;
    async fn bind_form_to_contract(
        &mut self,
        form_instance_id: &str,
        contract_id: &str,
    ) -> DbResult<bool>;
    async fn prior_contract_document(
        &mut self,
        contract_id: &str,
        template_id: &str,
    ) -> DbResult<Option<ContractIssuedLineage>>;
    async fn issue_from_form_instance(
        &mut self,
        request: &IssueDocumentRequest,
        artifacts: Arc<dyn VaultArtifactPort>,
    ) -> DbResult<VaultCommandResult>;
}

#[async_trait]
impl VaultRepository for VaultDao {
    async fn list_issued_documents(
        &mut self,
        actor: Option<&VaultActorScope>,
    ) -> DbResult<Vec<IssuedDocumentListItem>> {
        VaultDao::list_issued_documents(self, actor).await
    }

    async fn get_document(&mut self, document_id: &str) -> DbResult<Option<TransactionDocument>> {
        VaultDao::get_document(self, document_id).await
    }

    async fn list_by_deal(&mut self, deal_id: &str) -> DbResult<Vec<TransactionDocument>> {
        VaultDao::list_by_deal(self, deal_id).await
    }

    async fn create_document(
        &mut self,
        request: &CreateTransactionDocumentRequest,
    ) -> DbResult<TransactionDocument> {
        VaultDao::create_document(self, request).await
    }

    async fn transition_state(
        &mut self,
        request: &TransitionTransactionDocumentRequest,
    ) -> DbResult<VaultCommandResult> {
        VaultDao::transition_state(self, request).await
    }

    async fn issued_for_form_instance(
        &mut self,
        form_instance_id: &str,
    ) -> DbResult<Option<IssuedDocumentForFormInstance>> {
        VaultDao::issued_for_form_instance(self, form_instance_id).await
    }

    async fn next_issued_version(&mut self, request: &NextIssuedVersionRequest) -> DbResult<i32> {
        VaultDao::next_issued_version(self, request).await
    }

    async fn media_bytes(&mut self, media_id: &str) -> DbResult<Option<VaultMediaBytes>> {
        VaultDao::media_bytes(self, media_id).await
    }

    async fn public_listing_document_bytes(
        &mut self,
        media_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        VaultDao::public_listing_document_bytes(self, media_id).await
    }

    async fn form_contract_id(&mut self, form_instance_id: &str) -> DbResult<Option<String>> {
        VaultDao::form_contract_id(self, form_instance_id).await
    }

    async fn bind_form_to_contract(
        &mut self,
        form_instance_id: &str,
        contract_id: &str,
    ) -> DbResult<bool> {
        VaultDao::bind_form_to_contract(self, form_instance_id, contract_id).await
    }

    async fn prior_contract_document(
        &mut self,
        contract_id: &str,
        template_id: &str,
    ) -> DbResult<Option<ContractIssuedLineage>> {
        VaultDao::prior_contract_document(self, contract_id, template_id).await
    }

    async fn issue_from_form_instance(
        &mut self,
        request: &IssueDocumentRequest,
        artifacts: Arc<dyn VaultArtifactPort>,
    ) -> DbResult<VaultCommandResult> {
        VaultDao::issue_from_form_instance(self, request, move |render_request| {
            let artifacts = artifacts.clone();
            async move { artifacts.render_issued_document(render_request).await }
        })
        .await
    }
}

pub struct VaultService<R> {
    repository: R,
    runtime: ServiceRuntime,
    artifacts: Arc<dyn VaultArtifactPort>,
}

impl<R: VaultRepository> VaultService<R> {
    pub fn new(
        repository: R,
        artifacts: Arc<dyn VaultArtifactPort>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
            artifacts,
        }
    }

    pub async fn list_issued_documents(
        &mut self,
        actor: Option<&VaultActorScope>,
        context: &ServiceContext,
    ) -> Result<Vec<IssuedDocumentListItem>, CoreServiceError> {
        const OP: &str = "vault.listIssuedDocuments";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .list_issued_documents(actor)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn get_document(
        &mut self,
        document_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<TransactionDocument>, CoreServiceError> {
        const OP: &str = "vault.getDocument";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .get_document(document_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn list_by_deal(
        &mut self,
        deal_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<TransactionDocument>, CoreServiceError> {
        const OP: &str = "vault.listByDeal";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .list_by_deal(deal_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn create_document(
        &mut self,
        request: &CreateTransactionDocumentRequest,
        context: &ServiceContext,
    ) -> Result<TransactionDocument, CoreServiceError> {
        const OP: &str = "vault.createDocument";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let document = self.repository.create_document(request).await?;
            self.runtime
                .emit(
                    "vault.document_created",
                    Some(document.id.clone()),
                    BTreeMap::from([
                        ("documentId".into(), json!(document.id.clone())),
                        ("state".into(), json!(document.state.as_str())),
                        ("source".into(), json!(document.source.as_str())),
                    ]),
                    context,
                )
                .await?;
            Ok(document)
        }
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn transition_state(
        &mut self,
        request: &TransitionTransactionDocumentRequest,
        context: &ServiceContext,
    ) -> Result<VaultCommandResult, CoreServiceError> {
        const OP: &str = "vault.transitionState";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let command = self.repository.transition_state(request).await?;
            if command.outcome == VaultCommandOutcome::Success && !command.replayed {
                self.runtime
                    .emit(
                        "vault.document_state_changed",
                        command.aggregate_id.clone(),
                        BTreeMap::from([
                            ("commandId".into(), json!(command.command_id.clone())),
                            ("to".into(), json!(request.to.as_str())),
                        ]),
                        context,
                    )
                    .await?;
            }
            Ok(command)
        }
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn issued_for_form_instance(
        &mut self,
        form_instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<IssuedDocumentForFormInstance>, CoreServiceError> {
        const OP: &str = "vault.issuedForFormInstance";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .issued_for_form_instance(form_instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn next_issued_version(
        &mut self,
        request: &NextIssuedVersionRequest,
        context: &ServiceContext,
    ) -> Result<i32, CoreServiceError> {
        const OP: &str = "vault.nextIssuedVersion";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .next_issued_version(request)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn media_bytes(
        &mut self,
        media_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultMediaBytes>, CoreServiceError> {
        const OP: &str = "vault.mediaBytes";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .media_bytes(media_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn public_listing_document_bytes(
        &mut self,
        media_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultMediaBytes>, CoreServiceError> {
        const OP: &str = "vault.publicListingDocumentBytes";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.publicListingDocument.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .public_listing_document_bytes(media_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn form_contract_id(
        &mut self,
        form_instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<String>, CoreServiceError> {
        const OP: &str = "vault.formContractId";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .form_contract_id(form_instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn bind_form_to_contract(
        &mut self,
        form_instance_id: &str,
        contract_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "vault.bindFormToContract";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let bound = self
                .repository
                .bind_form_to_contract(form_instance_id, contract_id)
                .await?;
            if !bound {
                return Err(CoreServiceError::business(
                    "FORM_CONTRACT_CONFLICT",
                    "Form instance was not found or is already bound to another Contract.",
                ));
            }
            self.runtime
                .emit(
                    "vault.form_bound_to_contract",
                    Some(contract_id.to_owned()),
                    BTreeMap::from([
                        ("formInstanceId".into(), json!(form_instance_id)),
                        ("contractId".into(), json!(contract_id)),
                    ]),
                    context,
                )
                .await?;
            Ok(())
        }
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn prior_contract_document(
        &mut self,
        contract_id: &str,
        template_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<ContractIssuedLineage>, CoreServiceError> {
        const OP: &str = "vault.priorContractDocument";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .prior_contract_document(contract_id, template_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn issue_from_form_instance(
        &mut self,
        request: &IssueDocumentRequest,
        context: &ServiceContext,
    ) -> Result<VaultCommandResult, CoreServiceError> {
        const OP: &str = "vault.issueFromFormInstance";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.issue",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let command = self
                .repository
                .issue_from_form_instance(request, self.artifacts.clone())
                .await?;
            if command.outcome == VaultCommandOutcome::Success && !command.replayed {
                self.runtime
                    .emit(
                        "vault.document_issued",
                        command.aggregate_id.clone(),
                        BTreeMap::from([
                            ("commandId".into(), json!(command.command_id.clone())),
                            (
                                "formInstanceId".into(),
                                json!(request.form_instance_id.clone()),
                            ),
                            (
                                "result".into(),
                                command.value.clone().unwrap_or(json!(null)),
                            ),
                        ]),
                        context,
                    )
                    .await?;
            }
            Ok(command)
        }
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }
}

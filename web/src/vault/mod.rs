pub mod artifact;
pub mod forms_render;
pub mod pdf;
pub mod signing_certificate;
pub mod signing_overlay;

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{Database, DbResult, VaultDao};
use model::forms_applied_signature::FormAppliedSignature;
use model::{
    ContractIssuedLineage, CreateTransactionDocumentRequest, IssueDocumentRequest,
    IssuedDocumentForFormInstance, IssuedDocumentListItem, NextIssuedVersionRequest,
    TransactionDocument, TransitionTransactionDocumentRequest, VaultActorScope,
    VaultArtifactFailure, VaultCommandOutcome, VaultCommandResult, VaultMediaBytes,
    VaultRenderRequest, VaultRenderedArtifact,
};
use serde_json::json;
use services::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
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
pub trait VaultRepository: Send + Sync {
    fn database(&self) -> Option<Database> {
        None
    }
    async fn list_issued_documents(
        &self,
        actor: Option<&VaultActorScope>,
    ) -> DbResult<Vec<IssuedDocumentListItem>>;
    async fn get_document(&self, document_id: &str) -> DbResult<Option<TransactionDocument>>;
    async fn list_by_deal(&self, deal_id: &str) -> DbResult<Vec<TransactionDocument>>;
    async fn create_document(
        &self,
        request: &CreateTransactionDocumentRequest,
    ) -> DbResult<TransactionDocument>;
    async fn transition_state(
        &self,
        request: &TransitionTransactionDocumentRequest,
    ) -> DbResult<VaultCommandResult>;
    async fn issued_for_form_instance(
        &self,
        form_instance_id: &str,
    ) -> DbResult<Option<IssuedDocumentForFormInstance>>;
    async fn next_issued_version(&self, request: &NextIssuedVersionRequest) -> DbResult<i32>;
    async fn media_bytes(&self, media_id: &str) -> DbResult<Option<VaultMediaBytes>>;
    async fn public_listing_document_bytes(
        &self,
        media_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>>;
    async fn signing_document_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>>;
    async fn signing_signed_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>>;
    async fn completion_artifacts(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<VaultMediaBytes>>;
    async fn form_contract_id(&self, form_instance_id: &str) -> DbResult<Option<String>>;
    async fn bind_form_to_contract(
        &self,
        form_instance_id: &str,
        contract_id: &str,
    ) -> DbResult<bool>;
    async fn prior_contract_document(
        &self,
        contract_id: &str,
        template_id: &str,
    ) -> DbResult<Option<ContractIssuedLineage>>;
    /// The brokerage's standing pre-signature as it will be APPLIED to this document, resolved by the same rules
    /// issuance obeys. A refusal (`VaultArtifactFailure`) is that rule set saying the document cannot carry it.
    async fn resolve_applied_signatures(
        &self,
        request: &VaultRenderRequest,
    ) -> Result<Vec<FormAppliedSignature>, VaultArtifactFailure>;
    async fn issue_from_form_instance(
        &self,
        request: &IssueDocumentRequest,
        artifacts: Arc<dyn VaultArtifactPort>,
    ) -> DbResult<VaultCommandResult>;
}

#[async_trait]
impl VaultRepository for VaultDao {
    fn database(&self) -> Option<Database> {
        Some(VaultDao::database(self))
    }
    async fn list_issued_documents(
        &self,
        actor: Option<&VaultActorScope>,
    ) -> DbResult<Vec<IssuedDocumentListItem>> {
        VaultDao::list_issued_documents(self, actor).await
    }

    async fn get_document(&self, document_id: &str) -> DbResult<Option<TransactionDocument>> {
        VaultDao::get_document(self, document_id).await
    }

    async fn list_by_deal(&self, deal_id: &str) -> DbResult<Vec<TransactionDocument>> {
        VaultDao::list_by_deal(self, deal_id).await
    }

    async fn create_document(
        &self,
        request: &CreateTransactionDocumentRequest,
    ) -> DbResult<TransactionDocument> {
        VaultDao::create_document(self, request).await
    }

    async fn transition_state(
        &self,
        request: &TransitionTransactionDocumentRequest,
    ) -> DbResult<VaultCommandResult> {
        VaultDao::transition_state(self, request).await
    }

    async fn issued_for_form_instance(
        &self,
        form_instance_id: &str,
    ) -> DbResult<Option<IssuedDocumentForFormInstance>> {
        VaultDao::issued_for_form_instance(self, form_instance_id).await
    }

    async fn next_issued_version(&self, request: &NextIssuedVersionRequest) -> DbResult<i32> {
        VaultDao::next_issued_version(self, request).await
    }

    async fn media_bytes(&self, media_id: &str) -> DbResult<Option<VaultMediaBytes>> {
        VaultDao::media_bytes(self, media_id).await
    }

    async fn public_listing_document_bytes(
        &self,
        media_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        VaultDao::public_listing_document_bytes(self, media_id).await
    }

    async fn signing_document_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        VaultDao::signing_document_bytes(self, signature_request_id, recipient_id).await
    }

    async fn signing_signed_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        VaultDao::signing_signed_bytes(self, signature_request_id, recipient_id).await
    }

    async fn completion_artifacts(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<VaultMediaBytes>> {
        VaultDao::completion_artifacts(self, signature_request_id).await
    }

    async fn form_contract_id(&self, form_instance_id: &str) -> DbResult<Option<String>> {
        VaultDao::form_contract_id(self, form_instance_id).await
    }

    async fn bind_form_to_contract(
        &self,
        form_instance_id: &str,
        contract_id: &str,
    ) -> DbResult<bool> {
        VaultDao::bind_form_to_contract(self, form_instance_id, contract_id).await
    }

    async fn prior_contract_document(
        &self,
        contract_id: &str,
        template_id: &str,
    ) -> DbResult<Option<ContractIssuedLineage>> {
        VaultDao::prior_contract_document(self, contract_id, template_id).await
    }

    async fn resolve_applied_signatures(
        &self,
        request: &VaultRenderRequest,
    ) -> Result<Vec<FormAppliedSignature>, VaultArtifactFailure> {
        VaultDao::resolve_applied_signatures(self, request).await
    }

    async fn issue_from_form_instance(
        &self,
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
        &self,
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
        &self,
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
        &self,
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
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
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
        })
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn transition_state(
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
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
        })
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn issued_for_form_instance(
        &self,
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

    /// Render the current mutable form draft without issuing or persisting a document.
    ///
    /// The browser preview uses the same artifact port as issuance, but remains a query:
    /// no transaction_document row, receipt, version transition, or signature state is written.
    pub async fn render_form_preview(
        &self,
        request: VaultRenderRequest,
        context: &ServiceContext,
    ) -> Result<VaultRenderedArtifact, CoreServiceError> {
        const OP: &str = "vault.renderFormPreview";
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
            .artifacts
            .render_issued_document(request)
            .await
            .map_err(|failure| {
                CoreServiceError::business("VAULT_PREVIEW_RENDER_FAILED", failure.message)
            });

        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    /// The brokerage's standing pre-signature, resolved exactly as issuance resolves it, for a caller that only wants
    /// to SHOW it.
    ///
    /// The preview pane and the issued document must draw the same signature, and that is why both ask this one
    /// resolver: the pane that resolves it for itself drifts from the document the moment either side changes
    /// (`docs/agent/HANDOFF-docsign-native-2026-10-07.md`).
    ///
    /// It is a query: nothing is applied, nothing is written and no version moves. It resolves under
    /// `OwnerByConstruction`, so the person looking at the draft is not required to be the person who will sign it —
    /// while every other rule (the template's policy, the declared-signer check, the slot mapping, the protected
    /// asset) is the rule issuance obeys, so the pane cannot show what issuance would not apply.
    ///
    /// A REFUSAL is passed on rather than flattened to an empty list: the resolver only refuses what makes an issuance
    /// impossible, and a preview that silently drew nothing there would hide the very defect that makes the document
    /// unissuable.
    pub async fn resolve_applied_signatures(
        &self,
        request: &VaultRenderRequest,
        context: &ServiceContext,
    ) -> Result<Vec<FormAppliedSignature>, CoreServiceError> {
        const OP: &str = "vault.resolveAppliedSignatures";
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
            .resolve_applied_signatures(request)
            .await
            .map_err(|failure| match failure.outcome {
                // The rows were unreachable or the connection could not be acquired: infrastructure, and captured.
                VaultCommandOutcome::PreconditionFailure => CoreServiceError::infrastructure(
                    "VAULT_SIGNATURE_LOOKUP_FAILED",
                    failure.message,
                ),
                // Policy refused it. The document cannot carry her signature as configured — a business refusal the
                // caller must see, not error noise.
                _ => CoreServiceError::business("VAULT_SIGNATURE_REFUSED", failure.message),
            });

        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn next_issued_version(
        &self,
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
        &self,
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

    /// The page count of a document's PDF, read under the CALLER's authority (`vault.read`). `None` when the document
    /// has no PDF bytes, or they are not a readable PDF.
    pub async fn pdf_page_count(
        &self,
        document_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<u32>, CoreServiceError> {
        let Some(media_id) = self
            .get_document(document_id, context)
            .await?
            .and_then(|document| document.media_id)
        else {
            return Ok(None);
        };
        Ok(self
            .media_bytes(&media_id, context)
            .await?
            .and_then(|media| signing_overlay::page_count(&media.bytes)))
    }

    pub async fn public_listing_document_bytes(
        &self,
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

    /// The document a SIGNER is asked to sign, handed to that signer only.
    ///
    /// The third byte door the Vault owns, and the narrowest: its own action (`vault.signingDocument.read`, reserved to
    /// the recipient-bound actor the signer edge mints from a verified link), and its own SQL proof — the recipient must
    /// belong to this ISSUED, live envelope (`VaultDao::signing_document_bytes`). The actor must also be the recipient it
    /// asks for, so a caller cannot read one recipient's envelope under another's identity.
    pub async fn signing_document_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultMediaBytes>, CoreServiceError> {
        const OP: &str = "vault.signingDocumentBytes";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.signingDocument.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = async {
            let expected = format!("signature-recipient:{recipient_id}");
            if context.actor.id.as_deref() != Some(expected.as_str()) {
                return Err(CoreServiceError::business(
                    "SIGNING_DOCUMENT_FORBIDDEN",
                    "The signing document may be read only by its own recipient.",
                ));
            }
            self.repository
                .signing_document_bytes(signature_request_id, recipient_id)
                .await
                .map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    /// The SEALED copy of a completed envelope, handed to one of its own recipients. The fourth byte door: the same
    /// reserved action and recipient-bound actor as the original (`vault.signingDocument.read`), with its own SQL proof
    /// that the envelope is `completed` (`VaultDao::signing_signed_bytes`).
    pub async fn signing_signed_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultMediaBytes>, CoreServiceError> {
        const OP: &str = "vault.signingSignedBytes";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.signingDocument.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = async {
            let expected = format!("signature-recipient:{recipient_id}");
            if context.actor.id.as_deref() != Some(expected.as_str()) {
                return Err(CoreServiceError::business(
                    "SIGNING_DOCUMENT_FORBIDDEN",
                    "The signed copy may be read only by its own recipient.",
                ));
            }
            self.repository
                .signing_signed_bytes(signature_request_id, recipient_id)
                .await
                .map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    /// The files a completion email attaches, read by the email delivery worker and by nobody else
    /// (`vault.completionArtifacts.read` is reserved to that actor). The fifth byte door.
    pub async fn completion_artifacts(
        &self,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<VaultMediaBytes>, CoreServiceError> {
        const OP: &str = "vault.completionArtifacts";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.completionArtifacts.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .completion_artifacts(signature_request_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn form_contract_id(
        &self,
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
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
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
        })
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    pub async fn prior_contract_document(
        &self,
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
        &self,
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
        let result = db::service_mutation(self.repository.database(), async {
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
        })
        .await;
        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }
}

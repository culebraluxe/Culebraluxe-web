mod applied_signatures;
pub mod artifact;
pub mod forms_render;
pub mod pdf;
pub mod signing_certificate;
pub mod signing_overlay;

use crate::media::DocumentObjectWriter;
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
use services::{
    AbstractObjectStoreService, ObjectBucket, OperationKind, ServiceContext, ServiceInfrastructure,
    ServiceRuntime,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[async_trait]
pub trait VaultArtifactPort: Send + Sync {
    async fn render_issued_document(
        &self,
        request: VaultRenderRequest,
    ) -> Result<VaultRenderedArtifact, VaultArtifactFailure>;
}

/// The object writer issuance uses for its rendered PDF.
///
/// Uploading is a side effect the DB transaction cannot own, so it is a port: the DAO uploads through
/// this BEFORE it writes the `media` row, and calls `discard_issued` when the command fails afterwards.
/// `Ok(None)` means this deployment has no object store — the one case the legacy blob write still runs.
#[async_trait]
pub trait IssuedDocumentWriter: Send + Sync {
    async fn store_issued(
        &self,
        media_id: &str,
        filename: &str,
        bytes: &[u8],
    ) -> Result<Option<db::MediaObjectMetadata>, String>;
    async fn discard_issued(&self, key: &str);
}

/// The production writer: `DocumentObjectWriter` over the configured store. With no store (tests/dev)
/// it answers `None`, which keeps local issuance on the legacy blob path.
pub struct ObjectStoreDocumentWriter {
    store: Option<Arc<dyn AbstractObjectStoreService>>,
}

impl ObjectStoreDocumentWriter {
    pub fn new(store: Option<Arc<dyn AbstractObjectStoreService>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl IssuedDocumentWriter for ObjectStoreDocumentWriter {
    async fn store_issued(
        &self,
        media_id: &str,
        filename: &str,
        bytes: &[u8],
    ) -> Result<Option<db::MediaObjectMetadata>, String> {
        let Some(store) = self.store.as_ref() else {
            return Ok(None);
        };
        let stored = DocumentObjectWriter::new(store.clone())
            .put(media_id, filename, "application/pdf", bytes)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Some(db::MediaObjectMetadata {
            bucket: stored.metadata.bucket.as_str().to_owned(),
            key: stored.key,
            etag: stored.metadata.etag,
            sha256: stored.metadata.sha256,
            size: stored.metadata.size,
            status: "VERIFIED".to_owned(),
            visibility: Some("vault".to_owned()),
        }))
    }

    async fn discard_issued(&self, key: &str) {
        if let Some(store) = self.store.as_ref() {
            DocumentObjectWriter::new(store.clone()).discard(key).await;
        }
    }
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

    /// Storage pointer for the PDF, read without loading `file_data`. Test repositories return None.
    async fn media_object_location(
        &self,
        _media_id: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        Ok(None)
    }
    /// The guest listing document as a storage pointer (entitlement SQL unchanged). Test repositories
    /// return None, which makes an object-backed service answer "not found" instead of inventing bytes.
    async fn public_listing_document_object(
        &self,
        _media_id: &str,
    ) -> DbResult<Option<db::MediaObjectRow>> {
        Ok(None)
    }
    async fn signing_document_object(
        &self,
        _signature_request_id: &str,
        _recipient_id: &str,
    ) -> DbResult<Option<db::MediaObjectRow>> {
        Ok(None)
    }
    async fn signing_signed_object(
        &self,
        _signature_request_id: &str,
        _recipient_id: &str,
    ) -> DbResult<Option<db::MediaObjectRow>> {
        Ok(None)
    }
    async fn completion_artifact_objects(
        &self,
        _signature_request_id: &str,
    ) -> DbResult<Vec<db::MediaObjectRow>> {
        Ok(Vec::new())
    }
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
        writer: Arc<dyn IssuedDocumentWriter>,
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

    async fn media_object_location(
        &self,
        media_id: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        VaultDao::media_object_location(self, media_id).await
    }

    async fn public_listing_document_object(
        &self,
        media_id: &str,
    ) -> DbResult<Option<db::MediaObjectRow>> {
        VaultDao::public_listing_document_object(self, media_id).await
    }

    async fn signing_document_object(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<db::MediaObjectRow>> {
        VaultDao::signing_document_object(self, signature_request_id, recipient_id).await
    }

    async fn signing_signed_object(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<db::MediaObjectRow>> {
        VaultDao::signing_signed_object(self, signature_request_id, recipient_id).await
    }

    async fn completion_artifact_objects(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<db::MediaObjectRow>> {
        VaultDao::completion_artifact_objects(self, signature_request_id).await
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
        writer: Arc<dyn IssuedDocumentWriter>,
    ) -> DbResult<VaultCommandResult> {
        let upload_writer = writer.clone();
        VaultDao::issue_from_form_instance(
            self,
            request,
            move |render_request| {
                let artifacts = artifacts.clone();
                async move { artifacts.render_issued_document(render_request).await }
            },
            move |media_id: &str, filename: &str, bytes: &[u8]| {
                let writer = upload_writer.clone();
                let media_id = media_id.to_owned();
                let filename = filename.to_owned();
                let bytes = bytes.to_vec();
                async move {
                    writer
                        .store_issued(&media_id, &filename, &bytes)
                        .await
                        .map_err(|message| VaultArtifactFailure {
                            outcome: VaultCommandOutcome::PreconditionFailure,
                            message,
                        })
                }
            },
            move |key: &str| {
                let writer = writer.clone();
                let key = key.to_owned();
                async move { writer.discard_issued(&key).await }
            },
        )
        .await
    }
}

/// What a Vault byte route should return: the legacy PDF or a redirect to the object store.
#[derive(Debug)]
pub enum VaultRead {
    Bytes(VaultMediaBytes),
    Redirect(String),
}

pub struct VaultService<R> {
    repository: R,
    runtime: ServiceRuntime,
    artifacts: Arc<dyn VaultArtifactPort>,
    object_store: Option<Arc<dyn AbstractObjectStoreService>>,
    object_reads: bool,
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
            object_store: None,
            object_reads: false,
        }
    }

    pub fn with_object_store(mut self, store: Arc<dyn AbstractObjectStoreService>) -> Self {
        self.object_store = Some(store);
        self
    }

    pub fn with_object_reads(mut self, enabled: bool) -> Self {
        self.object_reads = enabled;
        self
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

    /// Phase 4 Vault resolver: only a verified Vault object gets a short-lived signed URL; every
    /// other document keeps the legacy PDF bytes. Authorization is the same `vault.read` check.
    pub async fn media_read(
        &self,
        media_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultRead>, CoreServiceError> {
        const OP: &str = "vault.mediaRead";
        let decision = authorize(
            &self.runtime,
            "vault",
            "vault.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            if self.object_reads {
                if let Some(store) = self.object_store.as_ref() {
                    if let Some(location) = self.repository.media_object_location(media_id).await? {
                        if location.status == "VERIFIED"
                            && location.visibility == "vault"
                            && ObjectBucket::from_db(&location.bucket) == Some(ObjectBucket::Vault)
                        {
                            match store
                                .signed_read_url(
                                    ObjectBucket::Vault,
                                    &location.key,
                                    Duration::from_secs(300),
                                )
                                .await
                            {
                                Ok(url) => return Ok(Some(VaultRead::Redirect(url))),
                                Err(error) => tracing::warn!(
                                    media_id,
                                    %error,
                                    "vault object URL failed; falling back to legacy PDF bytes"
                                ),
                            }
                        }
                    }
                }
            }
            let bytes = self.repository.media_bytes(media_id).await?;
            Ok(bytes.map(VaultRead::Bytes))
        }
        .await;

        audit_result(&self.runtime, "vault", OP, context, decision, &result).await?;
        result
    }

    /// The object pointer behind a document row, when it is a VERIFIED object in the Vault bucket. Anything
    /// else is not object-backed, and must never be served as if it were.
    fn verified_vault_object(row: &db::MediaObjectRow) -> Option<&str> {
        let object = &row.object;
        (object.status == "VERIFIED"
            && object.visibility.as_deref() == Some("vault")
            && ObjectBucket::from_db(&object.bucket) == Some(ObjectBucket::Vault))
        .then_some(object.key.as_str())
    }

    /// Turn a verified Vault pointer into a short-lived signed URL. Authorization already happened at the
    /// door; a configured store that cannot sign is an infrastructure failure, never a reason to serve the
    /// legacy blob.
    async fn object_redirect(
        &self,
        row: &db::MediaObjectRow,
    ) -> Result<VaultRead, CoreServiceError> {
        let key = Self::verified_vault_object(row).ok_or_else(|| {
            CoreServiceError::infrastructure(
                "VAULT_OBJECT_UNAVAILABLE",
                format!("media {} has no verified Vault object", row.id),
            )
        })?;
        let store = self.object_store.as_ref().ok_or_else(|| {
            CoreServiceError::infrastructure(
                "VAULT_OBJECT_STORE_UNAVAILABLE",
                "object storage is not configured for Vault reads",
            )
        })?;
        let url = store
            .signed_read_url(ObjectBucket::Vault, key, Duration::from_secs(300))
            .await
            .map_err(|error| {
                CoreServiceError::infrastructure("VAULT_OBJECT_URL_FAILED", error.to_string())
            })?;
        Ok(VaultRead::Redirect(url))
    }

    /// The bytes of a verified Vault object, for the consumers that need the file itself (a completion
    /// email attachment, the signer's preview render). The fetched bytes are checked against the row's own
    /// size and SHA-256, so a damaged or swapped object is refused rather than attached.
    async fn object_bytes(
        &self,
        row: &db::MediaObjectRow,
    ) -> Result<VaultMediaBytes, CoreServiceError> {
        let key = Self::verified_vault_object(row).ok_or_else(|| {
            CoreServiceError::infrastructure(
                "VAULT_OBJECT_UNAVAILABLE",
                format!("media {} has no verified Vault object", row.id),
            )
        })?;
        let store = self.object_store.as_ref().ok_or_else(|| {
            CoreServiceError::infrastructure(
                "VAULT_OBJECT_STORE_UNAVAILABLE",
                "object storage is not configured for Vault reads",
            )
        })?;
        let bytes = store
            .get_object(ObjectBucket::Vault, key)
            .await
            .map_err(|error| {
                CoreServiceError::infrastructure("VAULT_OBJECT_READ_FAILED", error.to_string())
            })?;
        if row.object.size > 0 && bytes.len() as i64 != row.object.size {
            return Err(CoreServiceError::infrastructure(
                "VAULT_OBJECT_INTEGRITY",
                format!(
                    "media {} object is {} bytes, expected {}",
                    row.id,
                    bytes.len(),
                    row.object.size
                ),
            ));
        }
        if let Some(expected) = row.object.sha256.as_deref() {
            let actual = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if !actual.eq_ignore_ascii_case(expected.trim()) {
                return Err(CoreServiceError::infrastructure(
                    "VAULT_OBJECT_INTEGRITY",
                    format!("media {} object SHA-256 does not match its record", row.id),
                ));
            }
        }
        Ok(VaultMediaBytes {
            bytes,
            filename: row.filename.clone(),
            mime_type: row.mime_type.clone(),
        })
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

    /// The guest's listing document as the browser route serves it: a short-lived signed Vault URL when this
    /// deployment has object storage, the legacy blob only when it has none. The authorization action and the
    /// operation name are the ones the anonymous policy admits (`vault.publicListingDocument.read` under the
    /// exact operation `vault.publicListingDocumentBytes`); nothing about the guest entitlement moves here.
    pub async fn public_listing_document_read(
        &self,
        media_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultRead>, CoreServiceError> {
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
        let result = async {
            if self.object_store.is_some() {
                let Some(row) = self
                    .repository
                    .public_listing_document_object(media_id)
                    .await?
                else {
                    return Ok(None);
                };
                return Ok(Some(self.object_redirect(&row).await?));
            }
            Ok(self
                .repository
                .public_listing_document_bytes(media_id)
                .await?
                .map(VaultRead::Bytes))
        }
        .await;
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
            if self.object_store.is_some() {
                let Some(row) = self
                    .repository
                    .signing_document_object(signature_request_id, recipient_id)
                    .await?
                else {
                    return Ok(None);
                };
                return Ok(Some(self.object_bytes(&row).await?));
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

    /// The signing document as the BROWSER door serves it: a short-lived signed Vault URL (302) instead of
    /// the PDF bytes. Same reserved action, same SQL proof, same actor binding as `signing_document_bytes`;
    /// only the answer shape changes, and the legacy blob is used only where no store exists.
    pub async fn signing_document_read(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultRead>, CoreServiceError> {
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
            if self.object_store.is_some() {
                let Some(row) = self
                    .repository
                    .signing_document_object(signature_request_id, recipient_id)
                    .await?
                else {
                    return Ok(None);
                };
                return Ok(Some(self.object_redirect(&row).await?));
            }
            Ok(self
                .repository
                .signing_document_bytes(signature_request_id, recipient_id)
                .await?
                .map(VaultRead::Bytes))
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
            if self.object_store.is_some() {
                let Some(row) = self
                    .repository
                    .signing_signed_object(signature_request_id, recipient_id)
                    .await?
                else {
                    return Ok(None);
                };
                return Ok(Some(self.object_bytes(&row).await?));
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

    /// The sealed copy as the BROWSER door serves it: a short-lived signed Vault URL (302) instead of the
    /// bytes. Same reserved action, recipient binding and `completed` SQL proof as `signing_signed_bytes`;
    /// the legacy blob is used only where no store exists.
    pub async fn signing_signed_read(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<VaultRead>, CoreServiceError> {
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
            if self.object_store.is_some() {
                let Some(row) = self
                    .repository
                    .signing_signed_object(signature_request_id, recipient_id)
                    .await?
                else {
                    return Ok(None);
                };
                return Ok(Some(self.object_redirect(&row).await?));
            }
            Ok(self
                .repository
                .signing_signed_bytes(signature_request_id, recipient_id)
                .await?
                .map(VaultRead::Bytes))
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
        let result = async {
            if self.object_store.is_some() {
                let rows = self
                    .repository
                    .completion_artifact_objects(signature_request_id)
                    .await?;
                let mut artifacts = Vec::with_capacity(rows.len());
                for row in rows {
                    artifacts.push(self.object_bytes(&row).await?);
                }
                return Ok(artifacts);
            }
            self.repository
                .completion_artifacts(signature_request_id)
                .await
                .map_err(Into::into)
        }
        .await;
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
            // The writer is bound to this deployment's store: present => the PDF is uploaded and verified
            // before its media row is written; absent (tests/dev) => issuance keeps the legacy blob path.
            let writer: Arc<dyn IssuedDocumentWriter> =
                Arc::new(ObjectStoreDocumentWriter::new(self.object_store.clone()));
            let command = self
                .repository
                .issue_from_form_instance(request, self.artifacts.clone(), writer)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::CasbinAuthorizationPort;
    use services::{
        CapturingAuditPort, CapturingDomainEventPort, ObjectMetadata, ObjectStoreError,
        ServiceActor, ServiceActorKind,
    };
    use std::collections::HashMap;

    /// A repository with just the object pointers/legacy bytes a test needs; every other operation is
    /// unreachable in these tests and panics rather than inventing an answer.
    #[derive(Default)]
    struct FakeRepository {
        public_listing_object: Option<db::MediaObjectRow>,
        public_listing_bytes: Option<VaultMediaBytes>,
        signing_object: Option<db::MediaObjectRow>,
        signing_signed_object: Option<db::MediaObjectRow>,
        signing_bytes: Option<VaultMediaBytes>,
        completion_objects: Vec<db::MediaObjectRow>,
        completion_bytes: Vec<VaultMediaBytes>,
    }

    #[async_trait]
    impl VaultRepository for FakeRepository {
        async fn list_issued_documents(
            &self,
            _actor: Option<&VaultActorScope>,
        ) -> DbResult<Vec<IssuedDocumentListItem>> {
            Ok(Vec::new())
        }
        async fn get_document(&self, _document_id: &str) -> DbResult<Option<TransactionDocument>> {
            Ok(None)
        }
        async fn list_by_deal(&self, _deal_id: &str) -> DbResult<Vec<TransactionDocument>> {
            Ok(Vec::new())
        }
        async fn create_document(
            &self,
            _request: &CreateTransactionDocumentRequest,
        ) -> DbResult<TransactionDocument> {
            unimplemented!("not exercised by the Vault read-door tests")
        }
        async fn transition_state(
            &self,
            _request: &TransitionTransactionDocumentRequest,
        ) -> DbResult<VaultCommandResult> {
            unimplemented!("not exercised by the Vault read-door tests")
        }
        async fn issued_for_form_instance(
            &self,
            _form_instance_id: &str,
        ) -> DbResult<Option<IssuedDocumentForFormInstance>> {
            Ok(None)
        }
        async fn next_issued_version(&self, _request: &NextIssuedVersionRequest) -> DbResult<i32> {
            Ok(1)
        }
        async fn media_bytes(&self, _media_id: &str) -> DbResult<Option<VaultMediaBytes>> {
            Ok(None)
        }
        async fn public_listing_document_object(
            &self,
            _media_id: &str,
        ) -> DbResult<Option<db::MediaObjectRow>> {
            Ok(self.public_listing_object.clone())
        }
        async fn signing_document_object(
            &self,
            _signature_request_id: &str,
            _recipient_id: &str,
        ) -> DbResult<Option<db::MediaObjectRow>> {
            Ok(self.signing_object.clone())
        }
        async fn signing_signed_object(
            &self,
            _signature_request_id: &str,
            _recipient_id: &str,
        ) -> DbResult<Option<db::MediaObjectRow>> {
            Ok(self.signing_signed_object.clone())
        }
        async fn completion_artifact_objects(
            &self,
            _signature_request_id: &str,
        ) -> DbResult<Vec<db::MediaObjectRow>> {
            Ok(self.completion_objects.clone())
        }
        async fn public_listing_document_bytes(
            &self,
            _media_id: &str,
        ) -> DbResult<Option<VaultMediaBytes>> {
            Ok(self.public_listing_bytes.clone())
        }
        async fn signing_document_bytes(
            &self,
            _signature_request_id: &str,
            _recipient_id: &str,
        ) -> DbResult<Option<VaultMediaBytes>> {
            Ok(self.signing_bytes.clone())
        }
        async fn signing_signed_bytes(
            &self,
            _signature_request_id: &str,
            _recipient_id: &str,
        ) -> DbResult<Option<VaultMediaBytes>> {
            Ok(self.signing_bytes.clone())
        }
        async fn completion_artifacts(
            &self,
            _signature_request_id: &str,
        ) -> DbResult<Vec<VaultMediaBytes>> {
            Ok(self.completion_bytes.clone())
        }
        async fn form_contract_id(&self, _form_instance_id: &str) -> DbResult<Option<String>> {
            Ok(None)
        }
        async fn bind_form_to_contract(
            &self,
            _form_instance_id: &str,
            _contract_id: &str,
        ) -> DbResult<bool> {
            Ok(false)
        }
        async fn prior_contract_document(
            &self,
            _contract_id: &str,
            _template_id: &str,
        ) -> DbResult<Option<ContractIssuedLineage>> {
            Ok(None)
        }
        async fn resolve_applied_signatures(
            &self,
            _request: &VaultRenderRequest,
        ) -> Result<Vec<FormAppliedSignature>, VaultArtifactFailure> {
            unimplemented!("not exercised by the Vault read-door tests")
        }
        async fn issue_from_form_instance(
            &self,
            _request: &IssueDocumentRequest,
            _artifacts: Arc<dyn VaultArtifactPort>,
            _writer: Arc<dyn IssuedDocumentWriter>,
        ) -> DbResult<VaultCommandResult> {
            unimplemented!("not exercised by the Vault read-door tests")
        }
    }

    struct NoopArtifacts;

    #[async_trait]
    impl VaultArtifactPort for NoopArtifacts {
        async fn render_issued_document(
            &self,
            _request: VaultRenderRequest,
        ) -> Result<VaultRenderedArtifact, VaultArtifactFailure> {
            unimplemented!("not exercised by the Vault read-door tests")
        }
    }

    /// An in-memory Vault bucket: the tests assert the service asks the STORE for bytes and a signed URL,
    /// never the legacy blob, whenever a store is configured.
    #[derive(Default)]
    struct FakeObjectStore {
        objects: HashMap<String, Vec<u8>>,
        deleted: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl FakeObjectStore {
        fn with_object(key: &str, bytes: &[u8]) -> Self {
            let mut objects = HashMap::new();
            objects.insert(key.to_owned(), bytes.to_vec());
            Self {
                objects,
                ..Default::default()
            }
        }

        fn deleted_keys(&self) -> Vec<String> {
            self.deleted.lock().expect("the delete log").clone()
        }
    }

    #[async_trait]
    impl AbstractObjectStoreService for FakeObjectStore {
        async fn put_object(
            &self,
            bucket: ObjectBucket,
            key: &str,
            bytes: &[u8],
            content_type: &str,
        ) -> Result<ObjectMetadata, ObjectStoreError> {
            Ok(ObjectMetadata {
                bucket,
                key: key.to_owned(),
                size: bytes.len() as i64,
                content_type: Some(content_type.to_owned()),
                etag: None,
                sha256: Some(hex(bytes)),
            })
        }
        async fn head_object(
            &self,
            _bucket: ObjectBucket,
            key: &str,
        ) -> Result<ObjectMetadata, ObjectStoreError> {
            Err(ObjectStoreError::not_found(key))
        }
        async fn get_object(
            &self,
            _bucket: ObjectBucket,
            key: &str,
        ) -> Result<Vec<u8>, ObjectStoreError> {
            self.objects
                .get(key)
                .cloned()
                .ok_or_else(|| ObjectStoreError::not_found(key))
        }
        async fn delete_object(
            &self,
            _bucket: ObjectBucket,
            key: &str,
        ) -> Result<(), ObjectStoreError> {
            self.deleted
                .lock()
                .expect("the delete log")
                .push(key.to_owned());
            Ok(())
        }
        async fn public_read_url(
            &self,
            _bucket: ObjectBucket,
            _key: &str,
        ) -> Result<String, ObjectStoreError> {
            Err(ObjectStoreError::configuration(
                "the Vault has no public URLs",
            ))
        }
        async fn signed_read_url(
            &self,
            bucket: ObjectBucket,
            key: &str,
            _ttl: Duration,
        ) -> Result<String, ObjectStoreError> {
            Ok(format!(
                "https://objects.example/{}/{}?sig=test",
                bucket.as_str(),
                key
            ))
        }
        async fn signed_write_url(
            &self,
            bucket: ObjectBucket,
            key: &str,
            _content_type: &str,
            _ttl: Duration,
        ) -> Result<String, ObjectStoreError> {
            Ok(format!(
                "https://objects.example/{}/{}?write=test",
                bucket.as_str(),
                key
            ))
        }
        async fn verify_object(
            &self,
            bucket: ObjectBucket,
            key: &str,
            expected_sha256: &str,
            expected_size: i64,
        ) -> Result<ObjectMetadata, ObjectStoreError> {
            Ok(ObjectMetadata {
                bucket,
                key: key.to_owned(),
                size: expected_size,
                content_type: Some("application/pdf".to_owned()),
                etag: None,
                sha256: Some(expected_sha256.to_owned()),
            })
        }
    }

    fn hex(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn object_row(media_id: &str, bytes: &[u8]) -> db::MediaObjectRow {
        db::MediaObjectRow {
            id: media_id.to_owned(),
            filename: "document.pdf".to_owned(),
            mime_type: "application/pdf".to_owned(),
            alt_text: None,
            media_type: "document".to_owned(),
            derivative_of: None,
            derivative_kind: None,
            object: db::MediaObjectMetadata {
                bucket: "vault".to_owned(),
                key: format!("documents/{media_id}/original.pdf"),
                etag: None,
                sha256: Some(hex(bytes)),
                size: bytes.len() as i64,
                status: "VERIFIED".to_owned(),
                visibility: Some("vault".to_owned()),
            },
        }
    }

    fn legacy_bytes(data: &[u8]) -> VaultMediaBytes {
        VaultMediaBytes {
            bytes: data.to_vec(),
            filename: "legacy.pdf".to_owned(),
            mime_type: "application/pdf".to_owned(),
        }
    }

    async fn test_service(
        repository: FakeRepository,
        store: Option<FakeObjectStore>,
    ) -> VaultService<FakeRepository> {
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(
                CasbinAuthorizationPort::new()
                    .await
                    .expect("the policy loads"),
            ),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let mut service = VaultService::new(repository, Arc::new(NoopArtifacts), infrastructure);
        if let Some(store) = store {
            service = service.with_object_store(Arc::new(store));
        }
        service.with_object_reads(true)
    }

    /// The anonymous public website, exactly as the guest door builds its actor.
    fn guest_context() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("public-website".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "vault-read-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    /// The recipient-bound actor the signer edge mints from a verified link.
    fn recipient_context(recipient_id: &str) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some(format!("signature-recipient:{recipient_id}")),
                kind: ServiceActorKind::System,
            },
            correlation_id: "vault-signing-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    /// The completion email's delivery worker: the only actor the artifacts action is reserved to.
    fn email_context() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some(crate::email::EMAIL_DELIVERY_ACTOR.into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "vault-completion-email-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    #[tokio::test]
    async fn the_guest_listing_door_redirects_to_a_signed_vault_url() {
        let bytes = b"%PDF-1.4 guest listing".to_vec();
        let repository = FakeRepository {
            public_listing_object: Some(object_row("media-guest", &bytes)),
            ..Default::default()
        };
        let store = FakeObjectStore::with_object("documents/media-guest/original.pdf", &bytes);
        let service = test_service(repository, Some(store)).await;

        let read = service
            .public_listing_document_read("media-guest", &guest_context())
            .await
            .expect("the guest door authorizes")
            .expect("the document is object-backed");
        match read {
            VaultRead::Redirect(url) => {
                assert!(
                    url.contains("documents/media-guest/original.pdf"),
                    "the redirect must name the document's own object, got {url}"
                );
            }
            VaultRead::Bytes(_) => {
                panic!("a configured store must answer the browser door with a redirect")
            }
        }
    }

    #[tokio::test]
    async fn the_guest_listing_door_uses_legacy_bytes_only_without_a_store() {
        let repository = FakeRepository {
            public_listing_bytes: Some(legacy_bytes(b"%PDF-1.4 legacy")),
            ..Default::default()
        };
        let service = test_service(repository, None).await;

        let read = service
            .public_listing_document_read("media-legacy", &guest_context())
            .await
            .expect("the guest door authorizes")
            .expect("a deployment without a store keeps the legacy blob");
        assert!(matches!(read, VaultRead::Bytes(_)));
    }

    #[tokio::test]
    async fn a_configured_store_never_falls_back_to_the_legacy_blob() {
        // The row has legacy bytes but no object pointer: with a store configured the door fails closed
        // instead of quietly serving the blob the cutover is removing.
        let repository = FakeRepository {
            public_listing_bytes: Some(legacy_bytes(b"%PDF-1.4 legacy")),
            ..Default::default()
        };
        let service = test_service(repository, Some(FakeObjectStore::default())).await;

        let read = service
            .public_listing_document_read("media-missing-object", &guest_context())
            .await
            .expect("the guest door authorizes");
        assert!(
            read.is_none(),
            "a store-backed deployment must not serve legacy bytes"
        );
    }

    #[tokio::test]
    async fn a_caller_the_policy_does_not_admit_is_refused_before_any_read() {
        let repository = FakeRepository {
            public_listing_bytes: Some(legacy_bytes(b"%PDF-1.4 legacy")),
            ..Default::default()
        };
        let service = test_service(repository, None).await;
        // Not the public website actor: the anonymous door's action is reserved to it.
        let stranger = ServiceContext {
            actor: ServiceActor {
                id: Some("some-other-system-actor".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "vault-read-test".into(),
            causation_id: None,
            principal: None,
        };
        let error = service
            .public_listing_document_read("media-1", &stranger)
            .await
            .expect_err("an actor the policy does not admit must be refused");
        assert_eq!(error.code(), "FORBIDDEN");
    }

    #[tokio::test]
    async fn the_signing_door_redirects_only_its_own_recipient() {
        let bytes = b"%PDF-1.4 signing".to_vec();
        let repository = FakeRepository {
            signing_object: Some(object_row("media-signing", &bytes)),
            ..Default::default()
        };
        let store = FakeObjectStore::with_object("documents/media-signing/original.pdf", &bytes);
        let service = test_service(repository, Some(store)).await;

        let read = service
            .signing_document_read(
                "request-1",
                "recipient-1",
                &recipient_context("recipient-1"),
            )
            .await
            .expect("the recipient-bound actor is admitted")
            .expect("the signing document is object-backed");
        assert!(matches!(read, VaultRead::Redirect(_)));

        let error = service
            .signing_document_read(
                "request-1",
                "recipient-1",
                &recipient_context("recipient-2"),
            )
            .await
            .expect_err("another recipient's identity may not read this envelope");
        assert_eq!(error.code(), "SIGNING_DOCUMENT_FORBIDDEN");
    }

    #[tokio::test]
    async fn completion_email_attachments_come_from_object_storage_with_integrity_checks() {
        let sealed = b"%PDF-1.4 sealed".to_vec();
        let certificate = b"%PDF-1.4 certificate".to_vec();
        let repository = FakeRepository {
            completion_objects: vec![
                object_row("media-sealed", &sealed),
                object_row("media-certificate", &certificate),
            ],
            ..Default::default()
        };
        let mut store =
            FakeObjectStore::with_object("documents/media-sealed/original.pdf", &sealed);
        store.objects.insert(
            "documents/media-certificate/original.pdf".into(),
            certificate.clone(),
        );
        let service = test_service(repository, Some(store)).await;

        let artifacts = service
            .completion_artifacts("request-1", &email_context())
            .await
            .expect("the completed envelope's artifacts fetch");
        assert_eq!(artifacts.len(), 2, "sealed copy plus certificate");
        assert_eq!(artifacts[0].bytes, sealed, "the sealed copy comes first");
        assert_eq!(artifacts[1].bytes, certificate);
    }

    #[tokio::test]
    async fn a_tampered_completion_artifact_is_refused_not_attached() {
        let sealed = b"%PDF-1.4 sealed".to_vec();
        let mut row = object_row("media-sealed", &sealed);
        // The record says these bytes; the store hands back different ones.
        row.object.sha256 = Some("0".repeat(64));
        let repository = FakeRepository {
            completion_objects: vec![row],
            ..Default::default()
        };
        let store = FakeObjectStore::with_object("documents/media-sealed/original.pdf", &sealed);
        let service = test_service(repository, Some(store)).await;

        let error = service
            .completion_artifacts("request-1", &email_context())
            .await
            .expect_err("an object that does not match its record must not be attached");
        assert_eq!(error.code(), "VAULT_OBJECT_INTEGRITY");
    }

    #[tokio::test]
    async fn the_issued_pdf_writer_uploads_under_its_media_id_before_the_row_exists() {
        let store = std::sync::Arc::new(FakeObjectStore::default());
        let writer = ObjectStoreDocumentWriter::new(Some(
            store.clone() as std::sync::Arc<dyn AbstractObjectStoreService>
        ));
        let bytes = b"%PDF-1.4 issued";

        let object = writer
            .store_issued("media-issued", "Contract v1.pdf", bytes)
            .await
            .expect("the store uploads")
            .expect("a configured store answers an object");
        // The key is the media id the DB row will carry, so the row and its object always agree.
        assert_eq!(object.key, "documents/media-issued/original.pdf");
        assert_eq!(object.bucket, "vault");
        assert_eq!(object.status, "VERIFIED");
        assert_eq!(object.visibility.as_deref(), Some("vault"));
        assert_eq!(object.size, bytes.len() as i64);
        assert_eq!(object.sha256.as_deref(), Some(hex(bytes).as_str()));
    }

    #[tokio::test]
    async fn the_issued_pdf_writer_keeps_the_legacy_path_only_without_a_store() {
        let writer = ObjectStoreDocumentWriter::new(None);
        assert!(
            writer
                .store_issued("media-issued", "Contract v1.pdf", b"%PDF-1.4 issued")
                .await
                .expect("no store is not an error")
                .is_none(),
            "without a store the DAO must take the legacy blob branch"
        );
    }

    #[tokio::test]
    async fn a_failed_issuance_discards_the_object_it_uploaded() {
        let store = std::sync::Arc::new(FakeObjectStore::default());
        let writer = ObjectStoreDocumentWriter::new(Some(
            store.clone() as std::sync::Arc<dyn AbstractObjectStoreService>
        ));
        writer
            .store_issued("media-discard", "Contract v1.pdf", b"%PDF-1.4 issued")
            .await
            .expect("the store uploads");
        writer
            .discard_issued("documents/media-discard/original.pdf")
            .await;
        assert_eq!(
            store.deleted_keys(),
            vec!["documents/media-discard/original.pdf".to_owned()]
        );
    }
}

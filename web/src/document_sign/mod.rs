use crate::email::{EmailRepository, EmailService, EMAIL_DELIVERY_ROUTING_KEY};
use crate::service_support::{audit_result, authorize, CoreServiceError};
use crate::signature::{SignatureRepository, SignatureService};
use crate::signer::{SignerRepository, SignerService};
use crate::vault::{VaultRepository, VaultService};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use db::{
    DbResult, DbTransaction, DocumentSignDao, EmailDao, FinalizeInputs, SignatureDao, SignerDao,
    VaultDao,
};
use model::{
    validate_document_sign_recipients, DocumentSignConfig, DocumentSignFinalizeResult,
    DocumentSignIssueResult, DocumentSignRecipient, DocumentSignSendResult, DocumentSignSnapshot, DocumentSignEnvelopeSummary, DocumentSignSweepResult, EmailMessageKind,
    ImportAnchorFieldsRequest, ImportAnchorFieldsResult,
    IssueDocumentSignRequest, PrepareDocumentSignRequest, PrepareSignatureRequest,
    PreparedSignatureRecipient, PutSignatureFieldRequest, QueueEmailRequest,
    RemoveSignatureFieldRequest, SetDocumentSignRecipientsRequest, SignatureField, SignatureFieldType,
    SendDocumentSignRequest, SendFieldPlacement, SignatureRecipientRole, SignatureRequestStatus,
    TemplateAnchor,
};
use serde_json::{json, Value};
use services::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy, ServiceInfrastructure,
    ServiceRuntime,
};
use std::sync::Arc;

pub const DOCUMENT_SIGN_SERVICE_ACTOR: &str = "document-sign-service";
const DEFAULT_EXPIRY_DAYS: i64 = 7;

#[async_trait]
pub trait DocumentSignRepository: Send + Sync {
    async fn config(&self, signature_request_id: &str) -> DbResult<Option<DocumentSignConfig>>;
    async fn list_envelopes(&self, limit: i64) -> DbResult<Vec<DocumentSignEnvelopeSummary>>;
    async fn recipients(&self, signature_request_id: &str) -> DbResult<Vec<DocumentSignRecipient>>;
    async fn recipients_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<DocumentSignRecipient>>;
    async fn fields(&self, signature_request_id: &str) -> DbResult<Vec<SignatureField>>;
    async fn fields_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<SignatureField>>;
    async fn create_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        subject: Option<&str>,
        signing_mode: model::DocumentSigningMode,
        expires_at: Option<DateTime<Utc>>,
    ) -> DbResult<DocumentSignConfig>;
    async fn put_field_tx(
        &self,
        tx: &mut DbTransaction,
        request: &PutSignatureFieldRequest,
    ) -> DbResult<Option<SignatureField>>;
    async fn remove_field_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        field_id: &str,
    ) -> DbResult<bool>;
    async fn lock_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<DocumentSignConfig>>;
    async fn mark_issued_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<bool>;
    async fn required_field_gaps_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<String>>;
    async fn store_audit_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<String>;
    async fn audit_media_for_request_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>>;
    async fn link_signed_media_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        media_id: &str,
    ) -> DbResult<()>;
    async fn store_signed_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<String>;
    async fn overdue_envelopes_tx(&self, tx: &mut DbTransaction) -> DbResult<Vec<String>>;
    /// The canonical status and owning document, read inside the caller's
    /// transaction. Finalize uses this instead of `signature.get` because
    /// the internal context carries no principal and must not depend on
    /// query grants.
    async fn canonical_status_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<(String, String)>>;
    async fn signed_media_for_request_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>>;
    async fn template_anchors_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<serde_json::Value>>;
}

#[async_trait]
impl DocumentSignRepository for DocumentSignDao {
    async fn config(&self, signature_request_id: &str) -> DbResult<Option<DocumentSignConfig>> {
        DocumentSignDao::config(self, signature_request_id).await
    }
    async fn list_envelopes(&self, limit: i64) -> DbResult<Vec<DocumentSignEnvelopeSummary>> {
        DocumentSignDao::list_envelopes(self, limit).await
    }
    async fn recipients(&self, signature_request_id: &str) -> DbResult<Vec<DocumentSignRecipient>> {
        DocumentSignDao::recipients(self, signature_request_id).await
    }
    async fn recipients_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<DocumentSignRecipient>> {
        DocumentSignDao::recipients_tx(self, tx, signature_request_id).await
    }
    async fn fields(&self, signature_request_id: &str) -> DbResult<Vec<SignatureField>> {
        DocumentSignDao::fields(self, signature_request_id).await
    }
    async fn fields_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<SignatureField>> {
        DocumentSignDao::fields_tx(self, tx, signature_request_id).await
    }
    async fn create_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        subject: Option<&str>,
        signing_mode: model::DocumentSigningMode,
        expires_at: Option<DateTime<Utc>>,
    ) -> DbResult<DocumentSignConfig> {
        DocumentSignDao::create_config_tx(
            self,
            tx,
            signature_request_id,
            subject,
            signing_mode,
            expires_at,
        )
        .await
    }
    async fn put_field_tx(
        &self,
        tx: &mut DbTransaction,
        request: &PutSignatureFieldRequest,
    ) -> DbResult<Option<SignatureField>> {
        DocumentSignDao::put_field_tx(self, tx, request).await
    }
    async fn remove_field_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        field_id: &str,
    ) -> DbResult<bool> {
        DocumentSignDao::remove_field_tx(self, tx, signature_request_id, field_id).await
    }
    async fn lock_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<DocumentSignConfig>> {
        DocumentSignDao::lock_config_tx(self, tx, signature_request_id).await
    }
    async fn mark_issued_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<bool> {
        DocumentSignDao::mark_issued_tx(self, tx, signature_request_id, expires_at).await
    }
    async fn required_field_gaps_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<String>> {
        DocumentSignDao::required_field_gaps_tx(self, tx, signature_request_id).await
    }
    async fn store_audit_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<String> {
        DocumentSignDao::store_audit_artifact_tx(
            self,
            tx,
            transaction_document_id,
            filename,
            mime_type,
            bytes,
        )
        .await
    }
    async fn audit_media_for_request_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>> {
        DocumentSignDao::audit_media_for_request_tx(self, tx, signature_request_id).await
    }
    async fn link_signed_media_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        media_id: &str,
    ) -> DbResult<()> {
        DocumentSignDao::link_signed_media_tx(self, tx, transaction_document_id, media_id).await
    }
    async fn store_signed_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<String> {
        DocumentSignDao::store_signed_artifact_tx(self, tx, filename, mime_type, bytes).await
    }
    async fn overdue_envelopes_tx(&self, tx: &mut DbTransaction) -> DbResult<Vec<String>> {
        DocumentSignDao::overdue_envelopes_tx(self, tx).await
    }
    async fn canonical_status_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<(String, String)>> {
        DocumentSignDao::canonical_status_tx(self, tx, signature_request_id).await
    }
    async fn signed_media_for_request_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>> {
        DocumentSignDao::signed_media_for_request_tx(self, tx, signature_request_id).await
    }
    async fn template_anchors_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<serde_json::Value>> {
        DocumentSignDao::template_anchors_tx(self, tx, signature_request_id).await
    }
}

pub struct DocumentSignService<R, SR, SGR, ER, VR> {
    repository: R,
    signature: Arc<SignatureService<SR>>,
    signer: Arc<SignerService<SGR>>,
    email: Arc<EmailService<ER>>,
    vault: Arc<VaultService<VR>>,
    runtime: ServiceRuntime,
}

impl<R, SR, SGR, ER, VR> DocumentSignService<R, SR, SGR, ER, VR>
where
    R: DocumentSignRepository,
    SR: SignatureRepository,
    SGR: SignerRepository,
    ER: EmailRepository,
    VR: VaultRepository,
{
    pub fn new(
        repository: R,
        signature: Arc<SignatureService<SR>>,
        signer: Arc<SignerService<SGR>>,
        email: Arc<EmailService<ER>>,
        vault: Arc<VaultService<VR>>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            signature,
            signer,
            email,
            vault,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &self,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<DocumentSignSnapshot>, CoreServiceError> {
        const OP: &str = "documentSign.get";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let Some(config) = self.repository.config(signature_request_id).await? else {
                return Ok(None);
            };
            let signature_request = self
                .signature
                .get(signature_request_id, context)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_NOT_FOUND",
                        "Canonical signature request not found.",
                    )
                })?;
            let recipients = self.repository.recipients(signature_request_id).await?;
            let fields = self.repository.fields(signature_request_id).await?;
            Ok(Some(DocumentSignSnapshot {
                signature_request,
                config,
                recipients,
                fields,
            }))
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    /// Recent native envelopes for the ops desk. Inline query: no mutation,
    /// no command receipt, the desk re-reads after each action.
    pub async fn list(
        &self,
        context: &ServiceContext,
    ) -> Result<Vec<DocumentSignEnvelopeSummary>, CoreServiceError> {
        const OP: &str = "documentSign.list";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list_envelopes(50).await.map_err(Into::into);
        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn fields(
        &self,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<SignatureField>, CoreServiceError> {
        const OP: &str = "documentSign.fields";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .fields(signature_request_id)
            .await
            .map_err(Into::into);
        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn prepare_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &PrepareDocumentSignRequest,
        context: &ServiceContext,
    ) -> Result<DocumentSignSnapshot, CoreServiceError> {
        const OP: &str = "documentSign.prepare";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            validate_prepare(request)?;
            let expires_at = parse_future_expiry(request.expires_at.as_deref())?;
            let recipients = prepared_recipients(&request.recipients);
            let internal = internal_context(context);
            let canonical = self
                .signature
                .prepare_transactional(
                    tx,
                    &PrepareSignatureRequest {
                        transaction_document_id: request.transaction_document_id.clone(),
                        recipients,
                        message: request.message.clone(),
                        created_by_user_id: context
                            .principal
                            .as_ref()
                            .map(|principal| principal.app_user_id.clone()),
                    },
                    &internal,
                )
                .await?;

            let config = if canonical.existing {
                self.repository
                    .lock_config_tx(tx, &canonical.signature_request.id)
                    .await?
                    .ok_or_else(|| {
                        CoreServiceError::business(
                            "DOCUMENT_SIGN_ACTIVE_SIGNATURE_EXISTS",
                            "This document already has an active non-native signature request.",
                        )
                    })?
            } else {
                self.repository
                    .create_config_tx(
                        tx,
                        &canonical.signature_request.id,
                        request.subject.as_deref(),
                        request.signing_mode,
                        expires_at.clone(),
                    )
                    .await?
            };

            let recipients = self
                .repository
                .recipients_tx(tx, &canonical.signature_request.id)
                .await?;
            if canonical.existing
                && !prepare_intent_matches(&config, &recipients, request, expires_at)
            {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_ALREADY_PREPARED",
                    "This document already has an active native signing draft with different intent.",
                ));
            }
            let fields = self
                .repository
                .fields_tx(tx, &canonical.signature_request.id)
                .await?;

            Ok(DocumentSignSnapshot {
                signature_request: canonical.signature_request,
                config,
                recipients,
                fields,
            })
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn set_recipients_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &SetDocumentSignRecipientsRequest,
        context: &ServiceContext,
    ) -> Result<Vec<DocumentSignRecipient>, CoreServiceError> {
        const OP: &str = "documentSign.setRecipients";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            ensure_mutable_config(&self.repository, tx, &request.signature_request_id).await?;
            let errors = validate_document_sign_recipients(&request.recipients);
            if !errors.is_empty() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_RECIPIENT_INVALID",
                    errors.join(" "),
                ));
            }
            let internal = internal_context(context);
            self.signature
                .replace_recipients_transactional(
                    tx,
                    &request.signature_request_id,
                    &prepared_recipients(&request.recipients),
                    &internal,
                )
                .await?;
            self.repository
                .recipients_tx(tx, &request.signature_request_id)
                .await
                .map_err(Into::into)
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn put_field_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &PutSignatureFieldRequest,
        context: &ServiceContext,
    ) -> Result<SignatureField, CoreServiceError> {
        const OP: &str = "documentSign.putField";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            ensure_mutable_config(&self.repository, tx, &request.signature_request_id).await?;
            validate_field(request)?;
            self.repository
                .put_field_tx(tx, request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_FIELD_INVALID",
                        "Field was not found in this signature request.",
                    )
                })
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn remove_field_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &RemoveSignatureFieldRequest,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "documentSign.removeField";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            ensure_mutable_config(&self.repository, tx, &request.signature_request_id).await?;
            if self
                .repository
                .remove_field_tx(tx, &request.signature_request_id, &request.field_id)
                .await?
            {
                Ok(())
            } else {
                Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_FIELD_INVALID",
                    "Field was not found in this signature request.",
                ))
            }
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    /// Prepare, place every signer's signature field, and issue — in the caller's one transaction, so an envelope is
    /// never left half-built. Each step is still authorized on its own (`documentSign.write`, then
    /// `documentSign.issue`), so this grants nothing the three commands did not.
    pub async fn send_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &SendDocumentSignRequest,
        context: &ServiceContext,
    ) -> Result<DocumentSignSendResult, CoreServiceError> {
        // The page count is read first (under the caller's authority) so a request for "the last page" is resolved
        // against the real document, and so a bad page is refused before anything is written.
        let page_count = self
            .vault
            .pdf_page_count(&request.transaction_document_id, context)
            .await?;
        let page = match &request.placement {
            SendFieldPlacement::Template => None,
            SendFieldPlacement::LastPage => Some(page_count.ok_or_else(|| {
                CoreServiceError::business(
                    "DOCUMENT_SIGN_FIELD_INVALID",
                    "The document has no readable PDF, so its last page is unknown.",
                )
            })? as i32),
            SendFieldPlacement::Page { page_number } => {
                if *page_number < 1 || page_count.is_some_and(|count| *page_number as u32 > count) {
                    return Err(CoreServiceError::business(
                        "DOCUMENT_SIGN_FIELD_INVALID",
                        match page_count {
                            Some(count) => format!(
                                "Page {page_number} does not exist: the document has {count} page(s)."
                            ),
                            None => "The signature page must be 1 or higher.".to_owned(),
                        },
                    ));
                }
                Some(*page_number)
            }
        };

        let snapshot = self
            .prepare_transactional(
                tx,
                &PrepareDocumentSignRequest {
                    transaction_document_id: request.transaction_document_id.clone(),
                    recipients: request.recipients.clone(),
                    subject: request.subject.clone(),
                    message: request.message.clone(),
                    signing_mode: request.signing_mode,
                    expires_at: request.expires_at.clone(),
                },
                context,
            )
            .await?;
        let signature_request_id = snapshot.signature_request.id.clone();

        match page {
            None => {
                self.import_fields_transactional(
                    tx,
                    &ImportAnchorFieldsRequest {
                        signature_request_id: signature_request_id.clone(),
                        anchors: Vec::new(),
                    },
                    context,
                )
                .await?;
            }
            Some(page_number) => {
                let owners: std::collections::BTreeSet<String> = snapshot
                    .fields
                    .iter()
                    .map(|field| field.recipient_id.clone())
                    .collect();
                let signers = snapshot
                    .recipients
                    .iter()
                    .filter(|recipient| recipient.role == SignatureRecipientRole::Signer)
                    .filter(|recipient| !owners.contains(&recipient.id));
                for (slot, recipient) in signers.enumerate() {
                    let (x, y, width, height) = default_signature_box(slot);
                    self.put_field_transactional(
                        tx,
                        &PutSignatureFieldRequest {
                            signature_request_id: signature_request_id.clone(),
                            field_id: None,
                            recipient_id: recipient.id.clone(),
                            // The key is unique per envelope, so each signer's box needs its own.
                            field_key: format!("signature-{}", recipient.id),
                            field_type: SignatureFieldType::Signature,
                            page_number,
                            position_x: x,
                            position_y: y,
                            width,
                            height,
                            required: true,
                            label: Some("Signature".into()),
                            configuration: json!({}),
                        },
                        context,
                    )
                    .await?;
                }
            }
        }

        let issued = self
            .issue_transactional(
                tx,
                &IssueDocumentSignRequest {
                    signature_request_id: signature_request_id.clone(),
                },
                context,
            )
            .await?;
        let recipients = self
            .repository
            .recipients_tx(tx, &signature_request_id)
            .await?;
        let fields = self
            .repository
            .fields_tx(tx, &signature_request_id)
            .await?;
        Ok(DocumentSignSendResult {
            snapshot: DocumentSignSnapshot {
                recipients,
                fields,
                ..snapshot
            },
            issued,
        })
    }

    pub async fn issue_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &IssueDocumentSignRequest,
        context: &ServiceContext,
    ) -> Result<DocumentSignIssueResult, CoreServiceError> {
        const OP: &str = "documentSign.issue";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.issue",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let config = self
                .repository
                .lock_config_tx(tx, &request.signature_request_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_NOT_FOUND",
                        "Native document-sign request not found.",
                    )
                })?;

            if config.issued_at.is_some() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_ALREADY_ISSUED",
                    "This document-sign request has already been issued.",
                ));
            }

            let gaps = self
                .repository
                .required_field_gaps_tx(tx, &request.signature_request_id)
                .await?;
            if !gaps.is_empty() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_FIELD_INVALID",
                    format!(
                        "{} signing recipient(s) have no required field.",
                        gaps.len()
                    ),
                ));
            }

            // Every field must sit on a page the document has: sealing fails on a missing page, and by then the
            // signers have already signed. Skipped only when the PDF itself cannot be read.
            if let Some((_, document_id)) = self
                .repository
                .canonical_status_tx(tx, &request.signature_request_id)
                .await?
            {
                if let Some(pages) = self.vault.pdf_page_count(&document_id, context).await? {
                    let fields = self
                        .repository
                        .fields_tx(tx, &request.signature_request_id)
                        .await?;
                    if let Some(field) = fields.iter().find(|field| {
                        field.page_number < 1 || field.page_number as u32 > pages
                    }) {
                        return Err(CoreServiceError::business(
                            "DOCUMENT_SIGN_FIELD_INVALID",
                            format!(
                                "Field '{}' is on page {}, but the document has {pages} page(s).",
                                field.field_key, field.page_number
                            ),
                        ));
                    }
                }
            }

            let recipients = self
                .repository
                .recipients_tx(tx, &request.signature_request_id)
                .await?;
            if recipients.is_empty() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_RECIPIENT_INVALID",
                    "At least one recipient is required before issue.",
                ));
            }

            let expires_at = match config.expires_at.as_deref() {
                Some(value) => DateTime::parse_from_rfc3339(value)
                    .map_err(|_| {
                        CoreServiceError::business(
                            "DOCUMENT_SIGN_EXPIRY_INVALID",
                            "Stored signing expiry is invalid.",
                        )
                    })?
                    .with_timezone(&Utc),
                None => Utc::now() + Duration::days(DEFAULT_EXPIRY_DAYS),
            };
            if expires_at <= Utc::now() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_EXPIRY_INVALID",
                    "Signing expiry must be in the future.",
                ));
            }

            let internal = internal_context(context);
            let mut invitation_message_ids = Vec::with_capacity(recipients.len());
            for recipient in &recipients {
                let grant = self
                    .signer
                    .issue_access_transactional(
                        tx,
                        &recipient.id,
                        Some(expires_at.clone()),
                        &internal,
                    )
                    .await?;
                let queued = self
                    .email
                    .queue_transactional(
                        tx,
                        &QueueEmailRequest {
                            message_kind: EmailMessageKind::SignatureInvitation,
                            recipient_email: recipient.email.clone(),
                            template_key: "document-sign.invitation".into(),
                            template_payload: json!({
                                "recipientName": recipient.name,
                                "signingUrl": grant.signing_url,
                                "subject": config.subject,
                            }),
                            dedupe_key: format!(
                                "signature-invite:{}:{}:v{}",
                                request.signature_request_id, recipient.id, grant.token_version
                            ),
                            correlation_id: Some(context.correlation_id.clone()),
                            causation_id: context.causation_id.clone(),
                        },
                        &internal,
                    )
                    .await?;
                invitation_message_ids.push(queued.message_id);
                self.signer
                    .mark_notified_transactional(
                        tx,
                        &recipient.id,
                        &request.signature_request_id,
                        &internal,
                    )
                    .await?;
            }

            self.signature
                .transition_transactional(
                    tx,
                    &request.signature_request_id,
                    SignatureRequestStatus::Sent,
                    &internal,
                )
                .await?;

            if !self
                .repository
                .mark_issued_tx(tx, &request.signature_request_id, expires_at.clone())
                .await?
            {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_ALREADY_ISSUED",
                    "This document-sign request changed while it was being issued.",
                ));
            }

            Ok(DocumentSignIssueResult {
                signature_request_id: request.signature_request_id.clone(),
                invitation_message_ids,
                expires_at: expires_at.to_rfc3339(),
            })
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn signer_completed_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        envelope_ready_to_finalize: bool,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        if !envelope_ready_to_finalize {
            return Ok(());
        }
        let internal = internal_context(context);
        self.signature
            .transition_transactional(
                tx,
                signature_request_id,
                SignatureRequestStatus::Signed,
                &internal,
            )
            .await?;
        // Completion is announced to every recipient in the same transaction
        // that flips the envelope: SMTP still happens asynchronously through
        // the outbox, so a mail outage cannot undo the signature.
        let recipients = self
            .repository
            .recipients_tx(tx, signature_request_id)
            .await?;
        for recipient in &recipients {
            self.email
                .queue_transactional(
                    tx,
                    &QueueEmailRequest {
                        message_kind: EmailMessageKind::SignatureCompleted,
                        recipient_email: recipient.email.clone(),
                        template_key: "document-sign.completed".into(),
                        template_payload: json!({
                            "recipientName": recipient.name,
                            "signatureRequestId": signature_request_id,
                        }),
                        dedupe_key: format!(
                            "signature-completed:{signature_request_id}:{}",
                            recipient.id
                        ),
                        correlation_id: Some(context.correlation_id.clone()),
                        causation_id: context.causation_id.clone(),
                    },
                    &internal,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn signer_declined_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        decliner_recipient_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        let internal = internal_context(context);
        self.signer
            .revoke_request_access_transactional(tx, signature_request_id, &internal)
            .await?;
        self.signature
            .transition_transactional(
                tx,
                signature_request_id,
                SignatureRequestStatus::Declined,
                &internal,
            )
            .await?;
        // Everyone except the decliner learns the envelope died; the decliner
        // already knows. Queued here so invitations and outcome share one
        // transaction with the status flip.
        let recipients = self
            .repository
            .recipients_tx(tx, signature_request_id)
            .await?;
        for recipient in recipients
            .iter()
            .filter(|recipient| recipient.id != decliner_recipient_id)
        {
            self.email
                .queue_transactional(
                    tx,
                    &QueueEmailRequest {
                        message_kind: EmailMessageKind::SignatureDeclined,
                        recipient_email: recipient.email.clone(),
                        template_key: "document-sign.declined".into(),
                        template_payload: json!({
                            "recipientName": recipient.name,
                            "signatureRequestId": signature_request_id,
                        }),
                        dedupe_key: format!(
                            "signature-declined:{signature_request_id}:{}",
                            recipient.id
                        ),
                        correlation_id: Some(context.correlation_id.clone()),
                        causation_id: context.causation_id.clone(),
                    },
                    &internal,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn void_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "documentSign.void";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.void",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            self.repository
                .lock_config_tx(tx, signature_request_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_NOT_FOUND",
                        "Native document-sign request not found.",
                    )
                })?;
            let internal = internal_context(context);
            self.signer
                .revoke_request_access_transactional(tx, signature_request_id, &internal)
                .await?;
            self.signature
                .transition_transactional(
                    tx,
                    signature_request_id,
                    SignatureRequestStatus::Voided,
                    &internal,
                )
                .await?;
            Ok(())
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    /// Re-send an invitation to one recipient of an issued envelope.
    /// A reminder reuses the live link; a resend rotates access (the old link
    /// dies with evidence) and sends a fresh invitation. Either way only the
    /// queue insert happens here — SMTP stays asynchronous.
    pub async fn resend_invitation_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        recipient_id: &str,
        as_reminder: bool,
        context: &ServiceContext,
    ) -> Result<String, CoreServiceError> {
        const OP: &str = "documentSign.resend";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let config = self
                .repository
                .lock_config_tx(tx, signature_request_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_NOT_FOUND",
                        "Native document-sign request not found.",
                    )
                })?;
            if config.issued_at.is_none() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_NOT_MUTABLE",
                    "There is nothing to resend before issue.",
                ));
            }
            let recipient = self
                .repository
                .recipients_tx(tx, signature_request_id)
                .await?
                .into_iter()
                .find(|recipient| recipient.id == recipient_id)
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_RECIPIENT_INVALID",
                        "Recipient does not belong to this signing request.",
                    )
                })?;
            let internal = internal_context(context);
            let state = self.signer.current_state(recipient_id).await?;
            if state.state.is_terminal() {
                return Err(CoreServiceError::business(
                    "SIGNER_ALREADY_TERMINAL",
                    "This signer can no longer be invited.",
                ));
            }
            let (signing_url, kind, template_key, dedupe_kind) = if as_reminder {
                (
                    self.signer.active_signing_url(tx, recipient_id).await?,
                    EmailMessageKind::SignatureReminder,
                    "document-sign.reminder",
                    "signature-reminder",
                )
            } else {
                let grant = self
                    .signer
                    .issue_access_transactional(tx, recipient_id, None, &internal)
                    .await?;
                (
                    grant.signing_url,
                    EmailMessageKind::SignatureInvitation,
                    "document-sign.invitation",
                    "signature-resend",
                )
            };
            let queued = self
                .email
                .queue_transactional(
                    tx,
                    &QueueEmailRequest {
                        message_kind: kind,
                        recipient_email: recipient.email.clone(),
                        template_key: template_key.into(),
                        template_payload: json!({
                            "recipientName": recipient.name,
                            "signingUrl": signing_url,
                            "subject": config.subject,
                        }),
                        dedupe_key: format!(
                            "{dedupe_kind}:{signature_request_id}:{recipient_id}:{}",
                            Utc::now().timestamp_millis()
                        ),
                        correlation_id: Some(context.correlation_id.clone()),
                        causation_id: context.causation_id.clone(),
                    },
                    &internal,
                )
                .await?;
            self.signer
                .mark_notified_transactional(tx, recipient_id, signature_request_id, &internal)
                .await?;
            Ok(queued.message_id)
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    /// Close a fully-signed envelope: build the deterministic audit artifact
    /// from the evidence ledger, store it through Vault-readable media,
    /// link it, and transition the canonical request to Completed.
    /// No authorize call here by design — like the signer-completed fan-in,
    /// this runs only inside the durable dispatcher, which owns auth.
    /// The signed PDF overlay itself is a future renderer step behind the
    /// same seam; this ships the complete, queryable audit trail now.
    /// Import template anchor blocks as recipient-owned native fields.
    /// Mirrors the retired BoldSign matcher: anchors group by (role, slot),
    /// each recipient claims the group named by its execution role and slot,
    /// and initials/date anchors ride with their signature group. A recipient
    /// with no group, or a group with no recipient, fails loud — a half-mapped
    /// envelope is a broken envelope. Geometry converts from PDF points
    /// (bottom-left) to field percentages (top-left); re-importing reuses
    /// existing keys, so geometry moves only through putField.
    pub async fn import_fields_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &ImportAnchorFieldsRequest,
        context: &ServiceContext,
    ) -> Result<ImportAnchorFieldsResult, CoreServiceError> {
        const OP: &str = "documentSign.importFields";
        let decision = authorize(
            &self.runtime,
            "document-sign",
            "documentSign.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            ensure_mutable_config(&self.repository, tx, &request.signature_request_id).await?;
            let anchors = if request.anchors.is_empty() {
                // No anchors supplied: read the issuing template's own
                // blocks from Vault's snapshot, so the desk button sends no
                // geometry at all — just the envelope.
                let raw = self
                    .repository
                    .template_anchors_tx(tx, &request.signature_request_id)
                    .await?
                    .ok_or_else(|| {
                        CoreServiceError::business(
                            "DOCUMENT_SIGN_FIELD_INVALID",
                            "The issued document carries no template anchor blocks.",
                        )
                    })?;
                serde_json::from_value::<Vec<TemplateAnchor>>(raw).map_err(|_| {
                    CoreServiceError::business(
                        "DOCUMENT_SIGN_FIELD_INVALID",
                        "The issued document's anchor blocks are unreadable.",
                    )
                })?
            } else {
                request.anchors.clone()
            };
            let recipients = self
                .repository
                .recipients_tx(tx, &request.signature_request_id)
                .await?;
            let groups = group_anchor_sets(&anchors)?;
            let known: std::collections::BTreeMap<String, String> = self
                .repository
                .fields_tx(tx, &request.signature_request_id)
                .await?
                .into_iter()
                .map(|field| (field.field_key, field.id))
                .collect();
            // Pass one maps every anchor to at most one recipient; pass two
            // inserts. Nothing is written before every group is claimed, so
            // an unmapped template fails before touching the envelope.
            let mut claimed = vec![false; groups.len()];
            let mut plan: Vec<(&model::DocumentSignRecipient, Vec<GroupedAnchor>)> = Vec::new();
            for (index, recipient) in recipients.iter().enumerate() {
                let role = recipient.execution_role.as_deref();
                let slot = recipient.execution_slot_id.as_deref();
                // An explicit claim needs a named role or slot: a recipient
                // naming neither cannot take a slotted group by default, or
                // one group would land on every slotless recipient in turn.
                // Slotless recipients map positionally only when the counts
                // align; otherwise they simply claim nothing here.
                let explicit: Vec<usize> = groups
                    .iter()
                    .enumerate()
                    .filter(|(_, group)| {
                        (role.is_some() || slot.is_some())
                            && role.is_none_or(|role| role == group.role)
                            && slot.is_none_or(|slot| group.slot.as_deref() == Some(slot))
                    })
                    .map(|(position, _)| position)
                    .collect();
                let positions = if explicit.len() == 1 {
                    explicit
                } else if !explicit.is_empty() {
                    return Err(CoreServiceError::business(
                        "DOCUMENT_SIGN_FIELD_INVALID",
                        format!(
                            "Ambiguous template blocks for recipient {} (execution role + slot must name one block).",
                            recipient.email
                        ),
                    ));
                } else if role.is_none() && slot.is_none() && groups.len() == recipients.len() {
                    vec![index]
                } else {
                    Vec::new()
                };
                let mut selected = Vec::new();
                for position in positions {
                    claimed[position] = true;
                    selected.extend(groups[position].anchors.clone());
                }
                plan.push((recipient, selected));
            }
            let unclaimed: Vec<String> = groups
                .iter()
                .enumerate()
                .filter(|(position, _)| !claimed[*position])
                .map(|(_, group)| match &group.slot {
                    Some(slot) => format!("{}:{slot}", group.role),
                    None => group.role.clone(),
                })
                .collect();
            if !unclaimed.is_empty() {
                return Err(CoreServiceError::business(
                    "DOCUMENT_SIGN_FIELD_INVALID",
                    format!(
                        "Template blocks no recipient claims: {}. Give a recipient the matching execution role + slot.",
                        unclaimed.join(", ")
                    ),
                ));
            }
            let mut created_field_ids = Vec::new();
            for (recipient, selected) in plan {
                for anchor in selected {
                    if let Some(id) = known.get(&anchor_key(&anchor)) {
                        // Re-importing a template reuses its fields: geometry
                        // moves through putField, never as an import side effect.
                        created_field_ids.push(id.clone());
                        continue;
                    }
                    let field = self
                        .put_field_transactional(
                            tx,
                            &PutSignatureFieldRequest {
                                signature_request_id: request.signature_request_id.clone(),
                                field_id: None,
                                recipient_id: recipient.id.clone(),
                                field_key: anchor_key(&anchor),
                                field_type: anchor.field_type(),
                                page_number: anchor.page_number(),
                                position_x: anchor.x_percent(),
                                position_y: anchor.y_percent(),
                                width: anchor.width_percent(),
                                height: anchor.height_percent(),
                                required: true,
                                label: None,
                                configuration: json!({}),
                            },
                            context,
                        )
                        .await?;
                    created_field_ids.push(field.id);
                }
            }
            Ok(ImportAnchorFieldsResult {
                signature_request_id: request.signature_request_id.clone(),
                created_field_ids,
            })
        }
        .await;

        audit_result(
            &self.runtime,
            "document-sign",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn finalize_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<DocumentSignFinalizeResult, CoreServiceError> {
        let internal = internal_context(context);
        let (status, transaction_document_id) = self
            .repository
            .canonical_status_tx(tx, signature_request_id)
            .await?
            .ok_or_else(|| {
                CoreServiceError::business(
                    "DOCUMENT_SIGN_NOT_FOUND",
                    "Canonical signature request not found.",
                )
            })?;
        if status == SignatureRequestStatus::Completed.as_str() {
            let audit_media_id = self
                .repository
                .audit_media_for_request_tx(tx, signature_request_id)
                .await?;
            let signed_media_id = self
                .repository
                .signed_media_for_request_tx(tx, signature_request_id)
                .await?;
            return Ok(DocumentSignFinalizeResult {
                signature_request_id: signature_request_id.to_owned(),
                audit_media_id,
                signed_media_id,
                already_completed: true,
            });
        }
        if status != SignatureRequestStatus::Signed.as_str() {
            return Err(CoreServiceError::business(
                "DOCUMENT_SIGN_NOT_MUTABLE",
                "Only a signed envelope can be finalized.",
            ));
        }
        if !self
            .signer
            .envelope_ready_tx(tx, signature_request_id)
            .await?
        {
            return Err(CoreServiceError::business(
                "DOCUMENT_SIGN_NOT_MUTABLE",
                "All required recipients must complete before finalize.",
            ));
        }
        let inputs = self
            .signer
            .finalize_inputs_tx(tx, signature_request_id)
            .await?;
        let finalized_at = Utc::now().to_rfc3339();
        let certificate = crate::vault::signing_certificate::render_completion_certificate(
            signature_request_id,
            &transaction_document_id,
            &inputs.recipients,
            &inputs.fields,
            &inputs.events,
            &finalized_at,
        )
        .map_err(|error| {
            CoreServiceError::business(
                "DOCUMENT_SIGN_FINALIZE_FAILED",
                format!("Completion certificate could not be rendered: {error}"),
            )
        })?;
        let audit_media_id = self
            .repository
            .store_audit_artifact_tx(
                tx,
                &transaction_document_id,
                &format!("signature-completion-{signature_request_id}.pdf"),
                "application/pdf",
                &certificate,
            )
            .await?;
        // The original bytes travel through Vault under the CALLER's
        // authority, never the internal service actor: sealing another
        // party's document is the operator's own grant or it does not happen.
        let original: Option<Vec<u8>> = match self
            .vault
            .get_document(&transaction_document_id, context)
            .await
            .map_err(CoreServiceError::from)?
            .and_then(|document| document.media_id)
        {
            Some(media_id) => self
                .vault
                .media_bytes(&media_id, context)
                .await
                .map_err(CoreServiceError::from)?
                .map(|media| media.bytes),
            None => None,
        };
        let signed_media_id = match original {
            Some(bytes) => {
                let overlay = seal_overlay(&inputs, &bytes)?;
                let media_id = self
                    .repository
                    .store_signed_artifact_tx(
                        tx,
                        &format!("signature-signed-{signature_request_id}.pdf"),
                        "application/pdf",
                        &overlay,
                    )
                    .await?;
                self.repository
                    .link_signed_media_tx(tx, &transaction_document_id, &media_id)
                    .await?;
                Some(media_id)
            }
            _ => None,
        };
        self.signer
            .append_finalize_evidence_tx(
                tx,
                signature_request_id,
                &audit_media_id,
                signed_media_id.as_deref(),
                &internal,
            )
            .await?;
        self.signature
            .transition_transactional(
                tx,
                signature_request_id,
                SignatureRequestStatus::Completed,
                &internal,
            )
            .await?;
        Ok(DocumentSignFinalizeResult {
            signature_request_id: signature_request_id.to_owned(),
            audit_media_id: Some(audit_media_id),
            signed_media_id,
            already_completed: false,
        })
    }

    /// Expire what the clock has passed: overdue recipients first, then
    /// overdue envelopes (access revoked, canonical status to Expired).
    /// Runs inside the durable dispatcher; auth comes from the command.
    /// Terminal states never reopen, and envelopes without an expiry clock
    /// are left alone no matter how old they are.
    pub async fn sweep_due_transactional(
        &self,
        tx: &mut DbTransaction,
        context: &ServiceContext,
    ) -> Result<DocumentSignSweepResult, CoreServiceError> {
        let internal = internal_context(context);
        let mut expired_recipients = Vec::new();
        for (recipient_id, signature_request_id) in self
            .signer
            .expire_overdue_recipients_tx(tx)
            .await
            .map_err(CoreServiceError::from)?
        {
            self.signer
                .append_sweep_evidence_tx(tx, &signature_request_id, &recipient_id, &internal)
                .await
                .map_err(CoreServiceError::from)?;
            expired_recipients.push(recipient_id);
        }
        let mut expired_envelopes = Vec::new();
        for signature_request_id in self.repository.overdue_envelopes_tx(tx).await? {
            self.signer
                .revoke_request_access_transactional(tx, &signature_request_id, &internal)
                .await?;
            self.signature
                .transition_transactional(
                    tx,
                    &signature_request_id,
                    SignatureRequestStatus::Expired,
                    &internal,
                )
                .await?;
            expired_envelopes.push(signature_request_id);
        }
        Ok(DocumentSignSweepResult {
            expired_recipients,
            expired_envelopes,
        })
    }
}

/// Printable text for one answered field. Signature and initials draw
/// the recipient's name in their adopted form; dates draw the completion
/// day; everything else draws its stored scalar. Unanswered or
/// non-scalar values draw nothing rather than a placeholder.
fn overlay_text(
    field_type: &str,
    value: &Option<serde_json::Value>,
    recipient_name: &str,
    completed_at: &Option<String>,
) -> Option<String> {
    match field_type {
        "signature" => Some(recipient_name.to_owned()),
        "initials" => {
            let marks: String = recipient_name
                .split_whitespace()
                .filter_map(|part| part.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase();
            if marks.is_empty() {
                None
            } else {
                Some(marks)
            }
        }
        "date" => completed_at
            .as_deref()
            .and_then(|at| at.get(..10))
            .map(str::to_owned)
            .or_else(|| {
                value
                    .as_ref()
                    .and_then(|value| value.as_str().map(str::to_owned))
            }),
        "checkbox" => match value {
            Some(serde_json::Value::Bool(true)) => Some("X".into()),
            _ => None,
        },
        _ => value.as_ref().and_then(|value| match value {
            serde_json::Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            }
            serde_json::Value::Number(number) => Some(number.to_string()),
            serde_json::Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        }),
    }
}

/// Seal the original bytes with every answered field, positioned by the
/// field geometry the template import recorded. Pure apart from parsing.
fn seal_overlay(inputs: &FinalizeInputs, original: &[u8]) -> Result<Vec<u8>, CoreServiceError> {
    use crate::vault::signing_overlay::{overlay_fields, OverlayField};
    let names: std::collections::BTreeMap<&str, &str> = inputs
        .recipients
        .iter()
        .map(|recipient| (recipient.id.as_str(), recipient.name.as_str()))
        .collect();
    let mut fields = Vec::new();
    for field in &inputs.fields {
        let name = names
            .get(field.recipient_id.as_str())
            .copied()
            .unwrap_or("");
        let Some(text) = overlay_text(&field.field_type, &field.value, name, &field.completed_at)
        else {
            continue;
        };
        fields.push(OverlayField {
            page_number: field.page_number,
            x_percent: field.position_x,
            y_percent: field.position_y,
            width_percent: field.width,
            height_percent: field.height,
            signature: field.field_type == "signature",
            text,
        });
    }
    overlay_fields(original, &fields).map_err(|error| {
        CoreServiceError::business(
            "DOCUMENT_SIGN_FINALIZE_FAILED",
            format!("Signed PDF could not be sealed: {error}"),
        )
    })
}

fn internal_context(context: &ServiceContext) -> ServiceContext {
    ServiceContext {
        actor: services::ServiceActor {
            id: Some(DOCUMENT_SIGN_SERVICE_ACTOR.into()),
            kind: services::ServiceActorKind::System,
        },
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        principal: None,
    }
}

fn prepared_recipients(
    recipients: &[model::DocumentSignRecipientInput],
) -> Vec<PreparedSignatureRecipient> {
    recipients
        .iter()
        .map(|recipient| PreparedSignatureRecipient {
            role: recipient.role,
            name: recipient.name.clone(),
            email: recipient.email.clone(),
            order: recipient.signer_order,
            signing_step: recipient.signing_step,
            execution_role: recipient.execution_role.clone(),
            execution_slot_id: recipient.execution_slot_id.clone(),
        })
        .collect()
}

fn prepare_intent_matches(
    config: &DocumentSignConfig,
    existing: &[DocumentSignRecipient],
    request: &PrepareDocumentSignRequest,
    requested_expiry: Option<DateTime<Utc>>,
) -> bool {
    if config.signing_mode != request.signing_mode || config.subject != request.subject {
        return false;
    }
    let stored_expiry = config
        .expires_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    if stored_expiry != requested_expiry {
        return false;
    }

    let mut requested = request.recipients.clone();
    requested.sort_by_key(|recipient| recipient.signer_order);
    if existing.len() != requested.len() {
        return false;
    }
    existing.iter().zip(requested.iter()).all(|(left, right)| {
        left.role == right.role
            && left.name.trim() == right.name.trim()
            && left.email.trim().eq_ignore_ascii_case(right.email.trim())
            && left.signer_order == right.signer_order
            && left.signing_step == right.signing_step
            && left.execution_role == right.execution_role
            && left.execution_slot_id == right.execution_slot_id
    })
}

/// A signature box in the lower part of the page, two to a row and filling upward. Page percent: x, y, width, height.
fn default_signature_box(slot: usize) -> (f64, f64, f64, f64) {
    const WIDTH: f64 = 38.0;
    const HEIGHT: f64 = 7.0;
    let column = (slot % 2) as f64;
    let row = (slot / 2) as f64;
    (
        8.0 + column * (WIDTH + 8.0),
        (86.0 - row * (HEIGHT + 3.0)).max(2.0),
        WIDTH,
        HEIGHT,
    )
}

fn validate_prepare(request: &PrepareDocumentSignRequest) -> Result<(), CoreServiceError> {
    if request.transaction_document_id.trim().is_empty() {
        return Err(CoreServiceError::business(
            "DOCUMENT_SIGN_DOCUMENT_REQUIRED",
            "transactionDocumentId is required.",
        ));
    }
    if request
        .subject
        .as_ref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(CoreServiceError::business(
            "DOCUMENT_SIGN_SUBJECT_INVALID",
            "subject must be 500 characters or fewer.",
        ));
    }
    let errors = validate_document_sign_recipients(&request.recipients);
    if !errors.is_empty() {
        return Err(CoreServiceError::business(
            "DOCUMENT_SIGN_RECIPIENT_INVALID",
            errors.join(" "),
        ));
    }
    Ok(())
}

fn parse_future_expiry(value: Option<&str>) -> Result<Option<DateTime<Utc>>, CoreServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| {
            CoreServiceError::business("DOCUMENT_SIGN_EXPIRY_INVALID", "expiresAt must be RFC3339.")
        })?
        .with_timezone(&Utc);
    if parsed <= Utc::now() {
        return Err(CoreServiceError::business(
            "DOCUMENT_SIGN_EXPIRY_INVALID",
            "expiresAt must be in the future.",
        ));
    }
    Ok(Some(parsed))
}

/// One parsed template anchor with its owning group key.
#[derive(Debug, Clone)]
struct GroupedAnchor {
    role: String,
    slot: Option<String>,
    kind: SignatureFieldType,
    page_number: i32,
    key: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl GroupedAnchor {
    fn field_type(&self) -> SignatureFieldType {
        self.kind
    }
    fn page_number(&self) -> i32 {
        self.page_number
    }
    fn x_percent(&self) -> f64 {
        self.x
    }
    fn y_percent(&self) -> f64 {
        self.y
    }
    fn width_percent(&self) -> f64 {
        self.width
    }
    fn height_percent(&self) -> f64 {
        self.height
    }
}

fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn parse_anchor(anchor: &TemplateAnchor, index: usize) -> Result<GroupedAnchor, CoreServiceError> {
    let invalid = |detail: &str| {
        CoreServiceError::business(
            "DOCUMENT_SIGN_FIELD_INVALID",
            format!("Template anchor {index} is unusable: {detail}."),
        )
    };
    if anchor.role.trim().is_empty() {
        return Err(invalid("missing role"));
    }
    let kind = match anchor.kind.as_str() {
        "signature" => SignatureFieldType::Signature,
        "initials" => SignatureFieldType::Initials,
        "date" => SignatureFieldType::Date,
        other => return Err(invalid(&format!("unknown kind {other}"))),
    };
    if anchor.page_index < 0
        || !anchor.page_width.is_finite()
        || anchor.page_width <= 0.0
        || !anchor.page_height.is_finite()
        || anchor.page_height <= 0.0
    {
        return Err(invalid("bad page"));
    }
    let rect = &anchor.rect;
    for value in [rect.x, rect.y, rect.width, rect.height] {
        if !value.is_finite() {
            return Err(invalid("bad rectangle"));
        }
    }
    if rect.x < 0.0 || rect.y < 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return Err(invalid("bad rectangle"));
    }
    if rect.y + rect.height > anchor.page_height {
        return Err(invalid("rectangle leaves the page"));
    }
    // PDF points run bottom-left; field percentages run top-left.
    let clamp = |value: f64| value.clamp(0.0, 100.0);
    let slot = anchor
        .slot_id
        .clone()
        .map(|slot| slot.trim().to_owned())
        .filter(|slot| !slot.is_empty());
    let key = format!(
        "{}-{}-{}-{}-{}",
        slug(&anchor.role),
        slot.as_deref().unwrap_or("open"),
        anchor.kind,
        anchor.page_index,
        index,
    );
    Ok(GroupedAnchor {
        role: anchor.role.trim().to_owned(),
        slot,
        kind,
        page_number: anchor.page_index + 1,
        key,
        x: clamp(rect.x / anchor.page_width * 100.0),
        y: clamp((1.0 - (rect.y + rect.height) / anchor.page_height) * 100.0),
        width: clamp(rect.width / anchor.page_width * 100.0),
        height: clamp(rect.height / anchor.page_height * 100.0),
    })
}

/// Group anchors by (role, slot) in first-seen order — the retired
/// BoldSign matcher's grouping, minus the provider.
fn group_anchor_sets(
    anchors: &[TemplateAnchor],
) -> Result<Vec<GroupedAnchorSet>, CoreServiceError> {
    let mut order: Vec<(String, Option<String>)> = Vec::new();
    let mut parsed = Vec::with_capacity(anchors.len());
    for (index, anchor) in anchors.iter().enumerate() {
        let entry = parse_anchor(anchor, index)?;
        if !order.contains(&(entry.role.clone(), entry.slot.clone())) {
            order.push((entry.role.clone(), entry.slot.clone()));
        }
        parsed.push(entry);
    }
    Ok(order
        .into_iter()
        .map(|(role, slot)| {
            let members = parsed
                .iter()
                .filter(|entry| entry.role == role && entry.slot == slot)
                .cloned()
                .collect();
            GroupedAnchorSet {
                role,
                slot,
                anchors: members,
            }
        })
        .collect())
}

struct GroupedAnchorSet {
    role: String,
    slot: Option<String>,
    anchors: Vec<GroupedAnchor>,
}

fn anchor_key(anchor: &GroupedAnchor) -> String {
    anchor.key.clone()
}

fn validate_field(request: &PutSignatureFieldRequest) -> Result<(), CoreServiceError> {
    let valid_geometry = request.page_number > 0
        && request.position_x >= 0.0
        && request.position_y >= 0.0
        && request.width > 0.0
        && request.height > 0.0
        && request.position_x + request.width <= 100.0
        && request.position_y + request.height <= 100.0;
    if request.recipient_id.trim().is_empty()
        || request.field_key.trim().is_empty()
        || !request.configuration.is_object()
        || !valid_geometry
    {
        return Err(CoreServiceError::business(
            "DOCUMENT_SIGN_FIELD_INVALID",
            "Field owner, key, object configuration and valid page geometry are required.",
        ));
    }
    Ok(())
}

async fn ensure_mutable_config<R: DocumentSignRepository>(
    repository: &R,
    tx: &mut DbTransaction,
    signature_request_id: &str,
) -> Result<DocumentSignConfig, CoreServiceError> {
    let config = repository
        .lock_config_tx(tx, signature_request_id)
        .await?
        .ok_or_else(|| {
            CoreServiceError::business(
                "DOCUMENT_SIGN_NOT_FOUND",
                "Native document-sign request not found.",
            )
        })?;
    if config.issued_at.is_some() {
        return Err(CoreServiceError::business(
            "DOCUMENT_SIGN_NOT_MUTABLE",
            "Issued recipients and fields are immutable.",
        ));
    }
    Ok(config)
}

fn capability(
    name: &str,
    kind: OperationKind,
    description: &str,
    authorization: &str,
    idempotent: bool,
    execution: ServiceExecutionPolicy,
) -> ServiceCapability {
    ServiceCapability {
        name: name.into(),
        kind,
        description: description.into(),
        authorization: authorization.into(),
        idempotent,
        execution,
    }
}

#[async_trait]
impl<R, SR, SGR, ER, VR> AbstractService for DocumentSignService<R, SR, SGR, ER, VR>
where
    R: DocumentSignRepository + 'static,
    SR: SignatureRepository + 'static,
    SGR: SignerRepository + 'static,
    ER: EmailRepository + 'static,
    VR: VaultRepository + 'static,
{
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "document-sign".into(),
            version: "1".into(),
            description: "Native CulebraLuxe document-sign orchestration service".into(),
            capabilities: vec![
                capability(
                    "documentSign.get",
                    OperationKind::Query,
                    "Read one native document-sign envelope.",
                    "documentSign.read",
                    true,
                    ServiceExecutionPolicy::inline(),
                ),
                capability(
                    "documentSign.fields",
                    OperationKind::Query,
                    "Read native recipient-owned signing fields.",
                    "documentSign.read",
                    true,
                    ServiceExecutionPolicy::inline(),
                ),
                capability(
                    "documentSign.list",
                    OperationKind::Query,
                    "List recent native envelopes with recipient progress.",
                    "documentSign.read",
                    true,
                    ServiceExecutionPolicy::inline(),
                ),
                capability(
                    "documentSign.prepare",
                    OperationKind::Command,
                    "Prepare a native signing envelope without external-provider delivery.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("transactionDocumentId"),
                ),
                capability(
                    "documentSign.setRecipients",
                    OperationKind::Command,
                    "Replace draft recipients before issue.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.putField",
                    OperationKind::Command,
                    "Create or edit a recipient-owned draft signing field.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.removeField",
                    OperationKind::Command,
                    "Remove a draft signing field.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.send",
                    OperationKind::Command,
                    "Prepare, place signature fields, and issue an envelope in one transaction.",
                    "documentSign.issue",
                    true,
                    ServiceExecutionPolicy::ordered("transactionDocumentId"),
                ),
                capability(
                    "documentSign.issue",
                    OperationKind::Command,
                    "Atomically issue a native envelope and queue signer invitations.",
                    "documentSign.issue",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.void",
                    OperationKind::Command,
                    "Void a native envelope.",
                    "documentSign.void",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.finalize",
                    OperationKind::Command,
                    "Close a fully-signed envelope with its audit artifact.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.sweepDue",
                    OperationKind::Command,
                    "Expire overdue recipients and envelopes.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::inline(),
                ),
                capability(
                    "documentSign.resend",
                    OperationKind::Command,
                    "Re-send an invitation (fresh link) or a reminder (live link) to one recipient.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
                capability(
                    "documentSign.importFields",
                    OperationKind::Command,
                    "Build recipient-owned fields from template anchor blocks.",
                    "documentSign.write",
                    true,
                    ServiceExecutionPolicy::ordered("signatureRequestId"),
                ),
            ],
            dependencies: vec![
                "signature".into(),
                "signer".into(),
                "email".into(),
                "vault".into(),
            ],
            invariants: vec![
                "signature_request is the only canonical envelope lifecycle.".into(),
                "Issued recipient identity and field ownership are immutable.".into(),
                "Issuance and durable invitation queueing commit atomically.".into(),
                "Document bytes remain owned and authorized by Vault.".into(),
                format!(
                    "Invitation delivery leaves the issue transaction through the existing {EMAIL_DELIVERY_ROUTING_KEY} MQ route."
                ),
            ],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "documentSign.get" => {
                let id = required_string(envelope, "signatureRequestId")?;
                serde_json::to_value(self.get(&id, context).await.map_err(service_error)?)
                    .map_err(serialization_error)
            }
            "documentSign.fields" => {
                let id = required_string(envelope, "signatureRequestId")?;
                serde_json::to_value(self.fields(&id, context).await.map_err(service_error)?)
                    .map_err(serialization_error)
            }
            "documentSign.list" => {
                serde_json::to_value(self.list(context).await.map_err(service_error)?)
                    .map_err(serialization_error)
            }
            "documentSign.prepare"
            | "documentSign.setRecipients"
            | "documentSign.putField"
            | "documentSign.removeField"
            | "documentSign.issue"
            | "documentSign.void"
            | "documentSign.resend"
            | "documentSign.finalize"
            | "documentSign.sweepDue"
            | "documentSign.send"
            | "documentSign.importFields" => Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                format!(
                    "{} must enter through the durable command dispatcher.",
                    envelope.operation
                ),
                false,
            )),
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "document-sign".into(),
                operation: operation.into(),
            }),
        }
    }
}

fn required_string(
    envelope: &ServiceEnvelope,
    field: &str,
) -> Result<String, ServiceDispatchError> {
    envelope
        .payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| ServiceDispatchError::InvalidPayload {
            domain: envelope.domain.clone(),
            operation: envelope.operation.clone(),
            message: format!("{field} is required."),
        })
}

fn service_error(error: CoreServiceError) -> ServiceDispatchError {
    match error {
        CoreServiceError::Business { code, message } => {
            ServiceDispatchError::business(code, message, false)
        }
        CoreServiceError::Database(error) => {
            ServiceDispatchError::infrastructure("DATABASE", error.to_string(), error.retryable)
        }
        CoreServiceError::Runtime(error) => {
            ServiceDispatchError::infrastructure("SERVICE_RUNTIME", error.to_string(), true)
        }
    }
}

fn serialization_error(error: serde_json::Error) -> ServiceDispatchError {
    ServiceDispatchError::infrastructure("SERVICE_SERIALIZATION_FAILED", error.to_string(), false)
}

pub type ProductionDocumentSignService =
    DocumentSignService<DocumentSignDao, SignatureDao, SignerDao, EmailDao, VaultDao>;

#[cfg(test)]
mod tests {
    use super::*;
    use model::SignatureFieldType;

    fn field() -> PutSignatureFieldRequest {
        PutSignatureFieldRequest {
            signature_request_id: "11111111-1111-4111-8111-111111111111".into(),
            field_id: None,
            recipient_id: "22222222-2222-4222-8222-222222222222".into(),
            field_key: "buyer.signature".into(),
            field_type: SignatureFieldType::Signature,
            page_number: 1,
            position_x: 10.0,
            position_y: 20.0,
            width: 25.0,
            height: 8.0,
            required: true,
            label: Some("Buyer signature".into()),
            configuration: json!({}),
        }
    }

    #[test]
    fn signature_fields_cannot_escape_the_pdf_page() {
        assert!(validate_field(&field()).is_ok());

        let mut bad = field();
        bad.position_x = 90.0;
        bad.width = 20.0;
        assert!(validate_field(&bad).is_err());

        let mut bad = field();
        bad.position_y = 97.0;
        bad.height = 4.0;
        assert!(validate_field(&bad).is_err());
    }

    #[test]
    fn signing_expiry_must_be_future_rfc3339() {
        assert!(parse_future_expiry(Some("not-a-date")).is_err());
        assert!(parse_future_expiry(Some("2000-01-01T00:00:00Z")).is_err());
        assert!(parse_future_expiry(None).unwrap().is_none());
    }
}

#[cfg(test)]
mod anchor_import_tests {
    use super::*;
    use model::{TemplateAnchor, TemplateAnchorRect};

    fn anchor(role: &str, slot: Option<&str>, kind: &str) -> TemplateAnchor {
        TemplateAnchor {
            role: role.into(),
            slot_id: slot.map(str::to_owned),
            kind: kind.into(),
            page_index: 0,
            page_width: 612.0,
            page_height: 792.0,
            rect: TemplateAnchorRect {
                x: 72.0,
                y: 650.0,
                width: 180.0,
                height: 20.0,
            },
        }
    }

    #[test]
    fn groups_form_by_role_and_slot_with_percentage_geometry() {
        let groups = group_anchor_sets(&[
            anchor("seller", Some("s1"), "signature"),
            anchor("seller", Some("s1"), "date"),
            anchor("buyer", None, "signature"),
        ])
        .expect("valid anchors group");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].anchors.len(), 2);
        let first = &groups[0].anchors[0];
        assert_eq!(first.page_number(), 1);
        assert!((first.x_percent() - 11.76).abs() < 0.01);
        // PDF bottom-left to percentage top-left.
        assert!((first.y_percent() - 15.40).abs() < 0.01);
        assert!(first.key.starts_with("seller-s1-signature-0-"));
    }

    #[test]
    fn bad_anchors_fail_loud() {
        assert!(group_anchor_sets(&[anchor("", Some("s1"), "signature")]).is_err());
        assert!(group_anchor_sets(&[anchor("seller", Some("s1"), "seal")]).is_err());
        let mut off_page = anchor("seller", Some("s1"), "signature");
        off_page.rect.y = 780.0;
        assert!(group_anchor_sets(&[off_page]).is_err());
    }
}

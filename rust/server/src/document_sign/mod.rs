use crate::email::{EmailRepository, EmailService, EMAIL_DELIVERY_ROUTING_KEY};
use crate::service_support::{audit_result, authorize, CoreServiceError};
use crate::signature::{SignatureRepository, SignatureService};
use crate::signer::{SignerRepository, SignerService};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use db::{DbResult, DbTransaction, DocumentSignDao, EmailDao, SignatureDao, SignerDao};
use domain::{
    validate_document_sign_recipients, DocumentSignConfig, DocumentSignIssueResult,
    DocumentSignRecipient, DocumentSignSnapshot, EmailMessageKind, IssueDocumentSignRequest,
    PrepareDocumentSignRequest, PrepareSignatureRequest, PreparedSignatureRecipient,
    PutSignatureFieldRequest, QueueEmailRequest, RemoveSignatureFieldRequest,
    SetDocumentSignRecipientsRequest, SignatureField,
    SignatureRequestStatus,
};
use serde_json::{json, Value};
use service::{
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
    async fn recipients(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<DocumentSignRecipient>>;
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
        signing_mode: domain::DocumentSigningMode,
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
}

#[async_trait]
impl DocumentSignRepository for DocumentSignDao {
    async fn config(&self, signature_request_id: &str) -> DbResult<Option<DocumentSignConfig>> {
        DocumentSignDao::config(self, signature_request_id).await
    }
    async fn recipients(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<DocumentSignRecipient>> {
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
        signing_mode: domain::DocumentSigningMode,
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
}

pub struct DocumentSignService<R, SR, SGR, ER> {
    repository: R,
    signature: Arc<SignatureService<SR>>,
    signer: Arc<SignerService<SGR>>,
    email: Arc<EmailService<ER>>,
    runtime: ServiceRuntime,
}

impl<R, SR, SGR, ER> DocumentSignService<R, SR, SGR, ER>
where
    R: DocumentSignRepository,
    SR: SignatureRepository,
    SGR: SignerRepository,
    ER: EmailRepository,
{
    pub fn new(
        repository: R,
        signature: Arc<SignatureService<SR>>,
        signer: Arc<SignerService<SGR>>,
        email: Arc<EmailService<ER>>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            signature,
            signer,
            email,
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
                    .issue_access_transactional(tx, &recipient.id, Some(expires_at.clone()), &internal)
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
                                request.signature_request_id,
                                recipient.id,
                                grant.token_version
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
        Ok(())
    }

    pub async fn signer_declined_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
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
}

fn internal_context(context: &ServiceContext) -> ServiceContext {
    ServiceContext {
        actor: service::ServiceActor {
            id: Some(DOCUMENT_SIGN_SERVICE_ACTOR.into()),
            kind: service::ServiceActorKind::System,
        },
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        principal: None,
    }
}

fn prepared_recipients(
    recipients: &[domain::DocumentSignRecipientInput],
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

fn parse_future_expiry(
    value: Option<&str>,
) -> Result<Option<DateTime<Utc>>, CoreServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| {
            CoreServiceError::business(
                "DOCUMENT_SIGN_EXPIRY_INVALID",
                "expiresAt must be RFC3339.",
            )
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
impl<R, SR, SGR, ER> AbstractService for DocumentSignService<R, SR, SGR, ER>
where
    R: DocumentSignRepository + 'static,
    SR: SignatureRepository + 'static,
    SGR: SignerRepository + 'static,
    ER: EmailRepository + 'static,
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
            "documentSign.prepare"
            | "documentSign.setRecipients"
            | "documentSign.putField"
            | "documentSign.removeField"
            | "documentSign.issue"
            | "documentSign.void" => Err(ServiceDispatchError::business(
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
    ServiceDispatchError::infrastructure(
        "SERVICE_SERIALIZATION_FAILED",
        error.to_string(),
        false,
    )
}

pub type ProductionDocumentSignService =
    DocumentSignService<DocumentSignDao, SignatureDao, SignerDao, EmailDao>;


#[cfg(test)]
mod tests {
    use super::*;
    use domain::SignatureFieldType;

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

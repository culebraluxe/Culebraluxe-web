use crate::luxesign::core::{upload_document, UploadedDocument};
use crate::media::DocumentObjectWriter;
use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{Database, DbFailure, DbResult, DbTransaction, MediaObjectMetadata, SignatureDao};
use model::{
    validate_signature_recipients, ApplySignatureStatusRequest, PrepareSignatureRequest,
    PreparedSignatureRecipient, SendSignatureRequest, SignatureArtifactDownload,
    SignatureCommandOutcome, SignatureCommandResult, SignatureProviderSendRequest,
    SignatureRequest, SignatureRequestResult, SignatureRequestStatus, SignatureStatusResult,
    SignatureWebhookVerification,
};
use serde_json::json;
use services::{
    AbstractObjectStoreService, OperationKind, ServiceContext, ServiceInfrastructure,
    ServiceRuntime, SignatureProvider,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait SignatureRepository: Send + Sync {
    fn database(&self) -> Option<Database> {
        None
    }
    async fn get(&self, id: &str) -> DbResult<Option<SignatureRequest>>;
    async fn active_for_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>>;
    async fn list_by_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>>;
    async fn prepare_tx(
        &self,
        _tx: &mut DbTransaction,
        _request: &PrepareSignatureRequest,
    ) -> DbResult<SignatureRequestResult> {
        Err(DbFailure::configuration(
            "signature.prepare_tx",
            "Repository does not support transactional signature preparation.",
        ))
    }
    async fn replace_recipients_tx(
        &self,
        _tx: &mut DbTransaction,
        _signature_request_id: &str,
        _recipients: &[PreparedSignatureRecipient],
    ) -> DbResult<bool> {
        Err(DbFailure::configuration(
            "signature.replace_recipients_tx",
            "Repository does not support transactional recipient replacement.",
        ))
    }
    async fn get_tx(
        &self,
        _tx: &mut DbTransaction,
        _signature_request_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        Err(DbFailure::configuration(
            "signature.get_tx",
            "Repository does not support transactional signature reads.",
        ))
    }
    async fn set_status_tx(
        &self,
        _tx: &mut DbTransaction,
        _signature_request_id: &str,
        _expected: SignatureRequestStatus,
        _target: SignatureRequestStatus,
    ) -> DbResult<Option<SignatureRequest>> {
        Err(DbFailure::configuration(
            "signature.set_status_tx",
            "Repository does not support transactional status updates.",
        ))
    }

    async fn send(
        &self,
        request: &SendSignatureRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn apply_status(
        &self,
        request: &ApplySignatureStatusRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn cancel(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn decline(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn reconciliation_needs_artifact(
        &self,
        event_id: &str,
        signature_request_id: &str,
    ) -> DbResult<bool>;
    async fn reconcile_completed(
        &self,
        event_id: &str,
        signature_request_id: &str,
        signed_artifact: Option<&SignatureArtifactDownload>,
        audit_artifact: Option<&SignatureArtifactDownload>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    /// Object-first completion: the service uploads the artifacts before this call and passes
    /// `(media_id, filename, mime_type, object)` for each. The default refuses because a repository
    /// that predates this seam cannot store object metadata.
    async fn reconcile_completed_with_objects(
        &self,
        _event_id: &str,
        _signature_request_id: &str,
        _signed_artifact: Option<(&str, &str, &str, &MediaObjectMetadata)>,
        _audit_artifact: Option<(&str, &str, &str, &MediaObjectMetadata)>,
        _actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        Err(DbFailure::configuration(
            "signature.reconcile_completed_with_objects",
            "Repository does not support object-first completion artifacts.",
        ))
    }
}

#[async_trait]
impl SignatureRepository for SignatureDao {
    fn database(&self) -> Option<Database> {
        Some(SignatureDao::database(self))
    }
    async fn get(&self, id: &str) -> DbResult<Option<SignatureRequest>> {
        SignatureDao::get(self, id).await
    }

    async fn active_for_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        SignatureDao::active_for_document(self, transaction_document_id).await
    }

    async fn list_by_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>> {
        SignatureDao::list_by_document(self, transaction_document_id).await
    }

    async fn prepare_tx(
        &self,
        tx: &mut DbTransaction,
        request: &PrepareSignatureRequest,
    ) -> DbResult<SignatureRequestResult> {
        SignatureDao::prepare_tx(self, tx, request).await
    }

    async fn replace_recipients_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        recipients: &[PreparedSignatureRecipient],
    ) -> DbResult<bool> {
        SignatureDao::replace_recipients_tx(self, tx, signature_request_id, recipients).await
    }

    async fn get_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        SignatureDao::get_tx(self, tx, signature_request_id).await
    }

    async fn set_status_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        expected: SignatureRequestStatus,
        target: SignatureRequestStatus,
    ) -> DbResult<Option<SignatureRequest>> {
        SignatureDao::set_status_tx(self, tx, signature_request_id, expected, target).await
    }

    async fn send(
        &self,
        request: &SendSignatureRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::send(self, request, actor_app_user_id).await
    }

    async fn apply_status(
        &self,
        request: &ApplySignatureStatusRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::apply_status(self, request, actor_app_user_id).await
    }

    async fn cancel(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::cancel(self, command_id, signature_request_id, actor_app_user_id).await
    }

    async fn decline(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::decline(self, command_id, signature_request_id, actor_app_user_id).await
    }

    async fn reconciliation_needs_artifact(
        &self,
        event_id: &str,
        signature_request_id: &str,
    ) -> DbResult<bool> {
        SignatureDao::reconciliation_needs_artifact(self, event_id, signature_request_id).await
    }

    async fn reconcile_completed(
        &self,
        event_id: &str,
        signature_request_id: &str,
        signed_artifact: Option<&SignatureArtifactDownload>,
        audit_artifact: Option<&SignatureArtifactDownload>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::reconcile_completed(
            self,
            event_id,
            signature_request_id,
            signed_artifact,
            audit_artifact,
            actor_app_user_id,
        )
        .await
    }

    async fn reconcile_completed_with_objects(
        &self,
        event_id: &str,
        signature_request_id: &str,
        signed_artifact: Option<(&str, &str, &str, &MediaObjectMetadata)>,
        audit_artifact: Option<(&str, &str, &str, &MediaObjectMetadata)>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::reconcile_completed_with_objects(
            self,
            event_id,
            signature_request_id,
            signed_artifact,
            audit_artifact,
            actor_app_user_id,
        )
        .await
    }
}

pub struct SignatureService<R> {
    repository: R,
    provider: Option<Arc<dyn SignatureProvider>>,
    runtime: ServiceRuntime,
    /// Present only when the Vault object store is configured; completion artifacts are then
    /// uploaded before their rows. Without it (tests/dev) artifacts stay byte-backed.
    documents: Option<Arc<DocumentObjectWriter>>,
}

impl<R: SignatureRepository> SignatureService<R> {
    pub fn new(
        repository: R,
        provider: Arc<dyn SignatureProvider>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            provider: Some(provider),
            runtime: ServiceRuntime::new(infrastructure),
            documents: None,
        }
    }

    pub fn new_optional(
        repository: R,
        provider: Option<Arc<dyn SignatureProvider>>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            provider,
            runtime: ServiceRuntime::new(infrastructure),
            documents: None,
        }
    }

    /// Attach the shared Vault document writer. Composition calls this when the object store is
    /// configured; without it this service keeps the legacy byte-backed artifact writes.
    pub fn with_object_store(mut self, store: Arc<dyn AbstractObjectStoreService>) -> Self {
        self.documents = Some(Arc::new(DocumentObjectWriter::new(store)));
        self
    }

    fn provider(&self) -> Result<&Arc<dyn SignatureProvider>, CoreServiceError> {
        self.provider.as_ref().ok_or_else(|| {
            CoreServiceError::business(
                "SIGNATURE_PROVIDER_NOT_CONFIGURED",
                "No external signature provider is configured for this operation.",
            )
        })
    }

    pub async fn get(
        &self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<SignatureRequest>, CoreServiceError> {
        const OP: &str = "signature.get";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(id).await.map_err(Into::into);
        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn active_for_document(
        &self,
        transaction_document_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<SignatureRequest>, CoreServiceError> {
        const OP: &str = "signature.activeForDocument";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .active_for_document(transaction_document_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn list_by_document(
        &self,
        transaction_document_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<SignatureRequest>, CoreServiceError> {
        const OP: &str = "signature.listByDocument";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .list_by_document(transaction_document_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn prepare_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &PrepareSignatureRequest,
        context: &ServiceContext,
    ) -> Result<SignatureRequestResult, CoreServiceError> {
        const OP: &str = "signature.prepare";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let canonical = request
            .recipients
            .iter()
            .map(PreparedSignatureRecipient::as_signature_recipient)
            .collect::<Vec<_>>();
        let errors = validate_signature_recipients(&canonical);
        let result = if request.transaction_document_id.trim().is_empty() {
            Err(CoreServiceError::business(
                "SIGNATURE_DOCUMENT_REQUIRED",
                "transactionDocumentId is required.",
            ))
        } else if request
            .message
            .as_ref()
            .is_some_and(|message| message.len() > 500)
        {
            Err(CoreServiceError::business(
                "SIGNATURE_MESSAGE_TOO_LONG",
                "message must be 500 characters or fewer.",
            ))
        } else if !errors.is_empty()
            || request
                .recipients
                .iter()
                .any(|recipient| recipient.signing_step < 1)
        {
            Err(CoreServiceError::business(
                "SIGNATURE_RECIPIENT_INVALID",
                if errors.is_empty() {
                    "Recipient signingStep must be positive.".into()
                } else {
                    errors.join(" ")
                },
            ))
        } else {
            self.repository
                .prepare_tx(tx, request)
                .await
                .map_err(Into::into)
        };

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn replace_recipients_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        recipients: &[PreparedSignatureRecipient],
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "signature.replaceRecipients";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = match self
            .repository
            .replace_recipients_tx(tx, signature_request_id, recipients)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(CoreServiceError::business(
                "SIGNATURE_NOT_MUTABLE",
                "Signature recipients can only be replaced while the request is in requested state.",
            )),
            Err(error) => Err(error.into()),
        };
        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn transition_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        target: SignatureRequestStatus,
        context: &ServiceContext,
    ) -> Result<SignatureStatusResult, CoreServiceError> {
        const OP: &str = "signature.transition";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let Some(current) = self.repository.get_tx(tx, signature_request_id).await? else {
                return Err(CoreServiceError::business(
                    "SIGNATURE_REQUEST_NOT_FOUND",
                    "Signature request not found.",
                ));
            };
            if current.status == target {
                return Ok(SignatureStatusResult {
                    luxesign_request: current,
                    transitioned: false,
                });
            }
            if !current.status.can_transition_to(target) {
                return Err(CoreServiceError::business(
                    "SIGNATURE_TRANSITION_INVALID",
                    format!(
                        "Transition {} -> {} is not allowed.",
                        current.status.as_str(),
                        target.as_str()
                    ),
                ));
            }
            let Some(updated) = self
                .repository
                .set_status_tx(tx, signature_request_id, current.status, target)
                .await?
            else {
                return Err(CoreServiceError::business(
                    "SIGNATURE_TRANSITION_CONFLICT",
                    "Signature request changed concurrently.",
                ));
            };
            Ok(SignatureStatusResult {
                luxesign_request: updated,
                transitioned: true,
            })
        }
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn send(
        &self,
        request: &SendSignatureRequest,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        const OP: &str = "signature.send";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            if request.transaction_document_id.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "SIGNATURE_DOCUMENT_REQUIRED",
                    "transactionDocumentId is required.",
                ));
            }
            if request
                .message
                .as_ref()
                .is_some_and(|message| message.len() > 500)
            {
                return Err(CoreServiceError::business(
                    "SIGNATURE_MESSAGE_TOO_LONG",
                    "message must be 500 characters or fewer.",
                ));
            }
            let recipient_errors = validate_signature_recipients(&request.recipients);
            if !recipient_errors.is_empty() {
                return Err(CoreServiceError::business(
                    "SIGNATURE_RECIPIENT_INVALID",
                    recipient_errors.join(" "),
                ));
            }

            let actor = actor_app_user_id(context);
            let canonical = self.repository.send(request, actor).await?;
            if canonical.outcome != SignatureCommandOutcome::Success {
                return Ok(canonical);
            }

            let value = canonical.value.clone().ok_or_else(|| {
                CoreServiceError::business(
                    "SIGNATURE_SEND_RESULT_INVALID",
                    "Canonical signature send returned no request value.",
                )
            })?;
            let recorded: SignatureRequestResult =
                serde_json::from_value(value).map_err(|error| {
                    CoreServiceError::business(
                        "SIGNATURE_SEND_RESULT_INVALID",
                        format!("Canonical signature send result is invalid: {error}"),
                    )
                })?;

            let provider = self.provider()?;
            let delivery = provider
                .send(SignatureProviderSendRequest {
                    signature_request_id: recorded.luxesign_request.id.clone(),
                    transaction_document_id: recorded
                        .luxesign_request
                        .transaction_document_id
                        .clone(),
                    recipients: request.recipients.clone(),
                    message: recorded.luxesign_request.message.clone(),
                    signature_role: request
                        .signature_role
                        .clone()
                        .or_else(|| request.execution_role.clone()),
                    signature_slot_id: request.execution_slot_id.clone(),
                    completion_recipient_emails: request.completion_recipient_emails.clone(),
                })
                .await
                .map_err(|message| {
                    CoreServiceError::business("SIGNATURE_PROVIDER_FAILURE", message)
                })?;

            let target = provider.map_status(&delivery.provider_status);
            let status = self
                .repository
                .apply_status(
                    &ApplySignatureStatusRequest {
                        command_id: Uuid::new_v4().to_string(),
                        signature_request_id: recorded.luxesign_request.id,
                        target_status: Some(target),
                    },
                    actor,
                )
                .await?;

            emit_status_event(&self.runtime, &status, target, context).await?;
            Ok(status)
        })
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn refresh_status(
        &self,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        const OP: &str = "signature.refreshStatus";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            let observed = self
                .provider()?
                .status(signature_request_id)
                .await
                .map_err(|message| {
                    CoreServiceError::business("SIGNATURE_PROVIDER_FAILURE", message)
                })?;
            let command = self
                .repository
                .apply_status(
                    &ApplySignatureStatusRequest {
                        command_id: Uuid::new_v4().to_string(),
                        signature_request_id: signature_request_id.to_owned(),
                        target_status: Some(observed.status),
                    },
                    actor_app_user_id(context),
                )
                .await?;
            emit_status_event(&self.runtime, &command, observed.status, context).await?;
            Ok(command)
        })
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn cancel(
        &self,
        command_id: &str,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        const OP: &str = "signature.cancel";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            let provider = self
                .provider()?
                .cancel(signature_request_id)
                .await
                .map_err(|message| {
                    CoreServiceError::business("SIGNATURE_PROVIDER_FAILURE", message)
                })?;
            if !provider.ok {
                return Ok(SignatureCommandResult {
                    command_id: command_id.to_owned(),
                    outcome: SignatureCommandOutcome::Conflict,
                    aggregate_id: Some(signature_request_id.to_owned()),
                    message: Some(provider.error.unwrap_or_else(|| {
                        "Provider revocation could not be proven; the signature request remains active."
                            .into()
                    })),
                    replayed: false,
                    value: None,
                });
            }

            let command = self
                .repository
                .cancel(
                    command_id,
                    signature_request_id,
                    actor_app_user_id(context),
                )
                .await?;
            emit_status_event(
                &self.runtime,
                &command,
                SignatureRequestStatus::Voided,
                context,
            )
            .await?;
            Ok(command)
        })
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn decline(
        &self,
        command_id: &str,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        const OP: &str = "signature.decline";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            let command = self
                .repository
                .decline(command_id, signature_request_id, actor_app_user_id(context))
                .await?;
            emit_status_event(
                &self.runtime,
                &command,
                SignatureRequestStatus::Declined,
                context,
            )
            .await?;
            Ok(command)
        })
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    async fn reconcile_completed_internal(
        &self,
        event_id: &str,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        let needs_artifact = self
            .repository
            .reconciliation_needs_artifact(event_id, signature_request_id)
            .await?;

        let signed_artifact = if needs_artifact {
            Some(
                self.provider()?
                    .download_signed_artifact(signature_request_id)
                    .await
                    .map_err(|message| {
                        CoreServiceError::business("SIGNATURE_ARTIFACT_DOWNLOAD_FAILED", message)
                    })?,
            )
        } else {
            None
        };
        let audit_artifact = if needs_artifact {
            self.provider()?
                .download_audit_trail(signature_request_id)
                .await
                .map_err(|message| {
                    CoreServiceError::business("SIGNATURE_AUDIT_DOWNLOAD_FAILED", message)
                })?
        } else {
            None
        };

        // Object-first: upload and verify the artifacts BEFORE the DAO opens its transaction, so the
        // rows it writes carry object metadata and never `media.file_data`. Without an object store
        // (tests/dev) the legacy byte path below is used.
        if let (Some(writer), Some(signed)) = (self.documents.as_deref(), signed_artifact.as_ref())
        {
            let signed_upload =
                match upload_document(writer, &signed.filename, &signed.mime_type, &signed.bytes)
                    .await
                {
                    Ok(document) => document,
                    Err(error) => return Err(error),
                };
            let audit_upload = match audit_artifact.as_ref() {
                Some(audit) => {
                    match upload_document(writer, &audit.filename, &audit.mime_type, &audit.bytes)
                        .await
                    {
                        Ok(document) => Some(document),
                        Err(error) => {
                            signed_upload.discard(writer).await;
                            return Err(error);
                        }
                    }
                }
                None => None,
            };
            let signed_object = signed_upload.object();
            let audit_object = audit_upload.as_ref().map(UploadedDocument::object);
            let signed_args = Some((
                signed_upload.media_id.as_str(),
                signed_upload.filename.as_str(),
                signed_upload.mime_type.as_str(),
                &signed_object,
            ));
            let audit_args =
                audit_upload
                    .as_ref()
                    .zip(audit_object.as_ref())
                    .map(|(document, object)| {
                        (
                            document.media_id.as_str(),
                            document.filename.as_str(),
                            document.mime_type.as_str(),
                            object,
                        )
                    });
            let result = self
                .repository
                .reconcile_completed_with_objects(
                    event_id,
                    signature_request_id,
                    signed_args,
                    audit_args,
                    actor_app_user_id(context),
                )
                .await;
            // Only a fresh success consumes the objects; a replay or a guard refusal writes no rows,
            // so those uploads are removed instead of leaking.
            let stored = matches!(
                &result,
                Ok(command)
                    if command.outcome == SignatureCommandOutcome::Success && !command.replayed
            );
            if !stored {
                signed_upload.discard(writer).await;
                if let Some(document) = &audit_upload {
                    document.discard(writer).await;
                }
            }
            return result.map_err(Into::into);
        }

        self.repository
            .reconcile_completed(
                event_id,
                signature_request_id,
                signed_artifact.as_ref(),
                audit_artifact.as_ref(),
                actor_app_user_id(context),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn reconcile_completed(
        &self,
        event_id: &str,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        const OP: &str = "signature.reconcileCompleted";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = self
            .reconcile_completed_internal(event_id, signature_request_id, context)
            .await;
        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn handle_webhook(
        &self,
        raw_payload: &str,
        signature: &str,
        context: &ServiceContext,
    ) -> Result<SignatureCommandResult, CoreServiceError> {
        const OP: &str = "signature.webhook";
        let decision = authorize(
            &self.runtime,
            "signature",
            "signature.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            let verified: SignatureWebhookVerification = self
                .provider()?
                .verify_webhook(raw_payload, signature)
                .await
                .map_err(|message| {
                    CoreServiceError::business("SIGNATURE_WEBHOOK_INVALID", message)
                })?;
            let target = verified.event.as_status();
            let status_command_id = Uuid::new_v4().to_string();
            let signature_request_id = verified.signature_request_id;
            let command = self
                .repository
                .apply_status(
                    &ApplySignatureStatusRequest {
                        command_id: status_command_id.clone(),
                        signature_request_id: signature_request_id.clone(),
                        target_status: Some(target),
                    },
                    actor_app_user_id(context),
                )
                .await?;
            emit_status_event(&self.runtime, &command, target, context).await?;
            if target == SignatureRequestStatus::Completed
                && command.outcome == SignatureCommandOutcome::Success
            {
                let reconciliation = self
                    .reconcile_completed_internal(
                        &status_command_id,
                        &signature_request_id,
                        context,
                    )
                    .await?;
                if reconciliation.outcome != SignatureCommandOutcome::Success {
                    return Err(CoreServiceError::business(
                        "SIGNATURE_RECONCILIATION_FAILED",
                        reconciliation
                            .message
                            .unwrap_or_else(|| "Signed-artifact reconciliation failed.".into()),
                    ));
                }
            }
            Ok(command)
        })
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }
}

fn actor_app_user_id(context: &ServiceContext) -> Option<&str> {
    context
        .principal
        .as_ref()
        .map(|principal| principal.app_user_id.as_str())
        .or(context.actor.id.as_deref())
}

async fn emit_status_event(
    runtime: &ServiceRuntime,
    command: &SignatureCommandResult,
    status: SignatureRequestStatus,
    context: &ServiceContext,
) -> Result<(), CoreServiceError> {
    if command.outcome != SignatureCommandOutcome::Success || command.replayed {
        return Ok(());
    }
    let transitioned = command
        .value
        .as_ref()
        .and_then(|value| serde_json::from_value::<SignatureStatusResult>(value.clone()).ok())
        .is_some_and(|result| result.transitioned);
    if !transitioned {
        return Ok(());
    }
    let event_type = match status {
        SignatureRequestStatus::Sent => Some("SIGNATURE_REQUEST_SENT"),
        SignatureRequestStatus::Completed => Some("SIGNATURE_REQUEST_COMPLETED"),
        SignatureRequestStatus::Declined => Some("SIGNATURE_REQUEST_DECLINED"),
        SignatureRequestStatus::Voided => Some("SIGNATURE_REQUEST_VOIDED"),
        _ => None,
    };
    if let Some(event_type) = event_type {
        runtime
            .emit(
                event_type,
                command.aggregate_id.clone(),
                BTreeMap::from([
                    (
                        "signatureRequestId".into(),
                        json!(command.aggregate_id.clone()),
                    ),
                    ("status".into(), json!(status.as_str())),
                ]),
                context,
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod object_first_reconcile_tests {
    use super::*;
    use crate::luxesign::core::fake_vault::FakeVault;
    use model::{
        SignatureProviderActionResult, SignatureProviderSendRequest, SignatureProviderSendResult,
        SignatureProviderStatusResult,
    };
    use services::{
        CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
        ServiceActorKind, ServicePrincipal,
    };
    use std::sync::Mutex;

    fn infrastructure() -> ServiceInfrastructure {
        ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        )
    }

    fn context() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("reconcile-tester".into()),
                kind: ServiceActorKind::User,
            },
            correlation_id: "reconcile-object-first".into(),
            causation_id: None,
            principal: Some(ServicePrincipal {
                app_user_id: "reconcile-tester".into(),
                level: "USER".into(),
                role_codes: vec!["user".into()],
                account_type: "internal".into(),
                entitlement_codes: vec![],
            }),
        }
    }

    fn artifact(filename: &str, bytes: &[u8]) -> SignatureArtifactDownload {
        SignatureArtifactDownload {
            bytes: bytes.to_vec(),
            filename: filename.to_owned(),
            mime_type: "application/pdf".to_owned(),
        }
    }

    struct FakeProvider {
        signed: SignatureArtifactDownload,
        audit: SignatureArtifactDownload,
    }

    #[async_trait]
    impl SignatureProvider for FakeProvider {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn map_status(&self, _provider_status: &str) -> SignatureRequestStatus {
            SignatureRequestStatus::Completed
        }

        async fn send(
            &self,
            _request: SignatureProviderSendRequest,
        ) -> Result<SignatureProviderSendResult, String> {
            unimplemented!("not exercised by reconcile")
        }

        async fn status(
            &self,
            _signature_request_id: &str,
        ) -> Result<SignatureProviderStatusResult, String> {
            unimplemented!("not exercised by reconcile")
        }

        async fn cancel(
            &self,
            _signature_request_id: &str,
        ) -> Result<SignatureProviderActionResult, String> {
            unimplemented!("not exercised by reconcile")
        }

        async fn verify_webhook(
            &self,
            _raw_payload: &str,
            _signature: &str,
        ) -> Result<SignatureWebhookVerification, String> {
            unimplemented!("not exercised by reconcile")
        }

        async fn download_signed_artifact(
            &self,
            _signature_request_id: &str,
        ) -> Result<SignatureArtifactDownload, String> {
            Ok(self.signed.clone())
        }

        async fn download_audit_trail(
            &self,
            _signature_request_id: &str,
        ) -> Result<Option<SignatureArtifactDownload>, String> {
            Ok(Some(self.audit.clone()))
        }
    }

    /// `(media_id, object key, filename)` as the object-first DAO seam received it.
    type ArtifactCall = Option<(String, String, String)>;

    #[derive(Default)]
    struct RepoState {
        object_calls: Vec<(ArtifactCall, ArtifactCall)>,
        legacy_calls: usize,
    }

    struct FakeRepo {
        replay: bool,
        state: Arc<Mutex<RepoState>>,
    }

    impl FakeRepo {
        fn new(replay: bool) -> Self {
            Self {
                replay,
                state: Arc::new(Mutex::new(RepoState::default())),
            }
        }

        fn handle(&self) -> Arc<Mutex<RepoState>> {
            self.state.clone()
        }

        fn command(&self) -> SignatureCommandResult {
            SignatureCommandResult {
                command_id: "signature.reconcile:evt-1".into(),
                outcome: SignatureCommandOutcome::Success,
                aggregate_id: Some("req-1".into()),
                message: None,
                replayed: self.replay,
                value: None,
            }
        }
    }

    #[async_trait]
    impl SignatureRepository for FakeRepo {
        async fn get(&self, _id: &str) -> DbResult<Option<SignatureRequest>> {
            unimplemented!("not exercised by reconcile")
        }

        async fn active_for_document(
            &self,
            _transaction_document_id: &str,
        ) -> DbResult<Option<SignatureRequest>> {
            unimplemented!("not exercised by reconcile")
        }

        async fn list_by_document(
            &self,
            _transaction_document_id: &str,
        ) -> DbResult<Vec<SignatureRequest>> {
            unimplemented!("not exercised by reconcile")
        }

        async fn send(
            &self,
            _request: &SendSignatureRequest,
            _actor_app_user_id: Option<&str>,
        ) -> DbResult<SignatureCommandResult> {
            unimplemented!("not exercised by reconcile")
        }

        async fn apply_status(
            &self,
            _request: &ApplySignatureStatusRequest,
            _actor_app_user_id: Option<&str>,
        ) -> DbResult<SignatureCommandResult> {
            unimplemented!("not exercised by reconcile")
        }

        async fn cancel(
            &self,
            _command_id: &str,
            _signature_request_id: &str,
            _actor_app_user_id: Option<&str>,
        ) -> DbResult<SignatureCommandResult> {
            unimplemented!("not exercised by reconcile")
        }

        async fn decline(
            &self,
            _command_id: &str,
            _signature_request_id: &str,
            _actor_app_user_id: Option<&str>,
        ) -> DbResult<SignatureCommandResult> {
            unimplemented!("not exercised by reconcile")
        }

        async fn reconciliation_needs_artifact(
            &self,
            _event_id: &str,
            _signature_request_id: &str,
        ) -> DbResult<bool> {
            Ok(true)
        }

        async fn reconcile_completed(
            &self,
            _event_id: &str,
            _signature_request_id: &str,
            _signed_artifact: Option<&SignatureArtifactDownload>,
            _audit_artifact: Option<&SignatureArtifactDownload>,
            _actor_app_user_id: Option<&str>,
        ) -> DbResult<SignatureCommandResult> {
            self.state.lock().unwrap().legacy_calls += 1;
            Ok(self.command())
        }

        async fn reconcile_completed_with_objects(
            &self,
            _event_id: &str,
            _signature_request_id: &str,
            signed_artifact: Option<(&str, &str, &str, &MediaObjectMetadata)>,
            audit_artifact: Option<(&str, &str, &str, &MediaObjectMetadata)>,
            _actor_app_user_id: Option<&str>,
        ) -> DbResult<SignatureCommandResult> {
            let describe = |artifact: (&str, &str, &str, &MediaObjectMetadata)| {
                (
                    artifact.0.to_owned(),
                    artifact.3.key.clone(),
                    artifact.1.to_owned(),
                )
            };
            self.state
                .lock()
                .unwrap()
                .object_calls
                .push((signed_artifact.map(describe), audit_artifact.map(describe)));
            Ok(self.command())
        }
    }

    fn provider() -> FakeProvider {
        FakeProvider {
            signed: artifact("signed.pdf", b"signed-bytes"),
            audit: artifact("audit.pdf", b"audit-bytes"),
        }
    }

    #[tokio::test]
    async fn artifacts_are_uploaded_before_the_dao_and_keyed_by_their_media_id() {
        let store = Arc::new(FakeVault::default());
        let repo = FakeRepo::new(false);
        let state = repo.handle();
        let service = SignatureService::new(repo, Arc::new(provider()), infrastructure())
            .with_object_store(store.clone());

        let command = service
            .reconcile_completed("evt-1", "req-1", &context())
            .await
            .expect("reconcile succeeds");

        assert_eq!(command.outcome, SignatureCommandOutcome::Success);
        assert!(!command.replayed);
        let state = state.lock().unwrap();
        assert_eq!(
            state.legacy_calls, 0,
            "the byte path is not used with a store"
        );
        assert_eq!(state.object_calls.len(), 1);
        let (signed, audit) = &state.object_calls[0];
        let (signed_id, signed_key, signed_name) = signed.as_ref().expect("signed artifact");
        assert_eq!(signed_name, "signed.pdf");
        assert_eq!(signed_key, &format!("documents/{signed_id}/original.pdf"));
        let (audit_id, audit_key, audit_name) = audit.as_ref().expect("audit artifact");
        assert_eq!(audit_name, "audit.pdf");
        assert_eq!(audit_key, &format!("documents/{audit_id}/original.pdf"));
        assert_eq!(store.puts().len(), 2);
        assert!(
            store.deletes().is_empty(),
            "a fresh success keeps its objects"
        );
    }

    #[tokio::test]
    async fn a_replayed_reconcile_discards_the_uploaded_objects() {
        let store = Arc::new(FakeVault::default());
        let repo = FakeRepo::new(true);
        let service = SignatureService::new(repo, Arc::new(provider()), infrastructure())
            .with_object_store(store.clone());

        let command = service
            .reconcile_completed("evt-1", "req-1", &context())
            .await
            .expect("reconcile returns the replay");

        assert!(command.replayed);
        assert_eq!(store.puts().len(), 2);
        assert_eq!(
            store.deletes(),
            store.puts(),
            "a replay wrote no rows, so every upload is removed"
        );
    }

    #[tokio::test]
    async fn without_an_object_store_the_legacy_byte_path_is_used() {
        let repo = FakeRepo::new(false);
        let state = repo.handle();
        let service = SignatureService::new(repo, Arc::new(provider()), infrastructure());

        let command = service
            .reconcile_completed("evt-1", "req-1", &context())
            .await
            .expect("legacy reconcile succeeds");

        assert_eq!(command.outcome, SignatureCommandOutcome::Success);
        let state = state.lock().unwrap();
        assert_eq!(state.legacy_calls, 1);
        assert!(state.object_calls.is_empty());
    }
}

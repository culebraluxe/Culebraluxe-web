use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, SignatureDao};
use domain::{
    validate_signature_recipients, ApplySignatureStatusRequest, SendSignatureRequest,
    SignatureArtifactDownload, SignatureCommandOutcome, SignatureCommandResult,
    SignatureProviderActionResult, SignatureProviderSendRequest, SignatureProviderSendResult,
    SignatureProviderStatusResult, SignatureRequest, SignatureRequestResult,
    SignatureRequestStatus, SignatureStatusResult, SignatureWebhookVerification,
};
use serde_json::json;
use service::{
    OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime, SignatureProvider,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait SignatureRepository: Send {
    async fn get(&mut self, id: &str) -> DbResult<Option<SignatureRequest>>;
    async fn active_for_document(
        &mut self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>>;
    async fn list_by_document(
        &mut self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>>;
    async fn send(
        &mut self,
        request: &SendSignatureRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn apply_status(
        &mut self,
        request: &ApplySignatureStatusRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn cancel(
        &mut self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn decline(
        &mut self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
    async fn reconciliation_needs_artifact(
        &mut self,
        event_id: &str,
        signature_request_id: &str,
    ) -> DbResult<bool>;
    async fn reconcile_completed(
        &mut self,
        event_id: &str,
        signature_request_id: &str,
        signed_artifact: Option<&SignatureArtifactDownload>,
        audit_artifact: Option<&SignatureArtifactDownload>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult>;
}

#[async_trait]
impl SignatureRepository for SignatureDao {
    async fn get(&mut self, id: &str) -> DbResult<Option<SignatureRequest>> {
        SignatureDao::get(self, id).await
    }

    async fn active_for_document(
        &mut self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        SignatureDao::active_for_document(self, transaction_document_id).await
    }

    async fn list_by_document(
        &mut self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>> {
        SignatureDao::list_by_document(self, transaction_document_id).await
    }

    async fn send(
        &mut self,
        request: &SendSignatureRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::send(self, request, actor_app_user_id).await
    }

    async fn apply_status(
        &mut self,
        request: &ApplySignatureStatusRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::apply_status(self, request, actor_app_user_id).await
    }

    async fn cancel(
        &mut self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::cancel(self, command_id, signature_request_id, actor_app_user_id).await
    }

    async fn decline(
        &mut self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        SignatureDao::decline(self, command_id, signature_request_id, actor_app_user_id).await
    }

    async fn reconciliation_needs_artifact(
        &mut self,
        event_id: &str,
        signature_request_id: &str,
    ) -> DbResult<bool> {
        SignatureDao::reconciliation_needs_artifact(self, event_id, signature_request_id).await
    }

    async fn reconcile_completed(
        &mut self,
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
}

pub struct SignatureService<R> {
    repository: R,
    provider: Arc<dyn SignatureProvider>,
    runtime: ServiceRuntime,
}

impl<R: SignatureRepository> SignatureService<R> {
    pub fn new(
        repository: R,
        provider: Arc<dyn SignatureProvider>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            provider,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &mut self,
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
        &mut self,
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
        &mut self,
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

    pub async fn send(
        &mut self,
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

        let result = async {
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

            let delivery = self
                .provider
                .send(SignatureProviderSendRequest {
                    signature_request_id: recorded.signature_request.id.clone(),
                    transaction_document_id: recorded
                        .signature_request
                        .transaction_document_id
                        .clone(),
                    recipients: request.recipients.clone(),
                    message: recorded.signature_request.message.clone(),
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

            let target = self.provider.map_status(&delivery.provider_status);
            let status = self
                .repository
                .apply_status(
                    &ApplySignatureStatusRequest {
                        command_id: Uuid::new_v4().to_string(),
                        signature_request_id: recorded.signature_request.id,
                        target_status: Some(target),
                    },
                    actor,
                )
                .await?;

            emit_status_event(&self.runtime, &status, target, context).await?;
            Ok(status)
        }
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn refresh_status(
        &mut self,
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

        let result = async {
            let observed = self
                .provider
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
        }
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn cancel(
        &mut self,
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

        let result = async {
            let provider = self
                .provider
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
        }
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    pub async fn decline(
        &mut self,
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

        let result = async {
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
        }
        .await;

        audit_result(&self.runtime, "signature", OP, context, decision, &result).await?;
        result
    }

    async fn reconcile_completed_internal(
        &mut self,
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
                self.provider
                    .download_signed_artifact(signature_request_id)
                    .await
                    .map_err(|message| {
                        CoreServiceError::business(
                            "SIGNATURE_ARTIFACT_DOWNLOAD_FAILED",
                            message,
                        )
                    })?,
            )
        } else {
            None
        };
        let audit_artifact = if needs_artifact {
            self.provider
                .download_audit_trail(signature_request_id)
                .await
                .map_err(|message| {
                    CoreServiceError::business("SIGNATURE_AUDIT_DOWNLOAD_FAILED", message)
                })?
        } else {
            None
        };

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
        &mut self,
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
        &mut self,
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

        let result = async {
            let verified: SignatureWebhookVerification = self
                .provider
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
                        reconciliation.message.unwrap_or_else(|| {
                            "Signed-artifact reconciliation failed.".into()
                        }),
                    ));
                }
            }
            Ok(command)
        }
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

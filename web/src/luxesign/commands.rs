//! Luxesign/signer durable command handlers.
//!
//! Extracted from the app-wide command dispatcher: this is the Luxesign domain's command surface
//! (envelope prepare/send/issue/void/finalize/sweep, fields, recipients; signer open/consent/
//! complete/decline). The dispatcher only registers them.

use crate::command_runtime::{
    command_event, core_command_error, decode_command_input, serialize_value, signer_result,
    validate_command_target, CommandDispatchError, CommandRegistry, DurableCommandHandler,
};
use crate::luxesign::ProductionLuxesignService;
use crate::signer::SignerService;
use async_trait::async_trait;
use db::{DbTransaction, SignerDao};
use model::{
    AcceptSignerConsentRequest, CompleteSignatureFieldRequest, CompleteSignerRequest,
    DeclineSignerRequest, ImportAnchorFieldsRequest, IssueLuxesignRequest, OpenSignerRequest,
    PrepareLuxesignRequest, PutSignatureFieldRequest, RemoveSignatureFieldRequest,
    SendLuxesignRequest, SetLuxesignRecipientsRequest,
};
use serde_json::{json, Value};
use services::{
    CommandDomainEvent, CommandEnvelope, CommandOutcome, CommandRequest, CommandResult,
    ServiceContext,
};
use std::sync::Arc;

/// Register every Luxesign/signer command handler in the dispatcher registry.
pub(crate) fn register(
    registry: &mut CommandRegistry,
    luxesign: Arc<ProductionLuxesignService>,
    signer: Arc<SignerService<SignerDao>>,
) -> Result<(), CommandDispatchError> {
    for kind in [
        LuxesignCommandKind::Prepare,
        LuxesignCommandKind::Send,
        LuxesignCommandKind::SetRecipients,
        LuxesignCommandKind::PutField,
        LuxesignCommandKind::RemoveField,
        LuxesignCommandKind::Issue,
        LuxesignCommandKind::Void,
        LuxesignCommandKind::Resend,
        LuxesignCommandKind::SweepDue,
        LuxesignCommandKind::ImportFields,
        LuxesignCommandKind::Finalize,
    ] {
        registry.register(Arc::new(LuxesignCommand {
            service: luxesign.clone(),
            kind,
        }))?;
    }
    for kind in [
        SignerCommandKind::Open,
        SignerCommandKind::AcceptConsent,
        SignerCommandKind::CompleteField,
        SignerCommandKind::Complete,
        SignerCommandKind::Decline,
    ] {
        registry.register(Arc::new(SignerCommand {
            signer: signer.clone(),
            luxesign: luxesign.clone(),
            kind,
        }))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum LuxesignCommandKind {
    Prepare,
    Send,
    SetRecipients,
    PutField,
    RemoveField,
    Issue,
    Void,
    Resend,
    Finalize,
    SweepDue,
    ImportFields,
}

impl LuxesignCommandKind {
    const fn command_type(self) -> &'static str {
        match self {
            Self::Prepare => "luxesign.prepare",
            Self::Send => "luxesign.send",
            Self::SetRecipients => "luxesign.setRecipients",
            Self::PutField => "luxesign.putField",
            Self::RemoveField => "luxesign.removeField",
            Self::Issue => "luxesign.issue",
            Self::Void => "luxesign.void",
            Self::Resend => "luxesign.resend",
            Self::Finalize => "luxesign.finalize",
            Self::SweepDue => "luxesign.sweepDue",
            Self::ImportFields => "luxesign.importFields",
        }
    }
}

struct LuxesignCommand {
    service: Arc<ProductionLuxesignService>,
    kind: LuxesignCommandKind,
}

#[async_trait]
impl DurableCommandHandler for LuxesignCommand {
    fn command_type(&self) -> &'static str {
        self.kind.command_type()
    }

    fn service_domain(&self) -> &'static str {
        "luxesign"
    }

    fn scheduling_payload(&self, request: &CommandRequest) -> Option<Value> {
        match self.kind {
            LuxesignCommandKind::Prepare | LuxesignCommandKind::Send => request
                .input
                .get("transactionDocumentId")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(|value| json!({ "transactionDocumentId": value })),
            _ => request
                .input
                .get("signatureRequestId")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .or(request.aggregate_id.as_deref())
                .map(|value| json!({ "signatureRequestId": value })),
        }
    }

    async fn handle(
        &self,
        tx: &mut DbTransaction,
        envelope: &CommandEnvelope,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError> {
        match self.kind {
            LuxesignCommandKind::Prepare => {
                let request: PrepareLuxesignRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "transaction_document",
                    &request.transaction_document_id,
                    "LUXESIGN_DOCUMENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let snapshot = match self
                    .service
                    .prepare_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => return core_command_error(envelope, None, error),
                };
                let signature_request_id = snapshot.luxesign_request.id.clone();
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(signature_request_id.clone()),
                    Some(serialize_value(&snapshot)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_PREPARED",
                    "luxesign_request",
                    &signature_request_id,
                    json!({
                        "signatureRequestId": signature_request_id,
                        "transactionDocumentId": snapshot.luxesign_request.transaction_document_id,
                    }),
                ));
                Ok(result)
            }
            LuxesignCommandKind::Send => {
                // The aggregate IS the document: a caller that names it once (as the aggregate) need not name it
                // again in the input. `validate_command_target` below still refuses the two disagreeing.
                let mut envelope = envelope.clone();
                if let Some(document) = envelope.aggregate_id.clone() {
                    envelope
                        .input
                        .entry("transactionDocumentId")
                        .or_insert(Value::String(document));
                }
                let envelope = &envelope;
                let request: SendLuxesignRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "transaction_document",
                    &request.transaction_document_id,
                    "LUXESIGN_DOCUMENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let sent = match self.service.send_transactional(tx, &request, context).await {
                    Ok(value) => value,
                    Err(error) => return core_command_error(envelope, None, error),
                };
                let signature_request_id = sent.snapshot.luxesign_request.id.clone();
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(signature_request_id.clone()),
                    Some(serialize_value(&sent)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_ISSUED",
                    "luxesign_request",
                    &signature_request_id,
                    json!({
                        "signatureRequestId": signature_request_id,
                        "transactionDocumentId": request.transaction_document_id,
                        "expiresAt": sent.issued.expires_at,
                        "invitationCount": sent.issued.invitation_message_ids.len(),
                    }),
                ));
                for message_id in &sent.issued.invitation_message_ids {
                    result.emitted_events.push(command_event(
                        envelope,
                        crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                        "email_message",
                        message_id,
                        json!({
                            "messageId": message_id,
                            "signatureRequestId": signature_request_id,
                        }),
                    ));
                }
                Ok(result)
            }
            LuxesignCommandKind::SetRecipients => {
                let request: SetLuxesignRecipientsRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &request.signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                let recipients = match self
                    .service
                    .set_recipients_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.signature_request_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(request.signature_request_id.clone()),
                    Some(serialize_value(&recipients)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_RECIPIENTS_SET",
                    "luxesign_request",
                    &request.signature_request_id,
                    json!({
                        "signatureRequestId": request.signature_request_id,
                        "recipientCount": recipients.len(),
                    }),
                ));
                Ok(result)
            }
            LuxesignCommandKind::PutField => {
                let request: PutSignatureFieldRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &request.signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                let field = match self
                    .service
                    .put_field_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.signature_request_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(request.signature_request_id.clone()),
                    Some(serialize_value(&field)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_FIELD_PUT",
                    "luxesign_request",
                    &request.signature_request_id,
                    json!({
                        "signatureRequestId": request.signature_request_id,
                        "fieldId": field.id,
                        "recipientId": field.recipient_id,
                    }),
                ));
                Ok(result)
            }
            LuxesignCommandKind::RemoveField => {
                let request: RemoveSignatureFieldRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &request.signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                if let Err(error) = self
                    .service
                    .remove_field_transactional(tx, &request, context)
                    .await
                {
                    return core_command_error(
                        envelope,
                        Some(request.signature_request_id.clone()),
                        error,
                    );
                }
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(request.signature_request_id.clone()),
                    None,
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_FIELD_REMOVED",
                    "luxesign_request",
                    &request.signature_request_id,
                    json!({
                        "signatureRequestId": request.signature_request_id,
                        "fieldId": request.field_id,
                    }),
                ));
                Ok(result)
            }
            LuxesignCommandKind::Issue => {
                let request: IssueLuxesignRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &request.signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                let issued = match self
                    .service
                    .issue_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.signature_request_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(request.signature_request_id.clone()),
                    Some(serialize_value(&issued)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_ISSUED",
                    "luxesign_request",
                    &request.signature_request_id,
                    json!({
                        "signatureRequestId": request.signature_request_id,
                        "expiresAt": issued.expires_at,
                        "invitationCount": issued.invitation_message_ids.len(),
                    }),
                ));
                for message_id in &issued.invitation_message_ids {
                    result.emitted_events.push(command_event(
                        envelope,
                        crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                        "email_message",
                        message_id,
                        json!({
                            "messageId": message_id,
                            "signatureRequestId": request.signature_request_id,
                        }),
                    ));
                }
                Ok(result)
            }
            LuxesignCommandKind::Void => {
                let signature_request_id = envelope
                    .input
                    .get("signatureRequestId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .or_else(|| envelope.aggregate_id.clone());
                let Some(signature_request_id) = signature_request_id else {
                    return Ok(CommandResult::failure(
                        envelope.command_id.clone(),
                        CommandOutcome::ValidationFailure,
                        envelope.aggregate_id.clone(),
                        "LUXESIGN_REQUEST_REQUIRED",
                        "luxesign.void requires signatureRequestId.",
                    ));
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                if let Err(error) = self
                    .service
                    .void_transactional(tx, &signature_request_id, context)
                    .await
                {
                    return core_command_error(envelope, Some(signature_request_id.clone()), error);
                }
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(signature_request_id.clone()),
                    None,
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_VOIDED",
                    "luxesign_request",
                    &signature_request_id,
                    json!({ "signatureRequestId": signature_request_id }),
                ));
                Ok(result)
            }
            LuxesignCommandKind::Resend => {
                let signature_request_id = envelope
                    .input
                    .get("signatureRequestId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .or_else(|| envelope.aggregate_id.clone());
                let recipient_id = envelope
                    .input
                    .get("recipientId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let (Some(signature_request_id), Some(recipient_id)) =
                    (signature_request_id, recipient_id)
                else {
                    return Ok(CommandResult::failure(
                        envelope.command_id.clone(),
                        CommandOutcome::ValidationFailure,
                        envelope.aggregate_id.clone(),
                        "LUXESIGN_RESEND_REQUIRED",
                        "luxesign.resend requires signatureRequestId and recipientId.",
                    ));
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                let as_reminder = envelope
                    .input
                    .get("reminder")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let message_id = match self
                    .service
                    .resend_invitation_transactional(
                        tx,
                        &signature_request_id,
                        &recipient_id,
                        as_reminder,
                        context,
                    )
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(signature_request_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(signature_request_id.clone()),
                    None,
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_INVITATION_RESENT",
                    "email_message",
                    &message_id,
                    json!({
                        "messageId": message_id,
                        "signatureRequestId": signature_request_id,
                        "recipientId": recipient_id,
                    }),
                ));
                // The message is only QUEUED by the service; this event is what hands it to the delivery worker.
                // Without it a resend (and a reminder) sat in the queue forever.
                result.emitted_events.push(command_event(
                    envelope,
                    crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                    "email_message",
                    &message_id,
                    json!({
                        "messageId": message_id,
                        "signatureRequestId": signature_request_id,
                    }),
                ));
                Ok(result)
            }
            LuxesignCommandKind::Finalize => {
                let signature_request_id = envelope
                    .input
                    .get("signatureRequestId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .or_else(|| envelope.aggregate_id.clone());
                let Some(signature_request_id) = signature_request_id else {
                    return Ok(CommandResult::failure(
                        envelope.command_id.clone(),
                        CommandOutcome::ValidationFailure,
                        envelope.aggregate_id.clone(),
                        "LUXESIGN_REQUEST_REQUIRED",
                        "luxesign.finalize requires signatureRequestId.",
                    ));
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                let finalized = match self
                    .service
                    .finalize_transactional(tx, &signature_request_id, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(signature_request_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(signature_request_id.clone()),
                    Some(serialize_value(&finalized)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_FINALIZED",
                    "luxesign_request",
                    &signature_request_id,
                    json!({
                        "signatureRequestId": signature_request_id,
                        "auditMediaId": finalized.audit_media_id,
                        "signedMediaId": finalized.signed_media_id,
                        "alreadyCompleted": finalized.already_completed,
                    }),
                ));
                for message_id in &finalized.notification_message_ids {
                    result.emitted_events.push(command_event(
                        envelope,
                        crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                        "email_message",
                        message_id,
                        json!({
                            "messageId": message_id,
                            "signatureRequestId": signature_request_id,
                        }),
                    ));
                }
                Ok(result)
            }
            LuxesignCommandKind::SweepDue => {
                let swept = match self.service.sweep_due_transactional(tx, context).await {
                    Ok(value) => value,
                    Err(error) => return core_command_error(envelope, None, error),
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    None,
                    Some(serialize_value(&swept)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_SWEPT",
                    "luxesign_request",
                    "",
                    json!({
                        "expiredRecipients": swept.expired_recipients,
                        "expiredEnvelopes": swept.expired_envelopes,
                        "reminders": swept.reminder_message_ids.len(),
                    }),
                ));
                for message_id in &swept.reminder_message_ids {
                    result.emitted_events.push(command_event(
                        envelope,
                        crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                        "email_message",
                        message_id,
                        json!({ "messageId": message_id }),
                    ));
                }
                Ok(result)
            }
            LuxesignCommandKind::ImportFields => {
                let request: ImportAnchorFieldsRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_request",
                    &request.signature_request_id,
                    "LUXESIGN_REQUEST_MISMATCH",
                ) {
                    return Ok(result);
                }
                let imported = match self
                    .service
                    .import_fields_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.signature_request_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = CommandResult::success(
                    envelope.command_id.clone(),
                    Some(request.signature_request_id.clone()),
                    Some(serialize_value(&imported)?),
                );
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_FIELDS_IMPORTED",
                    "luxesign_request",
                    &request.signature_request_id,
                    json!({
                        "signatureRequestId": request.signature_request_id,
                        "fieldCount": imported.created_field_ids.len(),
                    }),
                ));
                Ok(result)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SignerCommandKind {
    Open,
    AcceptConsent,
    CompleteField,
    Complete,
    Decline,
}

impl SignerCommandKind {
    const fn command_type(self) -> &'static str {
        match self {
            Self::Open => "signer.open",
            Self::AcceptConsent => "signer.acceptConsent",
            Self::CompleteField => "signer.completeField",
            Self::Complete => "signer.complete",
            Self::Decline => "signer.decline",
        }
    }
}

struct SignerCommand {
    signer: Arc<SignerService<SignerDao>>,
    luxesign: Arc<ProductionLuxesignService>,
    kind: SignerCommandKind,
}

#[async_trait]
impl DurableCommandHandler for SignerCommand {
    fn command_type(&self) -> &'static str {
        self.kind.command_type()
    }

    fn service_domain(&self) -> &'static str {
        "signer"
    }

    fn scheduling_payload(&self, request: &CommandRequest) -> Option<Value> {
        request
            .input
            .get("recipientId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .or(request.aggregate_id.as_deref())
            .map(|recipient_id| json!({ "recipientId": recipient_id }))
    }

    async fn handle(
        &self,
        tx: &mut DbTransaction,
        envelope: &CommandEnvelope,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError> {
        match self.kind {
            SignerCommandKind::Open => {
                let request: OpenSignerRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_recipient",
                    &request.recipient_id,
                    "SIGNER_RECIPIENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let action = match self.signer.open_transactional(tx, &request, context).await {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.recipient_id.clone()),
                            error,
                        )
                    }
                };
                Ok(signer_result(envelope, &action, "SIGNER_OPENED", None)?)
            }
            SignerCommandKind::AcceptConsent => {
                let request: AcceptSignerConsentRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_recipient",
                    &request.recipient_id,
                    "SIGNER_RECIPIENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let action = match self
                    .signer
                    .accept_consent_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.recipient_id.clone()),
                            error,
                        )
                    }
                };
                Ok(signer_result(
                    envelope,
                    &action,
                    "SIGNER_CONSENT_ACCEPTED",
                    None,
                )?)
            }
            SignerCommandKind::CompleteField => {
                let request: CompleteSignatureFieldRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_recipient",
                    &request.recipient_id,
                    "SIGNER_RECIPIENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let action = match self
                    .signer
                    .complete_field_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.recipient_id.clone()),
                            error,
                        )
                    }
                };
                Ok(signer_result(
                    envelope,
                    &action,
                    "SIGNER_FIELD_COMPLETED",
                    Some(json!({ "fieldId": request.field_id })),
                )?)
            }
            SignerCommandKind::Complete => {
                let request: CompleteSignerRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_recipient",
                    &request.recipient_id,
                    "SIGNER_RECIPIENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let action = match self
                    .signer
                    .complete_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.recipient_id.clone()),
                            error,
                        )
                    }
                };
                if let Err(error) = self
                    .luxesign
                    .signer_completed_transactional(
                        tx,
                        &action.signature_request_id,
                        action.envelope_ready_to_finalize,
                        context,
                    )
                    .await
                {
                    return core_command_error(envelope, Some(request.recipient_id.clone()), error);
                }
                let mut result = signer_result(envelope, &action, "SIGNER_COMPLETED", None)?;
                if action.envelope_ready_to_finalize {
                    result.emitted_events.push(command_event(
                        envelope,
                        "LUXESIGN_READY_TO_FINALIZE",
                        "luxesign_request",
                        &action.signature_request_id,
                        json!({
                            "signatureRequestId": action.signature_request_id,
                        }),
                    ));
                }
                Ok(result)
            }
            SignerCommandKind::Decline => {
                let request: DeclineSignerRequest = match decode_command_input(envelope) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                if let Some(result) = validate_command_target(
                    envelope,
                    "luxesign_recipient",
                    &request.recipient_id,
                    "SIGNER_RECIPIENT_MISMATCH",
                ) {
                    return Ok(result);
                }
                let action = match self
                    .signer
                    .decline_transactional(tx, &request, context)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.recipient_id.clone()),
                            error,
                        )
                    }
                };
                let notice_message_ids = match self
                    .luxesign
                    .signer_declined_transactional(
                        tx,
                        &action.signature_request_id,
                        &action.recipient_id,
                        request.reason.as_deref(),
                        context,
                    )
                    .await
                {
                    Ok(ids) => ids,
                    Err(error) => {
                        return core_command_error(
                            envelope,
                            Some(request.recipient_id.clone()),
                            error,
                        )
                    }
                };
                let mut result = signer_result(envelope, &action, "SIGNER_DECLINED", None)?;
                for message_id in &notice_message_ids {
                    result.emitted_events.push(command_event(
                        envelope,
                        crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                        "email_message",
                        message_id,
                        json!({
                            "messageId": message_id,
                            "signatureRequestId": action.signature_request_id,
                        }),
                    ));
                }
                result.emitted_events.push(command_event(
                    envelope,
                    "LUXESIGN_DECLINED",
                    "luxesign_request",
                    &action.signature_request_id,
                    json!({
                        "signatureRequestId": action.signature_request_id,
                        "recipientId": action.recipient_id,
                    }),
                ));
                Ok(result)
            }
        }
    }
}

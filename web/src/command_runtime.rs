use crate::contracts::ContractService;
use crate::email::EmailService;
use crate::luxesign::ProductionLuxesignService;
use crate::service_support::CoreServiceError;
use crate::signer::SignerService;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use db::{
    CommandReceiptDao, CommandReceiptRow, ContractDao, Database, DbFailure, DbTransaction,
    DomainEventOutboxDao, EmailDao, OutboxEventInput, SecurityAuditDao, SignerDao,
};
use model::{
    AcceptSignerConsentRequest, CompleteSignatureFieldRequest, CompleteSignerRequest,
    DeclineSignerRequest, ExecuteContractRequest, ImportAnchorFieldsRequest, IssueLuxesignRequest,
    OpenSignerRequest, PrepareLuxesignRequest, PutSignatureFieldRequest, QueueEmailRequest,
    RemoveSignatureFieldRequest, SendLuxesignRequest, SetLuxesignRecipientsRequest,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use services::{
    CommandDomainEvent, CommandEnvelope, CommandOutcome, CommandReceipt, CommandReceiptStatus,
    CommandRequest, CommandResult, ServiceContext, ServiceRuntimeError,
};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CommandDispatchError {
    #[error(transparent)]
    Database(#[from] DbFailure),
    #[error(transparent)]
    Service(#[from] CoreServiceError),
    #[error("command serialization failed: {0}")]
    Serialization(String),
    #[error("command registry invariant failed: {0}")]
    Registry(String),
    #[error(transparent)]
    Scheduler(#[from] services::ServiceDispatchError),
}

#[async_trait]
trait DurableCommandHandler: Send + Sync {
    fn command_type(&self) -> &'static str;
    fn service_domain(&self) -> &'static str;
    fn scheduling_payload(&self, request: &CommandRequest) -> Option<Value>;

    async fn handle(
        &self,
        tx: &mut DbTransaction,
        envelope: &CommandEnvelope,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError>;
}

#[derive(Default)]
struct CommandRegistry {
    handlers: HashMap<&'static str, Arc<dyn DurableCommandHandler>>,
}

impl CommandRegistry {
    fn register(
        &mut self,
        handler: Arc<dyn DurableCommandHandler>,
    ) -> Result<(), CommandDispatchError> {
        let command_type = handler.command_type();
        if self.handlers.insert(command_type, handler).is_some() {
            return Err(CommandDispatchError::Registry(format!(
                "duplicate command handler: {command_type}"
            )));
        }
        Ok(())
    }

    fn get(&self, command_type: &str) -> Option<Arc<dyn DurableCommandHandler>> {
        self.handlers.get(command_type).cloned()
    }
}

#[derive(Clone)]
pub struct CommandDispatcher {
    db: Database,
    receipts: CommandReceiptDao,
    outbox: DomainEventOutboxDao,
    audit: SecurityAuditDao,
    registry: Arc<CommandRegistry>,
}

impl CommandDispatcher {
    pub fn for_kernel(
        db: Database,
        contract: Arc<ContractService<ContractDao>>,
        luxesign: Arc<ProductionLuxesignService>,
        signer: Arc<SignerService<SignerDao>>,
        email: Arc<EmailService<EmailDao>>,
    ) -> Result<Self, CommandDispatchError> {
        let mut registry = CommandRegistry::default();
        registry.register(Arc::new(ContractExecuteCommand { service: contract }))?;
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
        registry.register(Arc::new(EmailQueueCommand { service: email }))?;

        Ok(Self {
            receipts: CommandReceiptDao::new(db.clone()),
            outbox: DomainEventOutboxDao::new(db.clone()),
            audit: SecurityAuditDao::new(db.clone()),
            db,
            registry: Arc::new(registry),
        })
    }

    pub fn scheduling_route(
        &self,
        request: &CommandRequest,
    ) -> Option<(&'static str, &'static str, Value)> {
        let handler = self.registry.get(&request.command_type)?;
        Some((
            handler.service_domain(),
            handler.command_type(),
            handler.scheduling_payload(request)?,
        ))
    }

    pub async fn execute(
        &self,
        request: &CommandRequest,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError> {
        if request.command_id.trim().is_empty() {
            return Ok(CommandResult::failure(
                "",
                CommandOutcome::ValidationFailure,
                request.aggregate_id.clone(),
                "COMMAND_ID_REQUIRED",
                "commandId is required.",
            ));
        }
        if request.command_type.trim().is_empty() {
            return Ok(CommandResult::failure(
                request.command_id.clone(),
                CommandOutcome::ValidationFailure,
                request.aggregate_id.clone(),
                "COMMAND_TYPE_REQUIRED",
                "commandType is required.",
            ));
        }
        if DateTime::parse_from_rfc3339(&request.requested_at).is_err() {
            return Ok(CommandResult::failure(
                request.command_id.clone(),
                CommandOutcome::ValidationFailure,
                request.aggregate_id.clone(),
                "COMMAND_REQUESTED_AT_INVALID",
                "requestedAt must be an RFC3339 timestamp.",
            ));
        }

        let Some(handler) = self.registry.get(&request.command_type) else {
            return Ok(CommandResult::failure(
                request.command_id.clone(),
                CommandOutcome::ValidationFailure,
                request.aggregate_id.clone(),
                "COMMAND_TYPE_UNKNOWN",
                format!(
                    "No command handler is registered for {}.",
                    request.command_type
                ),
            ));
        };

        let envelope = request.canonicalize(context);
        let fingerprint = intent_fingerprint(&envelope)?;

        if let Some(existing) = self.receipts.find(&envelope.command_id).await? {
            return replay_or_conflict(&existing, &envelope, &fingerprint);
        }

        let mut tx = self.db.begin("command.dispatch").await?;
        let claimed = self
            .receipts
            .claim_tx(
                &mut tx,
                &envelope.command_id,
                &envelope.command_type,
                &fingerprint,
                envelope.actor_app_user_id.as_deref(),
                &envelope.aggregate_type,
                envelope.aggregate_id.as_deref(),
                envelope.correlation_id.as_deref(),
                envelope.causation_id.as_deref(),
                &envelope.requested_at,
            )
            .await?;

        if !claimed {
            let existing = self
                .receipts
                .find_tx(&mut tx, &envelope.command_id)
                .await?
                .ok_or_else(|| {
                    CommandDispatchError::Registry(format!(
                        "receipt claim lost without a visible row: {}",
                        envelope.command_id
                    ))
                })?;
            let result = replay_or_conflict(&existing, &envelope, &fingerprint)?;
            tx.rollback().await?;
            return Ok(result);
        }

        let mut execution_context = context.clone();
        execution_context.causation_id = Some(envelope.command_id.clone());

        let result = handler.handle(&mut tx, &envelope, &execution_context).await;

        let mut result = match result {
            Ok(result) => result,
            Err(error) => {
                let _ = tx.rollback().await;
                tracing::error!(
                    target: "culebraluxe::command",
                    command_id = %envelope.command_id,
                    command_type = %envelope.command_type,
                    correlation_id = ?envelope.correlation_id,
                    error = %error,
                    "command transaction rolled back"
                );
                return Err(error);
            }
        };

        let outbox_events = result
            .emitted_events
            .iter()
            .map(outbox_event)
            .collect::<Vec<_>>();
        if let Err(error) = self.outbox.append_tx(&mut tx, &outbox_events).await {
            let _ = tx.rollback().await;
            return Err(error.into());
        }

        let error_code = result.error.as_ref().map(|error| error.code.as_str());
        let error_message = result.error.as_ref().map(|error| error.message.as_str());
        if let Err(error) = self
            .receipts
            .finalize_tx(
                &mut tx,
                &envelope.command_id,
                result.outcome.as_str(),
                result.aggregate_id.as_deref(),
                result.message.as_deref(),
                result.value.as_ref(),
                error_code,
                error_message,
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(error.into());
        }

        if let Err(error) = self
            .audit
            .record_tx(
                &mut tx,
                envelope.actor_app_user_id.as_deref(),
                &envelope.command_type,
                "rust-command",
                &json!({
                    "commandId": envelope.command_id,
                    "commandType": envelope.command_type,
                    "correlationId": envelope.correlation_id,
                    "causationId": envelope.causation_id,
                    "aggregateType": envelope.aggregate_type,
                    "aggregateId": result.aggregate_id,
                    "outcome": result.outcome.as_str(),
                    "errorCode": error_code,
                }),
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(error.into());
        }

        tx.commit().await?;
        result.receipt_id = Some(envelope.command_id.clone());

        tracing::info!(
            target: "culebraluxe::command",
            command_id = %envelope.command_id,
            command_type = %envelope.command_type,
            outcome = %result.outcome.as_str(),
            correlation_id = ?envelope.correlation_id,
            event_count = result.emitted_events.len(),
            "command committed"
        );

        Ok(result)
    }
}

fn replay_or_conflict(
    row: &CommandReceiptRow,
    envelope: &CommandEnvelope,
    fingerprint: &str,
) -> Result<CommandResult, CommandDispatchError> {
    if row.request_fingerprint.as_deref() != Some(fingerprint) {
        return Ok(CommandResult::failure(
            envelope.command_id.clone(),
            CommandOutcome::Conflict,
            envelope.aggregate_id.clone(),
            "COMMAND_ID_INTENT_CONFLICT",
            "This commandId is already bound to different command intent.",
        ));
    }

    if row.command_type.as_deref() != Some(envelope.command_type.as_str())
        || row.aggregate_type.as_deref() != Some(envelope.aggregate_type.as_str())
    {
        return Ok(CommandResult::failure(
            envelope.command_id.clone(),
            CommandOutcome::Conflict,
            envelope.aggregate_id.clone(),
            "COMMAND_ID_INTENT_CONFLICT",
            "This commandId is already bound to a different command target.",
        ));
    }

    let outcome = if row.outcome == "pending" {
        None
    } else {
        Some(CommandOutcome::parse(&row.outcome).ok_or_else(|| {
            CommandDispatchError::Registry(format!(
                "unknown stored command outcome '{}' for {}",
                row.outcome, row.command_id
            ))
        })?)
    };

    let receipt = CommandReceipt {
        command_id: row.command_id.clone(),
        outcome,
        status: match outcome {
            None => CommandReceiptStatus::Pending,
            Some(CommandOutcome::Success) => CommandReceiptStatus::Succeeded,
            Some(_) => CommandReceiptStatus::Failed,
        },
        aggregate_id: row.aggregate_id.clone(),
        message: row.message.clone(),
        created_at: Some(row.created_at.to_rfc3339()),
        actor_app_user_id: row.actor_app_user_id.clone(),
        command_type: row.command_type.clone(),
        correlation_id: row.correlation_id.clone(),
        causation_id: row.causation_id.clone(),
        aggregate_type: row.aggregate_type.clone(),
        result_payload: row.result_payload.clone(),
        error_code: row.error_code.clone(),
        error_message: row.error_message.clone(),
    };
    Ok(receipt.replay_result())
}

fn intent_fingerprint(envelope: &CommandEnvelope) -> Result<String, CommandDispatchError> {
    let intent = json!({
        "commandType": envelope.command_type,
        "actorAppUserId": envelope.actor_app_user_id,
        "aggregateType": envelope.aggregate_type,
        "aggregateId": envelope.aggregate_id,
        "input": envelope.input,
    });
    let canonical = canonical_json(intent);
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| CommandDispatchError::Serialization(error.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn canonical_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let mut result = Map::new();
            for key in keys {
                if let Some(value) = object.get(&key) {
                    result.insert(key, canonical_json(value.clone()));
                }
            }
            Value::Object(result)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_json).collect()),
        other => other,
    }
}

fn outbox_event(event: &CommandDomainEvent) -> OutboxEventInput {
    OutboxEventInput {
        event_id: event.event_id.clone(),
        event_type: event.event_type.clone(),
        actor_app_user_id: event.actor_app_user_id.clone(),
        aggregate_type: event.aggregate_type.clone(),
        aggregate_id: event.aggregate_id.clone(),
        correlation_id: event.correlation_id.clone(),
        causation_id: event.causation_id.clone(),
        occurred_at: event.occurred_at.clone(),
        payload: Value::Object(event.payload.clone()),
    }
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

struct EmailQueueCommand {
    service: Arc<EmailService<EmailDao>>,
}

#[async_trait]
impl DurableCommandHandler for EmailQueueCommand {
    fn command_type(&self) -> &'static str {
        "email.queue"
    }

    fn service_domain(&self) -> &'static str {
        "email"
    }

    fn scheduling_payload(&self, request: &CommandRequest) -> Option<Value> {
        request
            .input
            .get("dedupeKey")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|dedupe_key| json!({ "dedupeKey": dedupe_key }))
    }

    async fn handle(
        &self,
        tx: &mut DbTransaction,
        envelope: &CommandEnvelope,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError> {
        let request: QueueEmailRequest = match decode_command_input(envelope) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let queued = match self
            .service
            .queue_transactional(tx, &request, context)
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return core_command_error(envelope, envelope.aggregate_id.clone(), error)
            }
        };
        let mut result = CommandResult::success(
            envelope.command_id.clone(),
            Some(queued.message_id.clone()),
            Some(serialize_value(&queued)?),
        );
        if !queued.existing {
            result.emitted_events.push(command_event(
                envelope,
                crate::email::EMAIL_DELIVERY_ROUTING_KEY,
                "email_message",
                &queued.message_id,
                json!({ "messageId": queued.message_id }),
            ));
        }
        Ok(result)
    }
}

fn decode_command_input<T: DeserializeOwned>(
    envelope: &CommandEnvelope,
) -> Result<T, CommandResult> {
    serde_json::from_value(Value::Object(envelope.input.clone())).map_err(|error| {
        CommandResult::failure(
            envelope.command_id.clone(),
            CommandOutcome::ValidationFailure,
            envelope.aggregate_id.clone(),
            "COMMAND_INPUT_INVALID",
            format!("{} input is invalid: {error}", envelope.command_type),
        )
    })
}

fn serialize_value<T: serde::Serialize>(value: &T) -> Result<Value, CommandDispatchError> {
    serde_json::to_value(value)
        .map_err(|error| CommandDispatchError::Serialization(error.to_string()))
}

fn validate_command_target(
    envelope: &CommandEnvelope,
    aggregate_type: &str,
    target_id: &str,
    mismatch_code: &'static str,
) -> Option<CommandResult> {
    if envelope.aggregate_type != aggregate_type {
        return Some(CommandResult::failure(
            envelope.command_id.clone(),
            CommandOutcome::ValidationFailure,
            envelope.aggregate_id.clone(),
            mismatch_code,
            format!(
                "{} requires aggregateType='{}'.",
                envelope.command_type, aggregate_type
            ),
        ));
    }
    match envelope.aggregate_id.as_deref() {
        Some(aggregate_id) if aggregate_id == target_id => None,
        _ => Some(CommandResult::failure(
            envelope.command_id.clone(),
            CommandOutcome::ValidationFailure,
            envelope.aggregate_id.clone(),
            mismatch_code,
            "aggregateId must match the command input target id.",
        )),
    }
}

fn core_command_error(
    envelope: &CommandEnvelope,
    aggregate_id: Option<String>,
    error: CoreServiceError,
) -> Result<CommandResult, CommandDispatchError> {
    match error {
        CoreServiceError::Business { code, message } => Ok(CommandResult::failure(
            envelope.command_id.clone(),
            business_outcome(code),
            aggregate_id,
            code,
            message,
        )),
        CoreServiceError::Runtime(ServiceRuntimeError::Forbidden { reason, .. }) => {
            Ok(CommandResult::failure(
                envelope.command_id.clone(),
                CommandOutcome::Unauthorized,
                aggregate_id,
                "FORBIDDEN",
                reason,
            ))
        }
        other => Err(other.into()),
    }
}

fn business_outcome(code: &str) -> CommandOutcome {
    if code.ends_with("_NOT_FOUND") {
        CommandOutcome::NotFound
    } else if code.contains("ALREADY")
        || code.contains("CONFLICT")
        || code.ends_with("_NOT_MUTABLE")
    {
        CommandOutcome::Conflict
    } else if code.contains("REQUIRED")
        || code.contains("NOT_YOUR_TURN")
        || code.contains("INCOMPLETE")
        || code.contains("EXPIRED")
        || code.contains("REVOKED")
    {
        CommandOutcome::PreconditionFailure
    } else {
        CommandOutcome::ValidationFailure
    }
}

fn command_event(
    envelope: &CommandEnvelope,
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: &str,
    payload: Value,
) -> CommandDomainEvent {
    CommandDomainEvent {
        event_id: Uuid::new_v4().to_string(),
        event_type: event_type.to_owned(),
        occurred_at: Utc::now().to_rfc3339(),
        actor_app_user_id: envelope.actor_app_user_id.clone(),
        aggregate_type: aggregate_type.to_owned(),
        aggregate_id: aggregate_id.to_owned(),
        correlation_id: envelope.correlation_id.clone(),
        causation_id: Some(envelope.command_id.clone()),
        payload: payload.as_object().cloned().unwrap_or_default(),
    }
}

fn signer_result(
    envelope: &CommandEnvelope,
    action: &model::SignerActionResult,
    event_type: &str,
    extra_payload: Option<Value>,
) -> Result<CommandResult, CommandDispatchError> {
    let mut payload = json!({
        "signatureRequestId": action.signature_request_id,
        "recipientId": action.recipient_id,
        "state": action.state,
        "envelopeReadyToFinalize": action.envelope_ready_to_finalize,
    });
    if let (Some(target), Some(extra)) = (payload.as_object_mut(), extra_payload) {
        if let Some(extra) = extra.as_object() {
            target.extend(extra.clone());
        }
    }
    let mut result = CommandResult::success(
        envelope.command_id.clone(),
        Some(action.recipient_id.clone()),
        Some(serialize_value(action)?),
    );
    result.emitted_events.push(command_event(
        envelope,
        event_type,
        "luxesign_recipient",
        &action.recipient_id,
        payload,
    ));
    Ok(result)
}

struct ContractExecuteCommand {
    service: Arc<ContractService<ContractDao>>,
}

#[async_trait]
impl DurableCommandHandler for ContractExecuteCommand {
    fn command_type(&self) -> &'static str {
        "contract.execute"
    }

    fn service_domain(&self) -> &'static str {
        "contract"
    }

    fn scheduling_payload(&self, request: &CommandRequest) -> Option<Value> {
        request
            .aggregate_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .map(|contract_id| json!({ "contractId": contract_id }))
    }

    async fn handle(
        &self,
        tx: &mut DbTransaction,
        envelope: &CommandEnvelope,
        context: &ServiceContext,
    ) -> Result<CommandResult, CommandDispatchError> {
        if envelope.aggregate_type != "contract" {
            return Ok(CommandResult::failure(
                envelope.command_id.clone(),
                CommandOutcome::ValidationFailure,
                envelope.aggregate_id.clone(),
                "CONTRACT_AGGREGATE_TYPE_INVALID",
                "contract.execute requires aggregateType='contract'.",
            ));
        }
        let Some(contract_id) = envelope
            .aggregate_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(CommandResult::failure(
                envelope.command_id.clone(),
                CommandOutcome::ValidationFailure,
                None,
                "CONTRACT_ID_REQUIRED",
                "contract.execute requires aggregateId.",
            ));
        };

        if let Some(input_contract_id) = envelope
            .input
            .get("contractId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            if input_contract_id != contract_id {
                return Ok(CommandResult::failure(
                    envelope.command_id.clone(),
                    CommandOutcome::ValidationFailure,
                    Some(contract_id.clone()),
                    "CONTRACT_ID_MISMATCH",
                    "input.contractId must match aggregateId.",
                ));
            }
        }

        let evidence_document_id = match envelope.input.get("evidenceDocumentId") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
            Some(_) => {
                return Ok(CommandResult::failure(
                    envelope.command_id.clone(),
                    CommandOutcome::ValidationFailure,
                    Some(contract_id.clone()),
                    "EVIDENCE_DOCUMENT_ID_INVALID",
                    "evidenceDocumentId must be a non-empty string or null.",
                ));
            }
        };

        let request = ExecuteContractRequest {
            contract_id: contract_id.clone(),
            evidence_document_id,
        };

        let (contract, service_event) = match self
            .service
            .execute_transactional(tx, &request, context)
            .await
        {
            Ok(value) => value,
            Err(CoreServiceError::Business { code, message }) => {
                let outcome = match code {
                    "CONTRACT_NOT_FOUND" => CommandOutcome::NotFound,
                    "CONTRACT_EXECUTION_CONFLICT" => CommandOutcome::Conflict,
                    "CONTRACT_ALREADY_EXECUTED" => CommandOutcome::Conflict,
                    "CONTRACT_NOT_EXECUTABLE" => CommandOutcome::PreconditionFailure,
                    _ => CommandOutcome::ValidationFailure,
                };
                return Ok(CommandResult::failure(
                    envelope.command_id.clone(),
                    outcome,
                    Some(contract_id.clone()),
                    code,
                    message,
                ));
            }
            Err(CoreServiceError::Runtime(ServiceRuntimeError::Forbidden { reason, .. })) => {
                return Ok(CommandResult::failure(
                    envelope.command_id.clone(),
                    CommandOutcome::Unauthorized,
                    Some(contract_id.clone()),
                    "FORBIDDEN",
                    reason,
                ));
            }
            Err(error) => return Err(error.into()),
        };

        let event = CommandDomainEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: service_event.event_type.to_owned(),
            occurred_at: Utc::now().to_rfc3339(),
            actor_app_user_id: envelope.actor_app_user_id.clone(),
            aggregate_type: "contract".into(),
            aggregate_id: contract.id.clone(),
            correlation_id: envelope.correlation_id.clone(),
            causation_id: Some(envelope.command_id.clone()),
            payload: service_event.payload.into_iter().collect(),
        };

        let mut result = CommandResult::success(
            envelope.command_id.clone(),
            Some(contract.id.clone()),
            Some(json!({
                "contractId": contract.id,
                "status": contract.status,
                "executedAt": contract.executed_at,
                "evidenceDocumentId": contract.evidence_document_id,
            })),
        );
        result.emitted_events.push(event);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use services::{ServiceActor, ServiceActorKind};

    fn context() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("test".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "corr-1".into(),
            causation_id: Some("parent".into()),
            principal: None,
        }
    }

    #[test]
    fn fingerprint_ignores_transport_retry_metadata_but_not_intent() {
        let request = CommandRequest {
            command_id: "cmd-1".into(),
            command_type: "contract.execute".into(),
            aggregate_type: "contract".into(),
            aggregate_id: Some("c1".into()),
            requested_at: "2026-09-26T00:00:00Z".into(),
            input: Map::new(),
        };
        let first = request.canonicalize(&context());

        let mut retry_context = context();
        retry_context.correlation_id = "corr-2".into();
        retry_context.causation_id = Some("different-parent".into());
        let mut retry = request.clone();
        retry.requested_at = "2026-09-26T00:01:00Z".into();
        let second = retry.canonicalize(&retry_context);

        assert_eq!(
            intent_fingerprint(&first).unwrap(),
            intent_fingerprint(&second).unwrap()
        );

        let mut changed = second.clone();
        changed.aggregate_id = Some("c2".into());
        assert_ne!(
            intent_fingerprint(&first).unwrap(),
            intent_fingerprint(&changed).unwrap()
        );
    }
}

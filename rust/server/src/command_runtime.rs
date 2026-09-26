use crate::contracts::ContractService;
use crate::service_support::CoreServiceError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use db::{
    CommandReceiptDao, CommandReceiptRow, ContractDao, Database, DbFailure, DbTransaction,
    DomainEventOutboxDao, OutboxEventInput,
};
use domain::ExecuteContractRequest;
use serde_json::{json, Map, Value};
use service::{
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
    Scheduler(#[from] service::ServiceDispatchError),
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
    registry: Arc<CommandRegistry>,
}

impl CommandDispatcher {
    pub fn for_kernel(
        db: Database,
        contract: Arc<ContractService<ContractDao>>,
    ) -> Result<Self, CommandDispatchError> {
        let mut registry = CommandRegistry::default();
        registry.register(Arc::new(ContractExecuteCommand { service: contract }))?;

        Ok(Self {
            receipts: CommandReceiptDao::new(db.clone()),
            outbox: DomainEventOutboxDao::new(db.clone()),
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
    use service::{ServiceActor, ServiceActorKind};

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

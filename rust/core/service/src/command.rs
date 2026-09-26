use crate::ServiceContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcome {
    Success,
    ValidationFailure,
    NotFound,
    Conflict,
    Unauthorized,
    PreconditionFailure,
}

impl CommandOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::ValidationFailure => "validation_failure",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Unauthorized => "unauthorized",
            Self::PreconditionFailure => "precondition_failure",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "validation_failure" => Some(Self::ValidationFailure),
            "not_found" => Some(Self::NotFound),
            "conflict" => Some(Self::Conflict),
            "unauthorized" => Some(Self::Unauthorized),
            "precondition_failure" => Some(Self::PreconditionFailure),
            _ => None,
        }
    }

    pub fn retryable(self) -> bool {
        matches!(self, Self::Conflict)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRequest {
    pub command_id: String,
    pub command_type: String,
    pub aggregate_type: String,
    pub aggregate_id: Option<String>,
    pub requested_at: String,
    #[serde(default)]
    pub input: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    pub command_id: String,
    pub command_type: String,
    pub actor_app_user_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub requested_at: String,
    #[serde(default)]
    pub input: serde_json::Map<String, Value>,
}

impl CommandRequest {
    pub fn canonicalize(&self, context: &ServiceContext) -> CommandEnvelope {
        CommandEnvelope {
            command_id: self.command_id.clone(),
            command_type: self.command_type.clone(),
            actor_app_user_id: context
                .principal
                .as_ref()
                .map(|principal| principal.app_user_id.clone()),
            aggregate_type: self.aggregate_type.clone(),
            aggregate_id: self.aggregate_id.clone(),
            correlation_id: Some(context.correlation_id.clone()),
            causation_id: context.causation_id.clone(),
            requested_at: self.requested_at.clone(),
            input: self.input.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDomainEvent {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub actor_app_user_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub command_id: String,
    pub outcome: CommandOutcome,
    #[serde(default)]
    pub emitted_events: Vec<CommandDomainEvent>,
    pub aggregate_id: Option<String>,
    pub message: Option<String>,
    pub replayed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
}

impl CommandResult {
    pub fn success(
        command_id: impl Into<String>,
        aggregate_id: Option<String>,
        value: Option<Value>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            outcome: CommandOutcome::Success,
            emitted_events: Vec::new(),
            aggregate_id,
            message: None,
            replayed: false,
            value,
            error: None,
            receipt_id: None,
        }
    }

    pub fn failure(
        command_id: impl Into<String>,
        outcome: CommandOutcome,
        aggregate_id: Option<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self {
            command_id: command_id.into(),
            outcome,
            emitted_events: Vec::new(),
            aggregate_id,
            message: Some(message.clone()),
            replayed: false,
            value: None,
            error: Some(CommandError {
                code: code.into(),
                message,
                retryable: outcome.retryable(),
            }),
            receipt_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CommandReceiptStatus {
    Succeeded,
    Failed,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandReceipt {
    pub command_id: String,
    pub outcome: Option<CommandOutcome>,
    pub status: CommandReceiptStatus,
    pub aggregate_id: Option<String>,
    pub message: Option<String>,
    pub created_at: Option<String>,
    pub actor_app_user_id: Option<String>,
    pub command_type: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub aggregate_type: Option<String>,
    pub result_payload: Option<Value>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl CommandReceipt {
    pub fn replay_result(&self) -> CommandResult {
        match (self.status, self.outcome) {
            (CommandReceiptStatus::Succeeded | CommandReceiptStatus::Failed, Some(outcome)) => {
                CommandResult {
                    command_id: self.command_id.clone(),
                    outcome,
                    emitted_events: Vec::new(),
                    aggregate_id: self.aggregate_id.clone(),
                    message: self.message.clone(),
                    replayed: true,
                    value: self.result_payload.clone(),
                    error: self.error_code.as_ref().map(|code| CommandError {
                        code: code.clone(),
                        message: self
                            .error_message
                            .clone()
                            .or_else(|| self.message.clone())
                            .unwrap_or_else(|| "Command failed.".into()),
                        retryable: outcome.retryable(),
                    }),
                    receipt_id: Some(self.command_id.clone()),
                }
            }
            _ => CommandResult::failure(
                self.command_id.clone(),
                CommandOutcome::Conflict,
                self.aggregate_id.clone(),
                "COMMAND_IN_FLIGHT",
                "Command claim is in-flight (pending receipt); retry later.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServiceActor, ServiceActorKind, ServicePrincipal};

    #[test]
    fn trusted_context_overrides_actor_and_correlation() {
        let request = CommandRequest {
            command_id: "cmd-1".into(),
            command_type: "contract.execute".into(),
            aggregate_type: "contract".into(),
            aggregate_id: Some("c1".into()),
            requested_at: "2026-09-26T00:00:00Z".into(),
            input: Default::default(),
        };
        let context = ServiceContext {
            actor: ServiceActor {
                id: Some("edge".into()),
                kind: ServiceActorKind::User,
            },
            correlation_id: "trusted-correlation".into(),
            causation_id: Some("trusted-cause".into()),
            principal: Some(ServicePrincipal {
                app_user_id: "app-user".into(),
                level: "BUSINESS_POWER_USER".into(),
                role_codes: vec![],
                account_type: "internal".into(),
                entitlement_codes: vec![],
            }),
        };

        let envelope = request.canonicalize(&context);
        assert_eq!(envelope.actor_app_user_id.as_deref(), Some("app-user"));
        assert_eq!(
            envelope.correlation_id.as_deref(),
            Some("trusted-correlation")
        );
        assert_eq!(envelope.causation_id.as_deref(), Some("trusted-cause"));
    }

    #[test]
    fn pending_receipt_replays_as_retryable_conflict() {
        let receipt = CommandReceipt {
            command_id: "cmd-1".into(),
            outcome: None,
            status: CommandReceiptStatus::Pending,
            aggregate_id: None,
            message: None,
            created_at: None,
            actor_app_user_id: None,
            command_type: None,
            correlation_id: None,
            causation_id: None,
            aggregate_type: None,
            result_payload: None,
            error_code: None,
            error_message: None,
        };
        let result = receipt.replay_result();
        assert_eq!(result.outcome, CommandOutcome::Conflict);
        assert!(result.error.as_ref().is_some_and(|error| error.retryable));
    }
}

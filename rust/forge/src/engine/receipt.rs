//! Receipt replay rules from `db/workflow-command-receipt.ts`.
//! A stored `pending` is the claim sentinel, never a terminal engine outcome.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptOutcome {
    Success,
    ValidationFailure,
    NotFound,
    Conflict,
    Unauthorized,
    PreconditionFailure,
    Pending,
}

impl ReceiptOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::ValidationFailure => "validation_failure",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Unauthorized => "unauthorized",
            Self::PreconditionFailure => "precondition_failure",
            Self::Pending => "pending",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "success" => Self::Success,
            "validation_failure" => Self::ValidationFailure,
            "not_found" => Self::NotFound,
            "unauthorized" => Self::Unauthorized,
            "precondition_failure" => Self::PreconditionFailure,
            "pending" => Self::Pending,
            _ => Self::Conflict,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandReceipt {
    pub command_id: String,
    pub outcome: ReceiptOutcome,
    pub aggregate_id: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayDecision {
    pub outcome: ReceiptOutcome,
    pub message: Option<String>,
}

/// Missing or still-pending receipt => retryable conflict. Never a success.
pub fn replay_outcome(receipt: Option<&CommandReceipt>) -> ReplayDecision {
    match receipt {
        None => ReplayDecision {
            outcome: ReceiptOutcome::Conflict,
            message: Some("Command has no receipt; treat as in-flight.".into()),
        },
        Some(r) if r.outcome == ReceiptOutcome::Pending => ReplayDecision {
            outcome: ReceiptOutcome::Conflict,
            message: Some("Command claim is in-flight (pending receipt); retry later.".into()),
        },
        Some(r) => ReplayDecision {
            outcome: r.outcome.clone(),
            message: r.message.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_is_conflict() {
        let d = replay_outcome(None);
        assert_eq!(d.outcome, ReceiptOutcome::Conflict);
    }

    #[test]
    fn pending_is_conflict() {
        let r = CommandReceipt {
            command_id: "forge.completion:t1".into(),
            outcome: ReceiptOutcome::Pending,
            aggregate_id: None,
            message: None,
        };
        assert_eq!(replay_outcome(Some(&r)).outcome, ReceiptOutcome::Conflict);
    }

    #[test]
    fn success_replays() {
        let r = CommandReceipt {
            command_id: "forge.completion:t1".into(),
            outcome: ReceiptOutcome::Success,
            aggregate_id: Some("t1".into()),
            message: None,
        };
        assert_eq!(replay_outcome(Some(&r)).outcome, ReceiptOutcome::Success);
    }
}

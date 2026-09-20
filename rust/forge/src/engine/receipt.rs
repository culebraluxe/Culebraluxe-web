//! Receipt replay rules. A stored pending is the claim sentinel, never a terminal outcome.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptOutcome {
    Success, ValidationFailure, NotFound, Conflict, Unauthorized, PreconditionFailure, Pending,
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

pub fn replay_outcome(receipt: Option<&CommandReceipt>) -> ReplayDecision {
    match receipt {
        None => ReplayDecision { outcome: ReceiptOutcome::Conflict, message: Some("Command has no receipt; treat as in-flight.".into()) },
        Some(r) if r.outcome == ReceiptOutcome::Pending => ReplayDecision { outcome: ReceiptOutcome::Conflict, message: Some("Command claim is in-flight (pending receipt); retry later.".into()) },
        Some(r) => ReplayDecision { outcome: r.outcome.clone(), message: r.message.clone() },
    }
}

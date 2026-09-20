use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureRequestStatus {
    Requested,
    Sent,
    Viewed,
    Signed,
    Completed,
    Declined,
    Voided,
    Expired,
    Error,
}

impl SignatureRequestStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Sent => "sent",
            Self::Viewed => "viewed",
            Self::Signed => "signed",
            Self::Completed => "completed",
            Self::Declined => "declined",
            Self::Voided => "voided",
            Self::Expired => "expired",
            Self::Error => "error",
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Requested | Self::Sent | Self::Viewed | Self::Signed
        )
    }

    pub const fn can_transition_to(self, target: Self) -> bool {
        match self {
            Self::Requested => matches!(
                target,
                Self::Sent
                    | Self::Viewed
                    | Self::Signed
                    | Self::Completed
                    | Self::Declined
                    | Self::Voided
                    | Self::Expired
                    | Self::Error
            ),
            Self::Sent => matches!(
                target,
                Self::Viewed
                    | Self::Signed
                    | Self::Completed
                    | Self::Declined
                    | Self::Voided
                    | Self::Expired
                    | Self::Error
            ),
            Self::Viewed => matches!(
                target,
                Self::Signed
                    | Self::Completed
                    | Self::Declined
                    | Self::Voided
                    | Self::Expired
                    | Self::Error
            ),
            Self::Signed => matches!(
                target,
                Self::Completed | Self::Declined | Self::Voided | Self::Expired | Self::Error
            ),
            Self::Completed | Self::Declined | Self::Voided | Self::Expired | Self::Error => false,
        }
    }
}

impl TryFrom<&str> for SignatureRequestStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, String> {
        match value {
            "requested" => Ok(Self::Requested),
            "sent" => Ok(Self::Sent),
            "viewed" => Ok(Self::Viewed),
            "signed" => Ok(Self::Signed),
            "completed" => Ok(Self::Completed),
            "declined" => Ok(Self::Declined),
            "voided" => Ok(Self::Voided),
            "expired" => Ok(Self::Expired),
            "error" => Ok(Self::Error),
            other => Err(format!("unknown signature request status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureRecipientRole {
    Signer,
    Approver,
}

impl SignatureRecipientRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Signer => "signer",
            Self::Approver => "approver",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureRecipient {
    pub role: SignatureRecipientRole,
    pub name: String,
    pub email: String,
    pub order: i32,
    pub execution_role: Option<String>,
    pub execution_slot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureRequest {
    pub id: String,
    pub transaction_document_id: String,
    pub status: SignatureRequestStatus,
    pub message: Option<String>,
    pub execution_role: Option<String>,
    pub execution_slot_id: Option<String>,
    pub created_by_user_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendSignatureRequest {
    pub command_id: String,
    pub transaction_document_id: String,
    pub recipients: Vec<SignatureRecipient>,
    pub message: Option<String>,
    pub created_by_user_id: Option<String>,
    pub execution_role: Option<String>,
    pub execution_slot_id: Option<String>,
    pub slot_recipient_email: Option<String>,
    pub signature_role: Option<String>,
    pub completion_recipient_emails: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplySignatureStatusRequest {
    pub command_id: String,
    pub signature_request_id: String,
    pub target_status: Option<SignatureRequestStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureRequestResult {
    pub signature_request: SignatureRequest,
    pub existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureStatusResult {
    pub signature_request: SignatureRequest,
    pub transitioned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureCommandOutcome {
    Success,
    ValidationFailure,
    NotFound,
    Conflict,
    Unauthorized,
    PreconditionFailure,
}

impl SignatureCommandOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::ValidationFailure => "validation_failure",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Unauthorized => "unauthorized",
            Self::PreconditionFailure => "precondition_failure",
        }
    }
}

impl TryFrom<&str> for SignatureCommandOutcome {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "success" => Ok(Self::Success),
            "validation_failure" => Ok(Self::ValidationFailure),
            "not_found" => Ok(Self::NotFound),
            "conflict" => Ok(Self::Conflict),
            "unauthorized" => Ok(Self::Unauthorized),
            "precondition_failure" => Ok(Self::PreconditionFailure),
            other => Err(format!("unknown signature command outcome: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureCommandResult {
    pub command_id: String,
    pub outcome: SignatureCommandOutcome,
    pub aggregate_id: Option<String>,
    pub message: Option<String>,
    pub replayed: bool,
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureProviderEvent {
    Sent,
    Viewed,
    Signed,
    Completed,
    Declined,
    Voided,
    Expired,
    Error,
}

impl SignatureProviderEvent {
    pub const fn as_status(self) -> SignatureRequestStatus {
        match self {
            Self::Sent => SignatureRequestStatus::Sent,
            Self::Viewed => SignatureRequestStatus::Viewed,
            Self::Signed => SignatureRequestStatus::Signed,
            Self::Completed => SignatureRequestStatus::Completed,
            Self::Declined => SignatureRequestStatus::Declined,
            Self::Voided => SignatureRequestStatus::Voided,
            Self::Expired => SignatureRequestStatus::Expired,
            Self::Error => SignatureRequestStatus::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureProviderSendRequest {
    pub signature_request_id: String,
    pub transaction_document_id: String,
    pub recipients: Vec<SignatureRecipient>,
    pub message: Option<String>,
    pub signature_role: Option<String>,
    pub signature_slot_id: Option<String>,
    pub completion_recipient_emails: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureProviderSendResult {
    pub ok: bool,
    pub provider_status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureProviderStatusResult {
    pub status: SignatureRequestStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureProviderActionResult {
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureWebhookVerification {
    pub event: SignatureProviderEvent,
    pub signature_request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureArtifactDownload {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedParticipantSlot {
    pub role: String,
    pub slot_id: String,
    pub email: String,
    pub name: Option<String>,
    pub raw: Value,
}

pub fn normalize_signature_email(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn validate_signature_recipients(recipients: &[SignatureRecipient]) -> Vec<String> {
    if recipients.is_empty() {
        return vec!["At least one recipient is required.".into()];
    }

    let mut errors = Vec::new();
    let mut emails = BTreeSet::new();
    let mut orders = BTreeSet::new();
    let mut slots = BTreeSet::new();

    for recipient in recipients {
        if recipient.name.trim().is_empty() {
            errors.push("Recipient name is required.".into());
        }
        if recipient.email.trim().is_empty() {
            errors.push("Recipient email is required.".into());
        }
        if recipient.order < 1 {
            errors.push("Recipient order must be a positive integer.".into());
        }
        if !orders.insert(recipient.order) {
            errors.push(format!("Duplicate recipient order: {}.", recipient.order));
        }

        let has_role = recipient.execution_role.is_some();
        let has_slot = recipient.execution_slot_id.is_some();
        if has_role != has_slot {
            errors.push(
                "Recipient executionRole and executionSlotId must be supplied together.".into(),
            );
        }

        if let Some(slot_id) = recipient.execution_slot_id.as_deref() {
            if !slots.insert(slot_id.to_owned()) {
                errors.push(format!("Duplicate execution slot: {slot_id}."));
            }
        }

        let email = normalize_signature_email(&recipient.email);
        if !emails.insert(email) {
            errors.push(format!(
                "Each legal signer must use a unique email address; duplicate: {}.",
                recipient.email
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipient(email: &str, order: i32) -> SignatureRecipient {
        SignatureRecipient {
            role: SignatureRecipientRole::Signer,
            name: "Signer".into(),
            email: email.into(),
            order,
            execution_role: None,
            execution_slot_id: None,
        }
    }

    #[test]
    fn completed_signature_request_cannot_reopen() {
        assert!(!SignatureRequestStatus::Completed.can_transition_to(SignatureRequestStatus::Sent));
        assert!(SignatureRequestStatus::Sent.can_transition_to(SignatureRequestStatus::Completed));
    }

    #[test]
    fn legal_signers_require_unique_mailboxes() {
        let errors = validate_signature_recipients(&[
            recipient("same@example.test", 1),
            recipient("SAME@example.test", 2),
        ]);
        assert!(errors.iter().any(|error| error.contains("unique email")));
    }

    #[test]
    fn execution_role_and_slot_are_atomic() {
        let mut r = recipient("one@example.test", 1);
        r.execution_role = Some("BUYER".into());
        let errors = validate_signature_recipients(&[r]);
        assert!(errors
            .iter()
            .any(|error| error.contains("supplied together")));
    }
}

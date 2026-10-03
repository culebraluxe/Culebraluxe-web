use crate::signature::{SignatureRecipientRole, SignatureRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentSigningMode {
    Sequential,
    Parallel,
}

impl DocumentSigningMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Parallel => "parallel",
        }
    }
}

impl TryFrom<&str> for DocumentSigningMode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "sequential" => Ok(Self::Sequential),
            "parallel" => Ok(Self::Parallel),
            other => Err(format!("unknown document signing mode: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureFieldType {
    Signature,
    Initials,
    Name,
    Email,
    Date,
    Text,
    Number,
    Checkbox,
    Radio,
    Dropdown,
}

impl SignatureFieldType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Signature => "signature",
            Self::Initials => "initials",
            Self::Name => "name",
            Self::Email => "email",
            Self::Date => "date",
            Self::Text => "text",
            Self::Number => "number",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Dropdown => "dropdown",
        }
    }
}

impl TryFrom<&str> for SignatureFieldType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "signature" => Ok(Self::Signature),
            "initials" => Ok(Self::Initials),
            "name" => Ok(Self::Name),
            "email" => Ok(Self::Email),
            "date" => Ok(Self::Date),
            "text" => Ok(Self::Text),
            "number" => Ok(Self::Number),
            "checkbox" => Ok(Self::Checkbox),
            "radio" => Ok(Self::Radio),
            "dropdown" => Ok(Self::Dropdown),
            other => Err(format!("unknown signature field type: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSignRecipientInput {
    pub role: SignatureRecipientRole,
    pub name: String,
    pub email: String,
    pub signer_order: i32,
    pub signing_step: i32,
    pub execution_role: Option<String>,
    pub execution_slot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSignRecipient {
    pub id: String,
    pub signature_request_id: String,
    pub role: SignatureRecipientRole,
    pub name: String,
    pub email: String,
    pub signer_order: i32,
    pub signing_step: i32,
    pub execution_role: Option<String>,
    pub execution_slot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureField {
    pub id: String,
    pub signature_request_id: String,
    pub recipient_id: String,
    pub field_key: String,
    pub field_type: SignatureFieldType,
    pub page_number: i32,
    pub position_x: f64,
    pub position_y: f64,
    pub width: f64,
    pub height: f64,
    pub required: bool,
    pub label: Option<String>,
    pub configuration: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSignConfig {
    pub signature_request_id: String,
    pub subject: Option<String>,
    pub signing_mode: DocumentSigningMode,
    pub expires_at: Option<String>,
    pub issued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSignSnapshot {
    pub signature_request: SignatureRequest,
    pub config: DocumentSignConfig,
    pub recipients: Vec<DocumentSignRecipient>,
    pub fields: Vec<SignatureField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDocumentSignRequest {
    pub transaction_document_id: String,
    pub recipients: Vec<DocumentSignRecipientInput>,
    pub subject: Option<String>,
    pub message: Option<String>,
    pub signing_mode: DocumentSigningMode,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDocumentSignRecipientsRequest {
    pub signature_request_id: String,
    pub recipients: Vec<DocumentSignRecipientInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PutSignatureFieldRequest {
    pub signature_request_id: String,
    pub field_id: Option<String>,
    pub recipient_id: String,
    pub field_key: String,
    pub field_type: SignatureFieldType,
    pub page_number: i32,
    pub position_x: f64,
    pub position_y: f64,
    pub width: f64,
    pub height: f64,
    pub required: bool,
    pub label: Option<String>,
    #[serde(default)]
    pub configuration: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSignatureFieldRequest {
    pub signature_request_id: String,
    pub field_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDocumentSignRequest {
    pub signature_request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSignIssueResult {
    pub signature_request_id: String,
    pub invitation_message_ids: Vec<String>,
    pub expires_at: String,
}

pub fn validate_document_sign_recipients(recipients: &[DocumentSignRecipientInput]) -> Vec<String> {
    use std::collections::BTreeSet;

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
        if recipient.signer_order < 1 {
            errors.push("Recipient signerOrder must be positive.".into());
        }
        if recipient.signing_step < 1 {
            errors.push("Recipient signingStep must be positive.".into());
        }
        if !orders.insert(recipient.signer_order) {
            errors.push(format!(
                "Duplicate recipient signerOrder: {}.",
                recipient.signer_order
            ));
        }

        let email = recipient.email.trim().to_lowercase();
        if !emails.insert(email) {
            errors.push(format!(
                "Each legal signer must use a unique email address; duplicate: {}.",
                recipient.email
            ));
        }

        let has_role = recipient.execution_role.is_some();
        let has_slot = recipient.execution_slot_id.is_some();
        if has_role != has_slot {
            errors.push(
                "Recipient executionRole and executionSlotId must be supplied together.".into(),
            );
        }
        if let Some(slot) = recipient.execution_slot_id.as_deref() {
            if !slots.insert(slot.to_owned()) {
                errors.push(format!("Duplicate execution slot: {slot}."));
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipient(email: &str, order: i32, step: i32) -> DocumentSignRecipientInput {
        DocumentSignRecipientInput {
            role: SignatureRecipientRole::Signer,
            name: "Signer".into(),
            email: email.into(),
            signer_order: order,
            signing_step: step,
            execution_role: None,
            execution_slot_id: None,
        }
    }

    #[test]
    fn signing_steps_allow_parallel_groups_but_orders_stay_unique() {
        assert!(validate_document_sign_recipients(&[
            recipient("one@example.test", 1, 1),
            recipient("two@example.test", 2, 1),
            recipient("three@example.test", 3, 2),
        ])
        .is_empty());

        let errors = validate_document_sign_recipients(&[
            recipient("one@example.test", 1, 1),
            recipient("two@example.test", 1, 1),
        ]);
        assert!(errors.iter().any(|error| error.contains("signerOrder")));
    }
}

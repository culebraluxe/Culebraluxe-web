use crate::signature::{SignatureRecipientRole, SignatureRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuxesignSigningMode {
    Sequential,
    Parallel,
}

impl LuxesignSigningMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Parallel => "parallel",
        }
    }
}

impl TryFrom<&str> for LuxesignSigningMode {
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
pub struct LuxesignRecipientInput {
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
pub struct LuxesignRecipient {
    pub id: String,
    pub signature_request_id: String,
    pub role: SignatureRecipientRole,
    pub name: String,
    pub email: String,
    pub signer_order: i32,
    pub signing_step: i32,
    pub execution_role: Option<String>,
    pub execution_slot_id: Option<String>,
    /// Runtime state from `luxesign_recipient_state`, when the read joins
    /// it. Absent on writes and on reads that do not need liveness.
    #[serde(default)]
    pub state: Option<String>,
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
pub struct LuxesignConfig {
    pub signature_request_id: String,
    pub subject: Option<String>,
    pub signing_mode: LuxesignSigningMode,
    pub expires_at: Option<String>,
    pub issued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuxesignSnapshot {
    pub luxesign_request: SignatureRequest,
    pub config: LuxesignConfig,
    pub recipients: Vec<LuxesignRecipient>,
    pub fields: Vec<SignatureField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareLuxesignRequest {
    pub transaction_document_id: String,
    pub recipients: Vec<LuxesignRecipientInput>,
    pub subject: Option<String>,
    pub message: Option<String>,
    pub signing_mode: LuxesignSigningMode,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLuxesignRecipientsRequest {
    pub signature_request_id: String,
    pub recipients: Vec<LuxesignRecipientInput>,
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
pub struct IssueLuxesignRequest {
    pub signature_request_id: String,
}

/// Where each signer's signature goes when an envelope is sent in one step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SendFieldPlacement {
    /// The form's own fixed positions: the issuing template's anchor blocks. Each person signs only their own block.
    #[default]
    Template,
    /// One signature box per signer on the last page of the document.
    LastPage,
    /// One signature box per signer on this 1-based page.
    #[serde(rename_all = "camelCase")]
    Page { page_number: i32 },
}

/// Prepare, place the signature fields, and issue in ONE transaction: the envelope is never left half-built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendLuxesignRequest {
    pub transaction_document_id: String,
    pub recipients: Vec<LuxesignRecipientInput>,
    pub subject: Option<String>,
    pub message: Option<String>,
    pub signing_mode: LuxesignSigningMode,
    pub expires_at: Option<String>,
    #[serde(default)]
    pub placement: SendFieldPlacement,
    /// People copied on the completed document without signing (a listing's broker). The sender is always copied.
    #[serde(default)]
    pub copy_to: Vec<String>,
    /// Blocks whose party is not on this form; they are drawn on the document, but nobody signs them.
    #[serde(default)]
    pub absent_roles: Vec<String>,
    /// Remind a waiting signer this often, in days (default 3; 0 = never).
    #[serde(default)]
    pub reminder_every_days: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuxesignSendResult {
    pub snapshot: LuxesignSnapshot,
    pub issued: LuxesignIssueResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuxesignIssueResult {
    pub signature_request_id: String,
    pub invitation_message_ids: Vec<String>,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAnchorRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAnchor {
    pub role: String,
    pub slot_id: Option<String>,
    /// `signature`, `initials` or `date` — the Vault anchor vocabulary.
    pub kind: String,
    pub page_index: i32,
    pub page_width: f64,
    pub page_height: f64,
    pub rect: TemplateAnchorRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAnchorFieldsRequest {
    pub signature_request_id: String,
    /// Explicit anchors. Empty (the desk default) reads the issuing
    /// template's own blocks from Vault's snapshot instead.
    #[serde(default)]
    pub anchors: Vec<TemplateAnchor>,
    /// Blocks whose party is not on this form (an absent spouse). They are drawn on the document and carry no field.
    #[serde(default)]
    pub absent_roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAnchorFieldsResult {
    pub signature_request_id: String,
    pub created_field_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuxesignSweepResult {
    pub expired_recipients: Vec<String>,
    pub expired_envelopes: Vec<String>,
    /// Reminders queued for signers whose turn it is and who have not acted (one message each).
    #[serde(default)]
    pub reminder_message_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuxesignFinalizeResult {
    pub signature_request_id: String,
    pub audit_media_id: Option<String>,
    pub signed_media_id: Option<String>,
    pub already_completed: bool,
    /// The completion notices queued (signers, the copy list, and the sender), each with the signed document attached.
    #[serde(default)]
    pub notification_message_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LuxesignEnvelopeSummary {
    pub signature_request_id: String,
    pub transaction_document_id: String,
    pub subject: Option<String>,
    pub client_name: Option<String>,
    pub signing_mode: String,
    pub status: String,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub recipient_total: i64,
    pub completed_total: i64,
}

pub fn validate_luxesign_recipients(recipients: &[LuxesignRecipientInput]) -> Vec<String> {
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

    fn recipient(email: &str, order: i32, step: i32) -> LuxesignRecipientInput {
        LuxesignRecipientInput {
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
        assert!(validate_luxesign_recipients(&[
            recipient("one@example.test", 1, 1),
            recipient("two@example.test", 2, 1),
            recipient("three@example.test", 3, 2),
        ])
        .is_empty());

        let errors = validate_luxesign_recipients(&[
            recipient("one@example.test", 1, 1),
            recipient("two@example.test", 1, 1),
        ]);
        assert!(errors.iter().any(|error| error.contains("signerOrder")));
    }
}

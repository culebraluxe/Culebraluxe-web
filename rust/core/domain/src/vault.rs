use crate::FormSignerPerson;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionDocumentType {
    Agreement,
    Addendum,
    Disclosure,
    Title,
    Financing,
    Inspection,
    Appraisal,
    Closing,
    Other,
}

impl TransactionDocumentType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agreement => "agreement",
            Self::Addendum => "addendum",
            Self::Disclosure => "disclosure",
            Self::Title => "title",
            Self::Financing => "financing",
            Self::Inspection => "inspection",
            Self::Appraisal => "appraisal",
            Self::Closing => "closing",
            Self::Other => "other",
        }
    }
}

impl TryFrom<&str> for TransactionDocumentType {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "agreement" => Ok(Self::Agreement),
            "addendum" => Ok(Self::Addendum),
            "disclosure" => Ok(Self::Disclosure),
            "title" => Ok(Self::Title),
            "financing" => Ok(Self::Financing),
            "inspection" => Ok(Self::Inspection),
            "appraisal" => Ok(Self::Appraisal),
            "closing" => Ok(Self::Closing),
            "other" => Ok(Self::Other),
            other => Err(format!("unknown transaction document type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionDocumentState {
    Draft,
    Ready,
    Sent,
    Signed,
    Voided,
    Superseded,
}

impl TransactionDocumentState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Sent => "sent",
            Self::Signed => "signed",
            Self::Voided => "voided",
            Self::Superseded => "superseded",
        }
    }

    pub const fn can_transition_to(self, target: Self) -> bool {
        match self {
            Self::Draft => matches!(target, Self::Ready | Self::Voided | Self::Superseded),
            Self::Ready => matches!(target, Self::Sent | Self::Voided | Self::Superseded),
            Self::Sent => matches!(target, Self::Signed | Self::Voided),
            Self::Signed => matches!(target, Self::Voided | Self::Superseded),
            Self::Voided | Self::Superseded => false,
        }
    }
}

impl TryFrom<&str> for TransactionDocumentState {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "ready" => Ok(Self::Ready),
            "sent" => Ok(Self::Sent),
            "signed" => Ok(Self::Signed),
            "voided" => Ok(Self::Voided),
            "superseded" => Ok(Self::Superseded),
            other => Err(format!("unknown transaction document state: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionDocumentSource {
    Upload,
    Generated,
    Imported,
    Provider,
}

impl TransactionDocumentSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Generated => "generated",
            Self::Imported => "imported",
            Self::Provider => "provider",
        }
    }
}

impl TryFrom<&str> for TransactionDocumentSource {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "upload" => Ok(Self::Upload),
            "generated" => Ok(Self::Generated),
            "imported" => Ok(Self::Imported),
            "provider" => Ok(Self::Provider),
            other => Err(format!("unknown transaction document source: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedArtifactRef {
    pub media_id: String,
    pub signed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedDocumentEvidence {
    pub checksum_sha256: String,
    pub template_id: String,
    pub template_version: i32,
    pub source_snapshot: Value,
    pub issued_version: i32,
    pub form_instance_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDocument {
    pub id: String,
    pub deal_id: Option<String>,
    pub document_type: TransactionDocumentType,
    pub document_type_label: Option<String>,
    pub title: Option<String>,
    pub state: TransactionDocumentState,
    pub source: TransactionDocumentSource,
    pub source_system: Option<String>,
    pub source_external_id: Option<String>,
    pub prepared_by_user_id: Option<String>,
    pub party_person_id: Option<String>,
    pub media_id: Option<String>,
    pub signed_artifact: Option<SignedArtifactRef>,
    pub signed_audit_media_id: Option<String>,
    pub supersedes_document_id: Option<String>,
    pub issued_evidence: Option<IssuedDocumentEvidence>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTransactionDocumentRequest {
    pub deal_id: Option<String>,
    pub document_type: TransactionDocumentType,
    pub document_type_label: Option<String>,
    pub title: Option<String>,
    pub state: TransactionDocumentState,
    pub source: TransactionDocumentSource,
    pub source_system: Option<String>,
    pub source_external_id: Option<String>,
    pub prepared_by_user_id: Option<String>,
    pub party_person_id: Option<String>,
    pub media_id: Option<String>,
    pub signed_artifact: Option<SignedArtifactRef>,
    pub supersedes_document_id: Option<String>,
    pub issued_evidence: Option<IssuedDocumentEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionTransactionDocumentRequest {
    pub command_id: String,
    pub document_id: String,
    pub to: TransactionDocumentState,
    pub signed_artifact: Option<SignedArtifactRef>,
    pub actor_app_user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultCommandOutcome {
    Success,
    ValidationFailure,
    NotFound,
    Conflict,
    Unauthorized,
    PreconditionFailure,
}

impl VaultCommandOutcome {
    pub const fn as_str(&self) -> &'static str {
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

impl TryFrom<&str> for VaultCommandOutcome {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "success" => Ok(Self::Success),
            "validation_failure" => Ok(Self::ValidationFailure),
            "not_found" => Ok(Self::NotFound),
            "conflict" => Ok(Self::Conflict),
            "unauthorized" => Ok(Self::Unauthorized),
            "precondition_failure" => Ok(Self::PreconditionFailure),
            other => Err(format!("unknown command outcome: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCommandResult {
    pub command_id: String,
    pub outcome: VaultCommandOutcome,
    pub aggregate_id: Option<String>,
    pub message: Option<String>,
    pub replayed: bool,
    pub value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultActorScope {
    pub account_type: String,
    pub person_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedDocumentListItem {
    pub id: String,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub document_type_label: Option<String>,
    pub title: Option<String>,
    pub state: TransactionDocumentState,
    pub template_id: Option<String>,
    pub template_version: Option<i32>,
    pub issued_version: Option<i32>,
    pub issued_checksum_sha256: Option<String>,
    pub issued_by_display_name: Option<String>,
    pub party_name: Option<String>,
    pub property_name: Option<String>,
    pub deal_name: Option<String>,
    pub created_at: String,
    pub signed_artifact_available: bool,
    pub signed_audit_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedDocumentForFormInstance {
    pub document_id: String,
    pub issued_version: i32,
    pub checksum: String,
    pub created_at: String,
    pub media_id: Option<String>,
    pub source_snapshot: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextIssuedVersionRequest {
    pub contract_id: Option<String>,
    pub deal_id: Option<String>,
    pub template_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMediaBytes {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractIssuedLineage {
    pub id: String,
    pub issued_version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueDocumentRequest {
    pub command_id: String,
    pub form_instance_id: String,
    pub actor_app_user_id: Option<String>,
    pub issued_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultRenderRequest {
    pub form_instance_id: String,
    pub contract_id: Option<String>,
    pub template_id: String,
    pub template_version: i32,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub issued_version: i32,
    pub participants: Vec<FormSignerPerson>,
    pub actor_app_user_id: Option<String>,
    pub issued_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultRenderedArtifact {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub document_type_label: String,
    pub display_name: String,
    pub render_metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultArtifactFailure {
    pub outcome: VaultCommandOutcome,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_state_machine_matches_typescript_contract() {
        assert!(TransactionDocumentState::Draft.can_transition_to(TransactionDocumentState::Ready));
        assert!(TransactionDocumentState::Sent.can_transition_to(TransactionDocumentState::Signed));
        assert!(
            !TransactionDocumentState::Ready.can_transition_to(TransactionDocumentState::Signed)
        );
        assert!(
            !TransactionDocumentState::Voided.can_transition_to(TransactionDocumentState::Draft)
        );
    }
}

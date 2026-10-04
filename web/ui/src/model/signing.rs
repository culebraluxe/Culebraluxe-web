//! Signing DTOs: the shapes the public signer edge answers, decoded from JSON
//! text like every other catalogue type.

use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignerRecipient {
    pub id: String,
    pub signature_request_id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub signer_order: i32,
    pub signing_step: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignerField {
    pub id: String,
    pub signature_request_id: String,
    pub recipient_id: String,
    pub field_key: String,
    pub field_type: String,
    pub page_number: i32,
    pub required: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SignerSession {
    pub signature_request_id: String,
    pub recipient: SignerRecipient,
    pub state: String,
    pub fields: Vec<SignerField>,
    pub consented: bool,
    pub is_turn: bool,
    pub expires_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SigningEnvelopeSummary {
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SigningEnvelopeRecipient {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub signer_order: i32,
    pub signing_step: i32,
    pub state: Option<String>,
}

use crate::document_sign::{DocumentSignRecipient, SignatureField};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerState {
    Pending,
    Notified,
    Viewed,
    InProgress,
    Completed,
    Declined,
    Expired,
    Revoked,
}

impl SignerState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Notified => "notified",
            Self::Viewed => "viewed",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Declined => "declined",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Declined | Self::Expired | Self::Revoked
        )
    }
}

impl TryFrom<&str> for SignerState {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "notified" => Ok(Self::Notified),
            "viewed" => Ok(Self::Viewed),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "declined" => Ok(Self::Declined),
            "expired" => Ok(Self::Expired),
            "revoked" => Ok(Self::Revoked),
            other => Err(format!("unknown signer state: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerAccessGrant {
    pub access_id: String,
    pub recipient_id: String,
    pub token_version: i32,
    pub expires_at: String,
    pub token: String,
    pub signing_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerRecipientState {
    pub recipient_id: String,
    pub state: SignerState,
    pub notified_at: Option<String>,
    pub first_viewed_at: Option<String>,
    pub completed_at: Option<String>,
    pub declined_at: Option<String>,
    pub expired_at: Option<String>,
    pub last_activity_at: Option<String>,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerSession {
    pub signature_request_id: String,
    pub recipient: DocumentSignRecipient,
    pub state: SignerRecipientState,
    pub fields: Vec<SignatureField>,
    pub consented: bool,
    pub is_turn: bool,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSignerRequest {
    pub recipient_id: String,
    pub access_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptSignerConsentRequest {
    pub recipient_id: String,
    pub access_token: String,
    pub consent_version: String,
    pub consent_text: String,
    pub consent_text_sha256: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSignatureFieldRequest {
    pub recipient_id: String,
    pub access_token: String,
    pub field_id: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSignerRequest {
    pub recipient_id: String,
    pub access_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclineSignerRequest {
    pub recipient_id: String,
    pub access_token: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerActionResult {
    pub signature_request_id: String,
    pub recipient_id: String,
    pub state: SignerState,
    pub envelope_ready_to_finalize: bool,
}

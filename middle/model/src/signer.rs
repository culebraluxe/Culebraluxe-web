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
    /// The canonical envelope status (`sent`, `viewed`, `signed`, `completed`, …): the signer's page offers the sealed
    /// copy only once it is `completed`.
    #[serde(default)]
    pub envelope_status: String,
    #[serde(default)]
    pub document_title: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
/// The sender's note to the signers.
    #[serde(default)]
    pub message: Option<String>,
    /// Ids of this recipient's fields that already have an answer.
    #[serde(default)]
    pub answered_field_ids: Vec<String>,
    /// Everyone on the envelope, in signing order, so a signer can see how far along it is.
    #[serde(default)]
    pub parties: Vec<SignerParty>,
}

/// One person on the envelope, as another signer may see them: a name, a role and how far they are. No address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerParty {
    pub name: String,
    pub role: String,
    pub state: String,
    pub is_you: bool,
}

/// What the session needs beyond the recipient's own rows (read in one place so the screen draws one truth).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerSessionContext {
    pub envelope_status: String,
    pub document_title: Option<String>,
    pub subject: Option<String>,
    pub message: Option<String>,
    pub answered_field_ids: Vec<String>,
    pub parties: Vec<SignerParty>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSignerRequest {
    pub recipient_id: String,
    pub access_token: String,
    /// Where and on what the signer acted, stamped by the edge from the request itself (never from the body).
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
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
    /// Where and on what the signer acted, stamped by the edge from the request itself (never from the body).
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSignerRequest {
    pub recipient_id: String,
    pub access_token: String,
    /// Where and on what the signer acted, stamped by the edge from the request itself (never from the body).
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclineSignerRequest {
    pub recipient_id: String,
    pub access_token: String,
    pub reason: Option<String>,
    /// Where and on what the signer acted, stamped by the edge from the request itself (never from the body).
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerActionResult {
    pub signature_request_id: String,
    pub recipient_id: String,
    pub state: SignerState,
    pub envelope_ready_to_finalize: bool,
}

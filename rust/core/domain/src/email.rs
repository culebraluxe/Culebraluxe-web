use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailMessageKind {
    SignatureInvitation,
    SignatureReminder,
    SignatureCompleted,
    SignatureDeclined,
}

impl EmailMessageKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignatureInvitation => "signature_invitation",
            Self::SignatureReminder => "signature_reminder",
            Self::SignatureCompleted => "signature_completed",
            Self::SignatureDeclined => "signature_declined",
        }
    }
}

impl TryFrom<&str> for EmailMessageKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "signature_invitation" => Ok(Self::SignatureInvitation),
            "signature_reminder" => Ok(Self::SignatureReminder),
            "signature_completed" => Ok(Self::SignatureCompleted),
            "signature_declined" => Ok(Self::SignatureDeclined),
            other => Err(format!("unknown email message kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailMessageStatus {
    Queued,
    Sending,
    Sent,
    Failed,
    Dead,
    Cancelled,
}

impl EmailMessageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Dead => "dead",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for EmailMessageStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "queued" => Ok(Self::Queued),
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            "dead" => Ok(Self::Dead),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown email message status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailMessage {
    pub id: String,
    pub message_kind: EmailMessageKind,
    pub recipient_email: String,
    pub template_key: String,
    pub template_payload: Value,
    pub dedupe_key: String,
    pub status: EmailMessageStatus,
    pub provider_message_id: Option<String>,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub queued_at: String,
    pub sent_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEmailRequest {
    pub message_kind: EmailMessageKind,
    pub recipient_email: String,
    pub template_key: String,
    #[serde(default)]
    pub template_payload: Value,
    pub dedupe_key: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailQueueResult {
    pub message_id: String,
    pub existing: bool,
}

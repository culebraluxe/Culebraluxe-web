//! Moved from `applemail.rs` (move only): MailAddress, LandedAppleMail, MailObservation, MailNormalization and its impl.

#[allow(unused_imports)]
use super::*;

/// An address as the envelope index reports it: `{"address": "...", "name": "..."}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailAddress {
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// One `l_applemail` row as promotion reads it. The repository normalizes driver-native
/// values on the way out, so `occurred_at` arrives as an ISO-8601 UTC string.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LandedAppleMail {
    pub source_account: String,
    pub source_message_id: String,
    pub mailbox_kind: Option<String>,
    pub mailbox_name: Option<String>,
    pub local_id: Option<i64>,
    pub message_id: Option<String>,
    pub occurred_at: Option<String>,
    pub sender: Option<String>,
    #[serde(default)]
    pub to_recipients: Vec<MailAddress>,
    #[serde(default)]
    pub cc_recipients: Vec<MailAddress>,
    #[serde(default)]
    pub bcc_recipients: Vec<MailAddress>,
    pub subject: Option<String>,
}

/// One neutral observation: what one landed message means, with no Person attached.
#[derive(Debug, Clone, PartialEq)]
pub struct MailObservation {
    pub source_external_id: String,
    pub source_account: String,
    pub mailbox: String,
    /// The landed row's own local id. Never an index into a page — only a source value.
    pub uid: Option<i64>,
    pub uid_validity: String,
    pub occurred_at: String,
    pub direction: &'static str,
    pub external_email: String,
    pub display_name: Option<String>,
    pub subject: Option<String>,
}

/// What a normalization pass read, and what it refused to read. A skip is a decision, so it
/// is counted and reported rather than quietly dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MailNormalization {
    pub observations: Vec<MailObservation>,
    pub skipped: BTreeMap<String, i64>,
}

impl MailNormalization {
    pub(super) fn skip(&mut self, reason: &str) {
        *self.skipped.entry(reason.to_owned()).or_insert(0) += 1;
    }
}

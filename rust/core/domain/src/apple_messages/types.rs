//! Moved from `apple_messages.rs` (move only): AppleMessagesHandle, AppleMessagesMessage, AppleMessagesExport, IdentityEvidence, AppleHandleEvidence and its impl, AppleHandleLookup.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleMessagesHandle {
    pub rowid: Option<i64>,
    pub id: Option<String>,
    pub country: Option<String>,
    pub service: Option<String>,
    pub uncanonicalized_id: Option<String>,
    pub person_centric_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleMessagesMessage {
    pub rowid: Option<i64>,
    pub guid: Option<String>,
    pub chat_guid: Option<String>,
    pub handle_id: Option<i64>,
    pub handle_value: Option<String>,
    pub service: Option<String>,
    pub account: Option<String>,
    pub date: Option<f64>,
    /// The exporter spells this `dateISO` (serde's camelCase would produce `dateIso`).
    #[serde(default, rename = "dateISO")]
    pub date_iso: Option<String>,
    pub is_from_me: Option<i64>,
    pub text: Option<String>,
    pub has_attachments: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AppleMessagesExport {
    pub source_account: String,
    pub handles: Vec<AppleMessagesHandle>,
    pub messages: Vec<AppleMessagesMessage>,
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct IdentityEvidence {
    pub value: String,
    pub normalized: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppleHandleEvidence {
    pub source: String,
    pub source_account: String,
    pub source_identity_key: String,
    pub source_label: Option<String>,
    pub display_name: Option<String>,
    pub organization: Option<String>,
    pub emails: Vec<IdentityEvidence>,
    pub phones: Vec<IdentityEvidence>,
    pub first_observed_at: Option<String>,
    pub last_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub is_two_way: Option<bool>,
    pub is_owner_initiated: Option<bool>,
    pub is_automated_or_bulk: Option<bool>,
    pub is_organization_or_service: Option<bool>,
    pub known_apple_contact: Option<bool>,
    pub has_email: bool,
    pub has_phone: bool,
    pub coverage_note: Option<String>,
    pub evidence_fingerprint: String,
    /// Apple `handle` rowids this identity resolves to; used to materialize events.
    pub handle_rowids: Vec<i64>,
}

impl AppleHandleEvidence {
    /// Every 1:1 message counted for this identity (inbound + outbound).
    pub fn total_count(&self) -> i64 {
        self.inbound_count + self.outbound_count
    }
}

/// Everything the reconciliation decision needs, fetched in bulk by the caller so the pass stays
/// two reads rather than two reads per handle.
#[derive(Debug, Clone, Default)]
pub struct AppleHandleLookup {
    /// An explicit, durable source link already recorded for this identity.
    pub explicit_link: Option<String>,
    /// Distinct canonical owners of any normalized email on the evidence.
    pub email_owners: Vec<String>,
    /// Distinct canonical owners of any normalized phone on the evidence.
    pub phone_owners: Vec<String>,
    /// At least one email matched more than one person (a conflict, never a choice).
    pub email_multi_match: bool,
    /// At least one phone matched more than one person.
    pub phone_multi_match: bool,
}

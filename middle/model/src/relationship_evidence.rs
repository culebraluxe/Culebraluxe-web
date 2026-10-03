use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipEvidenceRow {
    pub id: String,
    pub source: String,
    pub source_account: String,
    pub source_identity_key: String,
    pub source_label: Option<String>,
    pub display_name: Option<String>,
    pub organization: Option<String>,
    pub emails: Value,
    pub phones: Value,
    pub first_observed_at: Option<String>,
    pub last_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: Option<i64>,
    pub outbound_count: Option<i64>,
    pub is_two_way: Option<bool>,
    pub is_owner_initiated: Option<bool>,
    pub is_automated_or_bulk: Option<bool>,
    pub is_organization_or_service: Option<bool>,
    pub known_apple_contact: Option<bool>,
    pub has_email: bool,
    pub has_phone: bool,
    pub coverage_note: Option<String>,
    pub review_state: String,
    pub match_method: Option<String>,
    pub match_confidence: Option<String>,
    pub canonical_person_id: Option<String>,
    pub match_reason: Option<String>,
    pub rule_version: Option<String>,
    pub evidence_fingerprint: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipEvidenceReview {
    pub rows: Vec<RelationshipEvidenceRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipReconcileResult {
    pub rows: Vec<RelationshipEvidenceRow>,
    pub tally: BTreeMap<String, i64>,
    pub canonical_linked: i64,
}

#[derive(Debug, Clone)]
pub struct RelationshipDecision {
    pub review_state: String,
    pub match_method: String,
    pub match_confidence: String,
    pub canonical_person_id: Option<String>,
    pub reason: String,
    pub rule_version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SupportBreakGlassReadiness {
    pub configured: bool,
    pub enabled: bool,
    pub root_resolvable: bool,
    pub root_active: bool,
    pub owner_role_present: bool,
    pub audit_table_available: bool,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SupportSecurityStatus {
    pub active_internal_users: i64,
    pub external_users: i64,
    pub users_with_no_role: i64,
    pub users_with_multiple_roles: i64,
    pub mapped_auth_identities: i64,
    pub unmapped_app_users: i64,
    pub owner_role_assignments: i64,
    pub inactive_users_with_active_role_mappings: i64,
    pub account_type_mismatch_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SupportSystemHealth {
    pub unresolved_intake_count: i64,
    pub open_task_count: i64,
    pub overdue_task_count: i64,
    pub active_deal_count: i64,
    pub under_contract_count: i64,
    pub active_property_count: i64,
    pub recent_interaction_at_label: Option<String>,
    pub interactions_last7_days: i64,
    pub persons_without_email_identity: i64,
    pub persons_without_phone_identity: i64,
    pub open_tasks_without_due_date: i64,
    pub active_properties_without_hero_media: i64,
    pub completed_showings_missing_completed_at: i64,
    pub scheduled_showings_missing_scheduled_at: i64,
    pub active_participants_with_ended_at: i64,
    pub other_participants_missing_role_label: i64,
    pub offers_with_cross_deal_parent: i64,
    pub showings_with_deal_property_mismatch: i64,
    pub completed_showings_missing_showing_interaction: i64,
    pub inactive_participants_without_ended_at: i64,
    pub public_properties_with_multiple_heroes: i64,
    pub hero_media_not_image: i64,
    pub account_type_mismatch_count: i64,
    pub active_app_users_without_role: i64,
    pub auth_identity_inactive_app_user: i64,
    pub owner_assignments: i64,
    pub multiple_owners: i64,
    pub auth_identity_without_usable_app_user: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowDefinitionSummary {
    pub definition_id: String,
    pub key: String,
    pub version: i32,
    pub name: String,
    pub status: String,
    pub instance_count: i64,
    pub active_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowInstanceSummary {
    pub instance_id: String,
    pub definition_key: String,
    pub definition_version: i32,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub active_token_count: i64,
    pub task_count: i64,
    pub event_count: i64,
    pub property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowAnomaly {
    pub kind: String,
    pub severity: String,
    pub instance_id: Option<String>,
    pub subject_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowDiagnosticsSummary {
    pub definition_count: i64,
    pub instance_total: i64,
    pub instance_active: i64,
    pub instance_completed: i64,
    pub instance_failed: i64,
    pub instance_other: i64,
    pub ready_engine_tasks: i64,
    pub correlated_open_canonical_tasks: i64,
    pub pending_jobs: i64,
    pub pending_receipts: i64,
    pub anomaly_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowDiagnosticsSnapshot {
    pub configured: bool,
    pub summary: WorkflowDiagnosticsSummary,
    pub definitions: Vec<WorkflowDefinitionSummary>,
    pub instances: Vec<WorkflowInstanceSummary>,
    pub anomalies: Vec<WorkflowAnomaly>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowDiagnosticsDetail {
    pub instance: WorkflowInstanceSummary,
    pub variables: Option<serde_json::Value>,
    pub node_labels: std::collections::BTreeMap<String, String>,
    pub tokens: Vec<serde_json::Value>,
    pub tasks: Vec<serde_json::Value>,
    pub jobs: Vec<serde_json::Value>,
    pub events: Vec<serde_json::Value>,
    pub correlations: Vec<serde_json::Value>,
    pub commands: Vec<serde_json::Value>,
}

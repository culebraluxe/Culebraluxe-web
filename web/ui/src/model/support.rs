//! Entitlements and the Support shapes: system health, environment, workflow diagnostics, WhatsApp, security, DB test.

#[allow(unused_imports)]
use super::*;

/// UI-only projection of the effective service grants. A missing projection
/// means controls remain hidden until an authenticated read succeeds.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalEntitlements {
    pub account_type: String,
    pub security_level: String,
    pub is_root: bool,
    pub entitlement_codes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSupportPage {
    /// `/portal/db-test` — the database/client diagnostic the pre-cutover screen showed.
    pub db_test: Option<PortalDbTest>,
    /// `/portal/settings` — the Security landing screen: its counts and the break-glass posture.
    pub security: Option<PortalSecurity>,
    /// `/portal/settings/users` — internal application users and effective primary roles.
    pub security_users: Vec<PortalSecurityUser>,
    /// `/portal/admin/whatsapp-meta` — what Meta says about this deployment's WhatsApp number.
    pub whats_app_meta: Option<PortalWhatsAppMeta>,
    /// `/portal/system-health` — the operational health snapshot, the environment posture, and the workflow diagnostics.
    pub system_health: Option<PortalSystemHealthPage>,
}

/// The system-health screen's whole payload: three reads the pre-cutover page made together.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSystemHealthPage {
    pub health: PortalSystemHealthSnapshot,
    pub environment: PortalEnvironmentReadiness,
    pub diagnostics: PortalWorkflowDiagnostics,
}

/// The operational health snapshot: counts and signals, field for field as `legacy/db/system-health.ts` returns them.
///
/// EVERY FIELD IS A COUNT OR A LABEL about something the application can be wrong about. Nothing here is a name, an address
/// or a credential — the snapshot answers "how is the book", not "who is in it".
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSystemHealthSnapshot {
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

/// Environment and secrets readiness: POSTURE, never values.
///
/// THIRTEEN BOOLEANS. Not one URL, key, token or secret — the projection itself is built that way, and this type carries it
/// through without widening it. "Configured" is the most any of these screens is allowed to know about a secret.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalEnvironmentReadiness {
    pub is_production: bool,
    pub database_configured: bool,
    pub database_dev_prod_separated: bool,
    pub auth_secret_configured: bool,
    pub auth_provider_configured: bool,
    pub break_glass_configured: bool,
    pub break_glass_enabled: bool,
    pub google_maps_key_configured: bool,
    pub google_maps_demo_key_absent_in_production: bool,
    pub mux_configured: bool,
    pub broker_signature_configured: bool,
    pub broker_signature_enabled: bool,
    pub all_production_required_configured: bool,
}

/// The workflow diagnostics snapshot: the counts, the definitions, the instances, and what the anomaly sweep found.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDiagnostics {
    /// Whether the engine's tables are present at all — the projection answers `false` on a database that predates them.
    pub configured: bool,
    pub summary: PortalWorkflowDiagnosticsSummary,
    pub definitions: Vec<PortalWorkflowDefinition>,
    pub instances: Vec<PortalWorkflowInstance>,
    pub anomalies: Vec<PortalWorkflowAnomaly>,
    /// ONE INSTANCE'S DETAIL, loaded when a row is opened and absent until then.
    ///
    /// IT RIDES IN THE SAME PAYLOAD as the list because the screen is one payload: a second route for "the detail of the row
    /// the operator just opened" would be a second shape to keep in step with this one, and the list that arrives with it is
    /// the same list — which is what lets the screen re-render the row it opened without asking for anything else.
    pub detail: Option<PortalWorkflowInstanceDetail>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDiagnosticsSummary {
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

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDefinition {
    pub definition_id: String,
    pub key: String,
    pub version: i64,
    pub name: String,
    pub status: String,
    pub instance_count: i64,
    pub active_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowInstance {
    pub instance_id: String,
    pub definition_key: String,
    pub definition_version: i64,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub active_token_count: i64,
    pub task_count: i64,
    pub event_count: i64,
    /// The property this instance is about, resolved by the projection when the subject names one.
    pub property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowAnomaly {
    pub kind: String,
    /// `info`, `warning` or `critical` — the projection's own vocabulary, rendered as its own pill.
    pub severity: String,
    pub instance_id: Option<String>,
    pub subject_id: Option<String>,
    pub message: String,
}

/// One instance, opened: its tokens, the tasks waiting on them, its jobs, events, task correlations and command receipts.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowInstanceDetail {
    pub instance_id: String,
    pub definition_key: String,
    pub definition_version: i64,
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub active_token_count: i64,
    pub task_count: i64,
    pub event_count: i64,
    /// The instance's variables, as the engine stored them — arbitrary JSON, shown as it is.
    pub variables: Option<serde_json::Value>,
    /// Node ids to their labels, so a token can be named rather than addressed.
    pub node_labels: std::collections::BTreeMap<String, String>,
    pub tokens: Vec<PortalWorkflowToken>,
    pub tasks: Vec<PortalWorkflowTask>,
    pub jobs: Vec<PortalWorkflowJob>,
    pub events: Vec<PortalWorkflowDiagnosticEvent>,
    pub correlations: Vec<PortalWorkflowCorrelation>,
    pub commands: Vec<PortalWorkflowCommand>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowToken {
    pub id: String,
    pub parent_token_id: Option<String>,
    pub node_id: String,
    pub status: String,
    pub outcome: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowTask {
    pub id: String,
    pub token_id: Option<String>,
    pub name: String,
    pub status: String,
    pub candidates: Vec<String>,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowJob {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub due_at: Option<String>,
}

/// One engine event on an instance, as the diagnostics detail shows it.
///
/// NAMED FOR ITS SCREEN: `PortalWorkflowEvent` is the workflows screen's own event shape, and the two are not the same thing
/// — one is a milestone of a definition-driven timeline, this is a row of the engine's log.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDiagnosticEvent {
    pub id: String,
    pub event_type: String,
    pub node_id: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowCorrelation {
    pub workflow_task_id: String,
    pub application_task_id: Option<String>,
    pub application_task_status: Option<String>,
    pub application_task_title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowCommand {
    pub command_id: String,
    pub command_type: String,
    pub node_id: String,
    pub outcome: String,
    pub message: Option<String>,
    pub receipt_outcome: Option<String>,
}

/// Which workflow instance is open on System Health, and what happened when we asked for it.
///
/// THE INTERACTION THE EARLIER CONVERSION DROPPED. Pre-cutover this was React state — `selectedId`, `loadingId`, `detail`,
/// `error` — mutated by `toggleInstance`. It is here because it is state: the same click that expands a row is a message, the
/// read it triggers is an effect, and what comes back is decided in `update` rather than in the component that drew it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowDiagnosticsState {
    /// The instance whose row is open, if any. NOT the same as `loading_instance`: a row can be open with nothing to show yet.
    pub selected_instance: Option<String>,
    /// The instance we have asked for and not yet heard about — what the loading line is keyed on.
    pub loading_instance: Option<String>,
    /// What the read said, when it said something bad. A missing instance is the ordinary case: the row was open, the id no
    /// longer resolves, and the operator is told so in the row rather than in a panel that steals the page.
    pub error: Option<String>,
    /// The open instance's detail, once it has arrived. Held here and not in the payload so the screen reads one place.
    pub detail: Option<PortalWorkflowInstanceDetail>,
}

/// The WhatsApp diagnostic: the four outcomes the pre-cutover screen distinguished, and the phone fields it printed.
///
/// THERE IS NO TOKEN FIELD HERE, and there must never be one. The access token is used inside the bridge and is not part of
/// this shape, so it cannot be serialised into a payload even by a mistake elsewhere.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWhatsAppMeta {
    pub waba_id: String,
    /// Whether a token exists at all — a yes/no about configuration, which is all a diagnostic needs to say.
    pub token_configured: bool,
    /// Meta's own words when it refused, or the absent-token notice, or nothing when the call succeeded.
    pub error: Option<String>,
    pub phones: Vec<PortalWhatsAppPhone>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWhatsAppPhone {
    pub id: Option<String>,
    pub display_phone_number: Option<String>,
    pub verified_name: Option<String>,
    pub quality_rating: Option<String>,
    pub code_verification_status: Option<String>,
}

/// The Security screen: operational counts, and break-glass posture.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSecurity {
    pub status: PortalSecurityStatus,
    pub break_glass: PortalBreakGlassReadiness,
    pub role_entitlements: Vec<PortalRoleEntitlements>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalRoleEntitlements {
    pub role_code: String,
    pub account_type: String,
    pub entitlement_codes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSecurityUser {
    pub app_user_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub account_type: String,
    pub active: bool,
    pub role_codes: Vec<String>,
    pub primary_role_code: Option<String>,
}

/// The nine counts, each named for what it counts. Counts of things, not values of anything.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalSecurityStatus {
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

/// Break-glass posture as six booleans — and nothing else, ever.
///
/// THIS TYPE MUST NOT GROW. The configuration behind it holds the root user's id and secret hash; what crosses is whether
/// each condition holds. A field added here is a field that could carry a secret to a browser, so the type is deliberately
/// six booleans and the payload module builds it field by field.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalBreakGlassReadiness {
    pub configured: bool,
    pub enabled: bool,
    pub root_resolvable: bool,
    pub root_active: bool,
    pub owner_role_present: bool,
    pub audit_table_available: bool,
}

/// The DB Test screen's read: whether the database answered, how many clients it holds, and the identity columns of each.
///
/// SMALLER THAN THE READ BEHIND IT, deliberately. `getClients()` returns a client's budget, preferences, priorities and
/// interests; a diagnostic screen printed the whole record as JSON and had no business carrying any of it.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDbTest {
    /// True when the read answered at all — see the payload: the flag is the absence of a failure, not a hopeful constant.
    pub connected: bool,
    pub client_count: i64,
    pub clients: Vec<PortalDbTestClient>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalDbTestClient {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

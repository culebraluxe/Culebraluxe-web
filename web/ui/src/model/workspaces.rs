//! Cabinet, Publishing, Catch-Up, Cockpit, activity, workflow lists, clients and communications.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCabinetPage {
    pub documents: Vec<PortalCabinetDocument>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCabinetDocument {
    pub id: String,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub document_type_label: Option<String>,
    pub title: Option<String>,
    pub state: String,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalPublishingPage {
    pub listings: Vec<PortalPublishingListing>,
    pub ready_count: i64,
    pub live_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalPublishingListing {
    pub property_id: String,
    pub name: String,
    pub status: String,
    pub slug: Option<String>,
    pub location: Option<String>,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub list_price: Option<String>,
    pub property_type: Option<String>,
    pub image_count: i64,
    pub video_count: i64,
    pub has_hero: bool,
    pub copy_ready: bool,
    pub media_ready: bool,
    pub website_ready: bool,
    pub facebook_ready: bool,
    pub stellar_package_ready: bool,
    pub listing_type: Option<String>,
    pub agent_mls_id: Option<String>,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCatchUpPage {
    pub generated_at: String,
    pub total: i64,
    pub high_priority_count: i64,
    pub items: Vec<PortalCatchUpItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCatchUpItem {
    pub person_id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub reason_code: String,
    pub reason: String,
    pub priority: i32,
    pub signal_at: String,
    pub signal_at_label: String,
    pub last_contact_at: Option<String>,
    pub last_contact_label: Option<String>,
    pub last_contact_channel: Option<String>,
    pub last_contact_direction: Option<String>,
    pub last_contact_summary: Option<String>,
    pub primary_phone: Option<String>,
    pub primary_email: Option<String>,
    pub active_deal_id: Option<String>,
    pub active_property_name: Option<String>,
    pub task_id: Option<String>,
    pub due_at_label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitPage {
    pub active_client_count: i64,
    pub live_deal_count: i64,
    pub upcoming_count: i64,
    pub under_contract_count: i64,
    pub active_workflow_count: i64,
    pub blocked_workflow_count: i64,
    pub overdue_tasks: Vec<PortalCockpitTask>,
    pub tasks_due_soon: Vec<PortalCockpitTask>,
    pub recent_interactions: Vec<PortalCockpitInteraction>,
    pub featured_deal: Option<PortalCockpitDeal>,
    pub pipeline: Vec<PortalCockpitStageCount>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitTask {
    pub id: String,
    pub person_id: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub due_at: Option<String>,
    pub due_at_label: Option<String>,
    pub context_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitInteraction {
    pub id: String,
    pub person_name: String,
    pub channel: String,
    pub occurred_at_label: String,
    pub summary: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitDeal {
    pub id: String,
    pub property_name: String,
    pub hero_media_id: Option<String>,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub closing_date: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCockpitStageCount {
    pub stage: String,
    pub count: i64,
}

/// One line of the unified activity feed, with the fields the live screen renders.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalActivityEntry {
    pub id: String,
    /// `website`, `email`, `call`, `imessage`, `sms`, `meeting`, `showing`, `document`, `manual`, `whatsapp` — labelled
    /// for display by the screen, not here.
    pub channel: String,
    /// Which way it went, when the channel has a direction.
    pub direction: Option<String>,
    /// Already formatted by the read model: the screen shows the label, it does not compute a date.
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    /// The person this line is about, and the key its link uses when there is one.
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    /// The deal this line belongs to, and the property that deal is about — two different names, which is why both are
    /// here.
    pub deal_id: Option<String>,
    pub deal_property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowList {
    pub configured: bool,
    pub items: Vec<PortalWorkflowSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowSummary {
    pub instance_id: String,
    pub workflow_name: String,
    pub workflow_version: i64,
    pub property_name: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub active_milestones: Vec<String>,
    pub open_task_count: i64,
    pub blocker_count: i64,
    pub responsible_party: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowDetail {
    pub instance_id: String,
    pub workflow_name: String,
    pub workflow_version: i64,
    pub property_name: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub responsible_party: Option<String>,
    pub started_at_label: String,
    pub timeline: Vec<PortalWorkflowTimelineItem>,
    pub milestones: Vec<PortalWorkflowMilestone>,
    pub open_task_count: i64,
    pub pending_timer_count: i64,
    pub blockers: Vec<String>,
    pub events: Vec<PortalWorkflowEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowTimelineItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub deadline: Option<String>,
    pub completed: bool,
    pub active: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowMilestone {
    pub id: String,
    pub label: String,
    pub owner: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWorkflowEvent {
    pub id: String,
    pub event_type: String,
    pub node_label: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientsPage {
    pub rows: Vec<PortalClientSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub selected: Option<PortalClientDetail>,
    pub comms: Option<PortalCommsPanel>,
    pub properties: Vec<PortalClientProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientSummary {
    pub id: String,
    pub display_name: String,
    pub name_resolved: bool,
    pub role: String,
    pub status: String,
    pub primary_email: Option<String>,
    pub primary_phone: Option<String>,
    pub observed_count: i64,
    pub two_way: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientDetail {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub budget_min: Option<f64>,
    pub budget_max: Option<f64>,
    pub timeline: Option<String>,
    pub assigned_agent: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsPanel {
    pub person_id: String,
    pub aggregate: PortalCommsAggregate,
    pub sources: Vec<PortalCommsSource>,
    pub moments: Vec<PortalCommsMoment>,
    pub moment_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsAggregate {
    pub observed_count: i64,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub two_way: bool,
    pub first_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub last_contact_at: Option<String>,
    pub last_contact_label: Option<String>,
    pub active_source_count: i64,
    pub source_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsSource {
    pub source: String,
    pub channel: String,
    pub label: String,
    pub total_count: i64,
    pub two_way: bool,
    pub last_context: Option<String>,
    pub last_contact_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalCommsMoment {
    pub id: String,
    pub channel: Option<String>,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientProperty {
    pub id: String,
    pub display_name: String,
    pub relation: String,
    pub relation_status: Option<String>,
    pub address: String,
}

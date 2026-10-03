//! Forms and Projects shapes: forms, templates, fields, signers; projects, work items, documents, calendar.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormsPage {
    pub items: Vec<PortalFormSummary>,
    pub selected: Option<PortalFormRecord>,
    pub template: Option<PortalFormTemplate>,
    pub issued: Option<PortalIssuedFormDocument>,
    pub signers: Vec<PortalFormSigner>,
    pub template_choices: Vec<PortalFormTemplateChoice>,
    /// Working-editor state owned by the reducer, never by Yew hooks.
    pub dirty: bool,
    pub saving: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormSummary {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub template_name: String,
    pub active_version: i32,
    pub status: String,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormRecord {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub template_name: String,
    pub active_version: i32,
    pub status: String,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub updated_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormTemplate {
    pub id: String,
    pub version: i32,
    pub active_version: i32,
    pub display_name: String,
    pub document_type_label: String,
    pub rendering_title: String,
    pub presentation: String,
    pub fields: Vec<PortalFormField>,
    pub sections: Vec<PortalFormSection>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormField {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
    pub options: Vec<String>,
    pub when: Option<PortalFormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormSection {
    pub name: String,
    pub label: String,
    pub editable: bool,
    pub when: Option<PortalFormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormWhen {
    pub field: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalIssuedFormDocument {
    pub document_id: String,
    pub issued_version: i32,
    pub checksum: String,
    pub created_at: String,
    pub media_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormSigner {
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalFormTemplateChoice {
    pub id: String,
    pub display_name: String,
    pub active_version: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectsPage {
    pub projects: Vec<PortalProject>,
    pub items: Vec<PortalProjectWorkItem>,
    pub dependencies: Vec<PortalWbsDependency>,
    pub documents: Vec<PortalProjectDocument>,
    pub media: Vec<PortalProjectMedia>,
    pub activity: Vec<PortalProjectActivity>,
    pub calendar: Vec<PortalProjectCalendarEvent>,
    /// Server-provided date used by the reducer's Today intent; views never read the clock.
    pub calendar_today: String,
    /// UI-only date cursor. Month/week/day/list project from the same value.
    pub calendar_cursor: String,
    /// "month" | "week" | "day" | "list".
    pub calendar_mode: String,
    /// Recurring EventKit edits apply to "this" occurrence or "future" occurrences.
    pub calendar_recurrence_scope: String,
    /// all = Apple schedule + project data; project = only project-linked work/showings.
    pub calendar_filter: String,
    pub calendar_selected_event_id: Option<String>,
    pub calendar_dragging_event_id: Option<String>,
    pub calendar_drag_target: Option<String>,
    pub calendar_loading: bool,
    pub calendar_loaded_start: Option<String>,
    pub calendar_loaded_end: Option<String>,
    /// Server-configurable display preference; timezone identity is deliberately separate.
    pub calendar_day_start_hour: u32,
    pub calendar_day_end_hour: u32,
    pub calendar_slot_minutes: u32,
    /// Native Timeline/Gantt UI state. The view owns no scheduling truth.
    /// "day" | "week" | "month" controls scale density only.
    pub timeline_mode: String,
    /// Empty means canonical WBS order; otherwise title, start or days.
    pub timeline_sort_key: String,
    pub timeline_sort_desc: bool,
    /// UI viewport anchor, independent of stored project dates.
    pub timeline_focus_date: Option<String>,
    /// Target chosen for the selected work item's dependency command.
    pub timeline_link_target_id: Option<String>,
    pub timeline_collapsed_items: BTreeSet<String>,
    pub timeline_dragging_item_id: Option<String>,
    /// "due" or "planned" while the native timeline is dragging.
    pub timeline_drag_kind: String,
    pub timeline_drag_target_date: Option<String>,
    pub identity_names: BTreeMap<String, String>,
    /// Workspace state lives with the payload and changes only in update().
    pub active_domain: String,
    pub selected_project_id: Option<String>,
    pub selected_node_id: Option<String>,
    pub active_view: String,
    pub catch_up: bool,
    /// The bottom selected-work pane is collapsible, matching the mature Projects workspace.
    pub work_collapsed: bool,
    pub work_dirty: bool,
    pub saving: bool,
    /// The Documents tab's filter: `all`, `document`, `photo` or `video` (empty means all).
    pub documents_filter: String,
    /// The document whose signed copy is being recorded, the date it was signed, and whether it is uploading.
    pub signing_document_id: Option<String>,
    pub signing_date: String,
    pub signing_busy: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalWbsDependency {
    pub project_id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProject {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub status: String,
    pub description: String,
    pub areas: Vec<String>,
    pub project_type: Option<String>,
    pub playbook_id: Option<String>,
    pub playbook_version: Option<i32>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectWorkItem {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub category: String,
    pub status: String,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub due_at: Option<String>,
    pub planned_start: Option<String>,
    pub planned_finish: Option<String>,
    pub owner: Option<String>,
    pub order: Option<i32>,
    pub entity: Option<PortalProjectEntity>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectEntity {
    pub entity_type: String,
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectDocument {
    pub id: String,
    pub property_id: Option<String>,
    pub title: String,
    pub state: String,
    pub template_id: Option<String>,
    pub template_version: Option<i32>,
    pub issued_version: Option<i32>,
    pub created_at: String,
    pub signed_artifact_available: bool,
    pub signed_audit_available: bool,
    /// The person the document is for.
    pub party_person_id: Option<String>,
    pub signed_at: Option<String>,
    /// The contract (form) this version belongs to.
    pub form_instance_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectMedia {
    pub id: String,
    pub property_id: String,
    pub media_type: String,
    pub role: String,
    pub sort_order: i32,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub created_at: Option<String>,
    pub url: String,
    /// A film's Mux playback id (videos only).
    pub mux_playback_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectActivity {
    pub id: String,
    pub person_id: Option<String>,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub channel: String,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub occurred_at_label: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    pub deal_property_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalProjectCalendarEvent {
    pub id: String,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub property_name: Option<String>,
    pub kind: String,
    pub source: String,
    pub location: Option<String>,
    pub provider_event_id: Option<String>,
    pub provider_series_id: Option<String>,
    pub recurring: bool,
    pub detached: bool,
}

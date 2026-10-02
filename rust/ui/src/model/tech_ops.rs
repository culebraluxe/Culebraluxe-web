//! The Tech cockpit's and the OPPS Data Workbench's read shapes.

#[allow(unused_imports)]
use super::*;

/// The portal's own page payload: what one portal screen renders, in the screen's real shape.
///
/// WHY THIS EXISTS. A portal screen used to arrive as `RustUiRow { id, cells, badge }` — a generic list with the fields
/// flattened in and the rest thrown away. That is a fine transport for a table and a wrong one for a screen: the
/// Activity feed renders a channel, a direction, a person, a summary and the property or deal a line belongs to, and
/// `cells` keeps none of those as fields. So a screen that is really ported gets a DTO of its own here — the read
/// model's fields, not a column list it has to decode.

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechPage {
    pub ready: bool,
    pub total_stories: i64,
    pub open_count: i64,
    pub backlog_count: i64,
    pub closed_count: i64,
    pub completion_percent: f64,
    pub active_work: Vec<PortalTechStory>,
    pub selected_story: Option<PortalTechStory>,
    pub selected_runs: Vec<PortalTechRun>,
    pub recorder_instance_id: Option<String>,
    pub hold: Option<PortalTechHold>,
    pub sorter_cards: Vec<PortalTechSorterCard>,
    pub sorter_columns: Vec<PortalTechSorterColumn>,
    pub engine_runs: Vec<PortalTechEngineRun>,
    pub queued_cards: Vec<PortalTechQueuedCard>,
    pub engine_read_ok: bool,
    pub queue_read_ok: bool,
    pub ledger: Option<PortalTechLedger>,
    pub staging_flight: Option<PortalTechFlight>,
    pub recent_flights: Vec<PortalTechFlight>,
    pub recent_history: Vec<PortalTechHistory>,
    /// Live Forge execution comes from the parent ForgeService, composed into this page by the server.
    pub live_ops: domain::ForgeLiveSnapshot,
    pub freshness: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechStory {
    pub id: String,
    pub workstream: String,
    pub operating_surface: Option<String>,
    pub title: String,
    pub priority: String,
    pub status: String,
    pub notes: Option<String>,
    pub batch: Option<i64>,
    pub goal: Option<String>,
    pub scope: Option<String>,
    pub dependencies: Option<String>,
    pub preconditions: Option<String>,
    pub architect_brief: Option<String>,
    pub context_refs: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub postconditions: Option<String>,
    pub completion: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechRun {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub result_status: Option<String>,
    pub run_type: Option<String>,
    pub agent_runtime: Option<String>,
    pub completion: Option<f64>,
    pub notes: Option<String>,
    pub commit_hash: Option<String>,
    pub tests_summary: Option<String>,
    pub execution_environment: Option<String>,
    pub run_phase: Option<String>,
    pub lead_decision: Option<String>,
    pub model_used: Option<String>,
    pub cost_widgets: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechHold {
    pub reason: Option<String>,
    pub originating_node: Option<String>,
    pub failure_class: Option<String>,
    pub resume_target: Option<String>,
    pub since: Option<String>,
    pub process_instance_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechSorterCard {
    pub id: String,
    pub column: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub completion: f64,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechSorterColumn {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechEngineRun {
    pub story_id: String,
    pub title: String,
    pub instance_id: String,
    pub last_node: Option<String>,
    pub status: String,
    pub attempts: i64,
    pub at: Option<String>,
    pub stale: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechQueuedCard {
    pub story_id: String,
    pub title: String,
    pub state: String,
    pub since: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechLedger {
    pub total_attempts: i64,
    pub stories: i64,
    pub completed: i64,
    pub failed: i64,
    pub interrupted: i64,
    pub worst_story_id: Option<String>,
    pub worst_attempts: Option<i64>,
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechFlight {
    pub id: String,
    pub label: Option<String>,
    pub status: String,
    pub scheduled_for: Option<String>,
    pub fired_at: Option<String>,
    pub created_at: String,
    pub model_policy: String,
    pub story_count: i64,
    pub queued_count: i64,
    pub skipped_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalTechHistory {
    pub id: String,
    pub title: String,
    pub latest_run_at: Option<String>,
    pub latest_run_result: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsRow {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub status: String,
    pub meta: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsStellar {
    pub listing_contract_date: Option<String>,
    pub expiration_date: Option<String>,
    pub listing_type: Option<String>,
    pub agent_mls_id: Option<String>,
    pub tax_id: Option<String>,
    pub tax_year: Option<String>,
    pub annual_tax: Option<String>,
    pub legal_description: Option<String>,
    pub zoning: Option<String>,
    pub total_area_sqft: Option<String>,
    pub heated_area_source: Option<String>,
    pub ownership_type: Option<String>,
    pub hoa_details: Option<String>,
    pub showing_instructions: Option<String>,
    pub occupant_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsProperty {
    pub id: String,
    pub source_metadata: BTreeMap<String, serde_json::Value>,
    pub regrid_fields: BTreeMap<String, serde_json::Value>,
    pub stellar_property_id: Option<String>,
    pub stellar_updated_at: Option<String>,
    pub name: String,
    pub slug: Option<String>,
    pub status: String,
    pub featured: bool,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub property_type: Option<String>,
    pub has_ocean_view: bool,
    pub has_bay_view: bool,
    pub has_beach_view: bool,
    pub has_harbor_view: bool,
    pub has_island_view: bool,
    pub has_mountain_view: bool,
    pub has_sunrise_view: bool,
    pub has_sunset_view: bool,
    pub has_water_access: bool,
    pub has_beach_access: bool,
    pub has_pool: bool,
    pub has_generator: bool,
    pub has_solar: bool,
    pub is_furnished: bool,
    pub is_gated: bool,
    pub list_price: Option<String>,
    pub original_list_price: Option<String>,
    pub location: Option<String>,
    pub address_line1: Option<String>,
    pub street_number: Option<String>,
    pub street_name: Option<String>,
    pub unit_number: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub iso_country_code: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub bedrooms: Option<String>,
    pub bathrooms: Option<String>,
    pub bathrooms_full: Option<String>,
    pub bathrooms_half: Option<String>,
    pub square_feet: Option<String>,
    pub lot_size: Option<String>,
    pub lot_size_units: Option<String>,
    pub lot_size_acres: Option<String>,
    pub lot_size_sqft: Option<String>,
    pub road_frontage_feet: Option<String>,
    pub road_surface_type: Option<String>,
    pub lot_description: Option<String>,
    pub utilities_notes: Option<String>,
    pub catastro_number: Option<String>,
    pub buildability: Option<String>,
    pub slope_description: Option<String>,
    pub pool_potential: Option<String>,
    pub road_adjacency: Option<String>,
    pub utilities_availability: Option<String>,
    pub hoa_status: Option<String>,
    pub view_description: Option<String>,
    pub year_built: Option<String>,
    pub stories: Option<String>,
    pub parking_spaces: Option<String>,
    pub short_description: Option<String>,
    pub editorial_description: Option<String>,
    pub public_remarks: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub hero_title: Option<String>,
    pub tagline: Option<String>,
    pub architecture_notes: Option<String>,
    pub amenities_notes: Option<String>,
    pub lifestyle_notes: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_office: Option<String>,
    pub legal_owner_name: Option<String>,
    pub listing_identifier: Option<String>,
    pub registry_entry: Option<String>,
    pub finca_number: Option<String>,
    pub registry_section: Option<String>,
    pub seller_person_id: Option<String>,
    pub seller_name: Option<String>,
    pub seller_email: Option<String>,
    pub seller_phone: Option<String>,
    pub seller_location: Option<String>,
    pub archived: bool,
    pub image_count: i64,
    pub video_count: i64,
    pub document_count: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub stellar: PortalOpsStellar,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsPerson {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub company: Option<String>,
    pub location: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// A human fixed this record by hand: the Apple Contacts promotion may add what is missing, but it
    /// does not overwrite these fields (`person.manual_override`, migration 256).
    pub manual_override: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsProject {
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsMediaAsset {
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
    pub mux_asset_id: Option<String>,
    pub mux_playback_id: Option<String>,
    pub duration_seconds: Option<String>,
    pub aspect_ratio: Option<String>,
    pub source_url: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalOpsWorkbenchPage {
    pub entity: String,
    pub rows: Vec<PortalOpsRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub property: Option<PortalOpsProperty>,
    pub person: Option<PortalOpsPerson>,
    pub project: Option<PortalOpsProject>,
    pub media: Vec<PortalOpsMediaAsset>,
}

//! Records, listing media, the storyboard, and the client room.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalRecordProperty {
    pub id: String,
    pub name: String,
    pub status: String,
    pub location: String,
    pub list_price: Option<String>,
    pub slug: Option<String>,
    pub archived: bool,
    pub image_count: i64,
    pub video_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalRecordsPage {
    pub rows: Vec<PortalRecordProperty>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub selected: Option<PortalRecordProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalListingProperty {
    pub id: String,
    pub name: String,
    pub status: String,
    pub slug: Option<String>,
    pub image_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalListingMediaPage {
    pub properties: Vec<PortalListingProperty>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub selected_id: Option<String>,
    pub selected: Option<PortalListingProperty>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardStory {
    pub id: String,
    pub title: String,
    pub priority: String,
    pub status: String,
    pub completion: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardGroup {
    pub group: String,
    pub stories: Vec<PortalStoryboardStory>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardPanel {
    pub bucket: String,
    pub count: i64,
    pub groups: Vec<PortalStoryboardGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardKpis {
    pub total: i64,
    pub open: i64,
    pub backlog: i64,
    pub blocked_hold: i64,
    pub complete: i64,
    pub next_version: i64,
    pub completion_percent: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardPanels {
    pub open: PortalStoryboardPanel,
    pub backlog: PortalStoryboardPanel,
    pub closed: PortalStoryboardPanel,
    pub next_version: PortalStoryboardPanel,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalStoryboardPage {
    pub kpis: PortalStoryboardKpis,
    pub panels: PortalStoryboardPanels,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientRoom {
    pub person_id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub transactions: Vec<PortalClientRoomTransaction>,
    pub projects: Vec<PortalClientRoomProject>,
    pub documents: Vec<PortalClientRoomDocument>,
    pub seller_listings: Vec<PortalClientRoomSellerListing>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientRoomTransaction {
    pub deal_id: String,
    pub property_id: String,
    pub property_name: String,
    pub property_location: Option<String>,
    pub stage: String,
    pub closing_date_label: Option<String>,
    pub next_task: Option<String>,
    pub next_task_due_label: Option<String>,
    pub open_task_count: i64,
    pub showing_count: i64,
    pub offer_count: i64,
    pub latest_offer_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientRoomProject {
    pub id: String,
    pub name: String,
    pub status: String,
    pub property_id: Option<String>,
    pub total_work_items: i64,
    pub completed_work_items: i64,
    pub progress_percent: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientRoomSellerListing {
    pub property_id: String,
    pub name: String,
    pub location: Option<String>,
    pub status: String,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub list_price: Option<String>,
    pub image_count: i64,
    pub video_count: i64,
    pub showing_count: i64,
    pub offer_count: i64,
    pub latest_deal_stage: Option<String>,
    pub project_name: Option<String>,
    pub project_status: Option<String>,
    pub total_work_items: i64,
    pub completed_work_items: i64,
    pub progress_percent: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortalClientRoomDocument {
    pub id: String,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub title: String,
    pub state: String,
    pub created_at_label: String,
    pub signed_artifact_available: bool,
}

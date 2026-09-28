use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientRoomSnapshot {
    pub person_id: String,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub transactions: Vec<ClientRoomTransaction>,
    pub projects: Vec<ClientRoomProject>,
    pub documents: Vec<ClientRoomDocument>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientRoomTransaction {
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientRoomProject {
    pub id: String,
    pub name: String,
    pub status: String,
    pub property_id: Option<String>,
    pub total_work_items: i64,
    pub completed_work_items: i64,
    pub progress_percent: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientRoomDocument {
    pub id: String,
    pub deal_id: Option<String>,
    pub property_id: Option<String>,
    pub title: String,
    pub state: String,
    pub created_at_label: String,
    pub signed_artifact_available: bool,
}

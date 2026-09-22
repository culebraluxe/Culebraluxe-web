use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CockpitTask {
    pub id: String,
    pub person_id: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub due_at: Option<String>,
    pub due_at_label: Option<String>,
    pub context_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CockpitInteraction {
    pub id: String,
    pub person_name: String,
    pub channel: String,
    pub occurred_at_label: String,
    pub summary: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CockpitDeal {
    pub id: String,
    pub property_name: String,
    pub hero_media_id: Option<String>,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub closing_date: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CockpitStageCount {
    pub stage: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CockpitSnapshot {
    pub active_client_count: i64,
    pub live_deal_count: i64,
    pub upcoming_count: i64,
    pub under_contract_count: i64,
    pub active_workflow_count: i64,
    pub blocked_workflow_count: i64,
    pub overdue_tasks: Vec<CockpitTask>,
    pub tasks_due_soon: Vec<CockpitTask>,
    pub recent_interactions: Vec<CockpitInteraction>,
    pub featured_deal: Option<CockpitDeal>,
    pub pipeline: Vec<CockpitStageCount>,
}

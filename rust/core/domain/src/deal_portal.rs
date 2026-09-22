use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealPortfolioSnapshot {
    pub deals: Vec<DealPortfolioItem>,
    pub contracts: Vec<DealContractPortfolioItem>,
    pub properties: Vec<DealableProperty>,
    pub users: Vec<DealOwnerCandidate>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealPortfolioItem {
    pub id: String,
    pub property_id: String,
    pub property_name: String,
    pub property_location: String,
    pub property_descriptor: Option<String>,
    pub hero_media_id: Option<String>,
    pub client_id: String,
    pub client_name: String,
    pub stage: String,
    pub list_price: Option<f64>,
    pub offer_price: Option<f64>,
    pub owner: String,
    pub closing_date: Option<String>,
    pub next_milestone: Option<String>,
    pub next_milestone_at: Option<String>,
    pub last_activity: Option<String>,
    pub last_activity_at: Option<String>,
    pub showing_count: i64,
    pub offer_count: i64,
    pub participant_count: i64,
    pub latest_offer_amount: Option<f64>,
    pub latest_offer_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealContractPortfolioItem {
    pub id: String,
    pub form_template_id: String,
    pub contract_type: String,
    pub property_id: String,
    pub property_label: Option<String>,
    pub status: String,
    pub process_instance_id: Option<String>,
    pub executed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealableProperty {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DealOwnerCandidate {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateDealRequest {
    pub property_id: String,
    pub client_person_id: String,
    pub owner_user_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateDealResult {
    pub id: String,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteIntakeRequest {
    pub submission_id: String,
    pub request_type: String,
    pub property_id: Option<String>,
    pub display_name: String,
    pub email: String,
    pub message: Option<String>,
    pub service: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteIntakeResult {
    pub accepted: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchupLeadRequest {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchupLeadResult {
    pub status: String,
    pub person_id: Option<String>,
    pub interaction_id: Option<String>,
}

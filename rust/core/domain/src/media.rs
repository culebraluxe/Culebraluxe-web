use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
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
}

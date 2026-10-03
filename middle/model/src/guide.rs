//! Public Island Guide read model.
//!
//! The guide is published website content, not marketing copy and not property data. It has its own domain type so
//! neither the Yew page nor the HTTP adapter has to know the guide_item schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideItem {
    pub slug: String,
    pub section: String,
    pub name: String,
    pub eyebrow: Option<String>,
    pub subtitle: Option<String>,
    pub area: Option<String>,
    pub description: String,
    pub note: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub website_url: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub sort_order: i32,
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
}

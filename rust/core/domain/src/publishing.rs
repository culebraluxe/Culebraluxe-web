use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublishingSnapshot {
    pub listings: Vec<PublishingListing>,
    pub ready_count: i64,
    pub live_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublishingListing {
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

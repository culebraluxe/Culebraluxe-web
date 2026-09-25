use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketingContentItem {
    pub key: String,
    pub label: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketingContentBlock {
    pub id: String,
    pub kind: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub eyebrow: Option<String>,
    pub body: Option<String>,
    pub cta_label: Option<String>,
    pub cta_href: Option<String>,
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
    pub items: Vec<MarketingContentItem>,
}

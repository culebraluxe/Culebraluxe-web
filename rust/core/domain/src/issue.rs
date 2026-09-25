use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueQueueRow {
    pub id: String,
    pub issue_type: String,
    pub severity: String,
    pub state: String,
    pub title: String,
    pub detail: Option<String>,
    pub domain_type: String,
    pub domain_id: String,
    pub detected_at: String,
    pub resolved_at: Option<String>,
    pub related_deal_id: Option<String>,
    pub property_name: Option<String>,
    pub client_name: Option<String>,
    pub closing_date: Option<String>,
    pub deal_stage: Option<String>,
    pub task_title: Option<String>,
    pub task_due_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuesPage {
    pub rows: Vec<IssueQueueRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub scope: String,
    pub state: String,
}

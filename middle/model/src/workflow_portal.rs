use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowPortalList {
    pub configured: bool,
    pub items: Vec<WorkflowPortalSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowPortalSummary {
    pub instance_id: String,
    pub workflow_name: String,
    pub workflow_version: i64,
    pub property_name: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub active_milestones: Vec<String>,
    pub open_task_count: i64,
    pub blocker_count: i64,
    pub responsible_party: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowPortalDetail {
    pub instance_id: String,
    pub workflow_name: String,
    pub workflow_version: i64,
    pub property_name: Option<String>,
    pub status: String,
    pub outcome: Option<String>,
    pub responsible_party: Option<String>,
    pub started_at_label: String,
    pub timeline: Vec<WorkflowPortalTimelineItem>,
    pub milestones: Vec<WorkflowPortalMilestone>,
    pub open_task_count: i64,
    pub pending_timer_count: i64,
    pub blockers: Vec<String>,
    pub events: Vec<WorkflowPortalEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowPortalTimelineItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub deadline: Option<String>,
    pub completed: bool,
    pub active: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowPortalMilestone {
    pub id: String,
    pub label: String,
    pub owner: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkflowPortalEvent {
    pub id: String,
    pub event_type: String,
    pub node_label: Option<String>,
    pub actor: Option<String>,
}

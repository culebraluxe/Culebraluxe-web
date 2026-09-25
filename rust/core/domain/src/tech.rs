use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechCockpitSnapshot {
    pub stories: Vec<Value>,
    pub executions: Vec<Value>,
    pub active_work: Vec<Value>,
    pub engine_runs: Vec<Value>,
    pub queued_cards: Vec<Value>,
    pub ledger: Option<Value>,
    pub recent_flights: Vec<Value>,
    pub staging_flight: Option<Value>,
    pub staging_items: Vec<Value>,
    pub selected_runs: Vec<Value>,
    pub recorder_instance_id: Option<String>,
    pub hold: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechCommandRequest {
    pub action: String,
    pub story_id: Option<String>,
    pub stop_after: Option<String>,
    pub scheduled_for: Option<String>,
    pub batch_id: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechCommandResult {
    pub ok: bool,
    pub message: String,
}

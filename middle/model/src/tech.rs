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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeLiveSnapshot {
    pub active_work: Vec<ForgeLiveWorkItem>,
    pub work_status: Vec<ForgeLiveWorkItem>,
    pub selected_story_id: Option<String>,
    pub current_run: Option<ForgeLiveRun>,
    pub node_activity: Vec<ForgeLiveNodeActivity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeLiveWorkItem {
    pub work_item_id: String,
    pub story_id: String,
    pub title: String,
    pub state: String,
    /// Operator-facing terminal bucket. Present only for the Engine Work Status feed:
    /// Done, Error, or Retry. Retry is a cleared engine-fault claim returned to Ready
    /// after a Story Run was already opened; a fresh Ready item is queue work, not Retry.
    pub status_bucket: Option<String>,
    pub kind: Option<String>,
    pub model_policy: Option<String>,
    pub claimed_by: Option<String>,
    pub error_text: Option<String>,
    pub queued_at: Option<String>,
    pub started_at: Option<String>,
    pub updated_at: Option<String>,
    pub finished_at: Option<String>,
    pub story_run_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeLiveRun {
    pub id: String,
    pub story_id: String,
    pub run_type: Option<String>,
    pub run_phase: Option<String>,
    pub agent_runtime: Option<String>,
    pub model_used: Option<String>,
    pub result_status: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub commit_hash: Option<String>,
    pub tests_summary: Option<String>,
    pub completion: Option<f64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub cost_usd: Option<f64>,
    pub cost_source: Option<String>,
    pub notes: Option<String>,
    pub evidence_detail: Option<String>,
    pub vendor_session_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeLiveNodeActivity {
    pub process_instance_id: String,
    pub node_id: String,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechCommandRequest {
    pub action: String,
    pub story_id: Option<String>,
    pub stop_after: Option<String>,
    pub launch_intent: Option<String>,
    pub scheduled_for: Option<String>,
    pub label: Option<String>,
    pub batch_id: Option<String>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechCommandResult {
    pub ok: bool,
    pub message: String,
    #[serde(default)]
    pub data: Value,
}

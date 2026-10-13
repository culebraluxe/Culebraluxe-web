use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Canonical Flight Recorder transaction contract.
///
/// The React console already consumes this JSON shape through the bounded island. Keeping this DTO identical lets the
/// read path move from TypeScript to Rust without coupling the visualization to the migration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderTransaction {
    pub transaction: FlightRecorderTransactionContext,
    pub workflows: Vec<FlightRecorderWorkflow>,
    pub events: Vec<FlightRecorderEvent>,
    pub instances: Option<FlightRecorderInstanceWindow>,
    /// The Forge Story Run behind this instance, when there is one. `None` means this instance is not a Forge run:
    /// a different statement from a run that recorded no turn, and the console prints them differently.
    pub story_run: Option<FlightRecorderStoryRun>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderTransactionContext {
    pub deal_id: Option<String>,
    pub property: Option<String>,
    pub client: Option<String>,
    pub correlation_id: Option<String>,
    pub status: Option<String>,
    pub initiated_by: Option<String>,
    pub initiated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderWorkflow {
    pub workflow_instance_id: String,
    pub definition_id: Option<String>,
    pub definition_key: Option<String>,
    pub definition_version: Option<i64>,
    pub definition_missing: bool,
    pub status: Option<String>,
    pub current_node_id: Option<String>,
    pub graph: Value,
    pub node_states: BTreeMap<String, FlightRecorderNodeRuntime>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderNodeRuntime {
    pub node_id: String,
    pub state: String,
    pub execution_count: i64,
    pub entered_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub last_outcome: Option<String>,
    pub trigger_event_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderEvent {
    pub event_id: String,
    pub occurred_at: String,
    pub event_type: String,
    pub source_system: String,
    pub summary: Option<String>,
    pub outcome: Option<String>,
    pub duration_ms: Option<i64>,
    pub trace_id: Option<String>,
    pub correlation_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub workflow_node_id: Option<String>,
    pub causation_id: Option<String>,
    pub command_id: Option<String>,
    pub domain_event_id: Option<String>,
    pub document_id: Option<String>,
    pub signature_request_id: Option<String>,
    pub metadata: Option<Value>,
    pub mapped_workflow_node: Option<FlightRecorderMappedNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderMappedNode {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub node_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderInstanceWindow {
    pub shown: i64,
    pub total: i64,
}

/// The Forge Story Run behind the process instance being read, with the turns recorded against it.
///
/// WHAT THE CONSOLE IS BEING TOLD. A process instance is the Flight Recorder's key, but a *turn* — one role's model
/// execution, recorded in `storyboard_run_trajectory` — belongs to a Story Run. This is the join between the two, and
/// it exists because the run has to be nameable even when it recorded no turn: an operator looking at a silent run
/// needs to know the run is Forge's and simply recorded nothing, not that the page is pointed at the wrong thing.
///
/// UNKNOWN IS NOT ZERO, and the shape keeps all three states apart to the screen. `usage_known` is `Some(true)` only
/// when the token and cost numbers beside it are the reading the harness reported for THAT turn, `Some(false)` when
/// the turn ended without a reading, and `None` when the row carries no reading at all (it predates turn-keyed rows,
/// or the path that wrote it captured none). A turn nobody measured is not a free turn.
///
/// The QA verdict travels RAW: `qa_passed` beside `qa_verified_sha`. It grades this run only when `qa_verified_sha`
/// names the candidate a turn produced, and that judgement is the reader's, because QA can run after the turn was
/// written — or never.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderStoryRun {
    pub run_id: String,
    pub story_id: String,
    pub phase: Option<String>,
    pub status: Option<String>,
    /// The run's accumulated spend. `None` is "no measured spend was recorded at all"; a value is a FLOOR when a turn
    /// of the run went unpriced, because that turn's spend is not in it. A reader may not present it as the run's cost.
    pub cost_usd: Option<f64>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    /// Where the run's number came from: `none` | `widgets` | `vendor`.
    pub cost_source: Option<String>,
    pub work_type: Option<String>,
    pub last_failure: Option<String>,
    pub qa_passed: Option<bool>,
    pub qa_verified_sha: Option<String>,
    /// True when the run has more recorded turns than this read returned; the list is then the OLDEST turns, never a
    /// silent sample. A truncated list must say so, or a count of turns becomes a claim about the run.
    pub turns_truncated: bool,
    pub turns: Vec<FlightRecorderTurn>,
}

/// One recorded turn of a Story Run: what the turn was, what it did, and what was measured of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FlightRecorderTurn {
    /// The turn's stable execution key within its run. `legacy:<profile>` on rows written before turns were keyed:
    /// a marker that the row predates turn identity, never a claimed attempt key.
    pub attempt_key: String,
    pub workflow_node: Option<String>,
    pub role: Option<String>,
    /// Story type and execution phase are joined from the run for each turn, so readers can segment turns without
    /// inferring them from a node name or from unrelated runs.
    pub work_type: Option<String>,
    pub execution_phase: Option<String>,
    /// 0 is the role's first turn; N>0 is a repair turn for the same role.
    pub role_attempt: Option<i32>,
    pub harness_profile_id: String,
    pub profile_version: Option<i32>,
    pub model: Option<String>,
    pub provider: Option<String>,
    /// The paid model generation this turn belongs to; `None` when no durable attempt controller owned it.
    pub generation_id: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    /// `completed`, `failed`, `cancelled` (interrupted) or `uncertain` (engine fault); `None` on pre-293 rows.
    pub outcome: Option<String>,
    pub first_edit_latency_ms: Option<i64>,
    pub tool_call_count: i64,
    pub edit_call_count: i64,
    pub shell_call_count: i64,
    pub structured_call_count: i64,
    pub verification_calls_after_last_edit: i64,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub cost_usd: Option<f64>,
    pub usage_known: Option<bool>,
    /// The candidate this turn produced, when it produced one: the SHA a QA verdict must name to be this turn's.
    pub candidate_sha: Option<String>,
}

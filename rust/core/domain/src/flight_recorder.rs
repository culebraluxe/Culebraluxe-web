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

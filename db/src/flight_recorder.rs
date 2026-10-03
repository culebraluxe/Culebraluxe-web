use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use model::{
    FlightRecorderEvent, FlightRecorderInstanceWindow, FlightRecorderMappedNode,
    FlightRecorderNodeRuntime, FlightRecorderTransaction, FlightRecorderTransactionContext,
    FlightRecorderWorkflow,
};
use futures_util::future::try_join_all;
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, DbFailure, DbResult};
mod node_states;
mod sibling_limit;
#[allow(unused_imports)]
pub use node_states::*;
#[allow(unused_imports)]
pub use sibling_limit::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, node: Option<&str>, at: &str, outcome: Option<&str>) -> TraceRow {
        TraceRow {
            id: Some(format!("{kind}-{at}")),
            event_type: kind.into(),
            system: "workflow".into(),
            occurred_at: DateTime::parse_from_rfc3339(at)
                .unwrap()
                .with_timezone(&Utc),
            duration_ms: None,
            outcome: outcome.map(str::to_owned),
            trace_id: None,
            correlation_id: None,
            causation_id: None,
            workflow_instance_id: Some("instance".into()),
            workflow_node_id: node.map(str::to_owned),
            command_id: None,
            domain_event_id: None,
            transaction_document_id: None,
            signature_request_id: None,
            summary: None,
            metadata: None,
            source_event_id: None,
        }
    }

    #[test]
    fn reconstructs_current_and_completed_nodes() {
        let graph = json!({
            "nodes": {
                "a": {"id":"a","type":"task","name":"A"},
                "b": {"id":"b","type":"task","name":"B"}
            }
        });
        let events = vec![
            row("NODE_ENTERED", Some("a"), "2026-09-22T12:00:00Z", None),
            row(
                "NODE_COMPLETED",
                Some("a"),
                "2026-09-22T12:01:00Z",
                Some("SUCCESS"),
            ),
            row("NODE_ENTERED", Some("b"), "2026-09-22T12:02:00Z", None),
        ];
        assert_eq!(current_node(&events).as_deref(), Some("b"));
        let states = node_states(&graph, &events, Some("b"));
        assert_eq!(states["a"].state, "COMPLETED");
        assert_eq!(states["b"].state, "CURRENT");
        assert_eq!(states["a"].duration_ms, Some(60_000));
    }

    #[test]
    fn reconstructs_forge_run_attempts() {
        let graph = json!({
            "nodes": {
                "architect": {"id":"architect","type":"task","name":"Architect"},
                "smith": {"id":"smith","type":"task","name":"Smith"}
            }
        });
        let mut start = row("RUN_START", Some("architect"), "2026-09-22T12:00:00Z", None);
        start.metadata = Some(json!({"attempt": 1}));
        let mut end = row(
            "RUN_END",
            Some("architect"),
            "2026-09-22T12:01:00Z",
            Some("allow"),
        );
        end.metadata = Some(json!({"attempt": 1, "status":"completed"}));
        let mut smith = row("RUN_START", Some("smith"), "2026-09-22T12:02:00Z", None);
        smith.metadata = Some(json!({"attempt": 1}));
        let events = vec![start, end, smith];

        assert_eq!(current_node(&events).as_deref(), Some("smith"));
        let states = node_states(&graph, &events, Some("smith"));
        assert_eq!(states["architect"].state, "COMPLETED");
        assert_eq!(states["architect"].execution_count, 1);
        assert_eq!(states["smith"].state, "CURRENT");
    }

    #[test]
    fn forge_run_pairs_by_attempt_even_when_observer_rows_are_out_of_order() {
        let graph = json!({"nodes":{"lead":{"id":"lead","type":"task","name":"Lead"}}});
        let mut end = row(
            "RUN_END",
            Some("lead"),
            "2026-09-22T12:00:00Z",
            Some("allow"),
        );
        end.metadata = Some(json!({"attempt": 1, "status":"completed"}));
        let mut start = row("RUN_START", Some("lead"), "2026-09-22T12:00:01Z", None);
        start.metadata = Some(json!({"attempt": 1}));
        let events = vec![end, start];

        assert_eq!(current_node(&events), None);
        let states = node_states(&graph, &events, None);
        assert_eq!(states["lead"].state, "COMPLETED");
    }

    #[test]
    fn interrupted_forge_run_is_failure_evidence() {
        let graph = json!({"nodes":{"qa":{"id":"qa","type":"task","name":"QA"}}});
        let mut start = row("RUN_START", Some("qa"), "2026-09-22T12:00:00Z", None);
        start.metadata = Some(json!({"attempt": 1}));
        let mut end = row("RUN_END", Some("qa"), "2026-09-22T12:00:30Z", Some("watch"));
        end.metadata = Some(json!({"attempt": 1, "status":"interrupted"}));
        let events = vec![start, end];

        // An interrupted attempt is terminal evidence for that attempt, not a phantom CURRENT node.
        assert_eq!(current_node(&events), None);
        let states = node_states(&graph, &events, None);
        assert_eq!(states["qa"].state, "FAILED");
    }

    #[test]
    fn completed_workflow_has_no_current_node() {
        let events = vec![
            row("NODE_ENTERED", Some("end"), "2026-09-22T12:00:00Z", None),
            row(
                "WORKFLOW_COMPLETED",
                None,
                "2026-09-22T12:01:00Z",
                Some("SUCCESS"),
            ),
        ];
        assert_eq!(current_node(&events), None);
    }

    #[test]
    fn rust_engine_event_vocabulary_reconstructs_task_state() {
        let graph = json!({
            "nodes": {
                "start": {"id":"start","type":"start","name":"Start"},
                "smith": {"id":"smith","type":"task","name":"Smith"}
            }
        });
        let events = vec![
            row(
                "process.started",
                Some("start"),
                "2026-09-22T12:00:00Z",
                None,
            ),
            row("token.moved", Some("smith"), "2026-09-22T12:00:01Z", None),
            row("task.created", Some("smith"), "2026-09-22T12:00:01Z", None),
        ];
        assert_eq!(current_node(&events).as_deref(), Some("smith"));
        let states = node_states(&graph, &events, Some("smith"));
        assert_eq!(states["smith"].state, "CURRENT");
    }

    #[test]
    fn rust_engine_process_completion_has_no_current_node() {
        let events = vec![
            row("task.created", Some("qa"), "2026-09-22T12:00:00Z", None),
            row("task.completed", Some("qa"), "2026-09-22T12:01:00Z", None),
            row("process.completed", None, "2026-09-22T12:01:01Z", None),
        ];
        assert_eq!(current_node(&events), None);
    }
}

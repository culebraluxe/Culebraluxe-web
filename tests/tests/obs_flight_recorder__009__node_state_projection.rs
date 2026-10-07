//! OBS.flight_recorder — node state projection (TST-OBS-FLIGHT-RECORDER-009).
//!
//! Contract: the console workflow view projects the persisted `nodeStates` runtime map onto the
//! exact graph nodes production stored — recorded states surface verbatim, a graph node with no
//! runtime entry reads `NOT_VISITED` (never a guess, never a panic), and a runtime entry for an
//! id outside the graph never invents a phantom node.
//!
//! Level: L2 Persistence — the production read-model in `ui::flight_recorder` (the same adapter
//! the TECH trace console renders), no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test obs_flight_recorder__009__node_state_projection

use model::FlightRecorderTransaction;
use serde_json::json;
use ui::flight_recorder::{self, FlightRecorderTrace};

fn transaction(value: serde_json::Value) -> FlightRecorderTransaction {
    serde_json::from_value(value).expect("the fixture is a valid transaction")
}

fn trace() -> FlightRecorderTrace {
    flight_recorder::adapt_flight_recorder_transaction(&transaction(json!({
        "transaction": {"correlationId": "projection-009"},
        "workflows": [{
            "workflowInstanceId": "wf-9",
            "definitionKey": "listing",
            "definitionVersion": 3,
            "currentNodeId": "b",
            "graph": {"nodes": {
                "a": {"id":"a","type":"task","name":"Draft"},
                "b": {"id":"b","type":"task","name":"Review"},
                "c": {"id":"c","type":"task","name":"Publish"}
            }},
            "nodeStates": {
                "a": {"nodeId":"a","state":"COMPLETED","executionCount":1},
                "b": {"nodeId":"b","state":"CURRENT","executionCount":2},
                "zzz": {"nodeId":"zzz","state":"COMPLETED","executionCount":1}
            }
        }],
        "events": [
            {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer"}
        ]
    })))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-OBS-FLIGHT-RECORDER-009).
fn obs_flight_recorder_009__node_state_projection() {
    let trace = trace();
    let workflow = trace
        .workflow
        .as_ref()
        .expect("the workflow view is carried through");
    assert_eq!(workflow.workflow_instance_id, "wf-9");
    assert_eq!(workflow.current_node_id.as_deref(), Some("b"));

    // Recorded runtime states surface verbatim on their graph nodes.
    let state_of = |id: &str| {
        workflow
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("graph node {id} is projected"))
            .state
            .clone()
    };
    assert_eq!(state_of("a"), "COMPLETED");
    assert_eq!(state_of("b"), "CURRENT");

    // A graph node with no runtime entry is honestly unvisited — never a fabricated state.
    assert_eq!(
        state_of("c"),
        "NOT_VISITED",
        "an unrecorded node reads NOT_VISITED instead of a guess"
    );

    // A runtime entry for an id outside the stored graph invents nothing.
    assert_eq!(
        workflow.nodes.len(),
        3,
        "only the persisted graph nodes are projected"
    );
    assert!(
        workflow.nodes.iter().all(|node| node.id != "zzz"),
        "a runtime entry without a graph node never becomes a phantom node"
    );

    // Node identity and names still come from the graph, not from the runtime map.
    let review = workflow.nodes.iter().find(|node| node.id == "b").unwrap();
    assert_eq!(review.name, "Review");
}

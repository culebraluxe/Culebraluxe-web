//! OBS.flight_recorder — sibling limits (TST-OBS-FLIGHT-RECORDER-010).
//!
//! Contract: a fan-out — one cause with several sibling effects — keeps every sibling: all N
//! edges survive, no sibling is collapsed into another, and the layered layout gives each
//! sibling its own row with a canvas tall enough to hold them all. The control case proves the
//! rule is about siblings: a linear same-subsystem chain still collapses to one node.
//!
//! Level: L2 Persistence — the production read-model in `ui::flight_recorder` (the same DAG and
//! layout the TECH trace console renders), no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test obs_flight_recorder__010__sibling_limits

use model::FlightRecorderTransaction;
use serde_json::json;
use ui::flight_recorder::{self, FlightRecorderTrace};

fn transaction(value: serde_json::Value) -> FlightRecorderTransaction {
    serde_json::from_value(value).expect("the fixture is a valid transaction")
}

/// One cause with three sibling effects, each its own source key.
fn fanout() -> FlightRecorderTrace {
    flight_recorder::adapt_flight_recorder_transaction(&transaction(json!({
        "transaction": {"correlationId": "siblings-010"},
        "workflows": [],
        "events": [
            {"eventId":"e0","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer"},
            {"eventId":"e1","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"DOCUMENT_CREATED","sourceSystem":"postgres","causationId":"e0"},
            {"eventId":"e2","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"DOCUMENT_UPDATED","sourceSystem":"postgres","causationId":"e0"},
            {"eventId":"e3","occurredAt":"2026-09-22T12:00:03.000Z","eventType":"DOCUMENT_UPDATED","sourceSystem":"postgres","causationId":"e0"}
        ]
    })))
}

/// A linear same-subsystem chain: the control case that MUST collapse.
fn chain() -> FlightRecorderTrace {
    flight_recorder::adapt_flight_recorder_transaction(&transaction(json!({
        "transaction": {"correlationId": "chain-010"},
        "workflows": [],
        "events": [
            {"eventId":"e1","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"DOCUMENT_CREATED","sourceSystem":"postgres"},
            {"eventId":"e2","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"DOCUMENT_UPDATED","sourceSystem":"postgres","causationId":"e1"},
            {"eventId":"e3","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"DOCUMENT_UPDATED","sourceSystem":"postgres","causationId":"e2"}
        ]
    })))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-OBS-FLIGHT-RECORDER-010).
fn obs_flight_recorder_010__sibling_limits() {
    let trace = fanout();
    let graph = flight_recorder::build_causal_graph(&trace.events);

    // Every sibling survives as its own node: the fan-out is never thinned.
    assert_eq!(
        graph.nodes.len(),
        4,
        "the cause plus all three siblings, got {:?}",
        graph.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
    for id in ["evt:e1", "evt:e2", "evt:e3"] {
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("sibling {id} keeps its own node"));
        assert_eq!(node.count, 1, "no sibling absorbs another");
    }

    // Every sibling edge survives: one cause, three effects, three edges.
    assert_eq!(
        graph.edges.len(),
        3,
        "a fan-out keeps every sibling edge, got {:?}",
        graph.edges
    );
    for id in ["evt:e1", "evt:e2", "evt:e3"] {
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.source == "evt:e0" && edge.target == id),
            "the cause links to sibling {id}"
        );
    }

    // The layout seats every sibling on its own row with room for all of them.
    let layout = flight_recorder::layout_causal_graph(&graph);
    assert_eq!(layout.nodes.len(), 4);
    let rows: Vec<f64> = layout
        .nodes
        .iter()
        .filter(|node| node.node.id != "evt:e0")
        .map(|node| node.y)
        .collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows[0] != rows[1] && rows[1] != rows[2] && rows[0] != rows[2],
        "siblings never share a row, got {rows:?}"
    );
    let single = flight_recorder::layout_causal_graph(&flight_recorder::build_causal_graph(
        &chain().events[..1],
    ));
    assert!(
        layout.height > single.height,
        "the canvas grows with the sibling count instead of clipping them"
    );

    // Control: a linear same-subsystem chain is NOT siblings, so it still collapses.
    let collapsed = flight_recorder::build_causal_graph(&chain().events);
    assert_eq!(
        collapsed.nodes.len(),
        1,
        "the linear chain fuses while the fan-out does not"
    );
    assert_eq!(collapsed.nodes[0].count, 3);
    assert!(collapsed.edges.is_empty());
}

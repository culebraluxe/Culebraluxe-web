//! OBS.flight_recorder — dedupe source-event key (TST-OBS-FLIGHT-RECORDER-008).
//!
//! Contract: the causal DAG groups events by their source identity key — stages that share one
//! `commandId` fuse into a single `cmd:<id>` node — while a `DOMAIN_EVENT_EMITTED` row keys on
//! its OWN `domainEventId` (`dom:<id>`) even when it carries the causing command's id, so it is
//! never fused into the command node. Duplicate causation references resolve to one edge.
//!
//! Level: L2 Persistence — the production read-model in `ui::flight_recorder` (the same adapter
//! the TECH trace console renders), no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test obs_flight_recorder__008__dedupe_source_event_key

use std::collections::HashSet;

use model::FlightRecorderTransaction;
use serde_json::json;
use ui::flight_recorder::{self, FlightRecorderTrace};

fn transaction(value: serde_json::Value) -> FlightRecorderTransaction {
    serde_json::from_value(value).expect("the fixture is a valid transaction")
}

fn trace() -> FlightRecorderTrace {
    flight_recorder::adapt_flight_recorder_transaction(&transaction(json!({
        "transaction": {"correlationId": "dedup-008"},
        "workflows": [],
        "events": [
            {"eventId":"e0","occurredAt":"2026-09-22T12:00:00.000Z","eventType":"RUN_START","sourceSystem":"forge_observer"},
            {"eventId":"e1","occurredAt":"2026-09-22T12:00:01.000Z","eventType":"COMMAND_REQUESTED","sourceSystem":"command","commandId":"cmd-7","causationId":"e0","summary":"Command SaveDraft requested"},
            {"eventId":"e2","occurredAt":"2026-09-22T12:00:02.000Z","eventType":"COMMAND_COMPLETED","sourceSystem":"command","commandId":"cmd-7","causationId":"e0","summary":"Command SaveDraft completed"},
            {"eventId":"e3","occurredAt":"2026-09-22T12:00:03.000Z","eventType":"DOMAIN_EVENT_EMITTED","sourceSystem":"domain","commandId":"cmd-7","domainEventId":"dom-9","causationId":"cmd-7","summary":"Domain event DraftSaved"},
            // Deliberate tag collision: type and source normalize identically, so without the
            // adapter's dedupe the tag list would carry the value twice.
            {"eventId":"e4","occurredAt":"2026-09-22T12:00:04.000Z","eventType":"hold","sourceSystem":"hold"}
        ]
    })))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-OBS-FLIGHT-RECORDER-008).
fn obs_flight_recorder_008__dedupe_source_event_key() {
    let trace = trace();
    let graph = flight_recorder::build_causal_graph(&trace.events);

    // The two command stages share one source key, so they fuse into one node.
    let command = graph
        .nodes
        .iter()
        .find(|node| node.id == "cmd:cmd-7")
        .expect("the shared command key fuses into one node");
    assert_eq!(
        command.count, 2,
        "both command stages live in the fused node"
    );

    // The domain row carries the causing command's id but keys on its OWN domain event id:
    // fusing it into the command node would silently erase a persisted fact.
    let domain = graph
        .nodes
        .iter()
        .find(|node| node.id == "dom:dom-9")
        .expect("a domain row is keyed on its own domain event id, never fused into its cause");
    assert_eq!(domain.count, 1);

    // e0 and the tag-collision probe each keep their own event-keyed node.
    assert!(
        graph.nodes.iter().any(|node| node.id == "evt:e0"),
        "the cause keeps its own node, got {:?}",
        graph.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
    assert!(
        graph.nodes.iter().any(|node| node.id == "evt:e4"),
        "an unrelated event is never absorbed into a sibling key"
    );
    assert_eq!(graph.nodes.len(), 4, "no key is lost and none is invented");

    // Both command stages cite the same cause: one deduplicated edge, not two.
    let cause_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.source == "evt:e0" && edge.target == "cmd:cmd-7")
        .count();
    assert_eq!(
        cause_edges, 1,
        "duplicate causation references resolve to one edge"
    );
    assert_eq!(
        graph.edges.len(),
        2,
        "cause->command and command->domain, nothing else"
    );

    // Tag lists never carry a duplicate, even when type and source normalize identically.
    for event in &trace.events {
        let mut seen = HashSet::new();
        assert!(
            event.tags.iter().all(|tag| seen.insert(tag.clone())),
            "event {} carries duplicate tags: {:?}",
            event.id,
            event.tags
        );
    }
    let probe = trace
        .events
        .iter()
        .find(|event| event.id == "e4")
        .expect("the collision probe is adapted");
    assert!(
        probe.tags.len() < 3,
        "the identical type/source pair is stored once, got {:?}",
        probe.tags
    );
}

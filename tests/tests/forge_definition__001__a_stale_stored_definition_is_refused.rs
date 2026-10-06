//! FORGE.DEFINITION — the workflow the engine runs is the workflow this build ships.
//!
//! The engine registers a definition `(key, version)` only when it is absent and otherwise adopts what is stored. A
//! graph edited under an unchanged version therefore never reached a database that already held that version:
//! production ran FORGE_SDLC v6 without the ASSAY branch at `execution_shape` for three weeks (a Lead that routed
//! ASSAY died with "No valid transition"), and every later fix to the XML was dark there.
//!
//!   * the stored graph round-trips through the database's JSON unchanged — so a guard that compares the stored graph
//!     with the embedded one cannot cry wolf on a healthy database;
//!   * a stored definition that DIFFERS under the same version stops the engine at boot, naming the remedy;
//!   * the ASSAY route exists in the shipped graph, so a Lead's ASSAY has somewhere to go.
//!
//! Level: L1, in-memory store.

use std::sync::Arc;

use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::runtime::ForgeRuntime;
use forge::engine::version_policy::graphs_equal;
use forge::engine::writer::RecordingWriter;
use forge::engine::FORGE_SDLC_VERSION;
use workflow::json_codec::{graph_from_json, graph_to_json};
use workflow::{MemoryStore, WorkflowEngine};

#[test]
fn the_shipped_definition_round_trips_through_the_stored_json_unchanged() {
    let shipped = forge_sdlc_definition();
    let stored =
        graph_from_json(&graph_to_json(&shipped.definition)).expect("the stored JSON reads back");
    assert!(
        graphs_equal(&stored, &shipped.definition),
        "a graph that does not survive its own storage would make the drift guard fail every healthy start"
    );
    assert_eq!(
        shipped.version, FORGE_SDLC_VERSION,
        "the XML and the constant name one version"
    );
}

#[test]
fn a_stored_definition_that_differs_under_the_same_version_stops_the_engine() {
    let memory = MemoryStore::new();
    // What an old deploy left behind: the same key and version, a different graph (no ASSAY branch).
    let mut stale = forge_sdlc_definition();
    let shape = stale
        .definition
        .nodes
        .get_mut("execution_shape")
        .expect("execution_shape exists");
    shape
        .decisions
        .as_mut()
        .expect("execution_shape is a decision")
        .retain(|arm| arm.transition != "assay");
    WorkflowEngine::new(memory.clone(), Default::default())
        .seed_definition(stale)
        .expect("the stale definition is stored first");

    let started = ForgeRuntime::from_store(
        memory,
        Arc::new(RecordingWriter::default()),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    );
    let error = match started {
        Ok(_) => panic!("a drifted stored definition must not run"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("version bump") && error.contains("FORGE_SDLC"),
        "the refusal names the remedy: {error}"
    );
}

#[test]
fn a_matching_stored_definition_starts_and_ships_the_assay_route() {
    let memory = MemoryStore::new();
    let runtime = ForgeRuntime::from_store(
        memory,
        Arc::new(RecordingWriter::default()),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    )
    .expect("an up-to-date store starts");
    drop(runtime);
    let shipped = forge_sdlc_definition();
    let shape = &shipped.definition.nodes["execution_shape"];
    assert!(
        shape
            .decisions
            .as_ref()
            .expect("a decision")
            .iter()
            .any(|arm| arm.condition.contains("'ASSAY'") && arm.transition == "assay"),
        "the shipped graph routes a Lead's ASSAY"
    );
}

//! WF.COMMAND — deterministic command ID (TST-WF-COMMAND-001).
//!
//! Contract: when the `WorkflowEngine` reaches a `command` node, the command id it mints is a **pure function** of
//! `(process_instance_id, node_id, visit_sequence)` — `command_id` at `rust/core/workflow/src/engine/handle_join.rs:373`,
//! the same derivation production runs. The same triple yields the same id whatever the wall clock says, so a retry
//! or a replay reuses the id instead of minting a new one, and the id is the global dedup key the store refuses to
//! duplicate.
//!
//! This file exercises the production boundary, not a re-declaration of it: the real `WorkflowEngine<MemoryStore>`
//! is driven through `start_process`, its `command` node calls the production `ApplicationPort` seam (faked at the
//! adapter boundary, so no live provider is touched), and the command id is read back both at the adapter and on the
//! durable event log through the production `Store`. Level: L3 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, DefinitionStatus, NodeDefinition, ProcessCommand, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TransitionDefinition, Value,
    WorkflowSubject,
};

const DEFINITION_KEY: &str = "TST-WF-COMMAND-001";
const DEFINITION_VERSION: i32 = 1;
/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// A second command node, so a run can observe two distinct command identities.
const SECOND_COMMAND_NODE: &str = "notify";
const COMMAND_TYPE: &str = "tst.emit";
const SECOND_COMMAND_TYPE: &str = "tst.notify";
const HARNESS: &str = "WorkflowHarness/L3 Composition";

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and always succeeds; it performs no I/O and names no
/// provider. The fake is the same adapter seam production substitutes, not a parallel command implementation.
#[derive(Clone, Default)]
struct RecordingApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
}

impl RecordingApplicationPort {
    fn new() -> Self {
        Self::default()
    }

    /// A snapshot of the requests recorded so far.
    fn requests(&self) -> Vec<ApplicationCommandRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl ApplicationPort for RecordingApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.clone());
        ApplicationCommandResult {
            command_id: request.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: None,
        }
    }

    fn read_facts(&self, _subject: &WorkflowSubject) -> Value {
        Value::object()
    }
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// A definition whose start node walks a chain of command nodes and then ends.
///
/// `start -> first command -> second command -> ... -> end`. On success each command node advances to the next, so
/// every command node in `command_nodes` runs exactly once for the instance.
fn definition(command_nodes: &[(&str, &str)]) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();

    let (first_node, _) = command_nodes[0];
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", first_node)]),
            ..Default::default()
        },
    );

    for (index, (node_id, command_type)) in command_nodes.iter().enumerate() {
        let target = command_nodes
            .get(index + 1)
            .map(|(next, _)| *next)
            .unwrap_or("end");
        nodes.insert(
            (*node_id).to_string(),
            NodeDefinition {
                id: (*node_id).to_string(),
                node_type: "command".to_string(),
                command_type: Some((*command_type).to_string()),
                transitions: Some(vec![transition("next", target)]),
                ..Default::default()
            },
        );
    }

    nodes.insert(
        "end".to_string(),
        NodeDefinition {
            id: "end".to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );

    ProcessDefinition {
        id: "tst-wf-command-001".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.COMMAND 001".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// Start a fresh engine at `millis`, register the definition, and run it to completion.
///
/// Returns the harness (so the store can be read back), the recording adapter, and the started instance id.
fn run_at(
    millis: i64,
    command_nodes: &[(&str, &str)],
) -> (EngineHarness, RecordingApplicationPort, String) {
    let app = RecordingApplicationPort::new();
    let recorder = app.clone();
    let harness =
        EngineHarness::with_application_port(TestClock::at_unix_millis(millis), Box::new(app));

    harness
        .engine()
        .seed_definition(definition(command_nodes))
        .expect("the command definition registers with the engine");
    let started = harness
        .engine()
        .start_process(StartProcessParams {
            definition_key: DEFINITION_KEY.to_string(),
            version: Some(DEFINITION_VERSION),
            business_key: None,
            variables: Value::object(),
            started_by: "tst".to_string(),
            tenant_id: None,
            subject: None,
        })
        .expect("the command process starts and drives to its end");

    (harness, recorder, started.process_instance_id)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-001); the file and the assay use it.
fn wf_command_001__deterministic_command_id() {
    // 1. Two independent executions of the same logical inputs, at two different clock instants. If the id read the
    //    clock, a random source, or any per-run state, these would differ.
    let (left_harness, left_port, left_instance) =
        run_at(1_600_000_000_000, &[(COMMAND_NODE, COMMAND_TYPE)]);
    let (right_harness, right_port, right_instance) =
        run_at(1_700_000_000_000, &[(COMMAND_NODE, COMMAND_TYPE)]);

    let left_requests = left_port.requests();
    let right_requests = right_port.requests();
    assert_eq!(
        left_requests.len(),
        1,
        "{HARNESS}: the command node runs exactly one command"
    );
    assert_eq!(
        right_requests.len(),
        1,
        "{HARNESS}: the command node runs exactly one command"
    );
    let left_id = &left_requests[0].command_id;
    let right_id = &right_requests[0].command_id;

    // The in-memory store mints the instance id deterministically, so the two runs share the whole triple — and must
    // therefore share the command id. This is the determinism the contract names.
    assert_eq!(
        left_instance, right_instance,
        "{HARNESS}: both runs start the same deterministic instance"
    );
    assert_eq!(
        left_id, right_id,
        "{HARNESS}: the same (instance, node, visit) yields one command id, whatever the clock"
    );

    // 2. It is the production derivation, not a re-declared one, and it has the shape of a content hash.
    assert_eq!(
        left_id,
        &command_id(&left_instance, COMMAND_NODE, 1),
        "{HARNESS}: the id is the production command_id of the visit"
    );
    assert_eq!(left_id.len(), 64, "{HARNESS}: a command id is a sha256 hex digest");
    assert!(
        left_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "{HARNESS}: a command id is lowercase hex"
    );

    // 3. The id reaches the durable log boundary: the command.requested event carries it.
    let requested = left_harness
        .store()
        .with_tx(|tx| tx.history(&left_instance, 128))
        .expect("the instance history reads")
        .into_iter()
        .find(|event| event.event_type == "command.requested")
        .expect("the engine records the command it requested");
    assert_eq!(
        requested.data.get("commandId").and_then(Value::as_str),
        Some(left_id.as_str()),
        "{HARNESS}: the durable event log carries the deterministic id"
    );

    // The run actually completed, so the boundary under test truly ran.
    let left_process = left_harness
        .store()
        .with_tx(|tx| tx.get_instance(&left_instance))
        .expect("the instance is readable");
    assert_eq!(
        left_process.status,
        ProcessStatus::Completed,
        "{HARNESS}: the command drove the process to completion"
    );

    // Reuse the right harness so its own run is not merely a copy: its command id already agreed above.
    let right_process = right_harness
        .store()
        .with_tx(|tx| tx.get_instance(&right_instance))
        .expect("the instance is readable");
    assert_eq!(right_process.status, ProcessStatus::Completed);

    // 4. NEGATIVE — the id is not a constant. A second command node, as its own input, gets its own id.
    let (two_harness, two_port, two_instance) = run_at(
        1_650_000_000_000,
        &[
            (COMMAND_NODE, COMMAND_TYPE),
            (SECOND_COMMAND_NODE, SECOND_COMMAND_TYPE),
        ],
    );
    let two_ids: Vec<String> = two_port
        .requests()
        .iter()
        .map(|request| request.command_id.clone())
        .collect();
    assert_eq!(
        two_ids.len(),
        2,
        "{HARNESS}: both command nodes run exactly once"
    );
    assert_eq!(
        two_ids[0],
        command_id(&two_instance, COMMAND_NODE, 1),
        "{HARNESS}: the first node keeps the production id"
    );
    assert_eq!(
        two_ids[1],
        command_id(&two_instance, SECOND_COMMAND_NODE, 1),
        "{HARNESS}: the second node gets its own production id"
    );
    assert_ne!(
        two_ids[0], two_ids[1],
        "{HARNESS}: two distinct commands must never share one id"
    );
    // The first node of the third run is, once more, the same (instance, node, visit) triple as the first run — so a
    // third, independent execution agrees too. The second node differs because its node identity differs.
    assert_eq!(
        two_ids[0], *left_id,
        "{HARNESS}: a third independent run of the same first visit agrees on the id"
    );
    assert_ne!(
        two_ids[1], *left_id,
        "{HARNESS}: a different node yields a different id"
    );

    // 5. NEGATIVE/FAULT — the deterministic id is the global dedup key. A command that carries an id already
    //    recorded is refused, even for a visit never used, so a non-deterministic id would be the only way to smuggle
    //    a duplicate through. The refusal is the store's own `COMMAND_DUPLICATE` guard.
    let two_process = two_harness
        .store()
        .with_tx(|tx| tx.get_instance(&two_instance))
        .expect("the instance is readable");
    let replay = ProcessCommand {
        process_instance_id: two_instance.clone(),
        token_id: two_process.root_token_id.clone().unwrap_or_default(),
        node_id: COMMAND_NODE.to_string(),
        // A visit that was never used: only the id itself can be what refuses this.
        visit_sequence: 99,
        command_id: two_ids[0].clone(),
        command_type: COMMAND_TYPE.to_string(),
        subject_type: None,
        subject_id: None,
        correlation_id: two_instance.clone(),
        causation_id: None,
        input: Value::object(),
        outcome: "success".to_string(),
        message: None,
    };
    let refusal = two_harness
        .store()
        .with_tx(|tx| tx.insert_command(replay.clone()))
        .expect_err("a command id already recorded must be refused");
    assert_eq!(
        refusal.code(),
        "COMMAND_DUPLICATE",
        "{HARNESS}: the deterministic id is the dedup key the store refuses to duplicate"
    );

    // 6. NEGATIVE — the id is bound to the whole identity triple, not to the node alone and not a global constant.
    //    Two instances started from the same definition at the same clock are two distinct identities, so the same
    //    node's first visit must mint two distinct command ids. If the derivation dropped the instance from its
    //    inputs (keying on node only), this is the case that would expose it.
    let starts = || StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: "tst".to_string(),
        tenant_id: None,
        subject: None,
    };
    let pair_app = RecordingApplicationPort::new();
    let pair_recorder = pair_app.clone();
    let pair_harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_650_000_000_000),
        Box::new(pair_app),
    );
    pair_harness
        .engine()
        .seed_definition(definition(&[(COMMAND_NODE, COMMAND_TYPE)]))
        .expect("the command definition registers with the engine");
    let first = pair_harness
        .engine()
        .start_process(starts())
        .expect("the first instance runs to its end");
    let second = pair_harness
        .engine()
        .start_process(starts())
        .expect("the second instance runs to its end");

    assert_ne!(
        first.process_instance_id, second.process_instance_id,
        "{HARNESS}: two starts in one engine are two instances"
    );
    let pair_requests = pair_recorder.requests();
    assert_eq!(
        pair_requests.len(),
        2,
        "{HARNESS}: each instance visits the command node exactly once"
    );
    assert_eq!(
        pair_requests[0].command_id,
        command_id(&first.process_instance_id, COMMAND_NODE, 1),
        "{HARNESS}: the first instance's id is its own triple's production id"
    );
    assert_eq!(
        pair_requests[1].command_id,
        command_id(&second.process_instance_id, COMMAND_NODE, 1),
        "{HARNESS}: the second instance's id is its own triple's production id"
    );
    assert_ne!(
        pair_requests[0].command_id, pair_requests[1].command_id,
        "{HARNESS}: distinct instances must not collide on a command id"
    );
    // The first of these instances is, once more, the same (instance, node, visit) triple as the first run — a fourth
    // independent execution of the same inputs, which must agree on the id.
    assert_eq!(
        pair_requests[0].command_id, *left_id,
        "{HARNESS}: an independent run of the same identity agrees on the id"
    );
}

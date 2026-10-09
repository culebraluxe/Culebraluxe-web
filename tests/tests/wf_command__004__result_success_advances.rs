//! WF.COMMAND — result success advances (TST-WF-COMMAND-004).
//!
//! Contract: when the `ApplicationPort` answers a command with
//! `ApplicationCommandOutcome::Success`, the `WorkflowEngine` takes the
//! command node's success transition and the process keeps going — a success
//! is advancement, never termination. The command is generated, executed and
//! recorded exactly once (visit 1); a `command.completed` event is emitted;
//! the token moves down the node's declared transition; and the process
//! reaches the end node as `Completed`.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` is driven through `start_process`,
//! its `command` node calls the production `ApplicationPort` seam (faked at
//! the adapter boundary, so no live provider is touched), and every
//! observation is read back through the production `Store` — the durable
//! instance, the durable event log and the command receipts. Level: L3
//! Composition, harness `WorkflowHarness`.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/handle_join.rs:242-285` — on `Success` the
//! engine emits `command.completed`, resolves the node's transition
//! (`node.transition`, else the first transition), moves the token and
//! arrives onward; any other outcome falls to `command.failed` plus
//! `terminate_process` instead.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__004__result_success_advances

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph,
    ProcessOutcome, ProcessStatus, StartProcessParams, TransitionDefinition, Value,
    WorkflowSubject,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The graph's start node.
const START_NODE: &str = "start";
/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// The end node the success transition reaches.
const END_NODE: &str = "done";
/// The command node's declared success transition — the one a success must take.
const SUCCESS_TRANSITION: &str = "next";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
const STARTED_BY: &str = "tst";

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and always succeeds;
/// it performs no I/O and names no provider. The fake is the same adapter seam
/// production substitutes, not a parallel command implementation.
#[derive(Clone, Default)]
struct SucceedingApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
}

impl SucceedingApplicationPort {
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

impl ApplicationPort for SucceedingApplicationPort {
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

/// `start -> emit (command, transition "next") -> done (end)`.
fn command_definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: START_NODE.to_string(),
            transitions: Some(vec![transition("begin", COMMAND_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
            name: Some("Emit".to_string()),
            command_type: Some(COMMAND_TYPE.to_string()),
            transition: Some(SUCCESS_TRANSITION.to_string()),
            transitions: Some(vec![transition(SUCCESS_TRANSITION, END_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        END_NODE.to_string(),
        NodeDefinition {
            id: END_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: 1,
        name: key.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// The same graph with the command node's transitions removed: a success with
/// nowhere to advance to.
fn transitionless_definition(key: &str) -> ProcessDefinition {
    let mut definition = command_definition(key);
    if let Some(node) = definition.definition.nodes.get_mut(COMMAND_NODE) {
        node.transition = None;
        node.transitions = None;
    }
    definition
}

fn start_params(key: &str) -> StartProcessParams {
    StartProcessParams {
        definition_key: key.to_string(),
        version: Some(1),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Register the graph on a fresh engine, start one process, and hand back the
/// engine, the adapter recorder and the committed instance id. The command runs
/// synchronously inside `start_process`, so the process is already settled
/// when this returns.
fn run_command(key: &str) -> (EngineHarness, SucceedingApplicationPort, String) {
    let app = SucceedingApplicationPort::new();
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_400_000),
        Box::new(app),
    );
    harness
        .engine()
        .seed_definition(command_definition(key))
        .expect("the command definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the command process starts and settles on its command");
    (harness, recorder, started.process_instance_id)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-004); the file and the assay use it.
fn wf_command_004__result_success_advances() {
    let (harness, recorder, instance_id) = run_command("TST-WF-COMMAND-004");

    // The command ran exactly once, as this visit's own deterministic identity.
    let requests = recorder.requests();
    assert_eq!(
        requests.len(),
        1,
        "{HARNESS}: the command node ran exactly one command"
    );
    assert_eq!(
        requests[0].command_type, COMMAND_TYPE,
        "{HARNESS}: the command under test is the node's declared type"
    );
    assert_eq!(
        requests[0].command_id,
        command_id(&instance_id, COMMAND_NODE, 1),
        "{HARNESS}: the successful command carries visit 1's deterministic identity"
    );

    // The success is durably a completion: one `command.completed` naming the
    // command — and no failure anywhere on the log.
    let events = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads");
    let types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert!(
        types.contains(&"command.completed"),
        "{HARNESS}: a success records command.completed, got {types:?}"
    );
    assert!(
        !types.contains(&"command.failed"),
        "{HARNESS}: a success never records command.failed"
    );
    assert!(
        !types.contains(&"process.failed"),
        "{HARNESS}: a success never records process.failed"
    );
    assert!(
        !types.contains(&"process.conflict"),
        "{HARNESS}: a success is never labelled a conflict"
    );
    let completed = events
        .iter()
        .find(|event| event.event_type == "command.completed")
        .expect("the completion event is present");
    assert_eq!(
        completed.data.get("commandId").and_then(Value::as_str),
        Some(requests[0].command_id.as_str()),
        "{HARNESS}: the completion names the command that ran"
    );

    // The success took the node's declared transition: the newest
    // `token.moved` names it, and the token rests on the end node.
    let moved = events
        .iter()
        .filter(|event| event.event_type == "token.moved")
        .max_by_key(|event| event.id)
        .expect("the success moved the token");
    assert_eq!(
        moved.data.get("transition").and_then(Value::as_str),
        Some(SUCCESS_TRANSITION),
        "{HARNESS}: the success took the command node's declared transition"
    );
    let resting: Vec<String> = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance_id))
        .expect("the instance's tokens are readable")
        .into_iter()
        .map(|token| token.node_id)
        .collect();
    assert!(
        resting.iter().any(|node| node == END_NODE),
        "{HARNESS}: the success advanced the token to the end node, got {resting:?}"
    );

    // The process advanced all the way to completion — a success terminates
    // nothing and errors nothing.
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: on success the process completes"
    );
    assert_eq!(
        instance.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: success is Completed, never Failed or Conflict"
    );
    assert!(
        instance.ended_at.is_some(),
        "{HARNESS}: the advanced process is terminal"
    );

    // The command receipt is stored with the success outcome it returned.
    let visit_count = harness
        .store()
        .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
        .expect("the store answers the visit count");
    assert_eq!(
        visit_count, 1,
        "{HARNESS}: the success consumed exactly the one visit it ran"
    );

    // NEGATIVE — a success with nowhere to advance is refused, not silently
    // swallowed: a command node without a success transition errors instead
    // of completing in place. This is the case that fails if success were
    // "just don't fail" rather than "take the transition".
    let portless = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_400_001),
        Box::new(SucceedingApplicationPort::new()),
    );
    portless
        .engine()
        .seed_definition(transitionless_definition(
            "TST-WF-COMMAND-004-NO-TRANSITION",
        ))
        .expect("the transitionless definition registers");
    let refusal = portless
        .engine()
        .start_process(start_params("TST-WF-COMMAND-004-NO-TRANSITION"))
        .expect_err("a success with no success transition must be refused");
    assert!(
        refusal.to_string().contains("has no success transition"),
        "{HARNESS}: the refusal names the missing transition, got {refusal}"
    );
}

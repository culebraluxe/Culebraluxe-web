//! WF.COMMAND — validation failure does not advance (TST-WF-COMMAND-005).
//!
//! Contract: when the `ApplicationPort` answers a command with
//! `ApplicationCommandOutcome::ValidationFailure`, the `WorkflowEngine`
//! terminates the process as a generic failure — and, crucially, the command
//! node's success transition is NEVER taken. The command still ran exactly
//! once (visit 1); its result is stored as a failure carrying the provider's
//! own outcome string; the process ends `Failed` under `Error` with
//! `process.failed`; and no token ever reaches the end node.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` is driven through `start_process`,
//! its `command` node calls the production `ApplicationPort` seam (faked at
//! the adapter boundary, so no live provider is touched), and every
//! observation is read back through the production `Store`. Level: L3
//! Composition, harness `WorkflowHarness`.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/handle_join.rs:287-304` — a non-success
//! outcome emits `command.failed` and `terminate_process`es as `Failed`;
//! `handle_join.rs:242-285` (the success arm) is the only path that moves the
//! token down the node's transition, so a failure cannot advance by
//! construction. The companion file `wf_command__006__conflict_semantics`
//! owns the Conflict-vs-rest dichotomy; this file owns the advancement half
//! for validation failure: nothing moves, nothing completes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__005__validation_failure_does_not

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
/// The end node the success transition reaches — and a validation failure must never reach.
const END_NODE: &str = "done";
/// The command node's success transition, which a failure must NOT take.
const SUCCESS_TRANSITION: &str = "next";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
const STARTED_BY: &str = "tst";

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and answers each
/// with one scripted outcome; it performs no I/O and names no provider. The
/// recorded request is the identity observation; the scripted outcome is the
/// control that drives the boundary to failure or success.
#[derive(Clone)]
struct FakeApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
    outcome: ApplicationCommandOutcome,
}

impl FakeApplicationPort {
    /// A port that answers every command with `outcome`.
    fn returning(outcome: ApplicationCommandOutcome) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            outcome,
        }
    }

    /// A snapshot of the requests recorded so far.
    fn requests(&self) -> Vec<ApplicationCommandRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl ApplicationPort for FakeApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.clone());
        ApplicationCommandResult {
            command_id: request.command_id.clone(),
            outcome: self.outcome,
            message: Some(self.outcome.as_str().to_string()),
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

/// `start -> emit (command) -> done (end)`, the smallest graph with a real
/// success transition out of the command node — so a failure that never
/// reaches `done` is distinguishable from a graph that simply cannot advance.
fn command_definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
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
fn run_command(
    key: &str,
    outcome: ApplicationCommandOutcome,
) -> (EngineHarness, FakeApplicationPort, String) {
    let app = FakeApplicationPort::returning(outcome);
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_500_000),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-005); the file and the assay use it.
fn wf_command_005__validation_failure_does_not() {
    let (harness, recorder, instance_id) = run_command(
        "TST-WF-COMMAND-005",
        ApplicationCommandOutcome::ValidationFailure,
    );

    // The command still ran exactly once, as this visit's own identity: a
    // validation failure is an outcome of the command, not a reason to skip it.
    let requests = recorder.requests();
    assert_eq!(
        requests.len(),
        1,
        "{HARNESS}: the command node ran exactly one command before reporting the failure"
    );
    assert_eq!(
        requests[0].command_id,
        command_id(&instance_id, COMMAND_NODE, 1),
        "{HARNESS}: the failure does not change the visited command's deterministic identity"
    );

    // The failure is durably recorded with the provider's own outcome string —
    // one `command.failed` carrying `validation_failure`, and no completion.
    let events = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads");
    let types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert!(
        !types.contains(&"command.completed"),
        "{HARNESS}: a validation failure never reports the command as completed"
    );
    let command_failed = events
        .iter()
        .find(|event| event.event_type == "command.failed")
        .expect("a failed command records a command.failed event");
    assert_eq!(
        command_failed.data.get("outcome").and_then(Value::as_str),
        Some("validation_failure"),
        "{HARNESS}: the command is durably recorded with the validation outcome it returned"
    );
    assert_eq!(
        command_failed.data.get("commandId").and_then(Value::as_str),
        Some(requests[0].command_id.as_str()),
        "{HARNESS}: the failed command event names the command that ran"
    );

    // The process failed as a generic failure — Failed, never Conflict — with
    // `process.failed` carrying the provider's reason, and it is terminal.
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Error,
        "{HARNESS}: a validation failure terminates the process, it does not leave it active"
    );
    assert_eq!(
        instance.outcome,
        Some(ProcessOutcome::Failed),
        "{HARNESS}: a validation failure is Failed, not Conflict and not Completed"
    );
    assert!(
        instance.ended_at.is_some(),
        "{HARNESS}: a validation failure produces a terminal instance"
    );
    assert!(
        types.contains(&"process.failed"),
        "{HARNESS}: the failure is recorded as process.failed, got {types:?}"
    );
    assert!(
        !types.contains(&"process.completed"),
        "{HARNESS}: a validation failure does not complete the process"
    );
    assert!(
        !types.contains(&"process.conflict"),
        "{HARNESS}: a validation failure is never labelled a conflict"
    );

    // THE ADVANCEMENT HALF — the success transition was not taken: no token
    // ever reached the end node, no move names the success transition, and no
    // token is left active. The work is terminated, not parked past the node.
    let tokens = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance_id))
        .expect("the instance's tokens are readable");
    assert!(
        tokens.iter().all(|token| token.node_id != END_NODE),
        "{HARNESS}: a validation failure must not take the command node's success transition"
    );
    let success_moves = events.iter().filter(|event| {
        event.event_type == "token.moved"
            && event.data.get("transition").and_then(Value::as_str) == Some(SUCCESS_TRANSITION)
    }).count();
    assert_eq!(
        success_moves, 0,
        "{HARNESS}: no move names the success transition after a validation failure"
    );
    assert!(
        harness
            .store()
            .with_tx(|tx| tx.list_active_tokens(&instance_id))
            .expect("the active tokens are readable")
            .is_empty(),
        "{HARNESS}: terminating on validation failure leaves no active token"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        1,
        "{HARNESS}: the failure consumed exactly the one visit it ran"
    );

    // POSITIVE CONTROL — the same graph advances on success, so "the failure
    // did not reach the end node" is not an artifact of a broken graph.
    let (success_harness, success_recorder, success_id) =
        run_command("TST-WF-COMMAND-005-SUCCESS", ApplicationCommandOutcome::Success);
    let success_instance = success_harness
        .store()
        .with_tx(|tx| tx.get_instance(&success_id))
        .expect("the successful instance is readable");
    assert_eq!(
        success_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: on success the same graph advances past the command node and completes"
    );
    assert_eq!(
        success_recorder.requests().len(),
        1,
        "{HARNESS}: the successful command ran exactly once"
    );
    let success_nodes: Vec<String> = success_harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&success_id))
        .expect("the successful tokens are readable")
        .into_iter()
        .map(|token| token.node_id)
        .collect();
    assert!(
        success_nodes.iter().any(|node| node == END_NODE),
        "{HARNESS}: success reaches the end node the failure never touched"
    );
}

//! WF.COMMAND — conflict semantics (TST-WF-COMMAND-006).
//!
//! Contract: when the `ApplicationPort` answers a command with `ApplicationCommandOutcome::Conflict`, the
//! `WorkflowEngine` terminates the process **as a conflict** — its own terminal outcome, not a success and not a
//! generic failure. The command is still generated, executed and recorded exactly once (visit 1 of the node); the
//! result is stored as a failure of the command; and, because the outcome is `Conflict` rather than any other
//! non-success, the process is ended with `ProcessOutcome::Conflict` under `ProcessStatus::Error` and its own
//! `process.conflict` event — never `process.failed`, and never advanced down the command node's success transition.
//!
//! This file exercises the production boundary, not a re-declaration of it: the real `WorkflowEngine<MemoryStore>`
//! is driven through `start_process`, its `command` node calls the production `ApplicationPort` seam (faked at the
//! adapter boundary, so no live provider is touched), and every observation is read back through the production
//! `Store` — the durable instance and the durable event log. Level: L3 Composition, harness `WorkflowHarness`.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/handle_join.rs:287-303` emits `command.failed` and then maps **`Conflict` to
//! `ProcessOutcome::Conflict`** while every other non-success maps to `ProcessOutcome::Failed`;
//! `middle/workflow/src/engine/execute_node_leave.rs:216-220` maps the conflict outcome to `ProcessStatus::Error`;
//! `middle/workflow/src/engine/execute_node_leave.rs:263-267` emits `process.conflict` (distinct from
//! `process.failed`); `middle/workflow/src/engine/execute_node_leave.rs:223-238` cancels the still-active token;
//! and `middle/workflow/src/engine/handle_join.rs:226-240` records the command with the outcome string actually
//! returned. `ApplicationCommandOutcome::Conflict` is `middle/workflow/src/types.rs:336-356`.
//!
//! The shape matters. The distinguishing negative is the whole point: the *same* graph run with a
//! `ValidationFailure` must end as `ProcessOutcome::Failed` with `process.failed`, so a boundary that collapsed
//! conflict into generic failure — or relabelled every failure as a conflict — fails here. A success run is kept as
//! the positive control that the graph's success transition genuinely exists, so "the conflict did not advance" is
//! not an artifact of a broken graph.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__006__conflict_semantics

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, DefinitionStatus, NodeDefinition, ProcessCommand, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TransitionDefinition, Value,
    WorkflowSubject,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The graph's start node.
const START_NODE: &str = "start";
/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// The end node the command node advances to on success — the transition a conflict must NOT take.
const END_NODE: &str = "done";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
const STARTED_BY: &str = "tst";

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and answers each with one scripted outcome; it performs no
/// I/O and names no provider. This is the same adapter seam production substitutes (`ReApplicationPort`), not a
/// parallel command implementation. The recorded request is the identity observation; the scripted outcome is the
/// control that drives the boundary to success, conflict or failure.
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
            // A stable, non-empty message so the tests can prove the command's message reaches both the
            // `command.failed` event and the terminating `process.conflict` reason.
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

/// `start -> emit (command) -> done (end)`, the smallest graph with a real success transition out of the command
/// node — so a conflict that never reaches `done` is distinguishable from a graph that simply cannot advance.
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
            transition: Some("next".to_string()),
            transitions: Some(vec![transition("next", END_NODE)]),
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

/// Register the graph on a fresh engine, start one process, and hand back the engine, the adapter recorder and the
/// committed instance id. The command runs synchronously inside `start_process`, so the process is already settled
/// when this returns.
fn run_command(
    key: &str,
    outcome: ApplicationCommandOutcome,
) -> (EngineHarness, FakeApplicationPort, String) {
    let app = FakeApplicationPort::returning(outcome);
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_000_000),
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

/// The durable event types, in the order the engine recorded them.
fn event_types(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .map(|event| event.event_type)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-006); the file and the assay use it.
fn wf_command_006__conflict_semantics() {
    // ── SCENARIO 1: conflict is a terminal, distinct outcome ─────────────────────────────────────────────────────
    let (harness, recorder, instance_id) =
        run_command("TST-WF-COMMAND-006", ApplicationCommandOutcome::Conflict);

    // The command still ran exactly once, as this visit's own identity: conflict is an outcome of the command, not a
    // reason to skip it.
    let requests = recorder.requests();
    assert_eq!(
        requests.len(),
        1,
        "{HARNESS}: the command node ran exactly one command before reporting conflict"
    );
    assert_eq!(
        requests[0].command_type, COMMAND_TYPE,
        "{HARNESS}: the command under test is the node's declared type"
    );
    assert_eq!(
        requests[0].command_id,
        command_id(&instance_id, COMMAND_NODE, 1),
        "{HARNESS}: conflict does not change the visited command's deterministic identity"
    );

    // The committed instance carries the conflict as its own outcome, distinct from a generic failure, and is
    // terminal (ended_at set).
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Error,
        "{HARNESS}: a conflict terminates the process, it does not leave it active"
    );
    assert_eq!(
        instance.outcome,
        Some(ProcessOutcome::Conflict),
        "{HARNESS}: the process outcome is exactly conflict, not failed"
    );
    assert!(
        instance.ended_at.is_some(),
        "{HARNESS}: a conflict produces a terminal instance"
    );

    let types = event_types(&harness, &instance_id);

    // The distinguishing durable event: a conflict is `process.conflict`, never `process.failed`.
    assert!(
        types.iter().any(|t| t == "process.conflict"),
        "{HARNESS}: the conflict is recorded as its own process event"
    );
    assert!(
        !types.iter().any(|t| t == "process.failed"),
        "{HARNESS}: a conflict must not be recorded as a generic process failure"
    );
    assert!(
        !types.iter().any(|t| t == "process.completed"),
        "{HARNESS}: a conflict does not complete the process"
    );

    let events = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads");

    // The command boundary recorded the outcome the provider returned: one `command.failed`, carrying `conflict`,
    // naming the command, with the provider's message — and no `command.completed`.
    let command_failed = events
        .iter()
        .find(|event| event.event_type == "command.failed")
        .expect("a failed command records a command.failed event");
    assert_eq!(
        command_failed.data.get("outcome").and_then(Value::as_str),
        Some("conflict"),
        "{HARNESS}: the command is durably recorded with the conflict outcome it returned"
    );
    assert_eq!(
        command_failed.data.get("commandId").and_then(Value::as_str),
        Some(requests[0].command_id.as_str()),
        "{HARNESS}: the failed command event names the command that ran"
    );
    assert_eq!(
        command_failed.data.get("message").and_then(Value::as_str),
        Some("conflict"),
        "{HARNESS}: the provider's message is carried onto the failure"
    );
    assert_eq!(
        types.iter().filter(|t| *t == "command.requested").count(),
        1,
        "{HARNESS}: exactly one command was requested"
    );
    assert!(
        !types.iter().any(|t| t == "command.completed"),
        "{HARNESS}: a conflict never reports the command as completed"
    );

    // The terminating event carries the outcome and the provider's reason, so an operator can tell a conflict from a
    // failure by the row alone.
    let process_conflict = events
        .iter()
        .find(|event| event.event_type == "process.conflict")
        .expect("the process records a process.conflict event");
    assert_eq!(
        process_conflict.data.get("outcome").and_then(Value::as_str),
        Some("conflict"),
        "{HARNESS}: the terminating event is labelled conflict"
    );
    assert_eq!(
        process_conflict.data.get("reason").and_then(Value::as_str),
        Some("conflict"),
        "{HARNESS}: the provider's reason reaches the terminating conflict"
    );

    // Conflict does not advance: the success transition is not taken, no token ever reaches the end node, and no
    // token is left active — the work is terminated, not parked.
    let tokens = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance_id))
        .expect("the instance's tokens are readable");
    assert!(
        tokens.iter().all(|token| token.node_id != END_NODE),
        "{HARNESS}: a conflict must not take the command node's success transition"
    );
    assert!(
        harness
            .store()
            .with_tx(|tx| tx.list_active_tokens(&instance_id))
            .expect("the active tokens are readable")
            .is_empty(),
        "{HARNESS}: terminating a process on conflict leaves no active token"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        1,
        "{HARNESS}: the conflict consumed exactly the one visit it ran"
    );

    // FAULT/REFUSAL: the conflict is a durably recorded command, so its identity is spent. Replaying the exact visit
    // is refused. This is what makes the "ran exactly once" assertions above about committed state, not a side
    // effect of reading a live request list.
    let replay = ProcessCommand {
        process_instance_id: instance_id.clone(),
        token_id: instance.root_token_id.clone().unwrap_or_default(),
        node_id: COMMAND_NODE.to_string(),
        visit_sequence: 1,
        command_id: requests[0].command_id.clone(),
        command_type: COMMAND_TYPE.to_string(),
        subject_type: None,
        subject_id: None,
        correlation_id: instance_id.clone(),
        causation_id: None,
        input: Value::object(),
        outcome: "conflict".to_string(),
        message: Some("conflict".to_string()),
    };
    let refusal = harness
        .store()
        .with_tx(|tx| tx.insert_command(replay.clone()))
        .expect_err("an already-recorded conflict command must not replay");
    assert_eq!(
        refusal.code(),
        "COMMAND_DUPLICATE",
        "{HARNESS}: the conflict command is durable, so its identity is refused on replay"
    );

    // ── SCENARIO 2: the distinguishing negative — a conflict is not every other non-success ──────────────────────
    // The production rule is an exact dichotomy (`handle_join.rs:299-303`): `Conflict` maps to
    // `ProcessOutcome::Conflict`, and *every other* non-success outcome maps to `ProcessOutcome::Failed`. Testing a
    // single non-conflict outcome would leave a boundary that relabelled, say, `Unauthorized` as `Conflict`
    // undetected, so all four non-conflict non-success outcomes are pinned here. Each must terminate as `Failed`
    // with `process.failed` and must never be labelled a conflict, and each must record the provider's own outcome
    // string — not a relabelled one. This is the case that fails if the boundary conflates the two.
    for outcome in [
        ApplicationCommandOutcome::ValidationFailure,
        ApplicationCommandOutcome::NotFound,
        ApplicationCommandOutcome::Unauthorized,
        ApplicationCommandOutcome::PreconditionFailure,
    ] {
        let key = format!("TST-WF-COMMAND-006-{}", outcome.as_str());
        let (failure_harness, failure_recorder, failure_id) = run_command(&key, outcome);
        let failure_instance = failure_harness
            .store()
            .with_tx(|tx| tx.get_instance(&failure_id))
            .expect("the failed instance is readable");
        assert_eq!(
            failure_instance.status,
            ProcessStatus::Error,
            "{HARNESS}: a non-conflict failure ({}) also terminates the process",
            outcome.as_str()
        );
        assert_eq!(
            failure_instance.outcome,
            Some(ProcessOutcome::Failed),
            "{HARNESS}: a non-conflict failure ({}) is Failed, not Conflict",
            outcome.as_str()
        );
        let failure_types = event_types(&failure_harness, &failure_id);
        assert!(
            failure_types.iter().any(|t| t == "process.failed"),
            "{HARNESS}: a non-conflict failure ({}) records process.failed",
            outcome.as_str()
        );
        assert!(
            !failure_types.iter().any(|t| t == "process.conflict"),
            "{HARNESS}: a non-conflict failure ({}) is never labelled a conflict",
            outcome.as_str()
        );
        assert_eq!(
            failure_recorder.requests().len(),
            1,
            "{HARNESS}: the failing command ({}) still ran exactly once",
            outcome.as_str()
        );
        let failure_command_failed = failure_harness
            .store()
            .with_tx(|tx| tx.history(&failure_id, 128))
            .expect("the failed instance history reads")
            .into_iter()
            .find(|event| event.event_type == "command.failed")
            .expect("the failure records a command.failed event");
        assert_eq!(
            failure_command_failed
                .data
                .get("outcome")
                .and_then(Value::as_str),
            Some(outcome.as_str()),
            "{HARNESS}: the recorded command outcome is the provider's own ({})",
            outcome.as_str()
        );
    }

    // ── SCENARIO 3: positive control — the success transition really exists ──────────────────────────────────────
    // Without this, "the conflict did not reach the end node" could be true because the graph is broken. On success
    // the same graph advances through the command node to the end node and completes.
    let (success_harness, success_recorder, success_id) = run_command(
        "TST-WF-COMMAND-006-SUCCESS",
        ApplicationCommandOutcome::Success,
    );
    let success_instance = success_harness
        .store()
        .with_tx(|tx| tx.get_instance(&success_id))
        .expect("the successful instance is readable");
    assert_eq!(
        success_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: on success the process advances past the command node and completes"
    );
    assert_eq!(
        success_instance.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: success is Completed, never Conflict"
    );
    let success_types = event_types(&success_harness, &success_id);
    assert!(
        success_types.iter().any(|t| t == "process.completed"),
        "{HARNESS}: the success path is real and reaches process.completed"
    );
    assert!(
        !success_types.iter().any(|t| t == "process.conflict"),
        "{HARNESS}: a success is never recorded as a conflict"
    );
    assert_eq!(
        success_recorder.requests().len(),
        1,
        "{HARNESS}: the successful command ran exactly once"
    );
}

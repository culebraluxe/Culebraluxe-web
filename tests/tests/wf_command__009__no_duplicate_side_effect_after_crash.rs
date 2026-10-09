//! WF.COMMAND — no duplicate side effect after crash (TST-WF-COMMAND-009).
//!
//! Contract: when the caller crashes after the command committed and retries
//! the driving operation, the retry executes nothing a second time. The task
//! completion is CAS-guarded (`TASK_ALREADY_COMPLETED`), so a post-commit
//! retry is refused before it can reach the command node; the adapter still
//! shows exactly one execution, the log still shows exactly one request, and
//! the completed run is undisturbed. The command's deterministic identity is
//! what makes the single execution recognizable as THE execution across the
//! crash, rather than merely "the first of two".
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` runs one command through the
//! production `ApplicationPort` seam (faked at the adapter boundary); the
//! "crash" is the caller losing its handle after the commit and retrying its
//! last operation — the real `complete_task` — and every observation (adapter
//! call count, receipt count, instance state) is read back through the
//! production `Store`. Level: L3 Composition, harness `WorkflowHarness`.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/engine_options.rs:369-380` — re-completing a
//! completed task is refused before any token, command or event is touched;
//! `handle_join.rs:201-202` — the retry could only re-derive the same visit
//! and the same deterministic id, so even a re-derived command would be the
//! same receipt, never a second side effect.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__009__no_duplicate_side_effect_after_crash

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TaskStatus,
    TransitionDefinition, Value, WorkflowSubject,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The graph's start node.
const START_NODE: &str = "start";
/// The task node the run parks on before the command step.
const TASK_NODE: &str = "wait";
/// The transition that leaves the task for the command node.
const TASK_TRANSITION: &str = "submit";
/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// The end node past the command.
const END_NODE: &str = "done";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
const STARTED_BY: &str = "tst";
const DEFINITION_VERSION: i32 = 1;

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and always succeeds;
/// it performs no I/O and names no provider. Its call count is the
/// side-effect observation: any duplicate execution after the crash would show
/// here.
#[derive(Clone, Default)]
struct RecordingApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
}

impl RecordingApplicationPort {
    fn new() -> Self {
        Self::default()
    }

    /// How many commands the adapter has executed.
    fn calls(&self) -> usize {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
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

/// `start -> wait (task) -> emit (command) -> done (end)`: the instance
/// commits and parks first, so the command runs in a later step — the shape a
/// crash can interrupt between the commit and the caller's observation.
fn definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", TASK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        TASK_NODE.to_string(),
        NodeDefinition {
            id: TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Wait".to_string()),
            transitions: Some(vec![transition(TASK_TRANSITION, COMMAND_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
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
        version: DEFINITION_VERSION,
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
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn complete_params(task_id: &str) -> CompleteTaskParams {
    CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(TASK_TRANSITION.to_string()),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-009); the file and the assay use it.
fn wf_command_009__no_duplicate_side_effect_after_crash() {
    const KEY: &str = "TST-WF-COMMAND-009";
    let app = RecordingApplicationPort::new();
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_900_000),
        Box::new(app),
    );
    harness
        .engine()
        .seed_definition(definition(KEY))
        .expect("the definition registers with the engine");

    // The run commits and parks; no command exists yet.
    let started = harness
        .engine()
        .start_process(start_params(KEY))
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id;
    let task_id = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .find(|task| task.status == TaskStatus::Ready)
        .expect("the process parks on a ready task before the command")
        .id;
    assert_eq!(
        recorder.calls(),
        0,
        "{HARNESS}: no command runs before the command node is reached"
    );

    // The driving operation commits: the command executes exactly once and the
    // run completes. The caller is about to crash before observing this.
    harness
        .engine()
        .complete_task(complete_params(&task_id))
        .expect("the task completes and drives the command");
    assert_eq!(
        recorder.calls(),
        1,
        "{HARNESS}: the command executed exactly once before the crash"
    );
    let completed = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        completed.status,
        ProcessStatus::Completed,
        "{HARNESS}: the pre-crash run completed"
    );

    // THE CRASH — the caller lost its handle after the commit and retries its
    // last operation. The retry is refused as already-completed BEFORE it can
    // touch the token, the command node or the adapter.
    let retry_refusal = harness
        .engine()
        .complete_task(complete_params(&task_id))
        .expect_err("retrying a completed task must be refused, not re-executed");
    assert_eq!(
        retry_refusal.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: the post-crash retry is refused as already done"
    );

    // No duplicate side effect: the adapter still shows one execution, the log
    // still shows one request, one visit is consumed, and the executed command
    // is visit 1's deterministic identity — recognizable across the crash as
    // THE execution, not the first of two.
    assert_eq!(
        recorder.calls(),
        1,
        "{HARNESS}: the post-crash retry executed nothing at the adapter"
    );
    let requested = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "command.requested")
        .collect::<Vec<_>>();
    assert_eq!(
        requested.len(),
        1,
        "{HARNESS}: the retry added no second request to the log"
    );
    assert_eq!(
        requested[0].data.get("commandId").and_then(Value::as_str),
        Some(command_id(&instance_id, COMMAND_NODE, 1).as_str()),
        "{HARNESS}: the single executed command is the crash-stable deterministic identity"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        1,
        "{HARNESS}: the retry consumed no further visit"
    );
    let settled = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is still readable");
    assert_eq!(
        (settled.status, settled.outcome),
        (ProcessStatus::Completed, Some(ProcessOutcome::Completed)),
        "{HARNESS}: the retry did not disturb the completed run"
    );
}

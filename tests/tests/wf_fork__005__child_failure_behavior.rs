//! WF.FORK — child failure behavior (TST-WF-FORK-005).
//!
//! Contract: a fork child's failure propagates according to the branch's required flag
//! (`resolve_process_after_token`, `middle/workflow/src/engine/execute_node_leave.rs:280-298`):
//!
//! - a **required** child that reaches a non-`Completed` end terminates the whole process (`terminate_process`,
//!   `execute_node_leave.rs:204-278`): the instance ends `Error`/`Failed` with one `process.failed` event, the
//!   sibling tokens are cancelled, their open tasks are obsoleted, and no further work on the dead branches is
//!   accepted (`TASK_NOT_ACTIONABLE` on complete; claims are refused at the instance gate with
//!   `PROCESS_NOT_ACTIVE`, since `claim_task` locks the instance before it reads the task);
//! - an **optional** child that reaches a non-`Completed` end terminates nothing: the process stays `Active`
//!   with no outcome, the required sibling stays alive, and the process still completes once the required
//!   branch finishes.
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process).
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__005__child_failure_behavior

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
/// Both branches required: a child failure must terminate the process.
const REQUIRED_KEY: &str = "TST-WF-FORK-005-REQUIRED";
/// One optional branch: its failure must terminate nothing.
const OPTIONAL_KEY: &str = "TST-WF-FORK-005-OPTIONAL";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const ACTOR: &str = "tst-actor";

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

/// `start -> fork(doomed -> task_doomed -> end_bad[Failed], steady -> task_steady -> end_good)`.
/// When `optional_doomed` is set, the doomed branch is optional and its failure must not terminate anything.
fn failure_definition(key: &str, optional_doomed: bool) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition("doomed", "task_doomed", optional_doomed.then_some(false)),
                transition("steady", "task_steady", None),
            ]),
            ..Default::default()
        },
    );
    for (task, end) in [("task_doomed", "end_bad"), ("task_steady", "end_good")] {
        nodes.insert(
            task.to_string(),
            NodeDefinition {
                id: task.to_string(),
                node_type: "task".to_string(),
                name: Some(task.to_string()),
                transitions: Some(vec![transition("finish", end, None)]),
                ..Default::default()
            },
        );
    }
    nodes.insert(
        "end_bad".to_string(),
        NodeDefinition {
            id: "end_bad".to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Failed),
            ..Default::default()
        },
    );
    nodes.insert(
        "end_good".to_string(),
        NodeDefinition {
            id: "end_good".to_string(),
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
        user_id: ACTOR.to_string(),
        form_data: Value::object(),
        transition_name: Some("finish".to_string()),
    }
}

fn tasks(harness: &EngineHarness, instance: &str) -> Vec<Task> {
    harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(instance))
        .expect("the instance tasks are readable")
}

fn task_at(harness: &EngineHarness, instance: &str, node: &str) -> Task {
    tasks(harness, instance)
        .into_iter()
        .find(|task| task.node_id.as_deref() == Some(node))
        .unwrap_or_else(|| panic!("a task is parked at {node}"))
}

fn task_by_id(harness: &EngineHarness, task_id: &str) -> Task {
    harness
        .store()
        .with_tx(|tx| tx.get_task(task_id))
        .expect("the task is readable")
}

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-005); the file and the assay use it.
fn wf_fork_005__child_failure_behavior() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(failure_definition(REQUIRED_KEY, false))
        .expect("the required-failure definition registers");
    harness
        .engine()
        .seed_definition(failure_definition(OPTIONAL_KEY, true))
        .expect("the optional-failure definition registers");

    // ── 1. REQUIRED CHILD FAILURE TERMINATES THE PROCESS ───────────────────────────────────────────────────
    // The doomed branch ends Failed while the steady branch is still parked. The process must terminate as
    // Error/Failed with exactly one `process.failed` event; the steady token is cancelled, its task obsoleted,
    // and no active token remains.
    let failing = harness
        .engine()
        .start_process(start_params(REQUIRED_KEY))
        .expect("the required-failure process starts");
    let failing_id = failing.process_instance_id.clone();
    let doomed = task_at(&harness, &failing_id, "task_doomed");
    let steady = task_at(&harness, &failing_id, "task_steady");
    harness
        .engine()
        .complete_task(complete_params(&doomed.id))
        .expect("the doomed branch reaches its Failed end");
    let ended = harness
        .store()
        .with_tx(|tx| tx.get_instance(&failing_id))
        .expect("the instance is readable");
    assert_eq!(
        ended.status,
        ProcessStatus::Error,
        "{HARNESS}: a required child that ends Failed terminates the process"
    );
    assert_eq!(
        ended.outcome,
        Some(ProcessOutcome::Failed),
        "{HARNESS}: the termination carries the Failed outcome"
    );
    assert_eq!(
        events_of_type(&harness, &failing_id, "process.failed").len(),
        1,
        "{HARNESS}: the required child failure is announced exactly once"
    );
    let steady_after = task_by_id(&harness, &steady.id);
    assert_eq!(
        steady_after.status,
        TaskStatus::Obsolete,
        "{HARNESS}: the surviving sibling's open task is obsoleted by the termination"
    );
    let active = harness
        .store()
        .with_tx(|tx| tx.list_active_tokens(&failing_id))
        .expect("the active tokens are readable");
    assert!(
        active.is_empty(),
        "{HARNESS}: the termination cancels every surviving sibling token"
    );

    // ── 2. NEGATIVE — NO WORK IS ACCEPTED ON THE DEAD BRANCHES ─────────────────────────────────────────────
    // The obsoleted sibling task refuses completion (`TASK_NOT_ACTIONABLE`). Claims never reach the task gate:
    // `claim_task` locks the instance first, so on the terminated process they are refused with
    // `PROCESS_NOT_ACTIVE`. Either way termination closes the branches; it does not merely mark the instance.
    let dead_complete = harness
        .engine()
        .complete_task(complete_params(&steady.id))
        .expect_err("a task obsoleted by termination must not complete");
    assert_eq!(
        dead_complete.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the dead branch refuses completion: {dead_complete}"
    );
    let dead_claim = harness
        .engine()
        .claim_task(&steady.id, "latecomer")
        .expect_err("a task obsoleted by termination must not be claimable");
    assert_eq!(
        dead_claim.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: the dead branch refuses claims at the instance gate: {dead_claim}"
    );

    // ── 3. OPTIONAL CHILD FAILURE TERMINATES NOTHING ───────────────────────────────────────────────────────
    // The same shape with the doomed branch optional: its Failed end leaves the process active with no outcome
    // and the required sibling alive, and the process still completes once the required branch finishes.
    let spared = harness
        .engine()
        .start_process(start_params(OPTIONAL_KEY))
        .expect("the optional-failure process starts");
    let spared_id = spared.process_instance_id.clone();
    let spared_doomed = task_at(&harness, &spared_id, "task_doomed");
    let spared_steady = task_at(&harness, &spared_id, "task_steady");
    harness
        .engine()
        .complete_task(complete_params(&spared_doomed.id))
        .expect("the optional branch reaches its Failed end");
    let mid = harness
        .store()
        .with_tx(|tx| tx.get_instance(&spared_id))
        .expect("the instance is readable");
    assert_eq!(
        mid.status,
        ProcessStatus::Active,
        "{HARNESS}: an optional child that ends Failed leaves the process active"
    );
    assert_eq!(
        mid.outcome, None,
        "{HARNESS}: an optional child failure sets no process outcome"
    );
    assert!(
        events_of_type(&harness, &spared_id, "process.failed").is_empty(),
        "{HARNESS}: an optional child failure emits no process.failed event"
    );
    let steady_after = task_by_id(&harness, &spared_steady.id);
    assert_eq!(
        steady_after.status,
        TaskStatus::Ready,
        "{HARNESS}: the required sibling survives the optional failure untouched"
    );
    harness
        .engine()
        .complete_task(complete_params(&spared_steady.id))
        .expect("the required branch finishes");
    let done = harness
        .store()
        .with_tx(|tx| tx.get_instance(&spared_id))
        .expect("the instance is readable");
    assert_eq!(
        done.status,
        ProcessStatus::Completed,
        "{HARNESS}: the process completes once its required branch finishes"
    );
}

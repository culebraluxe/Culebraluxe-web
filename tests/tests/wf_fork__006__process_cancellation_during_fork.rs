//! WF.FORK — process cancellation during fork (TST-WF-FORK-006).
//!
//! Contract: cancelling a process whose fork children are still parked closes every branch. `cancel_process`
//! (`middle/workflow/src/engine/engine_options.rs:465-488`) runs `terminate_process` with outcome `Cancelled`
//! (`middle/workflow/src/engine/execute_node_leave.rs:204-278`): the instance ends `Aborted`/`Cancelled` with one
//! `process.cancelled` event carrying the reason, every active token is completed as cancelled (one
//! `token.cancelled` event each), every open task is obsoleted, and the branches accept no further work.
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - cancelling a two-branch fork with both children parked aborts the process, cancels both tokens, obsoletes
//!   both tasks, and refuses completion on the dead branches (`TASK_NOT_ACTIONABLE`; claims stop earlier at the
//!   instance gate with `PROCESS_NOT_ACTIVE`);
//! - cancelling an already-cancelled process is idempotent (`Ok`), while cancelling a process that completed
//!   normally is refused with `PROCESS_NOT_ACTIVE` — cancellation closes live work, it is not a blank cheque.
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__006__process_cancellation_during_fork

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CancelProcessParams, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
/// The fork graph whose parked children are cancelled mid-flight.
const FORK_KEY: &str = "TST-WF-FORK-006";
/// The trivial graph that completes normally: the cancellation-refusal control.
const SIMPLE_KEY: &str = "TST-WF-FORK-006-SIMPLE";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const END_NODE: &str = "end";
const ACTOR: &str = "tst-actor";
const REASON: &str = "storm shutdown";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

fn fork_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition("left", "task_left"),
                transition("right", "task_right"),
            ]),
            ..Default::default()
        },
    );
    for task in ["task_left", "task_right"] {
        nodes.insert(
            task.to_string(),
            NodeDefinition {
                id: task.to_string(),
                node_type: "task".to_string(),
                name: Some(task.to_string()),
                transitions: Some(vec![transition("done", END_NODE)]),
                ..Default::default()
            },
        );
    }
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
        id: format!("{FORK_KEY}-def"),
        tenant_id: None,
        key: FORK_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: FORK_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// `start -> chore -> end`: completes normally so cancellation has nothing live to close.
fn simple_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", "chore")]),
            ..Default::default()
        },
    );
    nodes.insert(
        "chore".to_string(),
        NodeDefinition {
            id: "chore".to_string(),
            node_type: "task".to_string(),
            name: Some("Chore".to_string()),
            transitions: Some(vec![transition("done", END_NODE)]),
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
        id: format!("{SIMPLE_KEY}-def"),
        tenant_id: None,
        key: SIMPLE_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: SIMPLE_KEY.to_string(),
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

fn cancel_params(instance: &str) -> CancelProcessParams {
    CancelProcessParams {
        process_instance_id: instance.to_string(),
        actor: ACTOR.to_string(),
        reason: Some(REASON.to_string()),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-006); the file and the assay use it.
fn wf_fork_006__process_cancellation_during_fork() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(fork_definition())
        .expect("the fork definition registers");
    harness
        .engine()
        .seed_definition(simple_definition())
        .expect("the simple definition registers");

    // ── 1. CANCELLATION CLOSES EVERY PARKED BRANCH ───────────────────────────────────────────────────────────
    // Both fork children are parked when the process is cancelled. The instance must end Aborted/Cancelled with
    // one `process.cancelled` event carrying the reason, both tokens cancelled, and both tasks obsoleted.
    let running = harness
        .engine()
        .start_process(start_params(FORK_KEY))
        .expect("the fork process starts and parks both branches");
    let running_id = running.process_instance_id.clone();
    let left = task_at(&harness, &running_id, "task_left");
    let right = task_at(&harness, &running_id, "task_right");
    harness
        .engine()
        .cancel_process(cancel_params(&running_id))
        .expect("cancelling a live fork aborts the process");
    let cancelled = harness
        .store()
        .with_tx(|tx| tx.get_instance(&running_id))
        .expect("the instance is readable");
    assert_eq!(
        cancelled.status,
        ProcessStatus::Aborted,
        "{HARNESS}: cancelling a live fork aborts the process"
    );
    assert_eq!(
        cancelled.outcome,
        Some(ProcessOutcome::Cancelled),
        "{HARNESS}: the abort carries the Cancelled outcome"
    );
    let cancelled_events = events_of_type(&harness, &running_id, "process.cancelled");
    assert_eq!(
        cancelled_events.len(),
        1,
        "{HARNESS}: the cancellation is announced exactly once"
    );
    assert_eq!(
        cancelled_events[0]
            .data
            .get("reason")
            .and_then(Value::as_str),
        Some(REASON),
        "{HARNESS}: the cancellation event carries the given reason"
    );
    let token_cancelled = events_of_type(&harness, &running_id, "token.cancelled");
    assert_eq!(
        token_cancelled.len(),
        2,
        "{HARNESS}: both parked branch tokens are cancelled"
    );
    for task in [&left, &right] {
        assert_eq!(
            task_by_id(&harness, &task.id).status,
            TaskStatus::Obsolete,
            "{HARNESS}: the cancelled branch's open task is obsoleted"
        );
    }
    let active = harness
        .store()
        .with_tx(|tx| tx.list_active_tokens(&running_id))
        .expect("the active tokens are readable");
    assert!(
        active.is_empty(),
        "{HARNESS}: no branch token stays active after cancellation"
    );

    // ── 2. NEGATIVE — NO WORK IS ACCEPTED ON THE CANCELLED BRANCHES ──────────────────────────────────────────
    // A cancelled branch is closed: completing its task is refused (`TASK_NOT_ACTIONABLE`). Claims never reach
    // the task gate — `claim_task` locks the instance first, so on the aborted process they are refused with
    // `PROCESS_NOT_ACTIVE`.
    let dead_complete = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: left.id.clone(),
            user_id: ACTOR.to_string(),
            form_data: Value::object(),
            transition_name: Some("done".to_string()),
        })
        .expect_err("a task obsoleted by cancellation must not complete");
    assert_eq!(
        dead_complete.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the cancelled branch refuses completion: {dead_complete}"
    );
    let dead_claim = harness
        .engine()
        .claim_task(&right.id, "latecomer")
        .expect_err("a task obsoleted by cancellation must not be claimable");
    assert_eq!(
        dead_claim.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: the cancelled branch refuses claims at the instance gate: {dead_claim}"
    );

    // ── 3. IDEMPOTENT RE-CANCEL, AND NO CANCELLATION OF A COMPLETED PROCESS ──────────────────────────────────
    // Cancelling the already-cancelled process succeeds without a second announcement; cancelling a process
    // that completed normally is refused with `PROCESS_NOT_ACTIVE`.
    harness
        .engine()
        .cancel_process(cancel_params(&running_id))
        .expect("re-cancelling a cancelled process is idempotent");
    assert_eq!(
        events_of_type(&harness, &running_id, "process.cancelled").len(),
        1,
        "{HARNESS}: the idempotent re-cancel announces nothing further"
    );
    let simple = harness
        .engine()
        .start_process(start_params(SIMPLE_KEY))
        .expect("the simple process starts");
    let simple_id = simple.process_instance_id.clone();
    let chore = task_at(&harness, &simple_id, "chore");
    harness
        .engine()
        .claim_task(&chore.id, ACTOR)
        .expect("the chore is claimed");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: chore.id.clone(),
            user_id: ACTOR.to_string(),
            form_data: Value::object(),
            transition_name: Some("done".to_string()),
        })
        .expect("the chore completes");
    let done = harness
        .store()
        .with_tx(|tx| tx.get_instance(&simple_id))
        .expect("the instance is readable");
    assert_eq!(
        done.status,
        ProcessStatus::Completed,
        "{HARNESS}: the control process completes normally"
    );
    let refused = harness
        .engine()
        .cancel_process(cancel_params(&simple_id))
        .expect_err("cancelling a completed process must be refused");
    assert_eq!(
        refused.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: cancellation of a completed process is refused: {refused}"
    );
}

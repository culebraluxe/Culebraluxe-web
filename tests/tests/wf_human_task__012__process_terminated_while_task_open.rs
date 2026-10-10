//! WF.HUMAN_TASK — process terminated while task open (TST-WF-HUMAN-TASK-012).
//!
//! Contract: when a process is **terminated** (cancelled/aborted) while a human task is still open
//! (`Ready` status), the task transitions to `Obsolete` and the process status becomes `Aborted`.
//! The production boundary is `WorkflowEngine::cancel_process` at
//! `middle/workflow/src/engine/engine_options.rs:465`, which calls `terminate_process` to mark
//! all active tasks as `Obsolete`.
//!
//! This exercises the production boundary. The real `WorkflowEngine<MemoryStore>` is driven
//! through `seed_definition`, `start_process`, and `cancel_process`.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__012__process_terminated_while_task_open

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CancelProcessParams, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const TERMINATED_OPEN_KEY: &str = "TST-WF-HUMAN-TASK-012";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const TASK_NODE: &str = "review";
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
const MANAGER: &str = "manager";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

fn task_definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
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
            name: Some("Review".to_string()),
            candidate_groups: None, // open task
            transitions: Some(vec![transition(APPROVE, END_NODE)]),
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
            start_node_id: "start".to_string(),
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

fn start_and_park(harness: &EngineHarness, key: &str) -> (String, Task) {
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the process starts and parks on its human task");
    let instance = started.process_instance_id;
    let task = task_at(harness, &instance, TASK_NODE);
    (instance, task)
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

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

#[test]
#[allow(non_snake_case)]
fn wf_human_task_012__process_terminated_while_task_open() {
    let t0 = 1_650_000_000_000;
    let clock = TestClock::at_unix_millis(t0);
    let harness = EngineHarness::new(clock.clone());
    harness
        .engine()
        .seed_definition(task_definition(TERMINATED_OPEN_KEY))
        .expect("the task definition registers");

    // ── 1. START THE PROCESS, TASK IS OPEN (READY) ──────────────────────────────────────────────────────────────
    let (instance, parked) = start_and_park(&harness, TERMINATED_OPEN_KEY);
    assert_eq!(
        parked.status,
        TaskStatus::Ready,
        "{HARNESS}: task parks Ready"
    );
    assert_eq!(
        parked.assignee, None,
        "{HARNESS}: open task has no assignee"
    );
    assert!(
        parked.candidates.is_empty(),
        "{HARNESS}: open task has no candidates"
    );
    assert_eq!(instance_status(&harness, &instance), ProcessStatus::Active);

    // ── 2. TERMINATE THE PROCESS WHILE TASK IS OPEN ────────────────────────────────────────────────────────────
    harness
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance.clone(),
            actor: MANAGER.to_string(),
            reason: Some("process terminated while task open".to_string()),
        })
        .expect("cancel process succeeds");

    // ── 3. VERIFY PROCESS IS ABORTED ───────────────────────────────────────────────────────────────────────────
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Aborted,
        "{HARNESS}: process status becomes Aborted"
    );

    // ── 4. VERIFY THE OPEN TASK BECOMES OBSOLETE ───────────────────────────────────────────────────────────────
    let terminated_task = task_by_id(&harness, &parked.id);
    assert_eq!(
        terminated_task.status,
        TaskStatus::Obsolete,
        "{HARNESS}: open task becomes Obsolete when process is terminated"
    );
    // The task should retain its identity but be marked obsolete.
    assert_eq!(terminated_task.id, parked.id);
    assert_eq!(terminated_task.process_instance_id, instance);
    assert_eq!(terminated_task.node_id, Some(TASK_NODE.to_string()));

    // ── 5. VERIFY NO FURTHER OPERATIONS CAN SUCCEED ON THE OBSOLETE TASK ────────────────────────────────────────
    // Claim should fail (process not active).
    let claim_refused = harness
        .engine()
        .claim_task(&parked.id, "alice")
        .expect_err("claim on obsolete task fails");
    assert_eq!(
        claim_refused.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: claim on obsolete task fails with PROCESS_NOT_ACTIVE"
    );

    // Complete should fail (task not actionable).
    let complete_refused = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: "alice".to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("complete on obsolete task fails");
    assert_eq!(
        complete_refused.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: complete on obsolete task fails with TASK_NOT_ACTIONABLE"
    );

    // Release should fail (process not active).
    let release_refused = harness
        .engine()
        .release_task(&parked.id, "alice")
        .expect_err("release on obsolete task fails");
    // The process is Aborted, so PROCESS_NOT_ACTIVE is checked first.
    assert_eq!(
        release_refused.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: release on obsolete task fails with PROCESS_NOT_ACTIVE (process is Aborted)"
    );

    // ── 6. VERIFY A SECOND CANCEL IS IDEMPOTENT ─────────────────────────────────────────────────────────────────
    let second_cancel = harness.engine().cancel_process(CancelProcessParams {
        process_instance_id: instance.clone(),
        actor: MANAGER.to_string(),
        reason: Some("second cancel".to_string()),
    });
    assert!(
        second_cancel.is_ok(),
        "{HARNESS}: second cancel on already-aborted process is idempotent"
    );
    assert_eq!(instance_status(&harness, &instance), ProcessStatus::Aborted);
}

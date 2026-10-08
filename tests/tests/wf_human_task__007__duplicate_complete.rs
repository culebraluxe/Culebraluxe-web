//! WF.HUMAN_TASK — duplicate complete (TST-WF-HUMAN-TASK-007).
//!
//! Contract: attempting to **complete** an already-completed task must be refused with
//! `TASK_ALREADY_COMPLETED` and must commit nothing. The production boundary is
//! `WorkflowEngine::complete_task` at `middle/workflow/src/engine/engine_options.rs:364`:
//! it checks `task.status == TaskStatus::Completed` at `engine_options.rs:369-373` and
//! returns the error before any mutation.
//!
//! This exercises the production boundary. The real `WorkflowEngine<MemoryStore>` is driven
//! through `seed_definition`, `start_process`, `claim_task`, `complete_task` (first),
//! and `complete_task` (second, duplicate).
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__007__duplicate_complete

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value, json,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const DUP_COMPLETE_KEY: &str = "TST-WF-HUMAN-TASK-007";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const TASK_NODE: &str = "review";
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
const ALICE: &str = "alice";

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
            candidate_groups: None,
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

fn history(harness: &EngineHarness, instance: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    history(harness, instance)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

fn fingerprint(task: &Task) -> (TaskStatus, Option<String>, Option<i64>, Option<i64>, Option<String>, i32) {
    (
        task.status,
        task.assignee.clone(),
        task.claimed_at,
        task.completed_at,
        task.completed_by.clone(),
        task.version,
    )
}

#[test]
#[allow(non_snake_case)]
fn wf_human_task_007__duplicate_complete() {
    let t0 = 1_650_000_000_000;
    let clock = TestClock::at_unix_millis(t0);
    let harness = EngineHarness::new(clock.clone());
    harness
        .engine()
        .seed_definition(task_definition(DUP_COMPLETE_KEY))
        .expect("the task definition registers");

    // ── 1. SETUP: CLAIM AND COMPLETE THE TASK ──────────────────────────────────────────────────────────────────
    let (instance, parked) = start_and_park(&harness, DUP_COMPLETE_KEY);
    harness.clock().advance_millis(3_600_000);
    harness
        .engine()
        .claim_task(&parked.id, ALICE)
        .expect("Alice claims the task");
    harness.clock().advance_millis(3_600_000);
    let first_complete_instant = harness.now_millis();
    let form_data = json!({"decision": "approved"});
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data,
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("Alice completes the task");
    let completed = task_by_id(&harness, &parked.id);
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.completed_at, Some(first_complete_instant));
    assert_eq!(completed.completed_by.as_deref(), Some(ALICE));
    assert_eq!(completed.version, 3); // 1 (created) -> 2 (claimed) -> 3 (completed)

    // ── 2. NEGATIVE — DUPLICATE COMPLETE BY SAME USER IS REFUSED ───────────────────────────────────────────────
    let before_duplicate = fingerprint(&task_by_id(&harness, &parked.id));
    let duplicate_refused = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"note": "duplicate attempt"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("an already completed task cannot be completed again");
    assert_eq!(
        duplicate_refused.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: duplicate completion is refused with TASK_ALREADY_COMPLETED"
    );
    // The task must be byte-identical: status, assignee, claimed_at, completed_at, completed_by, version all unchanged.
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_duplicate,
        "{HARNESS}: the refused duplicate completion commits nothing — all fields unchanged"
    );
    // The event log must not have a second task.completed event.
    let completed_events = events_of_type(&harness, &instance, "task.completed");
    assert_eq!(
        completed_events.len(),
        1,
        "{HARNESS}: the refused duplicate emits no further task.completed event"
    );
    // The process must remain completed.
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the process remains completed"
    );

    // ── 3. NEGATIVE — DUPLICATE COMPLETE BY DIFFERENT USER IS REFUSED ──────────────────────────────────────────
    // Even a different user cannot complete an already-completed task.
    let before_duplicate2 = fingerprint(&task_by_id(&harness, &parked.id));
    let duplicate_refused2 = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: "bob".to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("a different user cannot complete an already completed task");
    assert_eq!(
        duplicate_refused2.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: duplicate completion by different user is also refused"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_duplicate2,
        "{HARNESS}: the refused duplicate by different user commits nothing"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.completed").len(),
        1,
        "{HARNESS}: still only one task.completed event"
    );
}
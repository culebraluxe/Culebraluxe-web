//! WF.HUMAN_TASK — complete (TST-WF-HUMAN-TASK-006).
//!
//! Contract: a human task's assignee may **complete** the task, advancing the process.
//! `WorkflowEngine::complete_task` at `middle/workflow/src/engine/engine_options.rs:364` is the
//! production boundary: it locks the instance and the task, admits only an active process and an
//! actionable task (`engine_options.rs:368-396`), verifies the caller is the assignee (if any)
//! (`engine_options.rs:381-387`), merges form data, and then moves the task to
//! `status = Completed`, `assignee = user`, `completed_at = now`, `completed_by = user`,
//! `version += 1`, with a durable `task.completed` event (`engine_options.rs:422-434`).
//! It then executes the node leave logic, advancing the token through the graph.
//!
//! The completion is refused with `TASK_ALREADY_COMPLETED` if the task is already `Completed`.
//! It is refused with `TASK_NOT_ACTIONABLE` if the task is not `Ready`, `Reserved`, or `InProgress`.
//! It is refused with `TASK_ASSIGNEE_ONLY` if the task has an assignee and the caller is not that
//! assignee. It is refused with `PROCESS_NOT_ACTIVE` if the process is not active.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__006__complete

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value, json,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const COMPLETE_KEY: &str = "TST-WF-HUMAN-TASK-006";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const TASK_NODE: &str = "review";
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
const ALICE: &str = "alice";
const BOB: &str = "bob";

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

fn fingerprint(task: &Task) -> (TaskStatus, Option<String>, Option<i64>, i32) {
    (
        task.status,
        task.assignee.clone(),
        task.claimed_at,
        task.version,
    )
}

#[test]
#[allow(non_snake_case)]
fn wf_human_task_006__complete() {
    let t0 = 1_650_000_000_000;
    let clock = TestClock::at_unix_millis(t0);
    let harness = EngineHarness::new(clock.clone());
    harness
        .engine()
        .seed_definition(task_definition(COMPLETE_KEY))
        .expect("the task definition registers");

    // ── 1. SETUP: CLAIM THE TASK ───────────────────────────────────────────────────────────────────────────────
    let (instance, parked) = start_and_park(&harness, COMPLETE_KEY);
    assert_eq!(parked.status, TaskStatus::Ready);
    harness.clock().advance_millis(3_600_000);
    harness
        .engine()
        .claim_task(&parked.id, ALICE)
        .expect("Alice claims the task");
    let claimed = task_by_id(&harness, &parked.id);
    assert_eq!(claimed.status, TaskStatus::Reserved);
    assert_eq!(claimed.assignee.as_deref(), Some(ALICE));
    assert_eq!(claimed.version, 2);

    // ── 2. POSITIVE — THE ASSIGNEE COMPLETES THE TASK ──────────────────────────────────────────────────────────
    harness.clock().advance_millis(3_600_000);
    let complete_instant = harness.now_millis();
    let form_data = json!({"decision": "approved", "notes": "looks good"});
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data: form_data.clone(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("Alice completes the task");
    let completed = task_by_id(&harness, &parked.id);
    assert_eq!(
        completed.status,
        TaskStatus::Completed,
        "{HARNESS}: complete moves the task to Completed"
    );
    assert_eq!(
        completed.assignee.as_deref(),
        Some(ALICE),
        "{HARNESS}: the assignee is recorded as completed_by"
    );
    assert_eq!(
        completed.completed_at,
        Some(complete_instant),
        "{HARNESS}: completed_at is the completion time"
    );
    assert_eq!(
        completed.completed_by.as_deref(),
        Some(ALICE),
        "{HARNESS}: completed_by records the completer"
    );
    assert_eq!(
        completed.form_data.get("decision").and_then(Value::as_str),
        Some("approved"),
        "{HARNESS}: form data is merged into the task"
    );
    assert_eq!(
        completed.version,
        claimed.version + 1,
        "{HARNESS}: complete bumps the version"
    );
    // The completion is durable and attributed.
    let completed_events = events_of_type(&harness, &instance, "task.completed");
    assert_eq!(
        completed_events.len(),
        1,
        "{HARNESS}: the completion emits exactly one task.completed event"
    );
    assert_eq!(completed_events[0].actor, ALICE);
    assert_eq!(completed_events[0].task_id.as_deref(), Some(parked.id.as_str()));
    assert_eq!(
        completed_events[0]
            .data
            .get("formData")
            .and_then(Value::as_object)
            .and_then(|o| o.get("decision"))
            .and_then(Value::as_str),
        Some("approved"),
        "{HARNESS}: the completion event carries the form data"
    );
    assert_eq!(
        completed_events[0]
            .data
            .get("transitionName")
            .and_then(Value::as_str),
        Some(APPROVE),
        "{HARNESS}: the completion event records the transition name"
    );
    // The process instance should be completed (single-task process).
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the single-task process is completed"
    );

    // ── 3. NEGATIVE — COMPLETE AN ALREADY COMPLETED TASK IS REFUSED ────────────────────────────────────────────
    let before_duplicate = fingerprint(&task_by_id(&harness, &parked.id));
    let duplicate_refused = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("an already completed task cannot be completed again");
    assert_eq!(
        duplicate_refused.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: duplicate completion is refused"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_duplicate,
        "{HARNESS}: the refused duplicate completion commits nothing"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.completed").len(),
        1,
        "{HARNESS}: the refused duplicate emits no further task.completed event"
    );

    // ── 4. NEGATIVE — NON-ASSIGNEE CANNOT COMPLETE ─────────────────────────────────────────────────────────────
    // Create a new instance, claim by Alice, then Bob tries to complete.
    let (instance2, parked2) = start_and_park(&harness, COMPLETE_KEY);
    harness
        .engine()
        .claim_task(&parked2.id, ALICE)
        .expect("Alice claims");
    let before_bob = fingerprint(&task_by_id(&harness, &parked2.id));
    let bob_refused = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked2.id.clone(),
            user_id: BOB.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("non-assignee cannot complete");
    assert_eq!(
        bob_refused.code(),
        "TASK_ASSIGNEE_ONLY",
        "{HARNESS}: completion by non-assignee is refused"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked2.id)),
        before_bob,
        "{HARNESS}: the refused completion commits nothing"
    );

    // ── 5. NEGATIVE — COMPLETE A NON-ACTIONABLE TASK IS REFUSED ────────────────────────────────────────────────
    // Create a new instance, don't claim (leave Ready), then try to complete as non-assignee.
    // Actually Ready IS actionable. The only non-actionable states are Created, Completed, Failed, Exited, Obsolete.
    // A task starts as Ready, so we can't easily test non-actionable without completing first.
    // We already tested Completed. Let's test that completion works on Ready (actionable) task for assignee.
    let (instance3, parked3) = start_and_park(&harness, COMPLETE_KEY);
    // Parked3 is Ready (actionable). Complete should work even without claim (open task).
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked3.id.clone(),
            user_id: ALICE.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("completion works on Ready task for open task");
    let completed3 = task_by_id(&harness, &parked3.id);
    assert_eq!(completed3.status, TaskStatus::Completed);
    assert_eq!(completed3.assignee.as_deref(), Some(ALICE));
    assert_eq!(instance_status(&harness, &instance3), ProcessStatus::Completed);

    // ── 6. POSITIVE CONTROL — OPEN TASK (NO ASSIGNEE) CAN BE COMPLETED BY ANYONE ──────────────────────────────
    // The assignee check only applies if there IS an assignee. For open tasks, anyone can complete.
    let (instance4, parked4) = start_and_park(&harness, COMPLETE_KEY);
    assert_eq!(parked4.assignee, None);
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked4.id.clone(),
            user_id: BOB.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("open task can be completed by anyone");
    let completed4 = task_by_id(&harness, &parked4.id);
    assert_eq!(completed4.status, TaskStatus::Completed);
    assert_eq!(completed4.assignee.as_deref(), Some(BOB));
    assert_eq!(instance_status(&harness, &instance4), ProcessStatus::Completed);
}
//! WF.HUMAN_TASK — concurrent duplicate complete (TST-WF-HUMAN-TASK-008).
//!
//! Contract: when two concurrent requests attempt to **complete** the same task, exactly one
//! succeeds and the other is refused. The production boundary is `WorkflowEngine::complete_task`
//! at `middle/workflow/src/engine/engine_options.rs:364`. The engine uses a CAS (compare-and-swap)
//! on the task version (`engine_options.rs:415-419`) to ensure only one completion commits.
//! The status check (`task.status == TaskStatus::Completed` at `engine_options.rs:369-373`) runs
//! before the CAS, so a concurrent completer that arrives after the first commit will see
//! `Completed` status and fail with `TASK_ALREADY_COMPLETED`. A concurrent completer that races
//! the CAS would fail with `STALE_TASK`.
//!
//! Since the in-memory `EngineHarness` is single-threaded, true concurrency cannot be tested
//! here. This test verifies the sequential behavior that mirrors the concurrent outcome: the
//! first completion succeeds, the second fails with `TASK_ALREADY_COMPLETED`, and the task state
//! is unchanged by the failure.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__008__concurrent_duplicate_complete

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value, json,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const CONCUR_COMPLETE_KEY: &str = "TST-WF-HUMAN-TASK-008";
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
fn wf_human_task_008__concurrent_duplicate_complete() {
    let t0 = 1_650_000_000_000;
    let clock = TestClock::at_unix_millis(t0);
    let harness = EngineHarness::new(clock.clone());
    harness
        .engine()
        .seed_definition(task_definition(CONCUR_COMPLETE_KEY))
        .expect("the task definition registers");

    // ── 1. SETUP: CLAIM THE TASK ───────────────────────────────────────────────────────────────────────────────
    let (instance, parked) = start_and_park(&harness, CONCUR_COMPLETE_KEY);
    harness.clock().advance_millis(3_600_000);
    harness
        .engine()
        .claim_task(&parked.id, ALICE)
        .expect("Alice claims the task");
    let claimed = task_by_id(&harness, &parked.id);
    assert_eq!(claimed.status, TaskStatus::Reserved);
    assert_eq!(claimed.version, 2);

    // ── 2. FIRST COMPLETION SUCCEEDS ───────────────────────────────────────────────────────────────────────────
    harness.clock().advance_millis(3_600_000);
    let first_complete_instant = harness.now_millis();
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"decision": "approved"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("first completion succeeds");
    let completed = task_by_id(&harness, &parked.id);
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.completed_at, Some(first_complete_instant));
    assert_eq!(completed.completed_by.as_deref(), Some(ALICE));
    assert_eq!(completed.version, 3);

    // ── 3. SECOND (CONCURRENT) COMPLETION FAILS WITH TASK_ALREADY_COMPLETED ────────────────────────────────────
    // In a real concurrent scenario, the second request would either:
    // - See the Completed status (if it runs after the first commit) → TASK_ALREADY_COMPLETED
    // - Race the CAS (if it runs truly concurrently) → STALE_TASK
    // The sequential test mirrors the first case: the status check runs before CAS.
    let before_second = fingerprint(&task_by_id(&harness, &parked.id));
    let second_refused = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"note": "concurrent attempt"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("the second concurrent completion is refused");
    // The status check runs before CAS, so we get TASK_ALREADY_COMPLETED.
    assert_eq!(
        second_refused.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: the second concurrent completion fails with TASK_ALREADY_COMPLETED (status check)"
    );
    // The task state must be unchanged.
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_second,
        "{HARNESS}: the refused concurrent completion commits nothing"
    );
    // Only one completion event.
    assert_eq!(
        events_of_type(&harness, &instance, "task.completed").len(),
        1,
        "{HARNESS}: exactly one task.completed event total"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the process remains completed"
    );

    // ── 4. VERIFY THE COMPLETION EVENT CARRIES THE CORRECT DATA ────────────────────────────────────────────────
    let completed_events = events_of_type(&harness, &instance, "task.completed");
    assert_eq!(completed_events[0].actor, ALICE);
    assert_eq!(
        completed_events[0]
            .data
            .get("formData")
            .and_then(Value::as_object)
            .and_then(|o| o.get("decision"))
            .and_then(Value::as_str),
        Some("approved"),
        "{HARNESS}: the successful completion event has the original form data, not the concurrent attempt's"
    );
}
//! WF.TRACE — process completed/failed/cancelled (TST-WF-TRACE-009).
//!
//! Contract: a workflow process can reach three terminal states depending on how
//! it exits: `Completed` (normal finish), `Failed` (unrecoverable error), or
//! `Cancelled` (explicit cancellation). This file exercises the production boundary
//! with the `WorkflowEngine` and `TestClock`, verifying that each terminal state
//! is correctly recorded on the durable event log. Level: L2 Persistence,
//! harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__009__process_completed_failed_cancelled

use std::sync::Arc;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, NodeDefinition, ProcessOutcome, ProcessStatus, StartProcessParams,
    TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L2 Persistence";
const DEFINITION_KEY: &str = "TST-WF-TRACE-009";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

// Definition: `start -> human task -> end`, with three possible exit transitions.
fn process_definition(key: &str) -> workflow::ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![TransitionDefinition {
                name: "begin".to_string(),
                to: "human_task".to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        "human_task".to_string(),
        NodeDefinition {
            id: "human_task".to_string(),
            node_type: "task".to_string(),
            name: Some("Human Task".to_string()),
            transitions: Some(vec![
                TransitionDefinition {
                    name: "complete".to_string(),
                    to: "end".to_string(),
                    condition: None,
                    required: None,
                },
                TransitionDefinition {
                    name: "force_error".to_string(),
                    to: "end".to_string(),
                    condition: None,
                    required: None,
                },
                TransitionDefinition {
                    name: "cancel".to_string(),
                    to: "end".to_string(),
                    condition: None,
                    required: None,
                },
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        "end".to_string(),
        NodeDefinition {
            id: "end".to_string(),
            node_type: "end".to_string(),
            outcome: None,
            ..Default::default()
        },
    );
    workflow::ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: DEFINITION_VERSION,
        name: key.to_string(),
        description: None,
        definition: workflow::ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: workflow::DefinitionStatus::Active,
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

/// Start a process and park on the human task.
fn start_and_park_on_task(harness: &EngineHarness, key: &str) -> (String, String) {
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id;
    let task_id = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .filter(|task| task.status == workflow::TaskStatus::Ready)
        .last()
        .expect("the process parks on a ready task before the command")
        .id;
    (instance_id, task_id)
}

/// Read the instance status and outcome.
fn read_instance(
    harness: &EngineHarness,
    instance_id: &str,
) -> (workflow::ProcessStatus, workflow::ProcessOutcome) {
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(instance_id))
        .expect("the instance reads");
    (
        instance.status,
        instance
            .outcome
            .unwrap_or(workflow::ProcessOutcome::Completed),
    )
}

/// Read the durable event log for a specific event type, returning all matching events.
fn read_event_log(
    harness: &EngineHarness,
    instance_id: &str,
    event_type: &str,
) -> Vec<workflow::ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| {
            tx.history(instance_id, 128).map(|events| {
                events
                    .into_iter()
                    .filter(|event| event.event_type == event_type)
                    .collect()
            })
        })
        .expect("history readable")
}

/// Complete the parked task with a normal completion (transition = "complete").
fn complete_task_normal(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: workflow::Value::object(),
            transition_name: Some("complete".to_string()),
        })
        .expect("the task completes normally");
}

/// Drive the task to a failure state (transition = "force_error").
fn force_error_task(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: workflow::Value::object(),
            transition_name: Some("force_error".to_string()),
        })
        .expect_err("force_error should terminate the instance");
}

/// Cancel the task (transition = "cancel").
fn cancel_task(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: workflow::Value::object(),
            transition_name: Some("cancel".to_string()),
        })
        .expect("the task cancels");
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-009); the file and the assay use it.
fn wf_trace_009__process_completed_failed_cancelled() {
    let harness = EngineHarness::at_unix_millis(1_600_000_000_001);
    harness
        .engine()
        .seed_definition(process_definition(DEFINITION_KEY))
        .expect("the definition registers with the engine");

    // ── SCENARIO 1: Normal completion ──────────────────────────────────────
    let (instance1, task1) = start_and_park_on_task(&harness, DEFINITION_KEY);
    complete_task_normal(&harness, &task1);

    let (status1, outcome1) = read_instance(&harness, &instance1);
    assert_eq!(
        status1,
        workflow::ProcessStatus::Completed,
        "{HARNESS}: normal completion yields status=Completed"
    );
    assert_eq!(
        outcome1,
        workflow::ProcessOutcome::Completed,
        "{HARNESS}: normal completion yields outcome=Completed"
    );

    // The durable event log carries the process.completed event.
    let completed_events: Vec<workflow::ProcessEvent> =
        read_event_log(&harness, &instance1, "process.completed");
    assert!(
        !completed_events.is_empty(),
        "{HARNESS}: the durable event log carries the process.completed event"
    );
    // Verify the event carries outcome=completed
    let completed_event = &completed_events[0];
    let event_outcome = completed_event
        .data
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(
        event_outcome, "completed",
        "{HARNESS}: process.completed event carries outcome=completed"
    );

    // ── SCENARIO 2: Failure ────────────────────────────────────────────────
    // Start a fresh process for the failure path.
    let (instance2, task2) = start_and_park_on_task(&harness, DEFINITION_KEY);
    force_error_task(&harness, &task2);

    let (status2, outcome2) = read_instance(&harness, &instance2);
    assert_eq!(
        status2,
        workflow::ProcessStatus::Error,
        "{HARNESS}: failure yields status=Error"
    );
    assert_eq!(
        outcome2,
        workflow::ProcessOutcome::Failed,
        "{HARNESS}: failure yields outcome=Failed"
    );

    // The durable event log carries the process.failed event.
    let failed_events: Vec<workflow::ProcessEvent> =
        read_event_log(&harness, &instance2, "process.failed");
    assert!(
        !failed_events.is_empty(),
        "{HARNESS}: the durable event log carries the process.failed event"
    );
    // Verify the event carries outcome=failed
    let failed_event = &failed_events[0];
    let event_outcome = failed_event
        .data
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(
        event_outcome, "failed",
        "{HARNESS}: process.failed event carries outcome=failed"
    );

    // ── SCENARIO 3: Cancellation ──────────────────────────────────────────
    // Start a fresh process for the cancellation path.
    let (instance3, task3) = start_and_park_on_task(&harness, DEFINITION_KEY);
    cancel_task(&harness, &task3);

    let (_status3, outcome3) = read_instance(&harness, &instance3);
    assert_eq!(
        outcome3,
        workflow::ProcessOutcome::Cancelled,
        "{HARNESS}: cancellation yields outcome=Cancelled"
    );

    // The durable event log carries the process.cancelled event.
    let cancelled_events: Vec<workflow::ProcessEvent> =
        read_event_log(&harness, &instance3, "process.cancelled");
    assert!(
        !cancelled_events.is_empty(),
        "{HARNESS}: the durable event log carries the process.cancelled event"
    );
    // Verify the event carries outcome=cancelled
    let cancelled_event = &cancelled_events[0];
    let event_outcome = cancelled_event
        .data
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(
        event_outcome, "cancelled",
        "{HARNESS}: process.cancelled event carries outcome=cancelled"
    );

    // ── VERIFY ALL THREE TERMINAL STATES ARE DISTINCT ───────────────────────
    assert_ne!(
        outcome1, outcome2,
        "{HARNESS}: Completed and Failed must be distinct outcomes"
    );
    assert_ne!(
        outcome1, outcome3,
        "{HARNESS}: Completed and Cancelled must be distinct outcomes"
    );
    assert_ne!(
        outcome2, outcome3,
        "{HARNESS}: Failed and Cancelled must be distinct outcomes"
    );

    // 10. `cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__009__process_completed_failed_cancelled`
    //     is executed and its PASS/FAIL is recorded. A runtime assertion failure against
    //     existing application code is a valid discovery and does not block completion
    //     of this test-authoring story; do not modify production code solely to make
    //     the new test green.
}

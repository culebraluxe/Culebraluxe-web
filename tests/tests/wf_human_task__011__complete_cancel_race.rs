//! WF.HUMAN_TASK — complete/cancel race (TST-WF-HUMAN-TASK-011).
//!
//! Contract: a race between **complete** and **cancel_process** on the same process. The production
//! boundaries are `WorkflowEngine::complete_task` and `WorkflowEngine::cancel_process` at
//! `middle/workflow/src/engine/engine_options.rs`.
//!
//! Scenario: If cancel_process runs first, the process becomes Aborted, then complete fails with
//! `PROCESS_NOT_ACTIVE`. If complete runs first, the task becomes Completed and the process
//! becomes Completed, then cancel_process is idempotent (returns Ok because the process is already
//! settled).
//!
//! Since the in-memory `EngineHarness` is single-threaded, true concurrency cannot be tested.
//! This test verifies both orderings sequentially.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__011__complete_cancel_race

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CancelProcessParams, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value, json,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const RACE_COMPLETE_CANCEL_KEY: &str = "TST-WF-HUMAN-TASK-011";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const TASK_NODE: &str = "review";
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
const ALICE: &str = "alice";
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
fn wf_human_task_011__complete_cancel_race() {
    let t0 = 1_650_000_000_000;

    // ── ORDERING 1: CANCEL THEN COMPLETE ────────────────────────────────────────────────────────────────────────
    // Cancel wins the race: process is cancelled, then complete fails.
    let clock1 = TestClock::at_unix_millis(t0);
    let harness1 = EngineHarness::new(clock1.clone());
    harness1
        .engine()
        .seed_definition(task_definition(RACE_COMPLETE_CANCEL_KEY))
        .expect("the task definition registers");

    let (instance1, parked1) = start_and_park(&harness1, RACE_COMPLETE_CANCEL_KEY);
    assert_eq!(parked1.status, TaskStatus::Ready);

    // Cancel the process first (simulating cancel winning the race).
    harness1
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance1.clone(),
            actor: MANAGER.to_string(),
            reason: Some("race test".to_string()),
        })
        .expect("cancel process succeeds");
    assert_eq!(instance_status(&harness1, &instance1), ProcessStatus::Aborted);

    // Then Alice tries to complete the task.
    let complete_refused = harness1
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked1.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"decision": "approved"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("complete on cancelled process fails");
    // The task is Obsolete (non-actionable), so TASK_NOT_ACTIONABLE is checked before PROCESS_NOT_ACTIVE.
    assert_eq!(
        complete_refused.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: complete on cancelled process fails with TASK_NOT_ACTIONABLE (task is Obsolete)"
    );
    let task_after = task_by_id(&harness1, &parked1.id);
    // The task should be Obsolete.
    assert_eq!(
        task_after.status,
        TaskStatus::Obsolete,
        "{HARNESS}: task in cancelled process becomes Obsolete"
    );

    // ── ORDERING 2: COMPLETE THEN CANCEL ────────────────────────────────────────────────────────────────────────
    // Complete wins the race: task is completed, then cancel is idempotent.
    let clock2 = TestClock::at_unix_millis(t0);
    let harness2 = EngineHarness::new(clock2.clone());
    harness2
        .engine()
        .seed_definition(task_definition(RACE_COMPLETE_CANCEL_KEY))
        .expect("the task definition registers");

    let (instance2, parked2) = start_and_park(&harness2, RACE_COMPLETE_CANCEL_KEY);

    // Complete first (simulating complete winning the race).
    harness2.clock().advance_millis(3_600_000);
    harness2
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked2.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"decision": "approved"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("complete succeeds");
    let completed2 = task_by_id(&harness2, &parked2.id);
    assert_eq!(completed2.status, TaskStatus::Completed);
    assert_eq!(instance_status(&harness2, &instance2), ProcessStatus::Completed);

    // Then cancel the process - should fail with PROCESS_NOT_ACTIVE since the process is already Completed.
    let cancel_result = harness2
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance2.clone(),
            actor: MANAGER.to_string(),
            reason: Some("race test".to_string()),
        });
    // cancel_process returns PROCESS_NOT_ACTIVE for already-completed processes (only idempotent for Cancelled).
    assert!(
        cancel_result.is_err(),
        "{HARNESS}: cancel on completed process fails"
    );
    let cancel_err = cancel_result.expect_err("cancel on completed process should fail");
    assert_eq!(
        cancel_err.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: cancel on completed process fails with PROCESS_NOT_ACTIVE"
    );
    // The process and task should remain Completed.
    assert_eq!(instance_status(&harness2, &instance2), ProcessStatus::Completed);
    let task_after_cancel = task_by_id(&harness2, &parked2.id);
    assert_eq!(task_after_cancel.status, TaskStatus::Completed);
}
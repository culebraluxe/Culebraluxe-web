//! WF.HUMAN_TASK — claim/cancel race (TST-WF-HUMAN-TASK-010).
//!
//! Contract: a race between **claim** and **cancel_process** on the same process. The production
//! boundaries are `WorkflowEngine::claim_task` and `WorkflowEngine::cancel_process` at
//! `middle/workflow/src/engine/engine_options.rs`.
//!
//! Scenario: If cancel_process runs first, the process becomes Cancelled, then claim fails with
//! `PROCESS_NOT_ACTIVE`. If claim runs first, the task becomes Reserved, then cancel_process
//! succeeds (cancelling the process and marking the task Obsolete).
//!
//! Since the in-memory `EngineHarness` is single-threaded, true concurrency cannot be tested.
//! This test verifies both orderings sequentially.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__010__claim_cancel_race

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CancelProcessParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph,
    ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus, TransitionDefinition,
    Value,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const RACE_CLAIM_CANCEL_KEY: &str = "TST-WF-HUMAN-TASK-010";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const TASK_NODE: &str = "review";
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
const ALICE: &str = "alice";
const BOB: &str = "bob";
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
fn wf_human_task_010__claim_cancel_race() {
    let t0 = 1_650_000_000_000;

    // ── ORDERING 1: CANCEL THEN CLAIM ──────────────────────────────────────────────────────────────────────────
    // Cancel wins the race: process is cancelled, then claim fails.
    let clock1 = TestClock::at_unix_millis(t0);
    let harness1 = EngineHarness::new(clock1.clone());
    harness1
        .engine()
        .seed_definition(task_definition(RACE_CLAIM_CANCEL_KEY))
        .expect("the task definition registers");

    let (instance1, parked1) = start_and_park(&harness1, RACE_CLAIM_CANCEL_KEY);
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
    assert_eq!(
        instance_status(&harness1, &instance1),
        ProcessStatus::Aborted
    );

    // Then Alice tries to claim the task.
    let claim_refused = harness1
        .engine()
        .claim_task(&parked1.id, ALICE)
        .expect_err("claim on cancelled process fails");
    assert_eq!(
        claim_refused.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: claim on cancelled process fails with PROCESS_NOT_ACTIVE"
    );
    let task_after = task_by_id(&harness1, &parked1.id);
    // The task should be Obsolete (cancelled process marks tasks obsolete).
    assert_eq!(
        task_after.status,
        TaskStatus::Obsolete,
        "{HARNESS}: task in cancelled process becomes Obsolete"
    );

    // ── ORDERING 2: CLAIM THEN CANCEL ──────────────────────────────────────────────────────────────────────────
    // Claim wins the race: task is claimed, then process is cancelled.
    let clock2 = TestClock::at_unix_millis(t0);
    let harness2 = EngineHarness::new(clock2.clone());
    harness2
        .engine()
        .seed_definition(task_definition(RACE_CLAIM_CANCEL_KEY))
        .expect("the task definition registers");

    let (instance2, parked2) = start_and_park(&harness2, RACE_CLAIM_CANCEL_KEY);

    // Claim first (simulating claim winning the race).
    harness2.clock().advance_millis(3_600_000);
    harness2
        .engine()
        .claim_task(&parked2.id, ALICE)
        .expect("claim succeeds");
    let claimed2 = task_by_id(&harness2, &parked2.id);
    assert_eq!(claimed2.status, TaskStatus::Reserved);
    assert_eq!(claimed2.assignee.as_deref(), Some(ALICE));

    // Then cancel the process.
    harness2
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance2.clone(),
            actor: MANAGER.to_string(),
            reason: Some("race test".to_string()),
        })
        .expect("cancel process succeeds after claim");
    assert_eq!(
        instance_status(&harness2, &instance2),
        ProcessStatus::Aborted
    );
    // The task should now be Obsolete.
    let task_after_cancel = task_by_id(&harness2, &parked2.id);
    assert_eq!(
        task_after_cancel.status,
        TaskStatus::Obsolete,
        "{HARNESS}: task in cancelled process becomes Obsolete even after claim"
    );
}

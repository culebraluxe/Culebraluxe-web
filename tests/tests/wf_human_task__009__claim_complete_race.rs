//! WF.HUMAN_TASK — claim/complete race (TST-WF-HUMAN-TASK-009).
//!
//! Contract: a race between **claim** and **complete** on the same task. The production boundaries
//! are `WorkflowEngine::claim_task` and `WorkflowEngine::complete_task` at
//! `middle/workflow/src/engine/engine_options.rs`. Both use CAS on the task version.
//!
//! Scenario: An open task (no assignee, no candidates) can be either claimed or completed by any
//! user. If claim runs first, the task becomes Reserved, then complete by the same user succeeds.
//! If complete runs first, the task becomes Completed, then claim fails with `TASK_NOT_CLAIMABLE`
//! (or `PROCESS_NOT_ACTIVE` if the process completes).
//!
//! Since the in-memory `EngineHarness` is single-threaded, true concurrency cannot be tested.
//! This test verifies both orderings sequentially.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__009__claim_complete_race

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    json, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const RACE_CLAIM_COMPLETE_KEY: &str = "TST-WF-HUMAN-TASK-009";
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
fn wf_human_task_009__claim_complete_race() {
    let t0 = 1_650_000_000_000;

    // ── ORDERING 1: CLAIM THEN COMPLETE (SAME USER) ────────────────────────────────────────────────────────────
    // This simulates: claim wins the race, then the same user completes.
    let clock1 = TestClock::at_unix_millis(t0);
    let harness1 = EngineHarness::new(clock1.clone());
    harness1
        .engine()
        .seed_definition(task_definition(RACE_CLAIM_COMPLETE_KEY))
        .expect("the task definition registers");

    let (instance1, parked1) = start_and_park(&harness1, RACE_CLAIM_COMPLETE_KEY);
    assert_eq!(parked1.status, TaskStatus::Ready);
    assert!(parked1.candidates.is_empty());

    // Claim first (simulating claim winning the race).
    harness1.clock().advance_millis(3_600_000);
    harness1
        .engine()
        .claim_task(&parked1.id, ALICE)
        .expect("claim succeeds");
    let claimed1 = task_by_id(&harness1, &parked1.id);
    assert_eq!(claimed1.status, TaskStatus::Reserved);
    assert_eq!(claimed1.assignee.as_deref(), Some(ALICE));

    // Then complete by the same user.
    harness1.clock().advance_millis(3_600_000);
    harness1
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked1.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"decision": "approved"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("complete after claim succeeds");
    let completed1 = task_by_id(&harness1, &parked1.id);
    assert_eq!(completed1.status, TaskStatus::Completed);
    assert_eq!(completed1.assignee.as_deref(), Some(ALICE));
    assert_eq!(
        instance_status(&harness1, &instance1),
        ProcessStatus::Completed
    );

    // ── ORDERING 2: COMPLETE THEN CLAIM (DIFFERENT USERS) ──────────────────────────────────────────────────────
    // This simulates: complete wins the race (by a different user on an open task), then claim fails.
    let clock2 = TestClock::at_unix_millis(t0);
    let harness2 = EngineHarness::new(clock2.clone());
    harness2
        .engine()
        .seed_definition(task_definition(RACE_CLAIM_COMPLETE_KEY))
        .expect("the task definition registers");

    let (instance2, parked2) = start_and_park(&harness2, RACE_CLAIM_COMPLETE_KEY);

    // Complete first by Alice (simulating complete winning the race on an open task).
    harness2.clock().advance_millis(3_600_000);
    harness2
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked2.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"decision": "approved"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("complete on open task succeeds");
    let completed2 = task_by_id(&harness2, &parked2.id);
    assert_eq!(completed2.status, TaskStatus::Completed);
    assert_eq!(completed2.assignee.as_deref(), Some(ALICE));
    assert_eq!(
        instance_status(&harness2, &instance2),
        ProcessStatus::Completed
    );

    // Then Bob tries to claim the already-completed task.
    let claim_refused = harness2
        .engine()
        .claim_task(&parked2.id, BOB)
        .expect_err("claim on completed task fails");
    // The process is completed, so PROCESS_NOT_ACTIVE is checked first.
    assert_eq!(
        claim_refused.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: claim on completed task's process fails with PROCESS_NOT_ACTIVE"
    );
    let task_after = task_by_id(&harness2, &parked2.id);
    assert_eq!(task_after.status, TaskStatus::Completed);

    // ── ORDERING 3: COMPLETE THEN CLAIM (SAME USER) ────────────────────────────────────────────────────────────
    // Complete by Alice, then Alice tries to claim (should also fail).
    let clock3 = TestClock::at_unix_millis(t0);
    let harness3 = EngineHarness::new(clock3.clone());
    harness3
        .engine()
        .seed_definition(task_definition(RACE_CLAIM_COMPLETE_KEY))
        .expect("the task definition registers");

    let (instance3, parked3) = start_and_park(&harness3, RACE_CLAIM_COMPLETE_KEY);
    harness3.clock().advance_millis(3_600_000);
    harness3
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked3.id.clone(),
            user_id: ALICE.to_string(),
            form_data: json!({"decision": "approved"}),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("complete succeeds");
    let claim_refused3 = harness3
        .engine()
        .claim_task(&parked3.id, ALICE)
        .expect_err("claim on completed task fails even for completer");
    assert_eq!(
        claim_refused3.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: claim by completer on completed task also fails"
    );
}

//! WF.HUMAN_TASK — release (TST-WF-HUMAN-TASK-005).
//!
//! Contract: a human task's assignee may **release** the task, returning it to the `Ready` state
//! for another user to claim. `WorkflowEngine::release_task` at
//! `middle/workflow/src/engine/engine_options.rs:258` is the production boundary: it locks the
//! instance and the task, admits only an active process and a task in `Reserved` or `InProgress`
//! status (`engine_options.rs:262-279`), verifies the caller is the current assignee
//! (`engine_options.rs:268-273`), and then moves the task to `status = Ready`, `assignee = None`,
//! `claimed_at = None`, `version += 1`, with a durable `task.released` event
//! (`engine_options.rs:292-303`).
//!
//! The release is refused with `TASK_ASSIGNEE_ONLY` if the caller is not the assignee. It is
//! refused with `TASK_NOT_RELEASABLE` if the task is not `Reserved` or `InProgress` (e.g., `Ready`,
//! `Completed`). It is refused with `PROCESS_NOT_ACTIVE` if the process is not active.
//!
//! This exercises the production boundary. The real `WorkflowEngine<MemoryStore>` is driven through
//! `seed_definition`, `start_process`, `claim_task` (to set an assignee), and `release_task`.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__005__release

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
const RELEASE_KEY: &str = "TST-WF-HUMAN-TASK-005";
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
fn wf_human_task_005__release() {
    let t0 = 1_650_000_000_000;
    let clock = TestClock::at_unix_millis(t0);
    let harness = EngineHarness::new(clock.clone());
    harness
        .engine()
        .seed_definition(task_definition(RELEASE_KEY))
        .expect("the task definition registers");

    // ── 1. SETUP: CLAIM THE TASK ───────────────────────────────────────────────────────────────────────────────
    let (instance, parked) = start_and_park(&harness, RELEASE_KEY);
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

    // ── 2. POSITIVE — THE ASSIGNEE RELEASES THE TASK ───────────────────────────────────────────────────────────
    harness.clock().advance_millis(3_600_000);
    harness
        .engine()
        .release_task(&parked.id, ALICE)
        .expect("the assignee releases the task");
    let released = task_by_id(&harness, &parked.id);
    assert_eq!(
        released.status,
        TaskStatus::Ready,
        "{HARNESS}: release returns the task to Ready"
    );
    assert_eq!(
        released.assignee, None,
        "{HARNESS}: release clears the assignee"
    );
    assert_eq!(
        released.claimed_at, None,
        "{HARNESS}: release clears claimed_at"
    );
    assert_eq!(
        released.version,
        claimed.version + 1,
        "{HARNESS}: release bumps the version"
    );
    // The release is durable and attributed.
    let released_events = events_of_type(&harness, &instance, "task.released");
    assert_eq!(
        released_events.len(),
        1,
        "{HARNESS}: the release emits exactly one task.released event"
    );
    assert_eq!(released_events[0].actor, ALICE);
    assert_eq!(released_events[0].task_id.as_deref(), Some(parked.id.as_str()));
    assert_eq!(released_events[0].process_instance_id, instance);

    // ── 3. NEGATIVE — A NON-ASSIGNEE CANNOT RELEASE ────────────────────────────────────────────────────────────
    // Claim again (by Alice), then Bob tries to release.
    harness
        .engine()
        .claim_task(&parked.id, ALICE)
        .expect("Alice claims again");
    let before_bob = fingerprint(&task_by_id(&harness, &parked.id));
    let refused = harness
        .engine()
        .release_task(&parked.id, BOB)
        .expect_err("only the assignee can release");
    assert_eq!(
        refused.code(),
        "TASK_ASSIGNEE_ONLY",
        "{HARNESS}: release by non-assignee is refused"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_bob,
        "{HARNESS}: the refused release commits nothing"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.released").len(),
        1,
        "{HARNESS}: the refused release emits no further task.released event"
    );

    // ── 4. NEGATIVE — RELEASE A READY TASK IS REFUSED ────────────────────────────────────────────────────────────
    // Release the task (back to Ready), then try to release again. The task has no assignee (cleared on
    // release), so the assignee check fails first with TASK_ASSIGNEE_ONLY (checked before status).
    harness
        .engine()
        .release_task(&parked.id, ALICE)
        .expect("Alice releases the task");
    let ready_task = task_by_id(&harness, &parked.id);
    assert_eq!(ready_task.status, TaskStatus::Ready);
    assert_eq!(ready_task.assignee, None);
    let before_ready_release = fingerprint(&ready_task);
    let refused_ready = harness
        .engine()
        .release_task(&parked.id, ALICE)
        .expect_err("a Ready task with no assignee cannot be released");
    assert_eq!(
        refused_ready.code(),
        "TASK_ASSIGNEE_ONLY",
        "{HARNESS}: release of a Ready task with no assignee is refused at the assignee check"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_ready_release,
        "{HARNESS}: the refused release of Ready task commits nothing"
    );

    // ── 5. NEGATIVE — RELEASE A COMPLETED TASK IS REFUSED ──────────────────────────────────────────────────────
    // Claim and complete, then try to release.
    harness
        .engine()
        .claim_task(&parked.id, ALICE)
        .expect("Alice claims");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked.id.clone(),
            user_id: ALICE.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("Alice completes");
    let complete_refused = harness
        .engine()
        .release_task(&parked.id, ALICE)
        .expect_err("a completed task cannot be released");
    // Process is completed, so PROCESS_NOT_ACTIVE is checked first.
    assert_eq!(
        complete_refused.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: a completed task's process is not active"
    );
}
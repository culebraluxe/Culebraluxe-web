//! WF.JOIN — required children all complete → one join (TST-WF-JOIN-001).
//!
//! Contract: at a `join`, the gate is that **every required child has arrived**. While any required child is still
//! active the join holds back; the moment the last required child arrives the join fires **exactly once** — one
//! `token.joined` event, one result token — and the process converges. Optional siblings are settled by the join;
//! two arrivals of required children cannot each fire their own join.
//!
//! This is the production boundary, exercised for real: the `WorkflowEngine<MemoryStore>` is driven through
//! `start_process` and `complete_task`, and the gate is `count_required_active_siblings`
//! (`middle/workflow/src/engine/handle_join.rs:38-44`, counting only `Active && required` tokens,
//! `middle/workflow/src/memory.rs:283-294`). The negative half proves the gate is the required-child set, not
//! "any child arrived": after the first of two required children completes, no join exists, the instance is still
//! active, and the sibling count is the reason.
//!
//! ```text
//! start -> fan (fork, two required branches)
//!            |-- approve (task) -> converge (join) -> settle (end)
//!            '-- review  (task) -> converge
//! ```
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Deterministic and isolated: fixed `TestClock`, in-memory
//! store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__001__required_children_all_complete_one_join

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-JOIN-001";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
const MAIN_NODE: &str = "approve";
const REVIEW_NODE: &str = "review";
const END_NODE: &str = "settle";
const BRANCH_TRANSITION: &str = "go";

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

/// `start -> fan (2 required branches) -> two task branches -> converge (join) -> settle (end)`.
fn definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            name: Some("Fan".to_string()),
            // Both branches declare no `required`; the fork rule
            // (`transition.required.unwrap_or(true)`, `execute_node_leave.rs:388`) makes them required.
            transitions: Some(vec![
                transition("main", MAIN_NODE, None),
                transition("review", REVIEW_NODE, None),
            ]),
            ..Default::default()
        },
    );
    for (id, name) in [(MAIN_NODE, "Approve"), (REVIEW_NODE, "Review")] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.to_string(),
                node_type: "task".to_string(),
                name: Some(name.to_string()),
                transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE, None)]),
                ..Default::default()
            },
        );
    }
    nodes.insert(
        JOIN_NODE.to_string(),
        NodeDefinition {
            id: JOIN_NODE.to_string(),
            node_type: "join".to_string(),
            name: Some("Converge".to_string()),
            transitions: Some(vec![transition("next", END_NODE, None)]),
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
        id: "tst-wf-join-001".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 001".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_params() -> StartProcessParams {
    StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
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
        .unwrap_or_else(|| panic!("a task exists at {node}"))
}

fn history(harness: &EngineHarness, instance: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(
    harness: &EngineHarness,
    instance: &str,
    event_type: &str,
) -> Vec<ProcessEvent> {
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

fn complete(harness: &EngineHarness, task_id: &str) -> workflow::Result<()> {
    harness.engine().complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(BRANCH_TRANSITION.to_string()),
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-001); the file and the assay use it.
fn wf_join_001__required_children_all_complete_one_join() {
    let clock = TestClock::at_unix_millis(1_700_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(definition())
        .expect("the definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the fork process starts and parks its branches");
    let instance = started.process_instance_id.clone();

    // Both branches parked as required: the join's gate counts exactly two active required siblings.
    let main_task = task_at(&harness, &instance, MAIN_NODE);
    let review_task = task_at(&harness, &instance, REVIEW_NODE);

    // ── NEGATIVE: one of two required children arriving must NOT fire the join ──────────────────────────────────
    complete(&harness, &main_task.id).expect("the first required branch completes into the join");
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: the join holds back while a required child is still active"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "token.joined").len(),
        0,
        "{HARNESS}: no join fires before every required child has arrived"
    );
    assert_eq!(
        task_at(&harness, &instance, REVIEW_NODE).status,
        TaskStatus::Ready,
        "{HARNESS}: the still-required sibling is untouched"
    );

    // ── POSITIVE: the last required child fires the join EXACTLY ONCE ───────────────────────────────────────────
    complete(&harness, &review_task.id).expect("the last required branch completes into the join");
    let joined = events_of_type(&harness, &instance, "token.joined");
    assert_eq!(
        joined.len(),
        1,
        "{HARNESS}: the last required arrival fires exactly one join"
    );
    assert_eq!(
        joined[0]
            .data
            .get("branches")
            .and_then(Value::as_array)
            .map(|b| b.len()),
        Some(2),
        "{HARNESS}: the join accounts for both required branches"
    );
    // Both branch tasks are spent; nothing remains parked for the completed process to wait on.
    assert_eq!(
        task_at(&harness, &instance, MAIN_NODE).status,
        TaskStatus::Completed,
        "{HARNESS}: the first branch completed"
    );
    assert_eq!(
        task_at(&harness, &instance, REVIEW_NODE).status,
        TaskStatus::Completed,
        "{HARNESS}: the second branch completed"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the join drives the process to completion"
    );
    // One downstream token only; re-entering either spent branch cannot fire a second join.
    let replay = complete(&harness, &main_task.id)
        .expect_err("a spent branch cannot re-enter the join");
    assert_eq!(
        replay.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: a spent branch is refused, not re-run"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "token.joined").len(),
        1,
        "{HARNESS}: the refused replay produces no second join"
    );
}

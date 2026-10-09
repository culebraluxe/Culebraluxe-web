//! WF.HUMAN_TASK — obsolete task cannot be acted upon (TST-WF-HUMAN-TASK-013).
//!
//! Contract: a task that has been *obsoleted* by the engine — here, the open task on an optional branch the join
//! retired — is no longer work. The production boundary refuses every action on it: completing it fails with
//! `TASK_NOT_ACTIONABLE` (`complete_task` obsolete-check, `engine_options.rs:375-381`) and claiming it fails at
//! the boundary's first gate because the join that obsoleted it also completed the process
//! (`PROCESS_NOT_ACTIVE`, `claim_task`, `engine_options.rs:194-201`). The refusal must commit nothing: the task
//! row keeps its `Obsolete` status, its version does not move, and no new lifecycle event is emitted. Obsolete is
//! terminal for the task even though the *process* the task belonged to later completed.
//!
//! This is the production boundary, not a re-declaration of it: the real `WorkflowEngine<MemoryStore>` is driven
//! through `start_process` and `complete_task`, and every assertion reads the row state and the durable event log
//! back through the production `Store`. No provider is touched.
//!
//! ```text
//! start -> fan (fork: 1 required + 1 optional)
//!            |-- main (required) -> approve (task) -> converge (join) -> settle (end)
//!            '-- hold (optional) -> parking (task) -> converge
//! ```
//!
//! `parking` is left parked while the required branch completes; the join then retires the optional branch, which
//! is what sets its task `Obsolete` — the exact production path under examination
//! (`handle_join.rs:46-80`, `obsolete_task` from the retirement loop, `execute_node_leave.rs:59-61`).
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated: fixed `TestClock`, in-memory
//! store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__013__obsolete_task_cannot_be_acted_upon

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L3 Composition";
const DEFINITION_KEY: &str = "TST-WF-HUMAN-TASK-013";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
const MAIN_NODE: &str = "approve";
const HOLD_NODE: &str = "parking";
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

/// `start -> fan -> one required branch (approve task) + one optional branch (parking task) -> converge -> settle`.
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
            transitions: Some(vec![
                transition("main", MAIN_NODE, None),
                transition("hold", HOLD_NODE, Some(false)),
            ]),
            ..Default::default()
        },
    );
    for (id, name) in [(MAIN_NODE, "Approve"), (HOLD_NODE, "Parking")] {
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
        id: "tst-wf-human-task-013".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.HUMAN_TASK 013".to_string(),
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

fn task_by_id(harness: &EngineHarness, id: &str) -> Task {
    harness
        .store()
        .with_tx(|tx| tx.get_task(id))
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

fn complete(harness: &EngineHarness, task_id: &str) -> workflow::Result<()> {
    harness.engine().complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(BRANCH_TRANSITION.to_string()),
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-HUMAN-TASK-013); the file and the assay use it.
fn wf_human_task_013__obsolete_task_cannot_be_acted_upon() {
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

    // The optional branch parks on a task; the required branch is the one the join will wait for.
    let hold_task = task_at(&harness, &instance, HOLD_NODE);
    assert_eq!(
        hold_task.status,
        TaskStatus::Ready,
        "{HARNESS}: the optional branch's task starts Ready"
    );
    let main_task = task_at(&harness, &instance, MAIN_NODE);

    // The required branch completes; the join retires the optional branch, which must obsolete its open task.
    complete(&harness, &main_task.id).expect("the required branch completes into the join");
    let obsoleted = task_by_id(&harness, &hold_task.id);
    assert_eq!(
        obsoleted.status,
        TaskStatus::Obsolete,
        "{HARNESS}: the join obsoletes the optional branch's open task"
    );
    // The obsoletion is announced exactly once, naming this task — not zero times, and not a second branch's task.
    let obsoleted_events = events_of_type(&harness, &instance, "task.obsoleted");
    assert_eq!(
        obsoleted_events.len(),
        1,
        "{HARNESS}: the join emits exactly one task.obsoleted"
    );
    assert_eq!(
        obsoleted_events[0].task_id.as_deref(),
        Some(hold_task.id.as_str()),
        "{HARNESS}: the obsoletion names the optional branch's task"
    );
    // The join completed the process, so the task stayed Obsolete while everything downstream finished.
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .expect("the instance is readable")
            .status,
        ProcessStatus::Completed,
        "{HARNESS}: the join drove the process to completion"
    );

    // ── NEGATIVE / REFUSAL: the obsolete task cannot be completed ────────────────────────────────────────────────
    // Completing is the one thing the process did not do for it: Obsolete must refuse. The refusal commits nothing —
    // the row keeps its exact shape and no lifecycle event is added.
    let version_before = obsoleted.version;
    let refused =
        complete(&harness, &hold_task.id).expect_err("an obsolete task cannot be completed");
    assert_eq!(
        refused.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the refusal names the not-actionable rule, not a generic conflict"
    );
    let after = task_by_id(&harness, &hold_task.id);
    assert_eq!(
        after.status,
        TaskStatus::Obsolete,
        "{HARNESS}: the refused completion leaves the task Obsolete"
    );
    assert_eq!(
        after.version, version_before,
        "{HARNESS}: a refused completion does not bump the task version"
    );
    assert_eq!(
        after.completed_at, None,
        "{HARNESS}: a refused completion records no completion instant"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.completed").len(),
        1,
        "{HARNESS}: the refused completion emits no task.completed (the only one is the required branch's)"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.obsoleted").len(),
        1,
        "{HARNESS}: the refused completion emits no second task.obsoleted"
    );

    // ── NEGATIVE / REFUSAL: the obsolete task cannot be claimed ──────────────────────────────────────────────────
    // `claim_task` orders its gates instance-first (`engine_options.rs:194-207`), so on a process the join has
    // already completed the refusal is named PROCESS_NOT_ACTIVE — the claim is refused either way, which is the
    // contract under test. It must commit nothing: no assignee, no version bump, no task.claimed event.
    let claimed = harness
        .engine()
        .claim_task(&hold_task.id, STARTED_BY)
        .expect_err("an obsolete task cannot be claimed");
    assert_eq!(
        claimed.code(),
        "PROCESS_NOT_ACTIVE",
        "{HARNESS}: the claim is refused at the boundary's first gate"
    );
    let after_claim = task_by_id(&harness, &hold_task.id);
    assert_eq!(
        after_claim.status,
        TaskStatus::Obsolete,
        "{HARNESS}: a refused claim leaves the task Obsolete"
    );
    assert_eq!(
        after_claim.assignee, None,
        "{HARNESS}: a refused claim records no assignee"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.claimed").len(),
        0,
        "{HARNESS}: a refused claim emits no task.claimed"
    );

    // ── NEGATIVE / CONTROL: the refusal is specific to the obsolete task, not the boundary of claim ─────────────
    // The refusal must be caused by the obsolete status. The required branch's task is Completed, so acting on it is
    // also refused — but with a *different* rule (`TASK_ALREADY_COMPLETED` on complete). A boundary that refused
    // everything with the same code would satisfy the assertions above while no longer distinguishing "obsolete":
    // naming the distinct rule proves the obsolete refusal is a real check, not a blanket denial.
    let spent = complete(&harness, &main_task.id)
        .expect_err("the spent required task cannot be completed again");
    assert_eq!(
        spent.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: a completed task is a different refusal from an obsolete one"
    );
}

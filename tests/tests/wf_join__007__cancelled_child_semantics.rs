//! WF.JOIN — cancelled child semantics (TST-WF-JOIN-007).
//!
//! Contract, two halves, both from the production rules:
//!
//! A. **An optional child that ends cancelled neither terminates the process nor blocks the join.** An optional
//!    token that reaches an end node declared `ProcessOutcome::Cancelled` is concluded `Cancelled` itself
//!    (`token_outcome_for_end`, `execute_node_leave.rs:44`), and because `token.required == false` the
//!    process-level consequence is skipped — `resolve_process_after_token` only terminates the process for a
//!    *required* cancelled token (`execute_node_leave.rs:286-297`). That concluded sibling is also not "skipped"
//!    later: the join's skip set is the optional siblings *still active*
//!    (`list_optional_active_siblings`, `memory.rs:296-308`), so the cancelled-concluded branch earns no
//!    `token.skipped` — it had already exited. When the required siblings all arrive, the join still fires exactly
//!    once.
//!
//! B. **A required child that ends cancelled terminates the process as cancelled.** The same helper's other arm
//!    (`resolve_process_after_token`) terminates the whole process for a required cancelled child, cancelling every
//!    other active token and opening no join: the process ends `Aborted`/`Cancelled`, and nothing reactivates it.
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Two graphs, two cases. Deterministic and isolated: fixed
//! `TestClock`, in-memory store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__007__cancelled_child_semantics

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TokenOutcome, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const STARTED_BY: &str = "tst";
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
const MAIN_NODE: &str = "approve";
const OPTIONAL_TASK_NODE: &str = "scuttle";
const OPTIONAL_END_NODE: &str = "void";
const DECIDE_NODE: &str = "decide";
const HOLD_NODE: &str = "hold";
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

/// Case A: `fan` forks a required branch (approve task) and an *optional* branch whose chain ends cancelled.
///
/// ```text
/// start -> fan (fork)
///            |-- main   (required) -> approve (task) -> converge (join) -> settle (end)
///            '-- extra  (optional) -> scuttle (task) -> void (end, outcome Cancelled)
/// ```
fn optional_cancelled_graph() -> ProcessDefinition {
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
                transition("extra", OPTIONAL_TASK_NODE, Some(false)),
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        MAIN_NODE.to_string(),
        NodeDefinition {
            id: MAIN_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Approve".to_string()),
            transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPTIONAL_TASK_NODE.to_string(),
        NodeDefinition {
            id: OPTIONAL_TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Scuttle".to_string()),
            transitions: Some(vec![transition(BRANCH_TRANSITION, OPTIONAL_END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPTIONAL_END_NODE.to_string(),
        NodeDefinition {
            id: OPTIONAL_END_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Cancelled),
            ..Default::default()
        },
    );
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
        id: "tst-wf-join-007-a".to_string(),
        tenant_id: None,
        key: "TST-WF-JOIN-007-A".to_string(),
        version: 1,
        name: "TST WF.JOIN 007 A".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// Case B: `fan` forks two required branches, one of which completes into a cancelled end node.
///
/// ```text
/// start -> fan (fork)
///            |-- main (required) -> decide (task) -> void (end, outcome Cancelled)
///            '-- hold (required) -> hold (task) -> converge (join) -> settle (end)
/// ```
fn required_cancelled_graph() -> ProcessDefinition {
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
                transition("main", DECIDE_NODE, None),
                transition("hold", HOLD_NODE, None),
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE_NODE.to_string(),
        NodeDefinition {
            id: DECIDE_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Decide".to_string()),
            transitions: Some(vec![transition(BRANCH_TRANSITION, OPTIONAL_END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPTIONAL_END_NODE.to_string(),
        NodeDefinition {
            id: OPTIONAL_END_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Cancelled),
            ..Default::default()
        },
    );
    nodes.insert(
        HOLD_NODE.to_string(),
        NodeDefinition {
            id: HOLD_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Hold".to_string()),
            transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE, None)]),
            ..Default::default()
        },
    );
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
        id: "tst-wf-join-007-b".to_string(),
        tenant_id: None,
        key: "TST-WF-JOIN-007-B".to_string(),
        version: 1,
        name: "TST WF.JOIN 007 B".to_string(),
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
        version: Some(1),
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

fn tokens(harness: &EngineHarness, instance: &str) -> Vec<workflow::Token> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance))
        .expect("the instance tokens are readable")
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

fn instance_outcome(harness: &EngineHarness, instance: &str) -> Option<ProcessOutcome> {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .outcome
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-007); the file and the assay use it.
fn wf_join_007__cancelled_child_semantics() {
    // ── CASE A: an OPTIONAL child that ends cancelled neither terminates the process nor blocks the join ──────
    let clock = TestClock::at_unix_millis(1_700_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(optional_cancelled_graph())
        .expect("the case-A definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params("TST-WF-JOIN-007-A"))
        .expect("the case-A process starts and parks its branches");
    let instance = started.process_instance_id.clone();

    // The optional branch runs to its cancelled end; the process must NOT terminate at that moment.
    let optional_task = task_at(&harness, &instance, OPTIONAL_TASK_NODE);
    complete(&harness, &optional_task.id)
        .expect("the optional branch's task completes into its cancelled end");
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: an optional cancelled child does not terminate the process"
    );
    assert_eq!(
        instance_outcome(&harness, &instance),
        None,
        "{HARNESS}: an optional cancelled child records no process outcome"
    );
    let cancelled_token = tokens(&harness, &instance)
        .into_iter()
        .find(|t| t.node_id == OPTIONAL_END_NODE)
        .expect("the cancelled end token exists");
    assert_eq!(
        cancelled_token.outcome,
        Some(TokenOutcome::Cancelled),
        "{HARNESS}: the optional child is concluded Cancelled"
    );

    // The required branch completes; the join fires once. The cancelled-concluded optional sibling is not
    // "skipped" — only still-active optional siblings are — and it does not hold the join back.
    let main_task = task_at(&harness, &instance, MAIN_NODE);
    complete(&harness, &main_task.id).expect("the required branch completes into the join");
    let joined = events_of_type(&harness, &instance, "token.joined");
    assert_eq!(
        joined.len(),
        1,
        "{HARNESS}: the join fires once despite the optional branch's cancelled ending"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "token.skipped").len(),
        0,
        "{HARNESS}: the cancelled-concluded optional sibling is not skip-stamped — it had already exited"
    );
    assert_eq!(
        tokens(&harness, &instance)
            .iter()
            .find(|t| t.node_id == OPTIONAL_END_NODE)
            .expect("the cancelled end token still exists")
            .outcome,
        Some(TokenOutcome::Cancelled),
        "{HARNESS}: the optional child's cancelled conclusion is preserved through the join"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the process completes past the join"
    );

    // ── CASE B: a REQUIRED child that ends cancelled terminates the process ─────────────────────────────────────
    let clock_b = TestClock::at_unix_millis(1_700_000_000_000);
    let harness_b = EngineHarness::new(clock_b);
    harness_b
        .engine()
        .seed_definition(required_cancelled_graph())
        .expect("the case-B definition registers with the engine");
    let started_b = harness_b
        .engine()
        .start_process(start_params("TST-WF-JOIN-007-B"))
        .expect("the case-B process starts and parks its branches");
    let instance_b = started_b.process_instance_id.clone();
    let hold_task = task_at(&harness_b, &instance_b, HOLD_NODE);
    let decide_task = task_at(&harness_b, &instance_b, DECIDE_NODE);

    // The required cancelled child terminates the process immediately, cancelling the other branch too.
    complete(&harness_b, &decide_task.id)
        .expect("the required decide task completes into its cancelled end");
    assert_eq!(
        instance_status(&harness_b, &instance_b),
        ProcessStatus::Aborted,
        "{HARNESS}: a required cancelled child aborts the process"
    );
    assert_eq!(
        instance_outcome(&harness_b, &instance_b),
        Some(ProcessOutcome::Cancelled),
        "{HARNESS}: the cancellation outcome is recorded"
    );
    assert_eq!(
        task_at(&harness_b, &instance_b, HOLD_NODE).status,
        TaskStatus::Obsolete,
        "{HARNESS}: the sibling branch's task is obsoleted by the termination"
    );
    assert_eq!(
        events_of_type(&harness_b, &instance_b, "token.joined").len(),
        0,
        "{HARNESS}: no join fires on a cancelled process"
    );
    // The obsoleted sibling cannot be acted on after the fact: termination stuck.
    let revived = complete(&harness_b, &hold_task.id)
        .expect_err("the cancelled process's obsoleted task cannot be completed");
    assert_eq!(
        revived.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the cancelled process cannot be reactivated through its obsoleted task"
    );
    assert_eq!(
        instance_status(&harness_b, &instance_b),
        ProcessStatus::Aborted,
        "{HARNESS}: the refused completion left the process cancelled"
    );
}

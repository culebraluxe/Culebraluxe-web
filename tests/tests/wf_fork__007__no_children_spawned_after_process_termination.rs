//! WF.FORK — no children spawned after process termination (TST-WF-FORK-007).
//!
//! Contract: the static fork checks the process is still live **before every child it spawns**. `handle_fork`
//! (`middle/workflow/src/engine/execute_node_leave.rs:375-422`) locks the instance ahead of each transition
//! (`execute_node_leave.rs:387-390`) and stops the fan-out the moment the process is no longer `Active` — so a
//! branch that terminates the process (a required token reaching a `Failed` end, `resolve_process_after_token`
//! at `execute_node_leave.rs:280-298`) prevents every later sibling from ever existing: no child row, no
//! `token.forked` event, no task.
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - a fork whose **first** transition ends `Failed` spawns exactly that one child and then stops: one
//!   `token.forked` event (the first transition only), one child token, no tasks, and the process ends
//!   `Error`/`Failed`;
//! - the control fork whose branches all park spawns **all** of its children (three events, three children,
//!   three parked tasks, process still `Active`) — the stop is caused by the termination, not by the fork
//!   refusing to fan out. (A first branch that ends `Completed` synchronously would complete the whole process
//!   before later siblings spawn — `check_process_completion` with zero active tokens — so the control parks
//!   instead; the parked shape is what proves the fan-out itself is complete.)
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__007__no_children_spawned_after_process_termination

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent, ProcessGraph, ProcessOutcome,
    ProcessStatus, StartProcessParams, TaskStatus, TokenStatus, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
/// The fork whose first branch terminates the process: later siblings must never exist.
const HALTING_KEY: &str = "TST-WF-FORK-007-HALTING";
/// The control fork whose first branch completes: every sibling must spawn.
const FLOWING_KEY: &str = "TST-WF-FORK-007-FLOWING";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> fork(first -> end_first, second -> task_b, third -> task_c)`. The first branch ends with
/// `first_outcome`; the later siblings only exist when the process survives it.
fn halting_definition() -> ProcessDefinition {
    fork_shape(HALTING_KEY, HaltingFirst::End(ProcessOutcome::Failed))
}

/// `start -> fork(first -> task_a, second -> task_b, third -> task_c)`: every branch parks, so the whole fan-out
/// completes and the process stays alive on its parked children. The control proves the fork fans out fully
/// when nothing terminates it — the stop in the halting case is caused by the termination, not by the fork.
fn flowing_definition() -> ProcessDefinition {
    fork_shape(FLOWING_KEY, HaltingFirst::Task)
}

/// What the fork's first transition targets: a synchronous end (which may terminate the process mid-fan-out)
/// or a parking task (which never completes synchronously, so the fan-out always finishes).
enum HaltingFirst {
    End(ProcessOutcome),
    Task,
}

fn fork_shape(key: &str, first: HaltingFirst) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE)]),
            ..Default::default()
        },
    );
    let first_target = match first {
        HaltingFirst::End(outcome) => {
            nodes.insert(
                "end_first".to_string(),
                NodeDefinition {
                    id: "end_first".to_string(),
                    node_type: "end".to_string(),
                    outcome: Some(outcome),
                    ..Default::default()
                },
            );
            "end_first"
        }
        HaltingFirst::Task => "task_a",
    };
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition("first", first_target),
                transition("second", "task_b"),
                transition("third", "task_c"),
            ]),
            ..Default::default()
        },
    );
    for task in ["task_a", "task_b", "task_c"] {
        nodes.insert(
            task.to_string(),
            NodeDefinition {
                id: task.to_string(),
                node_type: "task".to_string(),
                name: Some(task.to_string()),
                transitions: Some(vec![transition("done", "end_rest")]),
                ..Default::default()
            },
        );
    }
    nodes.insert(
        "end_rest".to_string(),
        NodeDefinition {
            id: "end_rest".to_string(),
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
            start_node_id: START_NODE.to_string(),
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

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-007); the file and the assay use it.
fn wf_fork_007__no_children_spawned_after_process_termination() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(halting_definition())
        .expect("the halting fork definition registers");
    harness
        .engine()
        .seed_definition(flowing_definition())
        .expect("the flowing fork definition registers");

    // ── 1. TERMINATION STOPS THE FAN-OUT — the first branch kills the process, the rest never exist ──────────
    // The first transition's target ends Failed (required by default), so arriving it terminates the process.
    // The fork must stop: exactly one `token.forked` event naming the first transition, exactly one child
    // token, no tasks anywhere, and the process ends Error/Failed.
    let halting = harness
        .engine()
        .start_process(start_params(HALTING_KEY))
        .expect("the halting fork starts");
    let halting_id = halting.process_instance_id.clone();
    let forked = events_of_type(&harness, &halting_id, "token.forked");
    assert_eq!(
        forked.len(),
        1,
        "{HARNESS}: terminating the process mid-fork leaves exactly one fork event"
    );
    assert_eq!(
        forked[0].data.get("transition").and_then(Value::as_str),
        Some("first"),
        "{HARNESS}: the surviving fork event is the first transition — the one that terminated the process"
    );
    let children = harness
        .store()
        .with_tx(|tx| tx.list_children(&halting.root_token_id))
        .expect("the fork children are readable");
    assert_eq!(
        children.len(),
        1,
        "{HARNESS}: no child row exists for a transition the terminated process never reached"
    );
    assert_eq!(
        children[0].node_id, "end_first",
        "{HARNESS}: the single child is the terminating branch"
    );
    assert_eq!(
        children[0].status,
        TokenStatus::Completed,
        "{HARNESS}: the terminating branch completed at its end"
    );
    let tasks = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&halting_id))
        .expect("the instance tasks are readable");
    assert!(
        tasks.is_empty(),
        "{HARNESS}: a sibling task that was never spawned leaves no task row"
    );
    let ended = harness
        .store()
        .with_tx(|tx| tx.get_instance(&halting_id))
        .expect("the instance is readable");
    assert_eq!(
        ended.status,
        ProcessStatus::Error,
        "{HARNESS}: the first branch terminates the process as Error"
    );
    assert_eq!(
        ended.outcome,
        Some(ProcessOutcome::Failed),
        "{HARNESS}: the termination carries the Failed outcome"
    );

    // ── 2. CONTROL — a surviving process fans out completely ─────────────────────────────────────────────────
    // The parking shape fans out all three children: three fork events (one per transition), three child rows,
    // three parked tasks, and the process stays active. The stop above is caused by the termination, not by the
    // fork refusing to fan out.
    let flowing = harness
        .engine()
        .start_process(start_params(FLOWING_KEY))
        .expect("the flowing fork starts");
    let flowing_id = flowing.process_instance_id.clone();
    let flowing_forked = events_of_type(&harness, &flowing_id, "token.forked");
    assert_eq!(
        flowing_forked.len(),
        3,
        "{HARNESS}: a surviving process emits one fork event per transition"
    );
    let mut names: Vec<&str> = flowing_forked
        .iter()
        .map(|event| {
            event
                .data
                .get("transition")
                .and_then(Value::as_str)
                .expect("every fork event names its transition")
        })
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["first", "second", "third"],
        "{HARNESS}: every declared transition fans out when the process survives"
    );
    let flowing_children = harness
        .store()
        .with_tx(|tx| tx.list_children(&flowing.root_token_id))
        .expect("the fork children are readable");
    assert_eq!(
        flowing_children.len(),
        3,
        "{HARNESS}: a surviving process leaves all three child rows"
    );
    let flowing_tasks = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&flowing_id))
        .expect("the instance tasks are readable");
    assert_eq!(
        flowing_tasks.len(),
        3,
        "{HARNESS}: all three task branches park when nothing terminates the fan-out"
    );
    for task in &flowing_tasks {
        assert_eq!(
            task.status,
            TaskStatus::Ready,
            "{HARNESS}: the surviving branches park Ready"
        );
    }
    let live = harness
        .store()
        .with_tx(|tx| tx.get_instance(&flowing_id))
        .expect("the instance is readable");
    assert_eq!(
        live.status,
        ProcessStatus::Active,
        "{HARNESS}: the control process stays active on its parked branches"
    );
}

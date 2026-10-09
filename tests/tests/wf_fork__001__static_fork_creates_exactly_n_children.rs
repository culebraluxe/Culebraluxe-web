//! WF.FORK — static fork creates exactly N children (TST-WF-FORK-001).
//!
//! Contract: a static `fork` node fans out to **exactly** its declared transitions — one child token per
//! transition, no more and no fewer. `handle_fork` (`middle/workflow/src/engine/execute_node_leave.rs:375-422`)
//! completes the parent token, then walks `node.transitions` in order: one `token.forked` event per transition
//! naming the parent token and the transition (`execute_node_leave.rs:407-418`), and the child arrives at the
//! transition's target immediately (`execute_node_leave.rs:419`).
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - a fork with three transitions spawns exactly three children: three `token.forked` events (one per declared
//!   transition, each naming the parent token), three child token rows (each parented to the fork token, each at
//!   its transition's target), the parent token completed, and three `Ready` tasks parked — with four tokens in
//!   total, so no phantom fourth child exists;
//! - a fork with **zero** transitions spawns nothing: no children, no `token.forked` events, no tasks — the
//!   parent completes and the process completes through it (the early-return path for a transition-less node,
//!   `execute_node_leave.rs:22-40`).
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__001__static_fork_creates_exactly_n_children

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent, ProcessGraph,
    ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus, TokenStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
/// The three-branch fork graph.
const FORK_KEY: &str = "TST-WF-FORK-001";
/// The transition-less fork graph: the zero-children control.
const EMPTY_KEY: &str = "TST-WF-FORK-001-EMPTY";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const END_NODE: &str = "end";
/// The three branch targets; the transition names match so each event is attributable to its declaration.
const BRANCHES: [(&str, &str); 3] = [("a", "task_a"), ("b", "task_b"), ("c", "task_c")];

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

fn fork_definition(key: &str, branches: &[(&str, &str)]) -> ProcessDefinition {
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
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(
                branches
                    .iter()
                    .map(|(name, to)| transition(name, to))
                    .collect(),
            ),
            ..Default::default()
        },
    );
    for (_, target) in branches {
        nodes.insert(
            target.to_string(),
            NodeDefinition {
                id: target.to_string(),
                node_type: "task".to_string(),
                name: Some(target.to_string()),
                transitions: Some(vec![transition("done", END_NODE)]),
                ..Default::default()
            },
        );
    }
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

fn tasks(harness: &EngineHarness, instance: &str) -> Vec<Task> {
    harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(instance))
        .expect("the instance tasks are readable")
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-001); the file and the assay use it.
fn wf_fork_001__static_fork_creates_exactly_n_children() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(fork_definition(FORK_KEY, &BRANCHES))
        .expect("the three-branch fork definition registers");
    harness
        .engine()
        .seed_definition(fork_definition(EMPTY_KEY, &[]))
        .expect("the transition-less fork definition registers");

    // ── 1. THREE TRANSITIONS, THREE CHILDREN ────────────────────────────────────────────────────────────────
    // The fork's three declared transitions produce exactly three children: three `token.forked` events (one per
    // transition, each naming the parent token), three child token rows at the three targets, the parent
    // completed, and three parked tasks. Four tokens in total pins "exactly": a phantom fourth child fails here.
    let started = harness
        .engine()
        .start_process(start_params(FORK_KEY))
        .expect("the fork process starts and fans out");
    let instance = started.process_instance_id.clone();

    let forked = events_of_type(&harness, &instance, "token.forked");
    assert_eq!(
        forked.len(),
        3,
        "{HARNESS}: three declared transitions emit exactly three token.forked events"
    );
    for (name, target) in BRANCHES {
        let event = forked
            .iter()
            .find(|event| event.data.get("transition").and_then(Value::as_str) == Some(name))
            .unwrap_or_else(|| panic!("a token.forked event names transition '{name}'"));
        assert_eq!(
            event.data.get("parentTokenId").and_then(Value::as_str),
            Some(started.root_token_id.as_str()),
            "{HARNESS}: the '{name}' fork event names the fork token as its parent"
        );
        assert_eq!(
            event.node_id.as_deref(),
            Some(target),
            "{HARNESS}: the '{name}' fork event arrives at its declared target '{target}'"
        );
    }

    let children = harness
        .store()
        .with_tx(|tx| tx.list_children(&started.root_token_id))
        .expect("the fork children are readable");
    assert_eq!(
        children.len(),
        3,
        "{HARNESS}: three declared transitions leave exactly three child token rows"
    );
    for (name, target) in BRANCHES {
        let child = children
            .iter()
            .find(|child| child.node_id == target)
            .unwrap_or_else(|| panic!("a child token is parked at '{target}'"));
        assert_eq!(
            child.status,
            TokenStatus::Active,
            "{HARNESS}: the '{name}' child is active at its branch target"
        );
        assert_eq!(
            child.parent_token_id.as_deref(),
            Some(started.root_token_id.as_str()),
            "{HARNESS}: the '{name}' child is parented to the fork token"
        );
    }

    let parent = harness
        .store()
        .with_tx(|tx| tx.get_token(&started.root_token_id))
        .expect("the fork token is readable");
    assert_eq!(
        parent.status,
        TokenStatus::Completed,
        "{HARNESS}: the fork token completes once it has fanned out"
    );

    let all_tokens = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance))
        .expect("the instance tokens are readable");
    assert_eq!(
        all_tokens.len(),
        4,
        "{HARNESS}: the instance holds the parent plus exactly three children — no phantom fourth"
    );

    let parked = tasks(&harness, &instance);
    assert_eq!(
        parked.len(),
        3,
        "{HARNESS}: each child parks exactly one human task on its branch"
    );
    for task in &parked {
        assert_eq!(
            task.status,
            TaskStatus::Ready,
            "{HARNESS}: a freshly forked branch task parks Ready"
        );
    }
    let instance_row = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    assert_eq!(
        instance_row.status,
        ProcessStatus::Active,
        "{HARNESS}: the process stays active while its three children are parked"
    );

    // ── 2. NEGATIVE — ZERO TRANSITIONS, ZERO CHILDREN ───────────────────────────────────────────────────────
    // A fork that declares no transition spawns nothing: no children, no fork events, no tasks. The parent
    // completes and the process completes through it — "exactly N" holds for N = 0 as well.
    let empty = harness
        .engine()
        .start_process(start_params(EMPTY_KEY))
        .expect("the transition-less fork process starts");
    let empty_id = empty.process_instance_id.clone();
    let empty_children = harness
        .store()
        .with_tx(|tx| tx.list_children(&empty.root_token_id))
        .expect("the empty fork children are readable");
    assert!(
        empty_children.is_empty(),
        "{HARNESS}: a fork with no transitions spawns no children"
    );
    assert!(
        events_of_type(&harness, &empty_id, "token.forked").is_empty(),
        "{HARNESS}: a fork with no transitions emits no token.forked events"
    );
    assert!(
        tasks(&harness, &empty_id).is_empty(),
        "{HARNESS}: a fork with no transitions parks no tasks"
    );
    let empty_row = harness
        .store()
        .with_tx(|tx| tx.get_instance(&empty_id))
        .expect("the empty instance is readable");
    assert_eq!(
        empty_row.status,
        ProcessStatus::Completed,
        "{HARNESS}: the transition-less fork completes the process through its parent"
    );
}

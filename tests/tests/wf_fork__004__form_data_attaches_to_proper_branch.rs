//! WF.FORK — form data attaches to proper branch (TST-WF-FORK-004).
//!
//! Contract: when a dynamic fork names a `plan_variable`, each branch's task form receives **its own** plan
//! entry — the element at its own `splitBranchIndex` — together with the index and the fan-out width. The engine
//! reads `plan[branch_index]` from the instance variables after arriving each child at the branch task and
//! patches the ready task's form with `splitBranchIndex`, `splitBranchCount` and `splitBranch`
//! (`middle/workflow/src/engine/execute_node_leave.rs:497-526`). A branch shorter than the fan-out (no plan
//! entry at its index) gets `Null`, never a neighbour's entry.
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - with `plan = ["alpha", "beta"]` and two children, the task at index 0 carries `"alpha"` and the task at
//!   index 1 carries `"beta"`, each with its own index and count 2 — attachment is positional, not broadcast;
//! - with `plan = ["only"]` and three children, index 0 carries `"only"` while indices 1 and 2 carry `Null`
//!   with the correct index and count 3 — a short plan degrades to `Null` rather than shifting or duplicating
//!   entries across branches.
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__004__form_data_attaches_to_proper_branch

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph, ProcessOutcome,
    StartProcessParams, Task, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
const FORK_KEY: &str = "TST-WF-FORK-004";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const DYN_NODE: &str = "dyn";
const WORKER_NODE: &str = "worker";
const END_NODE: &str = "end";

fn fork_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![TransitionDefinition {
                name: "begin".to_string(),
                to: DYN_NODE.to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        DYN_NODE.to_string(),
        NodeDefinition {
            id: DYN_NODE.to_string(),
            node_type: "dynamic-fork".to_string(),
            count_variable: Some("n".to_string()),
            branch_command_type: Some("forge.noop".to_string()),
            branch_node: Some(WORKER_NODE.to_string()),
            join: Some("join".to_string()),
            plan_variable: Some("plan".to_string()),
            minimum: Some(1),
            maximum: Some(8),
            ..Default::default()
        },
    );
    nodes.insert(
        WORKER_NODE.to_string(),
        NodeDefinition {
            id: WORKER_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some(WORKER_NODE.to_string()),
            transitions: Some(vec![TransitionDefinition {
                name: "done".to_string(),
                to: END_NODE.to_string(),
                condition: None,
                required: None,
            }]),
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
        id: format!("{FORK_KEY}-def"),
        tenant_id: None,
        key: FORK_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: FORK_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_with(count: i64, plan: Vec<Value>) -> StartProcessParams {
    StartProcessParams {
        definition_key: FORK_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: {
            let mut variables = Value::object();
            variables.insert("n", Value::from(count));
            variables.insert("plan", Value::Array(plan));
            variables
        },
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Start one fan-out and read back the parked branch tasks.
fn branch_tasks(harness: &EngineHarness, count: i64, plan: Vec<Value>) -> Vec<Task> {
    let started = harness
        .engine()
        .start_process(start_with(count, plan))
        .expect("the dynamic fork starts and fans out");
    harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&started.process_instance_id))
        .expect("the instance tasks are readable")
}

/// The task parked at one branch index: its `(splitBranchIndex, splitBranchCount)` plus a clone of its
/// `splitBranch` entry, so the assertions name the exact row the plan attached to.
fn task_at_index(tasks: &[Task], index: i64) -> (i64, i64, Value) {
    let task = tasks
        .iter()
        .find(|task| {
            task.form_data.get("splitBranchIndex").and_then(Value::as_i64) == Some(index)
        })
        .unwrap_or_else(|| panic!("a branch task is parked at index {index}"));
    let count = task
        .form_data
        .get("splitBranchCount")
        .and_then(Value::as_i64)
        .expect("the branch task form carries splitBranchCount");
    let branch = task
        .form_data
        .get("splitBranch")
        .expect("the branch task form carries splitBranch")
        .clone();
    (index, count, branch)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-004); the file and the assay use it.
fn wf_fork_004__form_data_attaches_to_proper_branch() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(fork_definition())
        .expect("the dynamic fork definition registers");

    // ── 1. POSITIVE — each branch receives its own plan entry ────────────────────────────────────────────────
    // `plan = ["alpha", "beta"]` with two children: index 0 carries "alpha", index 1 carries "beta", each with
    // count 2. Attachment is positional — had the engine broadcast one entry, the two rows would agree and fail.
    let tasks = branch_tasks(&harness, 2, vec![Value::from("alpha"), Value::from("beta")]);
    assert_eq!(
        tasks.len(),
        2,
        "{HARNESS}: the two-way fan-out parks exactly two branch tasks"
    );
    let (index0, count0, branch0) = task_at_index(&tasks, 0);
    assert_eq!(
        (index0, count0),
        (0, 2),
        "{HARNESS}: the first branch carries its own index and the fan-out width"
    );
    assert_eq!(
        branch0.as_str(),
        Some("alpha"),
        "{HARNESS}: the first branch carries plan[0]"
    );
    let (index1, count1, branch1) = task_at_index(&tasks, 1);
    assert_eq!(
        (index1, count1),
        (1, 2),
        "{HARNESS}: the second branch carries its own index and the fan-out width"
    );
    assert_eq!(
        branch1.as_str(),
        Some("beta"),
        "{HARNESS}: the second branch carries plan[1], not a copy of plan[0]"
    );

    // ── 2. NEGATIVE — a short plan degrades to Null, never to a neighbour's entry ───────────────────────────
    // `plan = ["only"]` with three children: index 0 carries "only" while indices 1 and 2 carry Null, each with
    // the correct index and count 3. A plan read that shifted, wrapped or duplicated entries would attach the
    // wrong value to the wrong branch and fail here.
    let short = branch_tasks(&harness, 3, vec![Value::from("only")]);
    assert_eq!(
        short.len(),
        3,
        "{HARNESS}: the three-way fan-out parks exactly three branch tasks"
    );
    let (_, _, branch0) = task_at_index(&short, 0);
    assert_eq!(
        branch0.as_str(),
        Some("only"),
        "{HARNESS}: the covered branch still carries its plan entry"
    );
    for index in [1, 2] {
        let (found, count, branch) = task_at_index(&short, index);
        assert_eq!(
            (found, count),
            (index, 3),
            "{HARNESS}: the uncovered branch keeps its own index and the fan-out width"
        );
        assert!(
            branch.is_null(),
            "{HARNESS}: a branch with no plan entry carries Null, not another branch's entry"
        );
    }
}

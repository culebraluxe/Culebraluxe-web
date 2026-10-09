//! WF.FORK — branch index deterministic (TST-WF-FORK-003).
//!
//! Contract: every child of a dynamic fork carries a deterministic, dense, zero-based branch index. The engine
//! numbers children `0..count` in spawn order: each `token.forked` event carries `branchIndex: i` with
//! `dynamicCount: count` (`middle/workflow/src/engine/execute_node_leave.rs:482-493`), and when the branch
//! target is a task the same index is patched into the task's form as `splitBranchIndex` with `splitBranchCount`
//! (`execute_node_leave.rs:497-526`).
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - a four-way fan-out emits `branchIndex` `[0, 1, 2, 3]` with `dynamicCount` 4 on every event, and the four
//!   parked tasks carry `splitBranchIndex` `{0, 1, 2, 3}` with `splitBranchCount` 4 — dense, zero-based, and the
//!   two seams (event log and task form) agree;
//! - a second, independent run of the same definition with the same inputs emits the identical sequence, so the
//!   index is a function of the fan-out and not of process ids, timing or store state.
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__003__branch_index_deterministic

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent, ProcessGraph,
    ProcessOutcome, StartProcessParams, Task, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
const FORK_KEY: &str = "TST-WF-FORK-003";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const DYN_NODE: &str = "dyn";
const WORKER_NODE: &str = "worker";
const END_NODE: &str = "end";
/// The fan-out width both runs use; the expected index set is `0..FAN_OUT`.
const FAN_OUT: i64 = 4;

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

fn start_params() -> StartProcessParams {
    StartProcessParams {
        definition_key: FORK_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: {
            let mut variables = Value::object();
            variables.insert("n", Value::from(FAN_OUT));
            variables
        },
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Run one fan-out and return the `(branchIndex, dynamicCount)` pairs in emission order plus the
/// `(splitBranchIndex, splitBranchCount)` pairs read back from the parked task forms.
fn run_fanout(harness: &EngineHarness) -> (Vec<(i64, i64)>, Vec<(i64, i64)>) {
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the dynamic fork starts and fans out");
    let instance = started.process_instance_id.clone();
    let forked: Vec<ProcessEvent> = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 512))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "token.forked")
        .collect();
    let event_indices: Vec<(i64, i64)> = forked
        .iter()
        .map(|event| {
            let index = event
                .data
                .get("branchIndex")
                .and_then(Value::as_i64)
                .expect("every token.forked event carries branchIndex");
            let count = event
                .data
                .get("dynamicCount")
                .and_then(Value::as_i64)
                .expect("every token.forked event carries dynamicCount");
            (index, count)
        })
        .collect();
    let tasks: Vec<Task> = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance))
        .expect("the instance tasks are readable");
    let form_indices: Vec<(i64, i64)> = tasks
        .iter()
        .map(|task| {
            let index = task
                .form_data
                .get("splitBranchIndex")
                .and_then(Value::as_i64)
                .expect("every forked task form carries splitBranchIndex");
            let count = task
                .form_data
                .get("splitBranchCount")
                .and_then(Value::as_i64)
                .expect("every forked task form carries splitBranchCount");
            (index, count)
        })
        .collect();
    (event_indices, form_indices)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-003); the file and the assay use it.
fn wf_fork_003__branch_index_deterministic() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(fork_definition())
        .expect("the dynamic fork definition registers");

    // ── 1. DENSE AND ZERO-BASED — the events name 0..4 ──────────────────────────────────────────────────────
    // Four children emit `branchIndex` 0, 1, 2, 3 with `dynamicCount` 4 on every event: no 1-based offset, no
    // gap, no duplicate. (The history reads newest-first, so the emission order is normalised by sorting; step
    // 3 pins that the observed order itself is deterministic.) A numbering that started at 1 or skipped would
    // fail the exact set.
    let (first_events, first_forms) = run_fanout(&harness);
    let expected_events: Vec<(i64, i64)> = (0..FAN_OUT).map(|i| (i, FAN_OUT)).collect();
    let mut sorted_first = first_events.clone();
    sorted_first.sort_unstable();
    assert_eq!(
        sorted_first, expected_events,
        "{HARNESS}: the fork events number their branches 0..count"
    );

    // ── 2. THE TASK FORMS AGREE — the same index reaches the branch's own durable row ────────────────────────
    // Each parked task's form carries its branch index and the fan-out width. Sorted, the set is exactly
    // 0..4 with count 4 on every row: the form seam and the event seam cannot disagree about who is who.
    let mut sorted_forms = first_forms.clone();
    sorted_forms.sort_unstable();
    assert_eq!(
        sorted_forms, expected_events,
        "{HARNESS}: the parked task forms carry the same dense index set as the fork events"
    );

    // ── 3. DETERMINISTIC — an independent run emits the identical sequence ───────────────────────────────────
    // The same definition with the same inputs on a fresh process produces the same indices in the same
    // observed order: the index is a function of the fan-out, not of ids, timing or store state.
    let (second_events, second_forms) = run_fanout(&harness);
    assert_eq!(
        second_events, first_events,
        "{HARNESS}: a second run emits the identical branch index sequence"
    );
    let mut second_sorted = second_forms.clone();
    second_sorted.sort_unstable();
    assert_eq!(
        second_sorted, sorted_forms,
        "{HARNESS}: a second run parks the identical task-form index set"
    );
}

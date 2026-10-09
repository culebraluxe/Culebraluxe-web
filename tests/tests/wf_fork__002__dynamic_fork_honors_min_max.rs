//! WF.FORK — dynamic fork honors min/max (TST-WF-FORK-002).
//!
//! Contract: a `dynamic-fork` node fans out to `count_variable` children **clamped** to its `[minimum, maximum]`
//! bounds. `handle_dynamic_fork` (`middle/workflow/src/engine/execute_node_leave.rs:424-528`) reads the bounds
//! (defaulting to 2 and 8 when undeclared, `execute_node_leave.rs:435-436`), reads the requested count from the
//! instance variables (falling back to the minimum when the variable is absent or not a number,
//! `execute_node_leave.rs:437-443`), and spawns `raw.clamp(minimum, maximum)` children at the `branch_node`
//! (`execute_node_leave.rs:443-460`). A branch target that names no declared node is refused, never spawned.
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - above the maximum (`n = 10`, max 5) spawns exactly 5; below the minimum (`n = 1`, min 2) spawns exactly 2;
//!   inside the range (`n = 3`) spawns exactly 3; a missing or non-numeric count falls back to the minimum (2);
//! - the undeclared-bounds control spawns exactly 8 for `n = 100` and exactly 2 for a missing count, pinning the
//!   production defaults;
//! - a `branch_node` that names no declared node refuses the start outright — an unbounded or misaddressed fan-out
//!   can never silently spawn.
//!
//! Level L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_fork__002__dynamic_fork_honors_min_max

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent, ProcessGraph,
    ProcessOutcome, StartProcessParams, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";
/// The bounded dynamic fork: minimum 2, maximum 5.
const BOUNDED_KEY: &str = "TST-WF-FORK-002";
/// The unbounded dynamic fork: no minimum/maximum, so the production defaults (2 and 8) apply.
const DEFAULT_KEY: &str = "TST-WF-FORK-002-DEFAULTS";
/// The misaddressed dynamic fork: its branch node names no declared node, so every start is refused.
const GHOST_KEY: &str = "TST-WF-FORK-002-GHOST";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const DYN_NODE: &str = "dyn";
const WORKER_NODE: &str = "worker";
const END_NODE: &str = "end";
const COUNT_VARIABLE: &str = "n";

fn dynamic_definition(
    key: &str,
    minimum: Option<i32>,
    maximum: Option<i32>,
    branch_node: &str,
) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![workflow::TransitionDefinition {
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
            count_variable: Some(COUNT_VARIABLE.to_string()),
            branch_command_type: Some("forge.noop".to_string()),
            branch_node: Some(branch_node.to_string()),
            join: Some("join".to_string()),
            minimum,
            maximum,
            ..Default::default()
        },
    );
    nodes.insert(
        WORKER_NODE.to_string(),
        NodeDefinition {
            id: WORKER_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some(WORKER_NODE.to_string()),
            transitions: Some(vec![workflow::TransitionDefinition {
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

fn start_with(key: &str, variables: Value) -> StartProcessParams {
    StartProcessParams {
        definition_key: key.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables,
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn count_vars() -> Value {
    Value::object()
}

/// Instance variables with the fork count set to `n`.
fn count_is(n: Value) -> Value {
    let mut variables = Value::object();
    variables.insert(COUNT_VARIABLE, n);
    variables
}

/// Start one instance and count the children the fan-out parked: the worker tasks plus the `token.forked`
/// events. The two counts must agree — a child without a task, or an event without a child, fails the clause.
fn spawned(harness: &EngineHarness, key: &str, variables: Value) -> (usize, usize) {
    let started = harness
        .engine()
        .start_process(start_with(key, variables))
        .expect("the dynamic fork starts and fans out");
    let instance = started.process_instance_id.clone();
    let tasks = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance))
        .expect("the instance tasks are readable");
    let forked: Vec<ProcessEvent> = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 512))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "token.forked")
        .collect();
    assert_eq!(
        tasks.len(),
        forked.len(),
        "{HARNESS}: every forked child parks exactly one task and every event names a child"
    );
    (tasks.len(), forked.len())
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-FORK-002); the file and the assay use it.
fn wf_fork_002__dynamic_fork_honors_min_max() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(dynamic_definition(
            BOUNDED_KEY,
            Some(2),
            Some(5),
            WORKER_NODE,
        ))
        .expect("the bounded dynamic fork registers");
    harness
        .engine()
        .seed_definition(dynamic_definition(DEFAULT_KEY, None, None, WORKER_NODE))
        .expect("the default-bounds dynamic fork registers");
    harness
        .engine()
        .seed_definition(dynamic_definition(GHOST_KEY, Some(2), Some(5), "ghost"))
        .expect("the misaddressed dynamic fork registers");

    // ── 1. ABOVE THE MAXIMUM CLAMPS ──────────────────────────────────────────────────────────────────────────
    // `n = 10` against maximum 5 spawns exactly 5: the fan-out is bounded no matter what the variable asks for.
    let (tasks, forked) = spawned(&harness, BOUNDED_KEY, count_is(Value::from(10)));
    assert_eq!(
        (tasks, forked),
        (5, 5),
        "{HARNESS}: a count above the maximum spawns exactly the maximum"
    );

    // ── 2. BELOW THE MINIMUM CLAMPS ──────────────────────────────────────────────────────────────────────────
    // `n = 1` against minimum 2 spawns exactly 2: the fan-out never collapses below its lower bound.
    let (tasks, forked) = spawned(&harness, BOUNDED_KEY, count_is(Value::from(1)));
    assert_eq!(
        (tasks, forked),
        (2, 2),
        "{HARNESS}: a count below the minimum spawns exactly the minimum"
    );

    // ── 3. INSIDE THE RANGE PASSES THROUGH ───────────────────────────────────────────────────────────────────
    // `n = 3` inside [2, 5] spawns exactly 3: clamping only binds outside the range, it never rewrites a legal
    // count.
    let (tasks, forked) = spawned(&harness, BOUNDED_KEY, count_is(Value::from(3)));
    assert_eq!(
        (tasks, forked),
        (3, 3),
        "{HARNESS}: a count inside the range spawns exactly the requested count"
    );

    // ── 4. FAULT — A MISSING OR NON-NUMERIC COUNT FALLS BACK TO THE MINIMUM ─────────────────────────────────
    // No `n` at all, and an `n` that is not a number, both spawn exactly the minimum: the fork degrades to its
    // lower bound rather than spawning zero children or refusing the process.
    let (tasks, _) = spawned(&harness, BOUNDED_KEY, count_vars());
    assert_eq!(
        tasks, 2,
        "{HARNESS}: a missing count spawns exactly the minimum"
    );
    let (tasks, _) = spawned(&harness, BOUNDED_KEY, count_is(Value::from("many")));
    assert_eq!(
        tasks, 2,
        "{HARNESS}: a non-numeric count spawns exactly the minimum"
    );

    // ── 5. DEFAULT BOUNDS — the production 2/8 rule ──────────────────────────────────────────────────────────
    // With no declared bounds, `n = 100` spawns exactly 8 and a missing count spawns exactly 2: the defaults in
    // the production code are the bounds, not a suggestion.
    let (tasks, _) = spawned(&harness, DEFAULT_KEY, count_is(Value::from(100)));
    assert_eq!(
        tasks, 8,
        "{HARNESS}: undeclared bounds cap the fan-out at the production default maximum of 8"
    );
    let (tasks, _) = spawned(&harness, DEFAULT_KEY, count_vars());
    assert_eq!(
        tasks, 2,
        "{HARNESS}: undeclared bounds floor the fan-out at the production default minimum of 2"
    );

    // ── 6. NEGATIVE — A MISADDRESSED BRANCH IS REFUSED, NEVER SPAWNED ────────────────────────────────────────
    // A `branch_node` that names no declared node refuses the start outright: the engine never spawns children
    // it cannot address, so a typo in the fan-out cannot silently fan nowhere.
    let refused = harness
        .engine()
        .start_process(start_with(GHOST_KEY, count_is(Value::from(3))))
        .expect_err("a dynamic fork with no valid branch target must refuse the start");
    assert!(
        refused.to_string().contains("no valid branch"),
        "{HARNESS}: the refusal names the missing branch target: {refused}"
    );
}

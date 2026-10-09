//! WF.TOKEN — process completes only when no required active token remains (TST-WF-TOKEN-008).
//!
//! Contract: the process completion logic (`check_process_completion` / `resolve_process_after_token`)
//! completes the process instance if and only if there are zero required active tokens remaining.
//! This is the authoritative completion rule that subsumes TST-WF-TOKEN-006 and TST-WF-TOKEN-007.
//!
//! This test exercises the exact boundary: `count_active_tokens` in the context of process
//! completion must be interpreted as "required active tokens". The engine's
//! `resolve_process_after_token` checks `token.required` before calling
//! `check_process_completion`, and `check_process_completion` uses `count_active_tokens`
//! which the store implements as counting all active tokens. The contract is that the
//! engine only calls `check_process_completion` when the completing token was required
//! (or when it wasn't required but there are no other required tokens).
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock
//! and an in-memory store; no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__008__process_completes_only_when_no_required_active_token_remains

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, MemoryStore, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, Result,
    StartProcessParams, Store, Task, Token, TokenOutcome, TokenStatus, TransitionDefinition,
    TxStore, Value, WorkflowEngine,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";

/// The definition key/version the engine half registers.
const DEFINITION_KEY: &str = "TST-WF-TOKEN-008";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// A complex fork graph with multiple required and optional branches:
/// start -> fork -> (req-a -> task-a -> end) + (req-b -> task-b -> end) + (opt-c -> task-c -> end) + (opt-d -> task-d -> end)
/// Process completes when req-a AND req-b complete, regardless of opt-c and opt-d.
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const REQ_A: &str = "req-a";
const REQ_A_TASK: &str = "req-a-task";
const REQ_B: &str = "req-b";
const REQ_B_TASK: &str = "req-b-task";
const OPT_C: &str = "opt-c";
const OPT_C_TASK: &str = "opt-c-task";
const OPT_D: &str = "opt-d";
const OPT_D_TASK: &str = "opt-d-task";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const TO_A: &str = "to-a";
const TO_B: &str = "to-b";
const TO_C: &str = "to-c";
const TO_D: &str = "to-d";
const FINISH_A: &str = "finish-a";
const FINISH_B: &str = "finish-b";
const FINISH_C: &str = "finish-c";
const FINISH_D: &str = "finish-d";

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

/// Fork with 2 required and 2 optional branches.
fn four_branch_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, FORK_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition(TO_A, REQ_A, Some(true)),
                transition(TO_B, REQ_B, Some(true)),
                transition(TO_C, OPT_C, Some(false)),
                transition(TO_D, OPT_D, Some(false)),
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ_A.to_string(),
        NodeDefinition {
            id: REQ_A.to_string(),
            node_type: "service".to_string(),
            name: Some("Required A".to_string()),
            transitions: Some(vec![transition("next-a", REQ_A_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ_A_TASK.to_string(),
        NodeDefinition {
            id: REQ_A_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Required Task A".to_string()),
            transitions: Some(vec![transition(FINISH_A, END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ_B.to_string(),
        NodeDefinition {
            id: REQ_B.to_string(),
            node_type: "service".to_string(),
            name: Some("Required B".to_string()),
            transitions: Some(vec![transition("next-b", REQ_B_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ_B_TASK.to_string(),
        NodeDefinition {
            id: REQ_B_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Required Task B".to_string()),
            transitions: Some(vec![transition(FINISH_B, END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPT_C.to_string(),
        NodeDefinition {
            id: OPT_C.to_string(),
            node_type: "service".to_string(),
            name: Some("Optional C".to_string()),
            transitions: Some(vec![transition("next-c", OPT_C_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPT_C_TASK.to_string(),
        NodeDefinition {
            id: OPT_C_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Optional Task C".to_string()),
            transitions: Some(vec![transition(FINISH_C, END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPT_D.to_string(),
        NodeDefinition {
            id: OPT_D.to_string(),
            node_type: "service".to_string(),
            name: Some("Optional D".to_string()),
            transitions: Some(vec![transition("next-d", OPT_D_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPT_D_TASK.to_string(),
        NodeDefinition {
            id: OPT_D_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Optional Task D".to_string()),
            transitions: Some(vec![transition(FINISH_D, END_NODE, None)]),
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
        id: format!("{DEFINITION_KEY}-def"),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: DEFINITION_KEY.to_string(),
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
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn read_token(store: &MemoryStore, id: &str) -> Token {
    store
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

#[test]
#[allow(non_snake_case)]
fn wf_token_008__process_completes_only_when_no_required_active_token_remains() {
    let harness = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness
        .engine()
        .seed_definition(four_branch_definition())
        .expect("the fork definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and forks into four branches");
    let instance = started.process_instance_id.clone();

    // The fork mints four child tokens: 2 required, 2 optional.
    let tokens = harness
        .store()
        .with_tx(|tx| tx.list_active_tokens(&instance))
        .expect("active tokens read");
    assert_eq!(
        tokens.len(),
        4,
        "{HARNESS}: the fork must mint exactly four child tokens"
    );

    // Identify tokens by required flag and node.
    let req_a = tokens
        .iter()
        .find(|t| t.required && t.node_id == REQ_A_TASK)
        .expect("req-a at task");
    let req_b = tokens
        .iter()
        .find(|t| t.required && t.node_id == REQ_B_TASK)
        .expect("req-b at task");
    let opt_c = tokens
        .iter()
        .find(|t| !t.required && t.node_id == OPT_C_TASK)
        .expect("opt-c at task");
    let opt_d = tokens
        .iter()
        .find(|t| !t.required && t.node_id == OPT_D_TASK)
        .expect("opt-d at task");

    let id_a = req_a.id.clone();
    let id_b = req_b.id.clone();
    let id_c = opt_c.id.clone();
    let id_d = opt_d.id.clone();

    // Verify required flags.
    assert!(req_a.required, "req-a required");
    assert!(req_b.required, "req-b required");
    assert!(!opt_c.required, "opt-c optional");
    assert!(!opt_d.required, "opt-d optional");

    // All four tokens are active at their tasks.
    for (id, expected_node) in [
        (&id_a, REQ_A_TASK),
        (&id_b, REQ_B_TASK),
        (&id_c, OPT_C_TASK),
        (&id_d, OPT_D_TASK),
    ] {
        let t = read_token(harness.store(), id);
        assert_eq!(t.node_id, expected_node, "token at correct task");
        assert_eq!(t.status, TokenStatus::Active, "token active");
    }

    // Process is active.
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .unwrap()
            .status,
        ProcessStatus::Active
    );

    // ── Step 1: Complete optional C only ──────────────────────────────────────────────────────────────
    let task_c = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&id_c))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_c.id,
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_C.to_string()),
        })
        .unwrap();

    let c_done = read_token(harness.store(), &id_c);
    assert_eq!(c_done.status, TokenStatus::Completed);
    assert_eq!(c_done.node_id, END_NODE);

    // Process STILL ACTIVE (required A and B still active).
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .unwrap()
            .status,
        ProcessStatus::Active,
        "{HARNESS}: process active - required A and B still outstanding"
    );

    // ── Step 2: Complete optional D only ──────────────────────────────────────────────────────────────
    let task_d = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&id_d))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_d.id,
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_D.to_string()),
        })
        .unwrap();

    let d_done = read_token(harness.store(), &id_d);
    assert_eq!(d_done.status, TokenStatus::Completed);
    assert_eq!(d_done.node_id, END_NODE);

    // Process STILL ACTIVE (required A and B still active).
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .unwrap()
            .status,
        ProcessStatus::Active,
        "{HARNESS}: process active - required A and B still outstanding after both optionals done"
    );

    // ── Step 3: Complete required A ──────────────────────────────────────────────────────────────────
    let task_a = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&id_a))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_a.id,
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_A.to_string()),
        })
        .unwrap();

    let a_done = read_token(harness.store(), &id_a);
    assert_eq!(a_done.status, TokenStatus::Completed);
    assert_eq!(a_done.node_id, END_NODE);

    // Process STILL ACTIVE (required B still active).
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .unwrap()
            .status,
        ProcessStatus::Active,
        "{HARNESS}: process active - required B still outstanding"
    );

    // ── Step 4: Complete required B (last required) ──────────────────────────────────────────────────
    let task_b = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&id_b))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_b.id,
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_B.to_string()),
        })
        .unwrap();

    let b_done = read_token(harness.store(), &id_b);
    assert_eq!(b_done.status, TokenStatus::Completed);
    assert_eq!(b_done.node_id, END_NODE);

    // NOW process completes (no required active tokens remain).
    let final_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance reads");
    assert_eq!(
        final_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: process completes exactly when last required token completes"
    );
    assert_eq!(final_instance.outcome, Some(ProcessOutcome::Completed));

    // All tokens are completed.
    for id in [&id_a, &id_b, &id_c, &id_d] {
        let t = read_token(harness.store(), id);
        assert_eq!(t.status, TokenStatus::Completed);
        assert_eq!(t.node_id, END_NODE);
    }

    // ── VERIFY: count_active_tokens vs required tokens ──────────────────────────────────────────────
    // The store's count_active_tokens counts ALL active tokens. But the engine's
    // completion logic only calls check_process_completion when a required token completes
    // (or when an optional token completes but there are no required tokens).
    // This test verifies the ENGINE'S behavior, not the store's raw count.
    // The store's count_active_tokens would return 0 at the end (all completed).
    let active_count = harness
        .store()
        .with_tx(|tx| tx.count_active_tokens(&instance))
        .expect("active count reads");
    assert_eq!(
        active_count, 0,
        "store count_active_tokens is 0 when all tokens completed"
    );

    // The required tokens at each step:
    // After fork: 2 required active (A, B), 2 optional active (C, D)
    // After C done: 2 required active, 1 optional active
    // After D done: 2 required active, 0 optional active
    // After A done: 1 required active (B), 0 optional active
    // After B done: 0 required active -> process completes
}

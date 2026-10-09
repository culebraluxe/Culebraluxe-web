//! WF.TOKEN — required branch does prevent completion (TST-WF-TOKEN-007).
//!
//! Contract: when a fork mints a required child token (required=true), that token MUST
//! complete before the process can complete. The process remains active as long as any
//! required token is active. Only when ALL required tokens reach completion does the
//! process complete.
//!
//! This is why the contract is symmetric with TST-WF-TOKEN-006: required tokens are
//! counted by `count_required_active_siblings` at joins and by the completion logic
//! in `resolve_process_after_token` (which checks `token.required`).
//!
//! The CANCELLATION half below was authored in the same batch 60 (`5fdfbf4ae`) and went red on trunk
//! for one over-broad assertion: it read **every** token of a cancelled instance and demanded
//! `Cancelled` of all of them — including the fork's own parent token, which `handle_fork` had already
//! concluded `Completed` when it minted the branches
//! (`middle/workflow/src/engine/execute_node_leave.rs:385`). `cancel_process` → `terminate_process`
//! concludes the tokens it finds ACTIVE (`execute_node_leave.rs:223-238`); a token that finished
//! before the cancel is not rewritten. The assertions below say exactly that.
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock
//! and an in-memory store; no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__007__required_branch_does_prevent_completion

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
const DEFINITION_KEY: &str = "TST-WF-TOKEN-007";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// A fork-join graph with two required branches:
/// start -> fork -> (req-branch-1 -> task-1 -> end) + (req-branch-2 -> task-2 -> end)
/// The process must NOT complete until BOTH required branches complete.
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const REQ1_BRANCH: &str = "req1-branch";
const REQ1_TASK: &str = "req1-task";
const REQ2_BRANCH: &str = "req2-branch";
const REQ2_TASK: &str = "req2-task";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const TO_REQ1: &str = "to-req1";
const TO_REQ2: &str = "to-req2";
const FINISH_1: &str = "finish-1";
const FINISH_2: &str = "finish-2";

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

/// Fork with TWO required branches joining at a single end node.
fn fork_two_required_definition() -> ProcessDefinition {
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
                transition(TO_REQ1, REQ1_BRANCH, Some(true)),
                transition(TO_REQ2, REQ2_BRANCH, Some(true)),
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ1_BRANCH.to_string(),
        NodeDefinition {
            id: REQ1_BRANCH.to_string(),
            node_type: "service".to_string(),
            name: Some("Required Branch 1".to_string()),
            transitions: Some(vec![transition("next-1", REQ1_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ1_TASK.to_string(),
        NodeDefinition {
            id: REQ1_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Required Task 1".to_string()),
            transitions: Some(vec![transition(FINISH_1, END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ2_BRANCH.to_string(),
        NodeDefinition {
            id: REQ2_BRANCH.to_string(),
            node_type: "service".to_string(),
            name: Some("Required Branch 2".to_string()),
            transitions: Some(vec![transition("next-2", REQ2_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ2_TASK.to_string(),
        NodeDefinition {
            id: REQ2_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Required Task 2".to_string()),
            transitions: Some(vec![transition(FINISH_2, END_NODE, None)]),
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
fn wf_token_007__required_branch_does_prevent_completion() {
    let harness = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness
        .engine()
        .seed_definition(fork_two_required_definition())
        .expect("the fork definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and forks into two required branches");
    let instance = started.process_instance_id.clone();

    // The fork mints two child tokens, both required.
    let tokens = harness
        .store()
        .with_tx(|tx| tx.list_active_tokens(&instance))
        .expect("active tokens read");
    assert_eq!(
        tokens.len(),
        2,
        "{HARNESS}: the fork must mint exactly two child tokens"
    );

    // Both tokens are required.
    let token1 = tokens.iter().find(|t| t.node_id == REQ1_TASK);
    let token2 = tokens.iter().find(|t| t.node_id == REQ2_TASK);

    let token1 = token1.expect("{HARNESS}: one token at req1-task");
    let token2 = token2.expect("{HARNESS}: one token at req2-task");

    let id1 = token1.id.clone();
    let id2 = token2.id.clone();

    assert!(token1.required, "{HARNESS}: token1 must be required");
    assert!(token2.required, "{HARNESS}: token2 must be required");

    // Both tokens are active at their respective tasks.
    let t1_after_fork = read_token(harness.store(), &id1);
    let t2_after_fork = read_token(harness.store(), &id2);
    assert_eq!(t1_after_fork.node_id, REQ1_TASK);
    assert_eq!(t2_after_fork.node_id, REQ2_TASK);
    assert_eq!(t1_after_fork.status, TokenStatus::Active);
    assert_eq!(t2_after_fork.status, TokenStatus::Active);

    // Process is still active (has required tokens).
    let instance_after_fork = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("instance reads");
    assert_eq!(
        instance_after_fork.status,
        ProcessStatus::Active,
        "{HARNESS}: process is active with required tokens outstanding"
    );

    // Complete ONLY the first required branch's task.
    let task1 = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&id1))
        .expect("token1's task reads")
        .into_iter()
        .next()
        .expect("token1 has exactly one open task");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task1.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_1.to_string()),
        })
        .expect("completing first required task advances token1 to end node");

    // Token1 completes at end node.
    let t1_done = read_token(harness.store(), &id1);
    assert_eq!(t1_done.status, TokenStatus::Completed);
    assert_eq!(t1_done.node_id, END_NODE);

    // Token2 is STILL ACTIVE at its task.
    let t2_still_active = read_token(harness.store(), &id2);
    assert_eq!(t2_still_active.status, TokenStatus::Active);
    assert_eq!(t2_still_active.node_id, REQ2_TASK);

    // CRITICAL ASSERTION: Process MUST STILL BE ACTIVE because token2 (required) is still active.
    let instance_after_one = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("instance reads");
    assert_eq!(
        instance_after_one.status,
        ProcessStatus::Active,
        "{HARNESS}: process remains active while ANY required token is active"
    );

    // Now complete the second required branch's task.
    let task2 = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&id2))
        .expect("token2's task reads")
        .into_iter()
        .next()
        .expect("token2 has exactly one open task");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task2.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_2.to_string()),
        })
        .expect("completing second required task advances token2 to end node");

    // Token2 also completes.
    let t2_done = read_token(harness.store(), &id2);
    assert_eq!(t2_done.status, TokenStatus::Completed);
    assert_eq!(t2_done.node_id, END_NODE);

    // NOW the process completes (all required tokens done).
    let final_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance reads");
    assert_eq!(
        final_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: process completes when ALL required tokens complete"
    );

    // ── NEGATIVE CASE: cancel the process while a required token is active ────────────────────────────────────
    // Verify that cancelling terminates all tokens including required ones.
    let harness2 = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness2
        .engine()
        .seed_definition(fork_two_required_definition())
        .expect("definition registers");
    let started2 = harness2
        .engine()
        .start_process(start_params())
        .expect("process starts");
    let instance2 = started2.process_instance_id.clone();

    // The tokens ACTIVE at the moment of cancellation: the fork's two required children. The fork's
    // own token was already concluded when it minted them (`handle_fork` completes the parent,
    // `middle/workflow/src/engine/execute_node_leave.rs:385`), and cancellation does not rewrite a
    // token that had already finished — `terminate_process` walks `list_active_tokens` and nothing
    // else (`execute_node_leave.rs:223-238`). So this is the set the cancel can conclude.
    let active_before_cancel = harness2
        .store()
        .with_tx(|tx| tx.list_active_tokens(&instance2))
        .expect("active tokens read");
    assert_eq!(
        active_before_cancel.len(),
        2,
        "{HARNESS}: both required children are active at the moment of cancellation"
    );

    // Cancel the process immediately (while both required tokens are active).
    harness2
        .engine()
        .cancel_process(workflow::CancelProcessParams {
            process_instance_id: instance2.clone(),
            actor: STARTED_BY.to_string(),
            reason: Some("test cancellation".to_string()),
        })
        .expect("cancellation succeeds");

    // Nothing is left Active: cancellation concludes every token of the instance.
    let tokens2 = harness2
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance2))
        .expect("tokens for instance read");
    for token in &tokens2 {
        assert_eq!(
            token.status,
            TokenStatus::Completed,
            "{HARNESS}: cancellation concludes every token ({} at {})",
            token.id,
            token.node_id
        );
    }

    // A token cancelled before it finished is concluded `Cancelled` — the outcome that says the
    // branch did not earn completion.
    for caught in &active_before_cancel {
        let concluded = read_token(harness2.store(), &caught.id);
        assert_eq!(
            concluded.outcome,
            Some(TokenOutcome::Cancelled),
            "{HARNESS}: a branch cancelled mid-flight is concluded Cancelled"
        );
    }

    // The one token that had already finished keeps what it earned: cancellation does not rewrite
    // history, so the fork's parent token stays `Completed` (and it is the only such token).
    let pre_concluded: Vec<&Token> = tokens2
        .iter()
        .filter(|token| {
            !active_before_cancel
                .iter()
                .any(|active| active.id == token.id)
        })
        .collect();
    assert_eq!(
        pre_concluded.len(),
        1,
        "{HARNESS}: exactly one token — the fork's own — was concluded before the cancellation"
    );
    assert_eq!(
        pre_concluded[0].node_id, FORK_NODE,
        "{HARNESS}: the token concluded before the cancellation is the fork's parent"
    );
    assert_eq!(
        pre_concluded[0].outcome,
        Some(TokenOutcome::Completed),
        "{HARNESS}: a token that finished before the cancellation keeps its Completed outcome"
    );

    // The branches' open work is concluded with them: no task is left open on a cancelled run.
    assert!(
        harness2
            .store()
            .with_tx(|tx| tx.open_tasks_for_instance(&instance2))
            .expect("open tasks read")
            .is_empty(),
        "{HARNESS}: cancellation obsoletes the open tasks of the branches it cancels"
    );

    // Process instance is Aborted (cancelled outcome).
    let inst2 = harness2
        .store()
        .with_tx(|tx| tx.get_instance(&instance2))
        .expect("instance reads");
    assert_eq!(inst2.status, ProcessStatus::Aborted);
    assert_eq!(inst2.outcome, Some(ProcessOutcome::Cancelled));
}

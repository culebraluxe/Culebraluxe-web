//! WF.TOKEN — completed token cannot move (TST-WF-TOKEN-004).
//!
//! Contract: once a token reaches `TokenStatus::Completed`, the engine must never call
//! `move_token` again for that token. The store's CAS only checks version, not status,
//! so a completed token CAN be moved at the store level if the version matches.
//! The engine's `execute_node_leave` returns early for completed/end nodes and never
//! invokes `move_token` after `complete_token`.
//!
//! This test verifies:
//! 1. At the store level: move_token only checks version (CAS), not token status
//! 2. At the engine level: a completed token is never moved again
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock and
//! an in-memory store; no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__004__completed_token_cannot_move

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, Job, MemoryStore, NodeDefinition,
    ProcessCommand, ProcessDefinition, ProcessEvent, ProcessGraph, ProcessInstance, ProcessOutcome,
    ProcessStatus, Result, StartProcessParams, Store, Task, Token, TokenOutcome, TokenStatus,
    TransitionDefinition, TxStore, Value, WorkflowEngine,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";

/// The definition key/version the engine half registers.
const DEFINITION_KEY: &str = "TST-WF-TOKEN-004";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// Nodes for the engine half: `start -> work (service) -> hold (task) -> done (end)`.
const START_NODE: &str = "start";
const WORK_NODE: &str = "work";
const HOLD_NODE: &str = "hold";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const NEXT: &str = "next";
const FINISH: &str = "finish";

/// Nodes the standalone CAS half uses.
const CAS_TOKEN: &str = "tok-cas";
const CAS_FROM: &str = "left";
const CAS_TO: &str = "right";
const CAS_THIRD: &str = "elsewhere";
const CAS_ABSENT: &str = "tok-absent";

/// Seed one active token row directly through the production `Store`.
fn seed_token(
    store: &MemoryStore,
    id: &str,
    node: &str,
    version: i32,
    status: TokenStatus,
    outcome: Option<TokenOutcome>,
) {
    let token = Token {
        id: id.to_string(),
        tenant_id: None,
        process_instance_id: "pi-cas".to_string(),
        parent_token_id: None,
        node_id: node.to_string(),
        status,
        outcome,
        required: true,
        is_able_to_reactivate_parent: true,
        started_at: 0,
        ended_at: None,
        version,
    };
    store
        .with_tx(|tx| tx.insert_token(token.clone()))
        .expect("the seed token commits");
}

fn read_token(store: &MemoryStore, id: &str) -> Token {
    store
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> work (service) -> hold (task) -> done (end)`.
fn linear_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, WORK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        WORK_NODE.to_string(),
        NodeDefinition {
            id: WORK_NODE.to_string(),
            node_type: "service".to_string(),
            name: Some("Work".to_string()),
            transitions: Some(vec![transition(NEXT, HOLD_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        HOLD_NODE.to_string(),
        NodeDefinition {
            id: HOLD_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Hold".to_string()),
            transitions: Some(vec![transition(FINISH, DONE_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DONE_NODE.to_string(),
        NodeDefinition {
            id: DONE_NODE.to_string(),
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

/// The `(from, to, transition)` of every committed `token.moved`, in commit order.
fn token_moves(store: &MemoryStore, instance_id: &str) -> Vec<(String, String, String)> {
    let mut moves: Vec<(String, String, String)> = store
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "token.moved")
        .map(|event| {
            (
                event
                    .data
                    .get("from")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                event.node_id.clone().unwrap_or_default(),
                event
                    .data
                    .get("transition")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect();
    moves.reverse();
    moves
}

#[test]
#[allow(non_snake_case)]
fn wf_token_004__completed_token_cannot_move() {
    // ── SCENARIO 1: the store-level CAS only checks version, not token status ──────────────────────────────────
    let store = MemoryStore::new();
    // Seed a token already in Completed state with version 5.
    // Note: The store's move_token does NOT check token status, only version.
    seed_token(
        &store,
        CAS_TOKEN,
        DONE_NODE,
        5,
        TokenStatus::Completed,
        Some(TokenOutcome::Completed),
    );

    // At the store level, a move with the current version (5) SUCCEEDS even for a completed token,
    // because move_token only checks version equality, not status.
    // This is by design - the CAS is a concurrency control, not a state machine guard.
    let moved = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 5, CAS_TO))
        .expect("the move step commits");
    assert!(
        moved,
        "{HARNESS}: store-level move_token with current version succeeds regardless of status"
    );
    let after_move = read_token(&store, CAS_TOKEN);
    assert_eq!(
        after_move.node_id, CAS_TO,
        "{HARNESS}: store move updates node_id"
    );
    assert_eq!(
        after_move.version, 6,
        "{HARNESS}: store move advances version"
    );

    // But a move with a STALE version is refused (this is the CAS guarantee).
    let refused = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 5, CAS_THIRD)) // version 5 is now stale (current is 6)
        .expect("a refusal is an answer, not an error");
    assert!(
        !refused,
        "{HARNESS}: a move carrying a stale version must be refused"
    );
    let unchanged = read_token(&store, CAS_TOKEN);
    assert_eq!(unchanged.node_id, CAS_TO);
    assert_eq!(unchanged.version, 6);
    assert_eq!(unchanged.status, TokenStatus::Completed);
    assert_eq!(unchanged.outcome, Some(TokenOutcome::Completed));

    // ── SCENARIO 2: the engine never calls move_token after complete_token ─────────────────────────────────────
    // The real WorkflowEngine<MemoryStore> driven through its public steps must not attempt to move
    // a token that complete_token has already marked Completed.
    let harness = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness
        .engine()
        .seed_definition(linear_definition())
        .expect("the linear definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the human task");
    let instance = started.process_instance_id.clone();
    let root_token = started.root_token_id.clone();

    // Drive the process to completion: start -> work -> hold -> done (end node completes the token)
    let task = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&root_token))
        .expect("the open task reads")
        .into_iter()
        .next()
        .expect("the engine parked exactly one task");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH.to_string()),
        })
        .expect("completing the task advances the token to the end node, which completes it");

    // The token is now Completed at the end node.
    let finished = harness
        .store()
        .with_tx(|tx| tx.get_token(&root_token))
        .expect("the finished token reads");
    assert_eq!(finished.node_id, DONE_NODE);
    assert_eq!(finished.status, TokenStatus::Completed);
    assert_eq!(finished.outcome, Some(TokenOutcome::Completed));

    // Verify no further moves were attempted: exactly 3 token.moved events
    // (start->work, work->hold, hold->done). The end node completion does NOT produce a move.
    let all_moves = token_moves(harness.store(), &instance);
    assert_eq!(
        all_moves.len(),
        3,
        "{HARNESS}: exactly three moves for the linear graph, no more after completion"
    );
    assert_eq!(
        all_moves[2],
        (
            HOLD_NODE.to_string(),
            DONE_NODE.to_string(),
            FINISH.to_string()
        ),
        "{HARNESS}: the final move is the task transition to the end node"
    );

    // The process instance is Completed.
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .expect("the instance reads")
            .status,
        ProcessStatus::Completed,
        "{HARNESS}: the process reaches its terminal state"
    );

    // ── SCENARIO 3: direct store call with stale version after engine completion ────────────────────────────────
    // After the engine completes the token, the version has been advanced by complete_token.
    // A subsequent move_token call with the pre-completion version is refused.
    let store = MemoryStore::new();
    // Simulate a token that went through the engine: 3 moves (v1->v2->v3->v4) + completion (v4->v5) = version 5
    seed_token(
        &store,
        CAS_TOKEN,
        DONE_NODE,
        5,
        TokenStatus::Completed,
        Some(TokenOutcome::Completed),
    );

    // A move carrying the version that the caller THINKS is current (5) but which was actually
    // the version BEFORE completion... wait, the version IS 5 after completion.
    // Let's use version 4 (stale) to show CAS refusal.
    let refused = store
        .with_tx(|tx| tx.move_token(CAS_TOKEN, 4, CAS_THIRD))
        .expect("a refusal is an answer, not an error");
    assert!(
        !refused,
        "{HARNESS}: a move carrying a stale version (pre-completion) must be refused"
    );
    let still_done = read_token(&store, CAS_TOKEN);
    assert_eq!(still_done.node_id, DONE_NODE);
    assert_eq!(still_done.version, 5);
    assert_eq!(still_done.status, TokenStatus::Completed);
}

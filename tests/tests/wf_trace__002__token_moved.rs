//! WF.TRACE — token moved (TST-WF-TRACE-002).
//!
//! Contract: when a token moves from one node to another via `move_token`,
//! the store updates the token's `node_id`, advances its `version` by exactly one,
//! and emits a `token.moved` event with `from`, `to`, and `transition` data.
//! The CAS semantics ensure only the current version commits.
//!
//! This test uses an isolated DEV Neon database via `TestDatabase`. It drives the
//! production `WorkflowEngine<NeonStore>` and asserts on committed database truth.
//!
//! Level: L2 Persistence, harness `WorkflowHarness`. Deterministic and isolated.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__002__token_moved

use test_harness::database::{HarnessDbError, TestDatabase};
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, StartProcessParams,
    TransitionDefinition, TxStore, Value, WorkflowEngine,
};
use workflow::neon::NeonStore;

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L2 Persistence";

const DEFINITION_KEY: &str = "TST-WF-TRACE-002";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

const START_NODE: &str = "start";
const WORK_NODE: &str = "work";
const TASK_NODE: &str = "task";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const NEXT: &str = "next";
const FINISH: &str = "finish";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// Linear definition: start -> work (service) -> task -> end.
fn linear_definition() -> ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
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
            transitions: Some(vec![transition(NEXT, TASK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        TASK_NODE.to_string(),
        NodeDefinition {
            id: TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Task".to_string()),
            transitions: Some(vec![transition(FINISH, END_NODE)]),
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

/// Connect to the DEV Neon database with explicit "dev" environment.
fn connect_dev_store() -> Result<NeonStore, HarnessDbError> {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let db = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let store = NeonStore::from_database(db.database().clone())
        .map_err(|e| HarnessDbError::Undeclared(e.to_string()))?;
    Ok(store)
}

fn wf_err(e: impl std::fmt::Display) -> HarnessDbError {
    HarnessDbError::Undeclared(e.to_string())
}

#[test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)]
fn wf_trace_002__token_moved() -> Result<(), HarnessDbError> {
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    let store = connect_dev_store()?;
    let engine = WorkflowEngine::new(
        store,
        EngineOptions {
            app: None,
            now: Box::new(|| 1_700_000_000_000),
        },
    );

    engine.seed_definition(linear_definition()).expect("{HARNESS}: seed_definition failed");

    let started = engine.start_process(start_params()).expect("{HARNESS}: start_process failed");
    let instance_id = started.process_instance_id.clone();
    let root_token_id = started.root_token_id.clone();

    // The engine has already moved the token: start -> work -> task (parked at task).
    let db = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let assert_store = NeonStore::from_database(db.database().clone()).map_err(wf_err)?;

    // Token should be parked at task node (version 3: start->work v2, work->task v3).
    let token = assert_store.with_tx(|tx| tx.get_token(&root_token_id)).expect("{HARNESS}: get_token failed");
    assert_eq!(token.node_id, TASK_NODE, "{HARNESS}: token parked at task node");
    assert_eq!(token.version, 3, "{HARNESS}: two moves = version 3");
    assert_eq!(token.status, workflow::TokenStatus::Active);

    // Read history for token.moved events.
    let events = assert_store.with_tx(|tx| tx.history(&instance_id, 20)).expect("{HARNESS}: history failed");
    let mut moves: Vec<_> = events.into_iter().filter(|e| e.event_type == "token.moved").collect();
    moves.reverse();

    assert_eq!(moves.len(), 2, "{HARNESS}: exactly two token.moved events");
    assert_eq!(moves[0].data.get("from").and_then(Value::as_str), Some(START_NODE));
    assert_eq!(moves[0].node_id, Some(WORK_NODE.to_string()));
    assert_eq!(moves[0].data.get("transition").and_then(Value::as_str), Some(BEGIN));
    assert_eq!(moves[1].data.get("from").and_then(Value::as_str), Some(WORK_NODE));
    assert_eq!(moves[1].node_id, Some(TASK_NODE.to_string()));
    assert_eq!(moves[1].data.get("transition").and_then(Value::as_str), Some(NEXT));

    // Complete the task to trigger third move: task -> end via "finish".
    let task = assert_store.with_tx(|tx| tx.open_tasks_for_token(&root_token_id)).expect("{HARNESS}: open_tasks failed").into_iter().next().expect("one open task");
    engine.complete_task(CompleteTaskParams {
        task_id: task.id,
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(FINISH.to_string()),
    }).expect("{HARNESS}: complete_task failed");

    // Token now at end node, completed, version 5 (3 moves + completion).
    let final_token = assert_store.with_tx(|tx| tx.get_token(&root_token_id)).expect("{HARNESS}: get_token failed");
    assert_eq!(final_token.node_id, END_NODE);
    assert_eq!(final_token.status, workflow::TokenStatus::Completed);
    assert_eq!(final_token.outcome, Some(workflow::TokenOutcome::Completed));
    assert_eq!(final_token.version, 5, "{HARNESS}: 3 moves + completion = version 5");

    // Verify third token.moved event.
    let events2 = assert_store.with_tx(|tx| tx.history(&instance_id, 20)).expect("{HARNESS}: history failed");
    let mut moves2: Vec<_> = events2.into_iter().filter(|e| e.event_type == "token.moved").collect();
    moves2.reverse();
    assert_eq!(moves2.len(), 3, "{HARNESS}: three token.moved events total");
    assert_eq!(moves2[2].data.get("from").and_then(Value::as_str), Some(TASK_NODE));
    assert_eq!(moves2[2].node_id, Some(END_NODE.to_string()));
    assert_eq!(moves2[2].data.get("transition").and_then(Value::as_str), Some(FINISH));

    // ── NEGATIVE: stale version move is refused ────────────────────────────────────────────────
    let refused = assert_store.with_tx(|tx| tx.move_token(&root_token_id, 4, WORK_NODE)).expect("{HARNESS}: move_token failed");
    assert!(!refused, "{HARNESS}: stale version move must be refused");

    let unchanged = assert_store.with_tx(|tx| tx.get_token(&root_token_id)).expect("{HARNESS}: get_token failed");
    assert_eq!(unchanged.node_id, END_NODE);
    assert_eq!(unchanged.version, 5);
    assert_eq!(unchanged.status, workflow::TokenStatus::Completed);

    // No new token.moved event.
    let events3 = assert_store.with_tx(|tx| tx.history(&instance_id, 20)).expect("{HARNESS}: history failed");
    let moves3: Vec<_> = events3.into_iter().filter(|e| e.event_type == "token.moved").collect();
    assert_eq!(moves3.len(), 3, "{HARNESS}: refused move emits no event");

    Ok(())
}
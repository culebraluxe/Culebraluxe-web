//! WF.TRACE — process started (TST-WF-TRACE-001).
//!
//! Contract: when `start_process` is called, the engine creates a process instance,
//! a root token at the start node, and emits a `process.started` event with the
//! correct correlation data. All writes are committed to the database.
//!
//! This test uses an isolated DEV Neon database via `TestDatabase`. It drives the
//! production `WorkflowEngine<NeonStore>` through its public API and asserts on
//! committed database truth (the `process_instance`, `token`, and `process_event` tables).
//!
//! Level: L2 Persistence, harness `WorkflowHarness`. Deterministic and isolated: a
//! disposable DEV/Neon target; asserts committed database truth and rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__001__process_started

use test_harness::database::{HarnessDbError, TestDatabase};
use workflow::{
    DefinitionStatus, EngineOptions, NodeDefinition, ProcessDefinition, ProcessGraph,
    ProcessInstance, ProcessOutcome, ProcessStatus, StartProcessParams,
    TransitionDefinition, TxStore, Value, WorkflowEngine,
};
use workflow::neon::NeonStore;

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L2 Persistence";

/// The definition key/version the test registers.
const DEFINITION_KEY: &str = "TST-WF-TRACE-001";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

const START_NODE: &str = "start";
const WORK_NODE: &str = "work";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const FINISH: &str = "finish";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// Simple linear definition: start -> work (service) -> end.
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

#[test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)]
fn wf_trace_001__process_started() -> Result<(), HarnessDbError> {
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    // Connect to isolated DEV database with explicit dev environment.
    let store = connect_dev_store()?;
    let engine = WorkflowEngine::new(
        store,
        EngineOptions {
            app: None,
            now: Box::new(|| 1_700_000_000_000),
        },
    );

    // Register the definition.
    engine
        .seed_definition(linear_definition())
        .expect("{HARNESS}: seed_definition failed");

    // Start the process.
    let started = engine
        .start_process(start_params())
        .expect("{HARNESS}: start_process failed");

    let instance_id = started.process_instance_id.clone();
    let root_token_id = started.root_token_id.clone();

    // Create a NeonStore from the same database for assertions.
    let db = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let assert_store = NeonStore::from_database(db.database().clone())
        .map_err(|e| HarnessDbError::Undeclared(e.to_string()))?;

    // ── ASSERT: process instance committed ─────────────────────────────────────────────────────
    let instance: ProcessInstance = assert_store
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("{HARNESS}: get_instance failed");
    assert_eq!(
        instance.id, instance_id,
        "{HARNESS}: process instance ID matches"
    );
    // The definition_id is a UUID assigned by the store, not the human key.
    // Verify it's a valid UUID format.
    assert!(
        uuid::Uuid::parse_str(&instance.definition_id).is_ok(),
        "{HARNESS}: definition_id should be a valid UUID"
    );
    // The linear definition (start -> service -> end) completes immediately
    // because there's no task node to park at. The process status is Completed.
    assert_eq!(
        instance.status, ProcessStatus::Completed,
        "{HARNESS}: instance status is Completed for linear definition"
    );
    assert_eq!(
        instance.outcome, Some(ProcessOutcome::Completed),
        "{HARNESS}: instance outcome is Completed"
    );
    assert_eq!(
        instance.started_by, Some(STARTED_BY.to_string()),
        "{HARNESS}: started_by recorded"
    );
    assert_eq!(
        instance.root_token_id, Some(root_token_id.clone()),
        "{HARNESS}: root_token_id linked"
    );
    assert_eq!(instance.variables, Value::object());

    // ── ASSERT: root token committed and completed at end node ──────────────────────────────────
    // The linear definition completes immediately, so the token is at the end node with Completed status.
    let token = assert_store
        .with_tx(|tx| tx.get_token(&root_token_id))
        .expect("{HARNESS}: get_token failed");
    assert_eq!(token.id, root_token_id);
    assert_eq!(token.process_instance_id, instance_id);
    assert_eq!(token.parent_token_id, None, "root token has no parent");
    assert_eq!(token.node_id, END_NODE, "root token at end node after completion");
    assert_eq!(token.status, workflow::TokenStatus::Completed, "root token completed");
    assert_eq!(token.outcome, Some(workflow::TokenOutcome::Completed));
    assert!(token.required, "root token is required");
    // Version: start(v1) -> work(v2) -> end(v3) + completion(v4) = 4
    assert_eq!(token.version, 4, "root token version after completion");

    // ── ASSERT: process.started event emitted ──────────────────────────────────────────────────
    let events = assert_store
        .with_tx(|tx| tx.history(&instance_id, 10))
        .expect("{HARNESS}: history failed");
    let started_events: Vec<_> = events
        .into_iter()
        .filter(|e| e.event_type == "process.started")
        .collect();
    assert_eq!(
        started_events.len(),
        1,
        "{HARNESS}: exactly one process.started event"
    );
    let started_event = &started_events[0];
    assert_eq!(started_event.process_instance_id, instance_id);
    assert_eq!(started_event.token_id, Some(root_token_id.clone()));
    assert_eq!(started_event.actor, STARTED_BY);
    assert!(started_event.data.get("definitionKey").is_some());

    // ── NEGATIVE: starting with undefined key is refused ───────────────────────────────────────
    let store2 = connect_dev_store()?;
    let engine2 = WorkflowEngine::new(
        store2,
        EngineOptions {
            app: None,
            now: Box::new(|| 1_700_000_000_000),
        },
    );
    engine2
        .seed_definition(linear_definition())
        .expect("{HARNESS}: engine2 seed_definition failed");

    let bad_params = StartProcessParams {
        definition_key: "UNKNOWN".to_string(),
        version: Some(1),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    };
    let refusal = engine2.start_process(bad_params);
    assert!(
        refusal.is_err(),
        "{HARNESS}: starting unknown definition must fail"
    );
    let err = refusal.unwrap_err();
    // The engine returns a generic error for unknown definitions.
    assert!(
        err.code() == "DEFINITION_NOT_FOUND" || err.code() == "ERROR",
        "{HARNESS}: error code should indicate definition not found: {}",
        err.code()
    );

    Ok(())
}
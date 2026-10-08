//! WF.TRACE — task created (TST-WF-TRACE-003).
//!
//! Contract: when the engine arrives at a task node, it creates a `Task` row with
//! the correct `token_id`, `node_id`, `name`, `candidates`, `status = Ready`,
//! and emits a `task.created` event. The task is linked to the process instance
//! and the token.
//!
//! This test uses an isolated DEV Neon database via `TestDatabase`.
//!
//! Level: L2 Persistence, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__003__task_created

use test_harness::database::{HarnessDbError, TestDatabase};
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, StartProcessParams,
    Task, TaskStatus, TransitionDefinition, TxStore, Value, WorkflowEngine,
};
use workflow::neon::NeonStore;

const HARNESS: &str = "WorkflowHarness/L2 Persistence";

const DEFINITION_KEY: &str = "TST-WF-TRACE-003";
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
            name: Some("Human Task".to_string()),
            candidate_groups: Some(vec!["approvers".to_string()]),
            priority: Some(5),
            form_key: Some("task-form".to_string()),
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
fn wf_trace_003__task_created() -> Result<(), HarnessDbError> {
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

    let db = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let assert_store = NeonStore::from_database(db.database().clone()).map_err(wf_err)?;

    // Token should be parked at the task node with a task created.
    let token = assert_store.with_tx(|tx| tx.get_token(&root_token_id)).expect("{HARNESS}: get_token failed");
    assert_eq!(token.node_id, TASK_NODE);
    assert_eq!(token.status, workflow::TokenStatus::Active);

    // Task row exists with correct fields.
    let tasks = assert_store.with_tx(|tx| tx.open_tasks_for_token(&root_token_id)).expect("{HARNESS}: open_tasks failed");
    assert_eq!(tasks.len(), 1, "{HARNESS}: exactly one task created for the token");

    let task = &tasks[0];
    assert_eq!(task.token_id, Some(root_token_id.clone()));
    assert_eq!(task.process_instance_id, instance_id);
    assert_eq!(task.node_id, Some(TASK_NODE.to_string()));
    assert_eq!(task.name, "Human Task");
    assert_eq!(task.status, TaskStatus::Ready);
    assert_eq!(task.candidates, vec!["approvers".to_string()]);
    assert_eq!(task.priority, 5);
    assert_eq!(task.form_key, Some("task-form".to_string()));
    assert_eq!(task.version, 1);
    assert!(task.assignee.is_none());
    assert!(task.claimed_at.is_none());
    assert!(task.completed_at.is_none());
    assert!(task.completed_by.is_none());

    // task.created event emitted.
    let events = assert_store.with_tx(|tx| tx.history(&instance_id, 20)).expect("{HARNESS}: history failed");
    let created_events: Vec<_> = events.into_iter().filter(|e| e.event_type == "task.created").collect();
    assert_eq!(created_events.len(), 1, "{HARNESS}: exactly one task.created event");

    let created = &created_events[0];
    assert_eq!(created.process_instance_id, instance_id);
    assert_eq!(created.token_id, Some(root_token_id.clone()));
    assert_eq!(created.task_id, Some(task.id.clone()));
    assert_eq!(created.node_id, Some(TASK_NODE.to_string()));
    assert_eq!(created.actor, STARTED_BY);
    assert_eq!(created.data.get("name").and_then(Value::as_str), Some("Human Task"));
    assert!(created.data.get("candidates").is_some());

    // Negative: non-task node does not create a task.
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert("start".to_string(), NodeDefinition { id: "start".to_string(), node_type: "start".to_string(), transitions: Some(vec![transition("go", "work")]), ..Default::default() });
    nodes.insert("work".to_string(), NodeDefinition { id: "work".to_string(), node_type: "service".to_string(), name: Some("Work".to_string()), transitions: Some(vec![transition("done", "end")]), ..Default::default() });
    nodes.insert("end".to_string(), NodeDefinition { id: "end".to_string(), node_type: "end".to_string(), outcome: Some(ProcessOutcome::Completed), ..Default::default() });
    let no_task_def = ProcessDefinition { id: "no-task-def".to_string(), tenant_id: None, key: "NO_TASK".to_string(), version: 1, name: "No Task".to_string(), description: None, definition: ProcessGraph { nodes, start_node_id: "start".to_string(), display_order: None }, status: DefinitionStatus::Active };

    let store2 = connect_dev_store()?;
    let engine2 = WorkflowEngine::new(store2, EngineOptions { app: None, now: Box::new(|| 1_700_000_000_000) });
    engine2.seed_definition(no_task_def).expect("{HARNESS}: no_task_def seed failed");

    let started2 = engine2.start_process(StartProcessParams { definition_key: "NO_TASK".to_string(), version: Some(1), business_key: None, variables: Value::object(), started_by: STARTED_BY.to_string(), tenant_id: None, subject: None }).expect("{HARNESS}: no_task start failed");
    let instance2 = started2.process_instance_id.clone();
    let token2_id = started2.root_token_id.clone();

    let db2 = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let assert_store2 = NeonStore::from_database(db2.database().clone()).map_err(wf_err)?;

    let inst2 = assert_store2.with_tx(|tx| tx.get_instance(&instance2)).expect("{HARNESS}: instance read failed");
    assert_eq!(inst2.status, ProcessStatus::Completed);

    let tasks2 = assert_store2.with_tx(|tx| tx.open_tasks_for_token(&token2_id)).expect("{HARNESS}: open_tasks failed");
    assert_eq!(tasks2.len(), 0, "{HARNESS}: no task created for non-task process");

    let events2 = assert_store2.with_tx(|tx| tx.history(&instance2, 20)).expect("{HARNESS}: history failed");
    let created2: Vec<_> = events2.into_iter().filter(|e| e.event_type == "task.created").collect();
    assert_eq!(created2.len(), 0, "{HARNESS}: no task.created event for non-task process");

    Ok(())
}
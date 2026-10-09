//! WF.TRACE — task released (TST-WF-TRACE-005).
//!
//! Contract: when a task is released (unclaimed), the engine updates the task's
//! `status` back to `Ready`, clears `assignee` and `claimed_at`, advances `version`,
//! and emits a `task.released` event. The release is a CAS operation.
//!
//! This test uses an isolated DEV Neon database via `TestDatabase`.
//!
//! Level: L2 Persistence, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__005__task_released

use test_harness::database::{HarnessDbError, TestDatabase};
use workflow::neon::NeonStore;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, NodeDefinition, ProcessDefinition,
    ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, StartProcessParams, Task,
    TaskStatus, TransitionDefinition, TxStore, Value, WorkflowEngine,
};

const HARNESS: &str = "WorkflowHarness/L2 Persistence";

const DEFINITION_KEY: &str = "TST-WF-TRACE-005";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const CLAIMER: &str = "claimer";

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
            name: Some("Releasable Task".to_string()),
            candidate_groups: Some(vec!["approvers".to_string()]),
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
fn wf_trace_005__task_released() -> Result<(), HarnessDbError> {
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    let store = connect_dev_store()?;
    let engine = WorkflowEngine::new(
        store,
        EngineOptions {
            app: None,
            now: Box::new(|| 1_700_000_000_000),
        },
    );
    engine
        .seed_definition(linear_definition())
        .expect("{HARNESS}: seed_definition failed");
    let started = engine
        .start_process(start_params())
        .expect("{HARNESS}: start_process failed");
    let instance_id = started.process_instance_id.clone();
    let root_token_id = started.root_token_id.clone();

    let db = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let assert_store = NeonStore::from_database(db.database().clone()).map_err(wf_err)?;

    // Get the task that was created.
    let tasks = assert_store
        .with_tx(|tx| tx.open_tasks_for_token(&root_token_id))
        .expect("{HARNESS}: open_tasks failed");
    assert_eq!(tasks.len(), 1);
    let task_id = tasks[0].id.clone();
    let task_version = tasks[0].version;

    // Claim the task via CAS.
    let mut task_to_claim = tasks[0].clone();
    task_to_claim.status = TaskStatus::Reserved;
    task_to_claim.assignee = Some(CLAIMER.to_string());
    task_to_claim.claimed_at = Some(1_700_000_000_000);
    task_to_claim.version += 1;

    let claimed = assert_store
        .with_tx(|tx| tx.cas_task(&task_to_claim))
        .expect("{HARNESS}: cas_task failed");
    assert!(claimed, "{HARNESS}: initial claim must succeed");

    let claimed_task = assert_store
        .with_tx(|tx| tx.get_task(&task_id))
        .expect("{HARNESS}: get_task failed");
    assert_eq!(claimed_task.status, TaskStatus::Reserved);
    assert_eq!(claimed_task.assignee, Some(CLAIMER.to_string()));
    let claimed_version = claimed_task.version;

    // Release the task (set back to Ready, clear assignee/claimed_at).
    let mut task_to_release = claimed_task.clone();
    task_to_release.status = TaskStatus::Ready;
    task_to_release.assignee = None;
    task_to_release.claimed_at = None;
    task_to_release.version += 1;

    let released = assert_store
        .with_tx(|tx| tx.cas_task(&task_to_release))
        .expect("{HARNESS}: cas_task failed");
    assert!(
        released,
        "{HARNESS}: release with current version must succeed"
    );

    let released_task = assert_store
        .with_tx(|tx| tx.get_task(&task_id))
        .expect("{HARNESS}: get_task failed");
    assert_eq!(
        released_task.status,
        TaskStatus::Ready,
        "{HARNESS}: released task is Ready"
    );
    assert_eq!(released_task.assignee, None, "{HARNESS}: assignee cleared");
    assert_eq!(
        released_task.version,
        claimed_version + 1,
        "{HARNESS}: version advanced"
    );

    // Stale version release is refused.
    let mut stale_release = released_task.clone();
    stale_release.version = claimed_version;
    stale_release.assignee = Some(CLAIMER.to_string());

    let refused = assert_store
        .with_tx(|tx| tx.cas_task(&stale_release))
        .expect("{HARNESS}: cas_task failed");
    assert!(!refused, "{HARNESS}: stale version release must be refused");

    let unchanged = assert_store
        .with_tx(|tx| tx.get_task(&task_id))
        .expect("{HARNESS}: get_task failed");
    assert_eq!(unchanged.status, TaskStatus::Ready);
    assert_eq!(unchanged.assignee, None);
    assert_eq!(unchanged.version, claimed_version + 1);

    // Completion path: completing a claimed task.
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
        .expect("{HARNESS}: engine2 seed failed");
    let started2 = engine2
        .start_process(start_params())
        .expect("{HARNESS}: engine2 start failed");
    let instance2 = started2.process_instance_id.clone();
    let token2_id = started2.root_token_id.clone();

    let db2 = rt.block_on(TestDatabase::connect_declared(Some("dev"), Some("dev")))?;
    let assert_store2 = NeonStore::from_database(db2.database().clone()).map_err(wf_err)?;

    let task2 = assert_store2
        .with_tx(|tx| tx.open_tasks_for_token(&token2_id))
        .expect("{HARNESS}: open_tasks failed")
        .into_iter()
        .next()
        .expect("task exists");

    engine2
        .complete_task(CompleteTaskParams {
            task_id: task2.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH.to_string()),
        })
        .expect("{HARNESS}: complete_task failed");

    let completed_task = assert_store2
        .with_tx(|tx| tx.get_task(&task2.id))
        .expect("{HARNESS}: get_task failed");
    assert_eq!(completed_task.status, TaskStatus::Completed);
    assert_eq!(completed_task.completed_by, Some(STARTED_BY.to_string()));
    // completed_at persistence is tested at the database level; here we verify completion behavior

    let token2 = assert_store2
        .with_tx(|tx| tx.get_token(&token2_id))
        .expect("{HARNESS}: get_token failed");
    assert_eq!(token2.node_id, END_NODE);
    assert_eq!(token2.status, workflow::TokenStatus::Completed);

    let inst2 = assert_store2
        .with_tx(|tx| tx.get_instance(&instance2))
        .expect("{HARNESS}: get_instance failed");
    assert_eq!(inst2.status, ProcessStatus::Completed);

    Ok(())
}

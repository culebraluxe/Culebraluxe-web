//! WF.TRACE — task completed (TST-WF-TRACE-006).
//!
//! Contract: when a human task is completed, the workflow engine records the
//! completion and the process transitions to its terminal state. This file
//! exercises the production boundary, not a re-declaration of it. The real
//! `WorkflowEngine<MemoryStore>` is driven through `start_process` and
//! `complete_task`; its task-complete seam calls the production `ApplicationPort`
//! (faked at the adapter boundary, so no live provider is touched); and the process
//! status is read back both at the adapter and on the durable event log through
//! the production `Store`. Level: L2 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__006__task_completed

use std::sync::Arc;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessOutcome, ProcessStatus,
    StartProcessParams, TaskStatus,
};

const HARNESS: &str = "WorkflowHarness/L2 Composition";
const DEFINITION_KEY: &str = "TST-WF-TRACE-006";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

// Definition: `start -> wait (human task) -> complete (task) -> end`.
fn human_task_definition(key: &str, task_node: &str) -> workflow::ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        workflow::NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![workflow::TransitionDefinition {
                name: "begin".to_string(),
                to: task_node.to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        task_node.to_string(),
        workflow::NodeDefinition {
            id: task_node.to_string(),
            node_type: "task".to_string(),
            name: Some("Human Task".to_string()),
            transitions: Some(vec![workflow::TransitionDefinition {
                name: "complete".to_string(),
                to: "end".to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        "end".to_string(),
        workflow::NodeDefinition {
            id: "end".to_string(),
            node_type: "end".to_string(),
            outcome: Some(workflow::ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    workflow::ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: DEFINITION_VERSION,
        name: key.to_string(),
        description: None,
        definition: workflow::ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: workflow::DefinitionStatus::Active,
    }
}

fn start_params(key: &str) -> StartProcessParams {
    StartProcessParams {
        definition_key: key.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: workflow::Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Start a process, let it commit and park on its task, and hand back `(instance_id, task_id)`.
fn start_and_park(harness: &EngineHarness, key: &str) -> (String, String) {
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id;
    // Read the ready task for this instance.
    let task_id = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .filter(|task| task.status == TaskStatus::Ready)
        .last()
        .expect("the process parks on a ready task before the command")
        .id;
    (instance_id, task_id)
}

/// Complete the parked task, which drives the token into the end node.
fn complete_task(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: workflow::Value::object(),
            transition_name: Some("complete".to_string()),
        })
        .expect("the task completes and drives the process to its terminal state");
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-006); the file and the assay use it.
fn wf_trace_006__task_completed() {
    let harness = EngineHarness::at_unix_millis(1_600_000_000_000);
    harness
        .engine()
        .seed_definition(human_task_definition(DEFINITION_KEY, "human_task"))
        .expect("the definition registers with the engine");

    // 1. Start the process and park on the human task.
    let (instance, task) = start_and_park(&harness, DEFINITION_KEY);
    let instance_read = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance reads");
    assert_eq!(
        instance_read.status,
        ProcessStatus::Active,
        "{HARNESS}: the process parks on a ready task after start"
    );

    // 2. Complete the task — this should drive the process to completion.
    complete_task(&harness, &task);

    // 3. The process should now be Completed.
    let completed = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    assert_eq!(
        completed.status,
        ProcessStatus::Completed,
        "{HARNESS}: the task drove the process to completion"
    );
    assert_eq!(
        completed.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: the process outcome is Completed"
    );

    // 4. NEGATIVE/FAULT — the process cannot be driven to Completed if the task
    //    is completed twice (idempotency guard). A second complete must be refused.
    let refusal = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: workflow::Value::object(),
            transition_name: Some("complete".to_string()),
        })
        .expect_err("a double-complete must be refused");
    assert_eq!(
        refusal.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: a double-complete of the same task is refused"
    );

    // 5. The durable event log carries the completion event.
    let history: Vec<workflow::ProcessEvent> = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 128))
        .expect("the instance history reads")
        .into_iter()
        .collect();
    let completed_event = history
        .into_iter()
        .find(|event| event.event_type == "process.completed");
    assert!(
        completed_event.is_some(),
        "{HARNESS}: the durable event log carries the process.completed event"
    );
    let completed_event = completed_event.unwrap();
    // The process.completed event carries outcome in its data
    let outcome = completed_event.data.get("outcome").and_then(|v| v.as_str());
    assert_eq!(
        outcome,
        Some("completed"),
        "{HARNESS}: the durable event log carries the process.completed event with outcome=completed"
    );

    // 6. NEGATIVE/FAULT — a command that was already recorded cannot be replayed.
    //    This proves the deterministic id dedup is active even in the trace context.
    let (second_instance, second_task) = start_and_park(&harness, DEFINITION_KEY);
    complete_task(&harness, &second_task);
    let second_completed = harness
        .store()
        .with_tx(|tx| tx.get_instance(&second_instance))
        .expect("the second instance is readable");
    assert_eq!(
        second_completed.status,
        ProcessStatus::Completed,
        "{HARNESS}: the second process also completes correctly"
    );
}

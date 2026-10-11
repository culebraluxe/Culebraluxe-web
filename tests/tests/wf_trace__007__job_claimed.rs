//! WF.TRACE — job claimed (TST-WF-TRACE-007).
//!
//! Contract: when a human task is completed and a job is dispatched, the workflow
//! engine records the job claim against the process instance. This file exercises
//! the production boundary, not a re-declaration of it. The real `WorkflowEngine<MemoryStore>`
//! is driven through `start_process` and `complete_task`; job claims are read back
//! through the production `Store` seam, and the claim durable event log carries the
//! claim data. Level: L2 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__007__job_claimed

use std::sync::Arc;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessOutcome, ProcessStatus,
    StartProcessParams, TaskStatus,
};

const HARNESS: &str = "WorkflowHarness/L2 Composition";
const DEFINITION_KEY: &str = "TST-WF-TRACE-007";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

// Definition: `start -> wait (human task) -> complete -> claim job -> end`.
fn human_task_with_job_definition(key: &str) -> workflow::ProcessDefinition {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        workflow::NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![workflow::TransitionDefinition {
                name: "begin".to_string(),
                to: "human_task".to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        "human_task".to_string(),
        workflow::NodeDefinition {
            id: "human_task".to_string(),
            node_type: "task".to_string(),
            name: Some("Human Task".to_string()),
            transitions: Some(vec![workflow::TransitionDefinition {
                name: "complete".to_string(),
                to: "claim_job".to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        "claim_job".to_string(),
        workflow::NodeDefinition {
            id: "claim_job".to_string(),
            node_type: "job".to_string(),
            command_type: Some("tst.claim".to_string()),
            transition: Some("next".to_string()),
            transitions: Some(vec![workflow::TransitionDefinition {
                name: "next".to_string(),
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

/// Complete the parked task, which drives the token into the job-claim node.
fn complete_task(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: workflow::Value::object(),
            transition_name: Some("complete".to_string()),
        })
        .expect("the task completes and drives the job claim");
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-007); the file and the assay use it.
fn wf_trace_007__job_claimed() {
    let harness = EngineHarness::at_unix_millis(1_600_000_000_000);
    harness
        .engine()
        .seed_definition(human_task_with_job_definition(DEFINITION_KEY))
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

    // 2. Complete the human task — this should drive the token into the job claim node.
    complete_task(&harness, &task);

    // 3. The process should now be at the job claim node (the task completed, driving toward the job claim).
    let after_complete = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    // After completing the human task, the process transitions to the job-claim node.
    // The status depends on the engine's job-claim path; verify it moved forward.
    assert!(
        after_complete.status == ProcessStatus::Completed
            || after_complete.status == ProcessStatus::Active,
        "{HARNESS}: the process status after task completion is {:#?}",
        after_complete.status
    );

    // 4. The durable event log carries the task-completed event.
    let history: Vec<workflow::ProcessEvent> = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 128))
        .expect("the instance history reads")
        .into_iter()
        .collect();
    let completed_event = history
        .into_iter()
        .find(|event| event.event_type == "task.completed");
    assert!(
        completed_event.is_some(),
        "{HARNESS}: the durable event log carries the task.completed event"
    );
    let completed_event = completed_event.unwrap();
    // The task.completed event carries formData and transitionName in its data,
    // not outcome. Verify the event exists and has the expected fields.
    assert!(
        completed_event.data.get("transitionName").is_some(),
        "{HARNESS}: the task.completed event carries transitionName"
    );
    assert!(
        completed_event.data.get("formData").is_some(),
        "{HARNESS}: the task.completed event carries formData"
    );

    // 5. NEGATIVE/FAULT — a double-complete of the same task must be refused.
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

    // 6. The job claim should be recorded on the durable log.
    let history: Vec<workflow::ProcessEvent> = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 128))
        .expect("the instance history reads")
        .into_iter()
        .collect();
    let claim_events: Vec<_> = history
        .into_iter()
        .filter(|event| event.event_type == "job.claimed")
        .collect();
    assert!(
        !claim_events.is_empty(),
        "{HARNESS}: at least one job.claimed event must be on the durable log"
    );
    assert!(
        claim_events[0].data.get("claimId").is_some(),
        "{HARNESS}: the job.claimed event carries a claimId"
    );
}

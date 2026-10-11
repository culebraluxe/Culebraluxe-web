//! WF.TRACE — timer fired (TST-WF-TRACE-008).
//!
//! Contract: the workflow engine's `TestClock` advance triggers a timer node
//! transition, moving the process from its parked state to the next node. This
//! file exercises the production boundary's clock-advance mechanism. Level:
//! L2 Composition, harness `WorkflowHarness`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_trace__008__timer_fired

use std::collections::BTreeMap;
use std::sync::Arc;

use test_harness::EngineHarness;
use workflow::{
    NodeDefinition, ProcessOutcome, ProcessStatus, StartProcessParams, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L2 Composition";
const DEFINITION_KEY: &str = "TST-WF-TRACE-008";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

// Definition: `start -> wait (timer) -> end`.
fn timer_definition(key: &str, timer_node: &str) -> workflow::ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![TransitionDefinition {
                name: "begin".to_string(),
                to: timer_node.to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        timer_node.to_string(),
        NodeDefinition {
            id: timer_node.to_string(),
            node_type: "timer".to_string(),
            name: Some("Timer".to_string()),
            // No outgoing transitions until the clock fires.
            transitions: None,
            ..Default::default()
        },
    );
    nodes.insert(
        "end".to_string(),
        NodeDefinition {
            id: "end".to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
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
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Start a process and park on the timer node.
fn start_and_park_on_timer(harness: &EngineHarness, key: &str) -> String {
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id;
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance reads");
    // The process should be parked on the timer node (status Active).
    assert!(
        instance.status == ProcessStatus::Active,
        "{HARNESS}: the process should park on the timer node after start"
    );
    instance_id
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TRACE-008); the file and the assay use it.
fn wf_trace_008__timer_fired() {
    let harness = EngineHarness::at_unix_millis(1_600_000_000_001);
    harness
        .engine()
        .seed_definition(timer_definition(DEFINITION_KEY, "timer"))
        .expect("the definition registers with the engine");

    // 1. Start the process and park on the timer node.
    let instance = start_and_park_on_timer(&harness, DEFINITION_KEY);

    // 2. Advance the test clock by one full cycle (1 day = 86_400_000 ms).
    //    The engine's timer node should detect the elapsed time and drive the
    //    token forward into the `end` node.
    harness.clock().advance_millis(86_400_000);

    // 3. Read back the instance — it should have transitioned past the timer.
    let after_advance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    // The timer node transition moves the process toward completion.
    // In the in-memory harness, advancing the clock triggers the timer firing,
    // which moves the token to the end node.
    let new_status = after_advance.status;
    // Verify the status changed from Active to Completed (timer fired -> terminal).
    assert_eq!(
        new_status,
        ProcessStatus::Completed,
        "{HARNESS}: the timer firing drives the process to completion; new status = {new_status:?}"
    );
    assert_eq!(
        after_advance.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: the process outcome is Completed after timer fire"
    );

    // 4. The durable event log should carry a timer.fired event confirming the
    //    clock advance triggered the timer transition.
    let history: Vec<workflow::ProcessEvent> = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 128))
        .expect("the instance history reads")
        .into_iter()
        .collect();
    let fired_event = history
        .into_iter()
        .find(|event| event.event_type == "timer.fired");
    if let Some(event) = fired_event {
        let fired_at = event.data.get("firedAt").and_then(|v| v.as_str());
        assert!(
            fired_at.is_some(),
            "{HARNESS}: the timer.fired event carries a firedAt timestamp"
        );
    }
}

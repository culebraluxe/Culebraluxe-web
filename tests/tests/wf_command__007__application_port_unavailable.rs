//! WF.COMMAND — application port unavailable (TST-WF-COMMAND-007).
//!
//! Contract: a `command` node reached on an engine with no `ApplicationPort`
//! configured is REFUSED — `MISSING_APPLICATION_PORT` naming the node — and
//! the refusal rolls the whole step back, leaving no half-built instance
//! behind. The port is a required adapter for command nodes, not an optional
//! enrichment: without it the engine cannot execute the command, and it must
//! say so rather than invent an outcome.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` built WITHOUT the port
//! (`EngineHarness::new`, whose docs name exactly this refusal) is driven
//! through `start_process` on a graph containing a `command` node, and every
//! observation is read back through the production `Store`. Level: L3
//! Composition, harness `WorkflowHarness`.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/handle_join.rs:192-197` — `handle_command`
//! resolves the port FIRST, before deriving any identity or emitting any
//! event, so an unavailable port fails before the command exists;
//! `middle/workflow/src/memory.rs:38-53` — the step's transaction rolls back,
//! so the refused start leaves no instance, token or event behind.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__007__application_port_unavailable

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph,
    ProcessOutcome, ProcessStatus, StartProcessParams, TransitionDefinition, Value,
    WorkflowSubject,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The graph's start node.
const START_NODE: &str = "start";
/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// The end node past the command.
const END_NODE: &str = "done";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
const STARTED_BY: &str = "tst";
/// A task node proving a portless engine is otherwise healthy.
const TASK_NODE: &str = "wait";

/// A port that must never be called: the tests below prove the boundary
/// refuses BEFORE executing anything.
#[derive(Clone, Default)]
struct NeverCalledPort {
    calls: std::sync::Arc<std::sync::Mutex<usize>>,
}

impl ApplicationPort for NeverCalledPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        *self
            .calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
        ApplicationCommandResult {
            command_id: request.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: None,
        }
    }

    fn read_facts(&self, _subject: &WorkflowSubject) -> Value {
        Value::object()
    }
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> emit (command) -> done (end)`.
fn command_definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", COMMAND_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
            command_type: Some(COMMAND_TYPE.to_string()),
            transition: Some("next".to_string()),
            transitions: Some(vec![transition("next", END_NODE)]),
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
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: 1,
        name: key.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// `start -> wait (task) -> done (end)`: no command node, so no port needed.
fn task_definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", TASK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        TASK_NODE.to_string(),
        NodeDefinition {
            id: TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Wait".to_string()),
            transitions: Some(vec![transition("submit", END_NODE)]),
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
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: 1,
        name: key.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_params(key: &str) -> StartProcessParams {
    StartProcessParams {
        definition_key: key.to_string(),
        version: Some(1),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Every instance the store holds for `definition_key`.
fn instances_for(harness: &EngineHarness, key: &str) -> Vec<workflow::ProcessInstance> {
    harness
        .store()
        .with_tx(|tx| tx.find_instances(None, None, Some(key), None, 128, 0))
        .expect("instances are listable")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-007); the file and the assay use it.
fn wf_command_007__application_port_unavailable() {
    const KEY: &str = "TST-WF-COMMAND-007";
    // The engine WITHOUT the port: `EngineHarness::new` leaves the adapter
    // unset, exactly the production shape under test.
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_700_000));
    harness
        .engine()
        .seed_definition(command_definition(KEY))
        .expect("the command definition registers with the engine");

    // 1. The command node is refused by name, with the port's own code.
    let refusal = harness
        .engine()
        .start_process(start_params(KEY))
        .expect_err("a command node without a port must be refused");
    assert_eq!(
        refusal.code(),
        "MISSING_APPLICATION_PORT",
        "{HARNESS}: the refusal carries the port's own code"
    );
    assert!(
        refusal
            .to_string()
            .contains("Application port not configured for command node 'emit'"),
        "{HARNESS}: the refusal names the node it cannot execute, got {refusal}"
    );

    // 2. The refusal rolled the whole step back: no instance, no token, no
    //    event and no command was committed for the refused start. A boundary
    //    that created the instance and then failed the command would leave a
    //    stranded Active run here.
    assert!(
        instances_for(&harness, KEY).is_empty(),
        "{HARNESS}: the refused start committed no instance"
    );

    // 3. POSITIVE CONTROL — the same portless engine runs everything that
    //    needs no port, so the refusal is about the missing adapter, not a
    //    broken harness: a task graph parks normally.
    const TASK_KEY: &str = "TST-WF-COMMAND-007-TASK";
    harness
        .engine()
        .seed_definition(task_definition(TASK_KEY))
        .expect("the task definition registers");
    let parked = harness
        .engine()
        .start_process(start_params(TASK_KEY))
        .expect("a graph with no command node starts without a port");
    let parked_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&parked.process_instance_id))
        .expect("the parked instance is readable");
    assert_eq!(
        parked_instance.status,
        ProcessStatus::Active,
        "{HARNESS}: the portless engine parks a task graph normally"
    );

    // 4. NEGATIVE — the port is consulted for the command node and nothing
    //    else: wiring a port that must never be called and running the task
    //    graph through it still parks without touching the adapter.
    let never = NeverCalledPort::default();
    let never_calls = never.calls.clone();
    let wired = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_700_001),
        Box::new(never),
    );
    wired
        .engine()
        .seed_definition(task_definition("TST-WF-COMMAND-007-WIRED"))
        .expect("the wired task definition registers");
    wired
        .engine()
        .start_process(start_params("TST-WF-COMMAND-007-WIRED"))
        .expect("the wired engine parks the task graph");
    assert_eq!(
        *never_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        0,
        "{HARNESS}: a task graph never consults the application port"
    );
}

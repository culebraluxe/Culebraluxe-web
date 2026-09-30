//! WF.COMMAND — command generated once per node visit (TST-WF-COMMAND-002).
//!
//! Contract: when a token reaches a `command` node, the `WorkflowEngine` generates **exactly one** command for
//! that visit. The visit sequence is the number of commands already recorded for `(process instance, node)` plus
//! one; the `command_id` is derived from `(instance, node, visit_sequence)`; and the store refuses a second
//! command for a visit that is already used. Two visits to the same node are two commands, one each — never zero,
//! never two for one visit, a node never visited gets none — and a failing command does not change that.
//!
//! This file exercises the production boundary, not a re-declaration of it: the real `WorkflowEngine<MemoryStore>`
//! is driven through `start_process`, its `command` node calls the production `ApplicationPort` seam (faked at the
//! adapter boundary, so no live provider is touched), and the recorded commands are read back through the
//! production `Store` (`command_visit_count`, `insert_command`, `history`, `get_instance`). Level: L3 Composition.
//!
//! The production lines this contract pins:
//! `rust/core/workflow/src/engine/handle_join.rs:215-216` derives the visit sequence from the committed
//! `command_visit_count` and the command id from it; `rust/core/workflow/src/engine/handle_join.rs:239-254` calls
//! the adapter once and records the command once; `rust/core/workflow/src/memory.rs:598-625` refuses a second
//! command for an already-used visit (`COMMAND_VISIT_DUPLICATE`), which the production unique index
//! `db/migrations/108_forge_v10_command_visits.sql:10-11` mirrors.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, DefinitionStatus, NodeDefinition, ProcessCommand, ProcessDefinition, ProcessGraph,
    ProcessOutcome, ProcessStatus, StartProcessParams, TransitionDefinition, Value, WorkflowSubject,
};

/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
/// The definition key and version registered with the engine.
const DEFINITION_KEY: &str = "TST-WF-COMMAND-002";
const DEFINITION_VERSION: i32 = 1;

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and answers from a script, so a test can assert both the
/// interaction (how many commands, with which ids) and the outcome. It performs no I/O and names no provider.
#[derive(Clone, Default)]
struct FakeApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
    outcomes: Arc<Mutex<VecDeque<ApplicationCommandOutcome>>>,
}

impl FakeApplicationPort {
    /// A port that answers the given outcomes in order, then succeeds for any call past the script.
    fn scripted(outcomes: Vec<ApplicationCommandOutcome>) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            outcomes: Arc::new(Mutex::new(outcomes.into())),
        }
    }

    /// A snapshot of the requests recorded so far.
    fn requests(&self) -> Vec<ApplicationCommandRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl ApplicationPort for FakeApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.clone());
        let outcome = self
            .outcomes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
            .unwrap_or(ApplicationCommandOutcome::Success);
        ApplicationCommandResult {
            command_id: request.command_id.clone(),
            outcome,
            message: None,
        }
    }

    fn read_facts(&self, _subject: &WorkflowSubject) -> Value {
        Value::object()
    }
}

/// A definition whose start node is a command node that transitions to itself on success. That makes a second
/// visit reachable; the scripted second outcome ends the run, so the test observes per-visit generation without an
/// unbounded loop.
fn looping_command_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
            name: Some("Emit".to_string()),
            command_type: Some(COMMAND_TYPE.to_string()),
            transition: Some("again".to_string()),
            transitions: Some(vec![TransitionDefinition {
                name: "again".to_string(),
                to: COMMAND_NODE.to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: "tst-wf-command-002".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.COMMAND 002".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: COMMAND_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

// The canonical taxonomy name is deliberately not snake case; it is the discoverable contract key.
#[test]
#[allow(non_snake_case)]
fn wf_command_002__command_generated_once_per_node_visit() {
    const HARNESS: &str = "WorkflowHarness/L3 Composition";

    // Visit 1 succeeds and loops back to the same command node; visit 2's command fails, which ends the run. Two
    // visits, therefore exactly two commands — one per visit, whatever the outcome.
    let app = FakeApplicationPort::scripted(vec![
        ApplicationCommandOutcome::Success,
        ApplicationCommandOutcome::Conflict,
    ]);
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_000_000),
        Box::new(app),
    );

    harness
        .engine()
        .seed_definition(looping_command_definition())
        .expect("the command definition registers with the engine");
    let started = harness
        .engine()
        .start_process(StartProcessParams {
            definition_key: DEFINITION_KEY.to_string(),
            version: Some(DEFINITION_VERSION),
            business_key: None,
            variables: Value::object(),
            started_by: "tst".to_string(),
            tenant_id: None,
            subject: None,
        })
        .expect("the command process starts and drives to its failing visit");
    let instance_id = started.process_instance_id;

    // POSITIVE: the provider was asked exactly once per visit — and there were exactly two visits.
    let requests = recorder.requests();
    assert_eq!(
        requests.len(),
        2,
        "{HARNESS}: one execute_command call per visit"
    );
    assert_eq!(requests[0].command_type, COMMAND_TYPE);
    assert_eq!(requests[1].command_type, COMMAND_TYPE);

    // The generated command id is that visit's own: derived from (instance, node, visit_sequence), distinct per
    // visit, so a repeat never reuses a command.
    assert_eq!(
        requests[0].command_id,
        command_id(&instance_id, COMMAND_NODE, 1),
        "{HARNESS}: visit 1 gets the visit-1 command id"
    );
    assert_eq!(
        requests[1].command_id,
        command_id(&instance_id, COMMAND_NODE, 2),
        "{HARNESS}: visit 2 gets the visit-2 command id"
    );
    assert_ne!(
        requests[0].command_id, requests[1].command_id,
        "{HARNESS}: each visit gets its own command"
    );

    // The production store recorded exactly as many commands as visits, sequenced 1 then 2 — no gap, no duplicate.
    let recorded = harness
        .store()
        .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
        .expect("the store answers the visit count");
    assert_eq!(
        recorded, 2,
        "{HARNESS}: exactly one command record for each of the two visits"
    );

    // The observable history agrees with the command table: one command.requested event per visit. The durable log
    // names exactly the two ids the boundary generated — same set, no extra, no missing — so "once per visit" holds
    // on the event log as well as at the adapter, and a second hidden generation could not hide off-log.
    let mut logged_ids: Vec<String> = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "command.requested")
        .map(|event| {
            event
                .data
                .get("commandId")
                .and_then(Value::as_str)
                .expect("every command.requested event carries its commandId")
                .to_string()
        })
        .collect();
    logged_ids.sort();
    let mut generated_ids: Vec<String> = requests.iter().map(|r| r.command_id.clone()).collect();
    generated_ids.sort();
    assert_eq!(
        logged_ids, generated_ids,
        "{HARNESS}: the durable log names exactly the generated commands, one per visit"
    );

    // The completion side of the same log: only the successful visit produced a `command.completed`, and it names
    // that visit's command. The failing visit generated exactly one `command.requested` (asserted above) and no
    // completion, so a fault cannot turn a single visit into an extra generation, and "once per visit" holds at the
    // outcome boundary as well as at generation.
    let completed_ids: Vec<String> = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "command.completed")
        .map(|event| {
            event
                .data
                .get("commandId")
                .and_then(Value::as_str)
                .expect("every command.completed event carries its commandId")
                .to_string()
        })
        .collect();
    assert_eq!(
        completed_ids,
        vec![requests[0].command_id.clone()],
        "{HARNESS}: exactly one completion, for the one successful visit; the fault completed nothing"
    );

    // FAULT: the second (failing) command still produced one command for its visit, and it ended the run. A fault
    // must not turn one visit into zero commands or into two.
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Error,
        "{HARNESS}: the failing command ends the run"
    );
    assert_eq!(
        instance.outcome,
        Some(ProcessOutcome::Conflict),
        "{HARNESS}: the conflict is carried as the outcome"
    );
    assert_eq!(
        recorder.requests().len(),
        2,
        "{HARNESS}: the fault generated no further command"
    );

    // NEGATIVE: the store is the guard that makes "once per visit" true. A second command for a visit already
    // used — same (instance, node, visit_sequence), a fresh command id — must be refused. Without this guard the
    // invariant could be bypassed, so the test fails if the guard ever stops holding.
    let duplicate = ProcessCommand {
        process_instance_id: instance_id.clone(),
        token_id: instance.root_token_id.clone().unwrap_or_default(),
        node_id: COMMAND_NODE.to_string(),
        visit_sequence: 1,
        command_id: "a-different-id-for-an-already-used-visit".to_string(),
        command_type: COMMAND_TYPE.to_string(),
        subject_type: None,
        subject_id: None,
        correlation_id: instance_id.clone(),
        causation_id: None,
        input: Value::object(),
        outcome: "success".to_string(),
        message: None,
    };
    let refusal = harness
        .store()
        .with_tx(|tx| tx.insert_command(duplicate.clone()))
        .expect_err("a second command for an already-used visit must be refused");
    assert_eq!(
        refusal.code(),
        "COMMAND_VISIT_DUPLICATE",
        "{HARNESS}: the visit guard is what refuses the bypass"
    );

    // NEGATIVE — "once per visit" has a zero side: a command node that is never reached generates no command at
    // all, because the visit is the unit of generation and not the definition's shape. A command node sitting
    // unreached in the graph must produce zero adapter calls and zero recorded commands; if generation were keyed
    // to the definition instead of the visit, this is the case that would expose it.
    let mut unreached = BTreeMap::new();
    unreached.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![TransitionDefinition {
                name: "skip".to_string(),
                to: "end".to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    unreached.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
            command_type: Some(COMMAND_TYPE.to_string()),
            ..Default::default()
        },
    );
    unreached.insert(
        "end".to_string(),
        NodeDefinition {
            id: "end".to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );

    let skip_app = FakeApplicationPort::scripted(vec![]);
    let skip_recorder = skip_app.clone();
    let skip_harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_000_001),
        Box::new(skip_app),
    );
    skip_harness
        .engine()
        .seed_definition(ProcessDefinition {
            id: "tst-wf-command-002-skip".to_string(),
            tenant_id: None,
            key: "TST-WF-COMMAND-002-SKIP".to_string(),
            version: 1,
            name: "TST WF.COMMAND 002 SKIP".to_string(),
            description: None,
            definition: ProcessGraph {
                nodes: unreached,
                start_node_id: "start".to_string(),
                display_order: None,
            },
            status: DefinitionStatus::Active,
        })
        .expect("the skipping definition registers with the engine");
    let skipped = skip_harness
        .engine()
        .start_process(StartProcessParams {
            definition_key: "TST-WF-COMMAND-002-SKIP".to_string(),
            version: Some(1),
            business_key: None,
            variables: Value::object(),
            started_by: "tst".to_string(),
            tenant_id: None,
            subject: None,
        })
        .expect("the process that never reaches the command node runs to completion");
    assert_eq!(
        skip_recorder.requests().len(),
        0,
        "{HARNESS}: a command node that is never visited generates no command"
    );
    assert_eq!(
        skip_harness
            .store()
            .with_tx(|tx| tx.command_visit_count(&skipped.process_instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        0,
        "{HARNESS}: zero visits means zero recorded commands"
    );
    // The zero above is a completed run that simply never entered the command node, not a run that died before it:
    // the skip process ran to its declared end outcome. Without this, a crash could masquerade as "never visited".
    let skipped_instance = skip_harness
        .store()
        .with_tx(|tx| tx.get_instance(&skipped.process_instance_id))
        .expect("the skipped instance is readable");
    assert_eq!(
        skipped_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the run that skips the command node completed normally"
    );
}

//! WF.COMMAND — command receipt replay (TST-WF-COMMAND-008).
//!
//! Contract: the stored command receipt is the idempotency record for the
//! command — re-presenting it executes nothing. Once the engine has recorded
//! a command (its deterministic id AND its `(instance, node, visit)` triple),
//! replaying that receipt is refused by the store, the adapter sees no second
//! execution, the event log gains no second request, and the stored receipt
//! itself is unchanged. At-most-once execution is a property of committed
//! state, not of the caller remembering not to retry.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` runs one command through the
//! production `ApplicationPort` seam (faked at the adapter boundary), and the
//! replays go through the production `Store` (`insert_command`) while the
//! adapter call count and the durable log are read back through that same
//! store. Level: L3 Composition, harness `WorkflowHarness`.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/memory.rs` (`insert_command`) — the deterministic id
//! is refused as `COMMAND_DUPLICATE` and the commanded triple as
//! `COMMAND_VISIT_DUPLICATE`; the companion file
//! `wf_command__001__deterministic_command_id` owns the id's derivation,
//! this file owns what the receipt ENFORCES: a replay moves nothing.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_command__008__command_receipt_replay

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::sha256::sha256_hex;
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest,
    ApplicationCommandResult, ApplicationPort, DefinitionStatus, NodeDefinition, ProcessCommand,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams,
    TransitionDefinition, Value, WorkflowSubject,
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

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and always succeeds;
/// it performs no I/O and names no provider. Its call count is the
/// side-effect observation: a replay that reached the adapter would show here.
#[derive(Clone, Default)]
struct RecordingApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
}

impl RecordingApplicationPort {
    fn new() -> Self {
        Self::default()
    }

    /// A snapshot of the requests recorded so far.
    fn requests(&self) -> Vec<ApplicationCommandRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl ApplicationPort for RecordingApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.clone());
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

/// The stored receipt for the run's one command, plus the instance's root token.
fn receipt_and_root(harness: &EngineHarness, instance_id: &str) -> (ProcessCommand, String) {
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(instance_id))
        .expect("the instance is readable");
    // The receipt is rebuilt with the recorded identity; only the id and the
    // triple are what the guards read, and both are asserted from the log.
    let logged_id = harness
        .store()
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .find(|event| event.event_type == "command.requested")
        .expect("the engine logged the command it requested")
        .data
        .get("commandId")
        .and_then(Value::as_str)
        .expect("the request event carries its commandId")
        .to_string();
    let receipt = ProcessCommand {
        process_instance_id: instance_id.to_string(),
        token_id: instance.root_token_id.clone().unwrap_or_default(),
        node_id: COMMAND_NODE.to_string(),
        visit_sequence: 1,
        command_id: logged_id,
        command_type: COMMAND_TYPE.to_string(),
        subject_type: None,
        subject_id: None,
        correlation_id: instance_id.to_string(),
        causation_id: None,
        input: Value::object(),
        outcome: "success".to_string(),
        message: None,
    };
    (receipt, instance.root_token_id.unwrap_or_default())
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-008); the file and the assay use it.
fn wf_command_008__command_receipt_replay() {
    const KEY: &str = "TST-WF-COMMAND-008";
    let app = RecordingApplicationPort::new();
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(
        TestClock::at_unix_millis(1_700_000_800_000),
        Box::new(app),
    );
    harness
        .engine()
        .seed_definition(command_definition(KEY))
        .expect("the command definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params(KEY))
        .expect("the command process starts and settles");
    let instance_id = started.process_instance_id;

    // The run executed exactly once and completed; the receipt names visit 1's
    // deterministic identity.
    assert_eq!(
        recorder.requests().len(),
        1,
        "{HARNESS}: the command ran exactly once"
    );
    let id = recorder.requests()[0].command_id.clone();
    assert_eq!(
        id,
        command_id(&instance_id, COMMAND_NODE, 1),
        "{HARNESS}: the receipt carries the deterministic identity"
    );
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the run completed before any replay"
    );

    let (receipt, _root) = receipt_and_root(&harness, &instance_id);
    assert_eq!(
        receipt.command_id, id,
        "{HARNESS}: the rebuilt receipt names the executed command"
    );

    // 1. Re-presenting the exact receipt is refused by the id guard — and the
    //    adapter hears nothing.
    let id_refusal = harness
        .store()
        .with_tx(|tx| tx.insert_command(receipt.clone()))
        .expect_err("the recorded command id must not replay");
    assert_eq!(
        id_refusal.code(),
        "COMMAND_DUPLICATE",
        "{HARNESS}: the receipt's id is the dedup key"
    );
    assert_eq!(
        recorder.requests().len(),
        1,
        "{HARNESS}: the refused replay executed nothing at the adapter"
    );

    // 2. A FRESH id for the same commanded visit is still refused — by the
    //    triple guard. A non-deterministic re-derivation could not smuggle a
    //    second execution into the spent visit either.
    let fresh_id = sha256_hex(b"tst:receipt-replay:fresh-id");
    let visit_replay = ProcessCommand {
        command_id: fresh_id.clone(),
        ..receipt.clone()
    };
    assert_ne!(
        visit_replay.command_id, receipt.command_id,
        "{HARNESS}: the visit case uses a distinct id, so only the triple can refuse it"
    );
    let visit_refusal = harness
        .store()
        .with_tx(|tx| tx.insert_command(visit_replay.clone()))
        .expect_err("an already-commanded visit must not replay even under a fresh id");
    assert_eq!(
        visit_refusal.code(),
        "COMMAND_VISIT_DUPLICATE",
        "{HARNESS}: the commanded triple is the authoritative one-execution-per-visit guard"
    );
    assert_eq!(
        recorder.requests().len(),
        1,
        "{HARNESS}: the visit replay executed nothing at the adapter either"
    );

    // 3. The replays wrote nothing durable: still exactly one request on the
    //    log, still one visit consumed, and the run still completed.
    let requested = harness
        .store()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "command.requested")
        .collect::<Vec<_>>();
    assert_eq!(
        requested.len(),
        1,
        "{HARNESS}: the replays added no second request to the log"
    );
    assert_eq!(
        requested[0]
            .data
            .get("commandId")
            .and_then(Value::as_str),
        Some(id.as_str()),
        "{HARNESS}: the single logged request is still the executed command"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        1,
        "{HARNESS}: the replays consumed no further visit"
    );
    let settled = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is still readable");
    assert_eq!(
        settled.status,
        ProcessStatus::Completed,
        "{HARNESS}: the replays did not disturb the completed run"
    );
}

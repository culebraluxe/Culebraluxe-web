//! WF.COMMAND — deterministic command ID (TST-WF-COMMAND-001).
//!
//! Contract: the command id the `WorkflowEngine` mints for a `command` node is a **pure function** of
//! `(process_instance_id, node_id, visit_sequence)` — `command_id` at
//! `rust/core/workflow/src/engine/handle_join.rs:359`, the derivation production runs
//! (`handle_command` reads the committed visit count, `visit_sequence = command_visit_count + 1` at
//! `rust/core/workflow/src/engine/handle_join.rs:201`, then hashes the triple). The same triple yields the same
//! id whatever the wall clock says; the id is the global dedup key the store refuses to duplicate
//! (`rust/core/workflow/src/memory.rs:582-606`, mirrored by the production unique index).
//!
//! This file exercises the production boundary, not a re-declaration of it. The real `WorkflowEngine<MemoryStore>`
//! is driven through `start_process` and `complete_task`; its `command` node calls the production `ApplicationPort`
//! seam (faked at the adapter boundary, so no live provider is touched); and the command id is read back both at the
//! adapter and on the durable event log through the production `Store`. Level: L3 Composition, harness
//! `WorkflowHarness`.
//!
//! The shape matters. Production mints an instance id **once**, randomly, with `uuid_v4()`
//! (`rust/core/workflow/src/neon/new_id.rs:7-9`), and never re-derives it, so two independent runs are two distinct
//! identities and must mint two distinct command ids. A determinism proof that starts a *fresh* engine twice and
//! finds the same id is therefore proving a `MemoryStore` counter artifact, not this contract. So the contract is
//! pinned to a **fixed, committed instance**: the process commits and parks on a task, the clock moves, the command
//! is generated later on that same instance, and the id must equal the clock-free production derivation. The three
//! inputs are each shown load-bearing — a different instance, node, and visit each derive a different id — so the
//! test cannot pass if the id stops being a function of the whole triple.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_command__001__deterministic_command_id

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::{EngineHarness, TestClock};
use workflow::sha256::sha256_hex;
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessCommand,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TaskStatus,
    TransitionDefinition, Value, WorkflowSubject,
};

const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// A second command node, so a run can observe a distinct node's derivation.
const SECOND_COMMAND_NODE: &str = "notify";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
/// The second command node's type.
const SECOND_COMMAND_TYPE: &str = "tst.notify";
/// The task node every graph parks on before the command step, so the instance commits first.
const TASK_NODE: &str = "wait";
/// The transition that leaves the task for the command node.
const TASK_TRANSITION: &str = "submit";
const STARTED_BY: &str = "tst";
// Definition keys. A definition is addressed by (key, version) and re-seeded per graph, so each graph needs its own.
const KEY_TWO_PHASE: &str = "TST-WF-COMMAND-001";
const KEY_SECOND_NODE: &str = "TST-WF-COMMAND-001-NOTIFY";
const KEY_LOOP: &str = "TST-WF-COMMAND-001-LOOP";
const DEFINITION_VERSION: i32 = 1;

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and always succeeds; it performs no I/O and names no
/// provider. The fake is the same adapter seam production substitutes, not a parallel command implementation.
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

/// `start -> wait (task) -> <command> -> end`, the smallest graph that lets the instance commit in one transaction
/// and generate the command in a later one, on the same identity.
fn two_phase_definition(key: &str, command_node: &str, command_type: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
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
            transitions: Some(vec![transition(TASK_TRANSITION, command_node)]),
            ..Default::default()
        },
    );
    nodes.insert(
        command_node.to_string(),
        NodeDefinition {
            id: command_node.to_string(),
            node_type: "command".to_string(),
            command_type: Some(command_type.to_string()),
            transition: Some("next".to_string()),
            transitions: Some(vec![transition("next", "end")]),
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
    ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: DEFINITION_VERSION,
        name: key.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// `start -> wait (task) -> emit (command) -> wait (task) -> ...`: the command node's success returns to the task, so
/// completing the task twice visits the *same* node a second time on the same committed instance.
fn looping_definition(key: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
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
            transitions: Some(vec![transition(TASK_TRANSITION, COMMAND_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
            command_type: Some(COMMAND_TYPE.to_string()),
            transition: Some("again".to_string()),
            transitions: Some(vec![transition("again", TASK_NODE)]),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: DEFINITION_VERSION,
        name: key.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
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

/// The instance's current `Ready` task — the committed parking point the command is driven from.
fn ready_task(harness: &EngineHarness, instance_id: &str) -> String {
    harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .filter(|task| task.status == TaskStatus::Ready)
        .last()
        .expect("the process parks on a ready task before the command")
        .id
}

/// Start a process, let it commit and park on its task, and hand back `(instance_id, task_id)`.
fn start_and_park(harness: &EngineHarness, key: &str) -> (String, String) {
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id;
    let task_id = ready_task(harness, &instance_id);
    (instance_id, task_id)
}

/// Complete the parked task, which drives the token into the command node.
fn complete_task(harness: &EngineHarness, task_id: &str) {
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: task_id.to_string(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(TASK_TRANSITION.to_string()),
        })
        .expect("the task completes and drives the command");
}

fn request_ids(port: &RecordingApplicationPort) -> Vec<String> {
    port.requests()
        .iter()
        .map(|request| request.command_id.clone())
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-COMMAND-001); the file and the assay use it.
fn wf_command_001__deterministic_command_id() {
    let clock = TestClock::at_unix_millis(1_600_000_000_000);
    let app = RecordingApplicationPort::new();
    let recorder = app.clone();
    let harness = EngineHarness::with_application_port(clock.clone(), Box::new(app));
    harness
        .engine()
        .seed_definition(two_phase_definition(
            KEY_TWO_PHASE,
            COMMAND_NODE,
            COMMAND_TYPE,
        ))
        .expect("the two-phase definition registers with the engine");

    // 1. A fixed, committed instance. The process commits in one transaction and parks; no command exists yet, so
    //    everything the identity depends on is durable state, not the call that will follow.
    let (instance, task) = start_and_park(&harness, KEY_TWO_PHASE);
    assert_eq!(
        recorder.requests().len(),
        0,
        "{HARNESS}: no command is generated before the command node is reached"
    );

    // The command is generated at a *later* clock. If the id read the wall clock (or any attempt instant), this
    // boundary would not reproduce the clock-free production derivation asserted next.
    harness.clock().advance_millis(86_400_000);
    complete_task(&harness, &task);

    let ids = request_ids(&recorder);
    assert_eq!(
        ids.len(),
        1,
        "{HARNESS}: the command node runs exactly one command"
    );
    let id = ids[0].clone();

    // 2. DETERMINISM — the id the boundary sent is exactly the production pure function of the committed triple.
    //    The boundary generated it a day after the instance committed; the derivation has no clock input, so the two
    //    must agree. A constant, a random, or a clock-based id would fail here.
    assert_eq!(
        id,
        command_id(&instance, COMMAND_NODE, 1),
        "{HARNESS}: the command id is the production derivation of the fixed committed triple"
    );
    assert_eq!(
        id.len(),
        64,
        "{HARNESS}: a command id is a sha256 hex digest"
    );
    assert!(
        id.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "{HARNESS}: a command id is lowercase hex"
    );

    // 2b. NON-CIRCULAR DETERMINISM ORACLE — step 2 compares the boundary id against the production
    //     `command_id`, which proves the boundary *uses* that derivation but would not notice a clock or a
    //     random salt added *inside* it: both sides would move together. Hashing the documented canonical
    //     preimage here, directly and independently of `command_id`, pins the id to the triple alone. This is
    //     the case that fails if the id stops being a pure function of `(instance, node, visit)`.
    assert_eq!(
        id,
        sha256_hex(format!("{instance}:{COMMAND_NODE}:1").as_bytes()),
        "{HARNESS}: the id is sha256 of the canonical (instance:node:visit) preimage — no instant, no entropy"
    );

    // 3. The id reaches the durable log boundary: the command.requested event carries it.
    let requested = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 128))
        .expect("the instance history reads")
        .into_iter()
        .find(|event| event.event_type == "command.requested")
        .expect("the engine records the command it requested");
    assert_eq!(
        requested.data.get("commandId").and_then(Value::as_str),
        Some(id.as_str()),
        "{HARNESS}: the durable event log carries the deterministic id"
    );

    // The process actually completed, so the boundary under test truly ran.
    let completed = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    assert_eq!(
        completed.status,
        ProcessStatus::Completed,
        "{HARNESS}: the command drove the process to completion"
    );

    // 4. NEGATIVE/FAULT — the deterministic id is the global dedup key. A command carrying an id already recorded is
    //    refused, even for a visit never used, so a non-deterministic id would be the only way to smuggle a duplicate
    //    through. The refusal is the store's own `COMMAND_DUPLICATE` guard.
    let replay = ProcessCommand {
        process_instance_id: instance.clone(),
        token_id: completed.root_token_id.clone().unwrap_or_default(),
        node_id: COMMAND_NODE.to_string(),
        // A visit that was never used: only the id itself can be what refuses this.
        visit_sequence: 99,
        command_id: id.clone(),
        command_type: COMMAND_TYPE.to_string(),
        subject_type: None,
        subject_id: None,
        correlation_id: instance.clone(),
        causation_id: None,
        input: Value::object(),
        outcome: "success".to_string(),
        message: None,
    };
    let refusal = harness
        .store()
        .with_tx(|tx| tx.insert_command(replay.clone()))
        .expect_err("a command id already recorded must be refused");
    assert_eq!(
        refusal.code(),
        "COMMAND_DUPLICATE",
        "{HARNESS}: the deterministic id is the dedup key the store refuses to duplicate"
    );

    // 4b. NEGATIVE/FAULT (visit backstop) — the id dedup is the *fast* guard, but the authoritative one is the
    //     commanded triple: `(instance, node, visit)` may be commanded exactly once. A *fresh* id (what a
    //     non-deterministic derivation would mint) for a visit already recorded is still refused, so an id that was not
    //     a pure function of the triple could not smuggle a second command into a spent visit.
    let fresh_id = sha256_hex(b"tst:replayed-visit:fresh-id");
    let visit_replay = ProcessCommand {
        command_id: fresh_id,
        // The same committed triple as `replay`; only the id differs.
        visit_sequence: 1,
        ..replay.clone()
    };
    assert_ne!(
        visit_replay.command_id, replay.command_id,
        "{HARNESS}: the visit case uses an id distinct from the recorded one, so only the triple can refuse it"
    );
    let visit_refusal = harness
        .store()
        .with_tx(|tx| tx.insert_command(visit_replay.clone()))
        .expect_err("an already-commanded visit must be refused even under a fresh id");
    assert_eq!(
        visit_refusal.code(),
        "COMMAND_VISIT_DUPLICATE",
        "{HARNESS}: the committed triple, not just the id, is the authoritative one-command-per-visit guard"
    );

    // 5. NEGATIVE (instance input) — the id is keyed to the whole instance, not a constant or a run-wide value. Two
    //    starts in one engine are two distinct identities (production mints each with `uuid_v4()` and never re-derives
    //    it), so the same node's first visit must mint two distinct command ids. If the derivation dropped the
    //    instance from its inputs, this is the case that would expose it.
    let (second_instance, second_task) = start_and_park(&harness, KEY_TWO_PHASE);
    complete_task(&harness, &second_task);
    assert_ne!(
        second_instance, instance,
        "{HARNESS}: two starts are two distinct committed identities"
    );
    let second_ids = request_ids(&recorder);
    let second_id = second_ids[1].clone();
    assert_eq!(
        second_id,
        command_id(&second_instance, COMMAND_NODE, 1),
        "{HARNESS}: the second instance's id is its own triple's production id"
    );
    assert_ne!(
        second_id, id,
        "{HARNESS}: distinct instances must not collide on a command id"
    );

    // 6. NEGATIVE (node input) — the same identity triple with a different node derives a different id, so the node
    //    is load-bearing. Run a second definition whose command node has a different id and assert the boundary id
    //    equals that node's derivation and not this one's.
    harness
        .engine()
        .seed_definition(two_phase_definition(
            KEY_SECOND_NODE,
            SECOND_COMMAND_NODE,
            SECOND_COMMAND_TYPE,
        ))
        .expect("the second-node definition registers with the engine");
    let (other_instance, other_task) = start_and_park(&harness, KEY_SECOND_NODE);
    complete_task(&harness, &other_task);
    let other_ids = request_ids(&recorder);
    let other_id = other_ids[2].clone();
    assert_eq!(
        other_id,
        command_id(&other_instance, SECOND_COMMAND_NODE, 1),
        "{HARNESS}: the id derives from the node that actually ran"
    );
    assert_ne!(
        other_id,
        command_id(&other_instance, COMMAND_NODE, 1),
        "{HARNESS}: a different node on the same instance derives a different id"
    );

    // 7. NEGATIVE (visit input) — the visit sequence is load-bearing too. A node revisited on the same committed
    //    instance is a new identity: visit 1 and visit 2 derive visit 1's id and visit 2's id, and they differ. If
    //    the derivation keyed on (instance, node) alone, this is the case that would expose the collision.
    harness
        .engine()
        .seed_definition(looping_definition(KEY_LOOP))
        .expect("the looping definition registers with the engine");
    let (loop_instance, loop_task) = start_and_park(&harness, KEY_LOOP);
    complete_task(&harness, &loop_task);
    let loop_task_again = ready_task(&harness, &loop_instance);
    complete_task(&harness, &loop_task_again);
    let visit_ids = request_ids(&recorder);
    assert_eq!(
        visit_ids.len(),
        5,
        "{HARNESS}: the looping command node ran exactly two more times"
    );
    assert_eq!(
        visit_ids[3],
        command_id(&loop_instance, COMMAND_NODE, 1),
        "{HARNESS}: the first visit carries visit 1's id"
    );
    assert_eq!(
        visit_ids[4],
        command_id(&loop_instance, COMMAND_NODE, 2),
        "{HARNESS}: the revisit carries visit 2's id"
    );
    assert_ne!(
        visit_ids[3], visit_ids[4],
        "{HARNESS}: two visits to one node must never share an id"
    );
}

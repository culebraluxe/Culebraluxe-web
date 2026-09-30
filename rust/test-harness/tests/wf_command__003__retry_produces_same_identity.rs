//! WF.COMMAND — retry produces same identity (TST-WF-COMMAND-003).
//!
//! Contract: when a command step dies on a broken connection and the engine repeats it, the retry produces the
//! **same** command identity. The `WorkflowEngine` derives a command's id from `(process_instance_id, node_id,
//! visit_sequence)` — `command_id` at `rust/core/workflow/src/engine/handle_join.rs:359-363` — and every input is
//! persisted state: the instance id is committed before the command runs, and the visit sequence is
//! `command_visit_count + 1` read back from the store (`rust/core/workflow/src/engine/handle_join.rs:201-202`). A
//! step that dies on the connection commits nothing (the production transaction contract,
//! `rust/core/workflow/src/memory.rs:38-54`), so the retry re-reads the same count, regenerates the same id, and the
//! store records the command exactly once.
//!
//! The retry is **the production retry rule**, not a loop written here. Production repeats a failed step in
//! `TxStore::with_tx` through `repeat_connection_failures` (`rust/core/workflow/src/store.rs:144`), and the only
//! production store wired to it is `NeonStore` (`rust/core/workflow/src/neon/neon_store.rs:90-104`). That store needs
//! a real database, so the contract exercises the same rule at the `TxStore` seam: `FlakyConnectionStore` delegates
//! every byte of storage to the production `MemoryStore` and wraps the step in the production
//! `repeat_connection_failures`. The engine makes ONE `complete_task` call; the retry is invisible to it, exactly as
//! it is in production. If the production rule stopped repeating, the engine would see the connection failure and the
//! test would fail. No retry loop lives in this file.
//!
//! The retry is exercised on an instance that was **already committed** by an earlier, successful transaction: the
//! process starts and parks on a task node, the first attempt to drive the command out of that task dies mid-step,
//! and the retry re-drives the *same* instance. That shape matters — production mints an instance id once, with
//! `uuid_v4()` at `rust/core/workflow/src/ids.rs:5`, and never re-derives it — so a contract that only held
//! when a fresh `start_process` re-minted its instance would not be a production contract.
//!
//! The real `WorkflowEngine` is driven through `start_process` and `complete_task`; its `command` node calls the
//! production `ApplicationPort` seam — faked at the adapter boundary, so no live provider is touched — and the only
//! substitution is the deterministic broken-connection fault injected at the `TxStore` seam
//! (`rust/test-harness/src/fault.rs`). Level: L3 Composition.
//!
//! The fault is load-bearing: without a failed first attempt there is no retry, so the identity comparison would be
//! vacuous. The negative cases prove the test can fail — a genuine second run gets a distinct identity, and a
//! committed identity cannot be recorded twice.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use test_harness::fault::{Fault, FaultInjector};
use test_harness::TestClock;
use workflow::store::repeat_connection_failures;
use workflow::{
    command_id, ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult,
    ApplicationPort, CompleteTaskParams, DefinitionStatus, EngineOptions, MemoryStore,
    NodeDefinition, ProcessCommand, ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus,
    Result, StartProcessParams, Store, TaskStatus, TransitionDefinition, TxStore, Value,
    WorkflowEngine, WorkflowError, WorkflowSubject,
};

/// The command node the contract is about.
const COMMAND_NODE: &str = "emit";
/// The command type the node runs.
const COMMAND_TYPE: &str = "tst.emit";
/// The task node the run parks on before the command step, so the instance commits first.
const TASK_NODE: &str = "wait";
/// The transition that leaves the task for the command node.
const TASK_TRANSITION: &str = "submit";
/// The definition key and version registered with the engine.
const DEFINITION_KEY: &str = "TST-WF-COMMAND-003";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The production retry bound. `NeonStore::with_tx` takes this from the db policy
/// (`db::retry::policy()`, default 3); the rule itself does not care where the number comes from.
const RETRY_ATTEMPTS: u32 = 3;

/// The external-facing `ApplicationPort`, faked at the adapter seam.
///
/// It records every command the engine asks it to execute and always succeeds. It performs no I/O and names no
/// provider. Its `requests()` is the observation point for the identity: the id the engine hands the application is
/// the identity production would send, so comparing it across the two attempts is comparing the retry's identity.
#[derive(Clone, Default)]
struct FakeApplicationPort {
    requests: Arc<Mutex<Vec<ApplicationCommandRequest>>>,
}

impl FakeApplicationPort {
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

/// The production `MemoryStore` with a scripted broken connection, retried by the production rule.
///
/// This is a fault interposer at the `TxStore` seam, not a second store and not a second retry: every byte of
/// storage is `MemoryStore`'s, and the decision to repeat a broken step is `repeat_connection_failures` — the same
/// function `NeonStore::with_tx` calls in production. On the scripted connection fault the step body is still run
/// against the real store, then the transaction is forced to fail, exactly as a socket that dies mid-step fails the
/// body; `MemoryStore` rolls back and the repeated step sees the first attempt's state.
#[derive(Clone)]
struct FlakyConnectionStore {
    memory: MemoryStore,
    faults: FaultInjector,
}

impl FlakyConnectionStore {
    fn new(faults: FaultInjector) -> Self {
        Self {
            memory: MemoryStore::new(),
            faults,
        }
    }

    /// The untouched production store, for observation only.
    ///
    /// Reads go through the real `MemoryStore` so they neither consume a scripted fault nor perturb the state the
    /// engine's step is being retried against; the fault script belongs to the engine's own transactions.
    fn memory(&self) -> &MemoryStore {
        &self.memory
    }
}

impl TxStore for FlakyConnectionStore {
    fn with_tx<R, F>(&self, mut f: F) -> Result<R>
    where
        F: FnMut(&mut dyn Store) -> Result<R>,
    {
        let memory = &self.memory;
        let faults = &self.faults;
        repeat_connection_failures(
            || match faults.next_fault() {
                Fault::None => memory.with_tx(&mut f),
                Fault::Error { message, .. } => {
                    // Run the real step body first, so the attempt produces the identity it would have sent, then
                    // fail the connection. `MemoryStore` rolls the whole step back, so the repeated attempt re-reads
                    // the state the first attempt saw — the production transaction contract.
                    let _: Result<()> = memory.with_tx(|tx| {
                        let _ = f(tx);
                        Err(WorkflowError::unavailable(message.clone()))
                    });
                    Err(WorkflowError::unavailable(message))
                }
                other => {
                    panic!("the retry contract is scripted with None and Error only, got {other:?}")
                }
            },
            RETRY_ATTEMPTS,
            |_attempt| {},
        )
    }
}

/// A definition whose start node parks on a human task and then, once the task is completed, reaches a command node
/// that succeeds into an end node.
///
/// `start -> wait (task) -> emit (command) -> done (end)`. The task is what lets the instance commit in one
/// transaction and gives the command step a later, separate transaction to die in — the retry then re-drives the
/// same committed instance rather than starting a new one.
fn two_phase_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![TransitionDefinition {
                name: "begin".to_string(),
                to: TASK_NODE.to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        TASK_NODE.to_string(),
        NodeDefinition {
            id: TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Wait".to_string()),
            transitions: Some(vec![TransitionDefinition {
                name: TASK_TRANSITION.to_string(),
                to: COMMAND_NODE.to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        COMMAND_NODE.to_string(),
        NodeDefinition {
            id: COMMAND_NODE.to_string(),
            node_type: "command".to_string(),
            name: Some("Emit".to_string()),
            command_type: Some(COMMAND_TYPE.to_string()),
            transition: Some("done".to_string()),
            transitions: Some(vec![TransitionDefinition {
                name: "done".to_string(),
                to: "done".to_string(),
                condition: None,
                required: None,
            }]),
            ..Default::default()
        },
    );
    nodes.insert(
        "done".to_string(),
        NodeDefinition {
            id: "done".to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: "tst-wf-command-003".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.COMMAND 003".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
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

/// Park a fresh process on its task node and hand back the task id — the committed instance the retry will run on.
fn start_and_park(engine: &WorkflowEngine<FlakyConnectionStore>, instance_id: &str) -> String {
    engine
        .store()
        .memory()
        .with_tx(|tx| tx.tasks_for_instance(instance_id))
        .expect("the instance's tasks are readable")
        .into_iter()
        .next()
        .expect("the process parks on its task node before the command")
        .id
}

fn complete_the_task(engine: &WorkflowEngine<FlakyConnectionStore>, task_id: &str) -> Result<()> {
    engine.complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(TASK_TRANSITION.to_string()),
    })
}

#[test]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
fn wf_command_003__retry_produces_same_identity() {
    const HARNESS: &str = "WorkflowHarness/L3 Composition";

    let engine_clock = TestClock::at_unix_millis(1_700_000_000_000);

    // One scripted fault: the definition registration and the process start commit clean (the instance is durable
    // from here on), the first attempt to drive the command dies on a broken connection, and the production retry
    // rule repeats the step — which then runs clean.
    let faults = FaultInjector::scripted(vec![
        Fault::None,
        Fault::None,
        Fault::error(
            "DB_UNAVAILABLE",
            "error communicating with database: Broken pipe (os error 32)",
        ),
    ]);
    let fault_probe = faults.clone();
    let port = FakeApplicationPort::default();
    let recorder = port.clone();
    // A clock that moves on every read, so the failed attempt and the repeat necessarily run at different instants.
    // The identity must not change because wall time moved, so this is the clock-independence half of the contract.
    let engine = WorkflowEngine::new(
        FlakyConnectionStore::new(faults),
        EngineOptions {
            app: Some(Box::new(port)),
            now: Box::new(move || engine_clock.tick().timestamp_millis()),
        },
    );

    engine
        .seed_definition(two_phase_definition())
        .expect("the command definition registers with the engine");

    // PHASE 1 — the process starts and commits. Production mints this instance id once, randomly, and never again,
    // so everything the command identity depends on must be the durable state written here.
    let started = engine
        .start_process(start_params())
        .expect("the process starts and parks on its task");
    let instance_id = started.process_instance_id.clone();
    assert_eq!(
        recorder.requests().len(),
        0,
        "{HARNESS}: no command is generated before the command node is reached"
    );
    let task_id = start_and_park(&engine, &instance_id);
    assert_eq!(
        engine
            .store()
            .memory()
            .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        0,
        "{HARNESS}: the command node has not been visited yet"
    );

    // ATTEMPT + RETRY — ONE engine call. Completing the task drives the token into the command node; the first
    // attempt dies on the broken connection before it can commit, and `repeat_connection_failures` inside the
    // store's `with_tx` repeats the step. The engine never sees the failure, which is the production shape.
    complete_the_task(&engine, &task_id)
        .expect("the production retry repeats the broken step and the repeat commits");

    // The repeat actually happened: registration, start, the failed attempt, and the successful repeat.
    assert_eq!(
        fault_probe.fired(),
        4,
        "{HARNESS}: the connection failure was retried by the production rule, not swallowed"
    );

    let requests = recorder.requests();
    assert_eq!(
        requests.len(),
        2,
        "{HARNESS}: the failed attempt generated its identity and the repeat generated its own"
    );

    let attempt = &requests[0];
    let retry = &requests[1];
    assert_eq!(
        attempt.correlation_id, retry.correlation_id,
        "{HARNESS}: the retry is the same committed process instance, not a new one"
    );
    assert_eq!(
        attempt.correlation_id, instance_id,
        "{HARNESS}: both attempts carry the committed instance the process started with"
    );
    assert_eq!(
        attempt.command_id, retry.command_id,
        "{HARNESS}: retry produces the same identity"
    );
    assert_eq!(
        retry.command_id,
        command_id(&instance_id, COMMAND_NODE, 1),
        "{HARNESS}: the identity is that visit's own id, derived from the persisted instance"
    );

    // The commit is exactly once: the rolled-back attempt left no command, so the visit count is one. If the failed
    // attempt had committed, the repeat would have read count 1, minted visit 2, and produced a DIFFERENT id — which
    // is why this assertion makes the rollback load-bearing for the contract.
    assert_eq!(
        engine
            .store()
            .memory()
            .with_tx(|tx| tx.command_visit_count(&instance_id, COMMAND_NODE))
            .expect("the store answers the visit count"),
        1,
        "{HARNESS}: the failed attempt left no command; the retry recorded exactly one"
    );

    // The durable log agrees with the adapter. The failed attempt's transaction rolled back, so the log holds
    // exactly one `command.requested` event and it carries the retry's identity. A retry that re-recorded the
    // command would leave two events here; one whose identity drifted would leave a different id. Both are
    // violations of "retry produces same identity", and both are invisible to the adapter-only assertions above.
    let requested: Vec<_> = engine
        .store()
        .memory()
        .with_tx(|tx| tx.history(&instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == "command.requested")
        .collect();
    assert_eq!(
        requested.len(),
        1,
        "{HARNESS}: only the committed retry recorded a command; the failed attempt left no event"
    );
    assert_eq!(
        requested[0].data.get("commandId").and_then(Value::as_str),
        Some(retry.command_id.as_str()),
        "{HARNESS}: the durable log carries the same identity the adapter received"
    );

    let task = engine
        .store()
        .memory()
        .with_tx(|tx| tx.get_task(&task_id))
        .expect("the task is readable");
    assert_eq!(
        task.status,
        TaskStatus::Completed,
        "{HARNESS}: the retried step completed the task exactly once"
    );

    let process = engine
        .store()
        .memory()
        .with_tx(|tx| tx.get_instance(&instance_id))
        .expect("the instance is readable");
    assert_eq!(
        process.status,
        ProcessStatus::Completed,
        "{HARNESS}: the retried command drove the process to its end"
    );

    let committed_id = retry.command_id.clone();

    // NEGATIVE (bypass): a committed identity cannot be recorded a second time. A "retry" that did not roll back
    // would re-insert the same id and be refused here, so same-identity is only legal across a failed attempt.
    let duplicate = ProcessCommand {
        process_instance_id: instance_id.clone(),
        token_id: String::new(),
        node_id: COMMAND_NODE.to_string(),
        visit_sequence: 1,
        command_id: committed_id.clone(),
        command_type: COMMAND_TYPE.to_string(),
        subject_type: None,
        subject_id: None,
        correlation_id: instance_id.clone(),
        causation_id: None,
        input: Value::object(),
        outcome: "success".to_string(),
        message: None,
    };
    let refusal = engine
        .store()
        .memory()
        .with_tx(|tx| tx.insert_command(duplicate.clone()))
        .expect_err("a committed command id must not be recorded twice");
    assert_eq!(
        refusal.code(),
        "COMMAND_DUPLICATE",
        "{HARNESS}: the store guard is what stops a committed identity being replayed"
    );

    // NEGATIVE (distinct run): a genuinely new process is a NEW identity. The identity is derived from the instance,
    // not a constant, so the test would fail if a retry and a fresh run were collapsed.
    let second = engine
        .start_process(start_params())
        .expect("a second, distinct process commits");
    let second_task_id = start_and_park(&engine, &second.process_instance_id);
    complete_the_task(&engine, &second_task_id).expect("the second process drives its command");
    let second_identity = recorder.requests()[2].command_id.clone();
    assert_ne!(
        second.process_instance_id, instance_id,
        "{HARNESS}: the second run is a different committed instance"
    );
    assert_ne!(
        second_identity, committed_id,
        "{HARNESS}: a distinct run gets a distinct identity"
    );
    assert_eq!(
        second.process_instance_id,
        recorder.requests()[2].correlation_id,
        "{HARNESS}: the distinct identity belongs to the distinct instance"
    );
}

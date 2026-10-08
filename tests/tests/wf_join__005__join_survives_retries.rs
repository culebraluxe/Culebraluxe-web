//! WF.JOIN — join survives retries (TST-WF-JOIN-005).
//!
//! Contract: when the join step's transaction is interrupted by a transient infrastructure failure, the
//! **production** retry rule (`repeat_connection_failures`, `middle/workflow/src/store.rs:146-165` — the same
//! function `NeonStore::with_tx` runs) repeats the step, and the run converges on **one** legal durable state:
//! exactly one `token.joined` event, exactly one downstream token, the process Completed. The broken attempt is
//! atomic — its writes roll back — so the retried attempt sees exactly the state the first one saw, and nothing
//! from the failed attempt (a half-written join, a duplicate continuation token, a stray event) survives.
//!
//! This is the adversarial half: a scripted `DB_UNAVAILABLE` mid-join, faulted through a `TxStore` interposer
//! that delegates every byte of storage to the production `MemoryStore` and composes the production retry rule.
//! No second store, no second retry: the work discussed is `WorkflowEngine<FlakyConnectionStore>`'s —
//! `handle_join`, the token inventory, and the durable event log.
//!
//! ```text
//! start -> fan (fork, two required branches)
//!            |-- approve (task) -> converge (join) -> settle (end)
//!            '-- review  (task) -> converge
//! ```
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Deterministic and isolated: fixed `TestClock`, in-memory
//! store behind the scripted fault, no database, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__005__join_survives_retries

use std::collections::BTreeMap;

use test_harness::fault::{Fault, FaultInjector};
use test_harness::TestClock;
use workflow::store::repeat_connection_failures;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, MemoryStore, NodeDefinition,
    ProcessDefinition, ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus, Result,
    StartProcessParams, Store, Task, TaskStatus, TransitionDefinition, TxStore, Value,
    WorkflowEngine, WorkflowError,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-JOIN-005";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
const MAIN_NODE: &str = "approve";
const REVIEW_NODE: &str = "review";
const END_NODE: &str = "settle";
const BRANCH_TRANSITION: &str = "go";
const RETRY_ATTEMPTS: u32 = 3;

/// The production `MemoryStore` behind a scripted broken connection, retried by the production rule.
///
/// Fault interposer at the `TxStore` seam, not a second store and not a second retry: every byte of storage is
/// `MemoryStore`'s, and the decision to repeat is `repeat_connection_failures`. On the scripted fault the step
/// body runs against the real store and the transaction is then forced to fail, so the repeat observes exactly the
/// state the first attempt saw (`MemoryStore` restores its snapshot, `middle/workflow/src/memory.rs:38-53`).
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

    /// The untouched production store, for observation only: reads bypass the fault script.
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
                    let _: Result<()> = memory.with_tx(|tx| {
                        let _ = f(tx);
                        Err(WorkflowError::unavailable(message.clone()))
                    });
                    Err(WorkflowError::unavailable(message))
                }
                other => {
                    panic!("the join-retry contract is scripted with None and Error only, got {other:?}")
                }
            },
            RETRY_ATTEMPTS,
            |_attempt| {},
        )
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

/// `start -> fan (2 required branches) -> two task branches -> converge (join) -> settle (end)`.
fn definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            name: Some("Fan".to_string()),
            transitions: Some(vec![
                transition("main", MAIN_NODE),
                transition("review", REVIEW_NODE),
            ]),
            ..Default::default()
        },
    );
    for (id, name) in [(MAIN_NODE, "Approve"), (REVIEW_NODE, "Review")] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.to_string(),
                node_type: "task".to_string(),
                name: Some(name.to_string()),
                transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE)]),
                ..Default::default()
            },
        );
    }
    nodes.insert(
        JOIN_NODE.to_string(),
        NodeDefinition {
            id: JOIN_NODE.to_string(),
            node_type: "join".to_string(),
            name: Some("Converge".to_string()),
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
        id: "tst-wf-join-005".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 005".to_string(),
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

fn tasks(store: &MemoryStore, instance: &str) -> Vec<Task> {
    store
        .with_tx(|tx| tx.tasks_for_instance(instance))
        .expect("the instance tasks are readable")
}

fn task_at(store: &MemoryStore, instance: &str, node: &str) -> Task {
    tasks(store, instance)
        .into_iter()
        .find(|task| task.node_id.as_deref() == Some(node))
        .unwrap_or_else(|| panic!("a task exists at {node}"))
}

fn tokens(store: &MemoryStore, instance: &str) -> Vec<workflow::Token> {
    store
        .with_tx(|tx| tx.tokens_for_instance(instance))
        .expect("the instance tokens are readable")
}

fn history(store: &MemoryStore, instance: &str) -> Vec<ProcessEvent> {
    store
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(
    store: &MemoryStore,
    instance: &str,
    event_type: &str,
) -> Vec<ProcessEvent> {
    history(store, instance)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn complete(
    engine: &WorkflowEngine<FlakyConnectionStore>,
    task_id: &str,
) -> Result<()> {
    engine.complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(BRANCH_TRANSITION.to_string()),
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-005); the file and the assay use it.
fn wf_join_005__join_survives_retries() {
    let clock = TestClock::at_unix_millis(1_700_000_000_000);

    // One scripted fault on the join-firing transaction (the fourth transaction: seed, start,
    // complete `approve`, then the join-firing completion of `review`). The first attempt rolls back; the
    // production retry rule repeats it and the repeat commits.
    let faults = FaultInjector::scripted(vec![
        Fault::None,
        Fault::None,
        Fault::None,
        Fault::error(
            "DB_UNAVAILABLE",
            "error communicating with database: Broken pipe (os error 32)",
        ),
    ]);
    let fault_probe = faults.clone();
    let store = FlakyConnectionStore::new(faults);
    let reader = store.clone();
    let clock_for_engine = clock.clone();
    let engine = WorkflowEngine::new(
        store,
        EngineOptions {
            app: None,
            now: Box::new(move || clock_for_engine.now_millis()),
        },
    );

    engine
        .seed_definition(definition())
        .expect("the definition registers with the engine");
    let started = engine
        .start_process(start_params())
        .expect("the fork process starts and parks its branches");
    let instance = started.process_instance_id.clone();

    let main_task = task_at(reader.memory(), &instance, MAIN_NODE);
    complete(&engine, &main_task.id).expect("the first branch completes");

    // The join-firing completion dies once and is retried by the production rule.
    let review_task = task_at(reader.memory(), &instance, REVIEW_NODE);
    complete(&engine, &review_task.id)
        .expect("the production retry repeats the broken join step and the repeat commits");
    assert_eq!(
        fault_probe.fired(),
        5,
        "{HARNESS}: the broken join step was retried by the production rule, not swallowed"
    );

    // ONE legal durable state: the join fired exactly once, from the last branch arrival.
    let joined = events_of_type(reader.memory(), &instance, "token.joined");
    assert_eq!(
        joined.len(),
        1,
        "{HARNESS}: the join is emitted exactly once, even after a faulted attempt"
    );
    assert_eq!(
        joined[0]
            .data
            .get("branches")
            .and_then(Value::as_array)
            .map(|b| b.len()),
        Some(2),
        "{HARNESS}: the retried join accounts for both branches"
    );
    assert_eq!(
        joined[0].node_id.as_deref(),
        Some(JOIN_NODE),
        "{HARNESS}: the join is attributed to the join node"
    );
    let downstream: Vec<_> = tokens(reader.memory(), &instance)
        .into_iter()
        .filter(|t| t.node_id == END_NODE)
        .collect();
    assert_eq!(
        downstream.len(),
        1,
        "{HARNESS}: exactly one downstream token exists, even after a faulted attempt"
    );
    assert_eq!(
        joined[0].data.get("resultTokenId").and_then(Value::as_str),
        Some(downstream[0].id.as_str()),
        "{HARNESS}: the join names the single result token"
    );
    // The failed attempt left no trace: no half-finished join, no duplicate events of any kind.
    assert_eq!(
        events_of_type(reader.memory(), &instance, "token.skipped").len(),
        0,
        "{HARNESS}: the broken attempt did not retire any branch"
    );
    assert_eq!(
        events_of_type(reader.memory(), &instance, "token.forked").len(),
        2,
        "{HARNESS}: the fork was not replayed by the retry"
    );
    assert_eq!(
        events_of_type(reader.memory(), &instance, "task.completed").len(),
        2,
        "{HARNESS}: each branch task completed exactly once; the failed attempt committed none"
    );
    assert_eq!(
        task_at(reader.memory(), &instance, MAIN_NODE).status,
        TaskStatus::Completed,
        "{HARNESS}: the first branch's task is spent"
    );
    assert_eq!(
        task_at(reader.memory(), &instance, REVIEW_NODE).status,
        TaskStatus::Completed,
        "{HARNESS}: the second branch's task is spent exactly once"
    );
    let finished = reader
        .memory()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    assert_eq!(
        finished.status,
        ProcessStatus::Completed,
        "{HARNESS}: the retried join drove the process to completion"
    );
    assert_eq!(
        finished.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: completion carries the declared end outcome"
    );

    // NEGATIVE: the join is not re-enterable through a spent branch, even though the first attempt failed.
    let replay = complete(&engine, &review_task.id)
        .expect_err("the spent review branch cannot re-enter the join");
    assert_eq!(
        replay.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: the consumed branch refuses replay"
    );
    assert_eq!(
        events_of_type(reader.memory(), &instance, "token.joined").len(),
        1,
        "{HARNESS}: the refused replay produced no second join"
    );
}

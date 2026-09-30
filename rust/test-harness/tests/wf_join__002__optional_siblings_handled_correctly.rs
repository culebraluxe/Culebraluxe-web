//! WF.JOIN — optional siblings handled correctly (TST-WF-JOIN-002).
//!
//! Contract: at a `join`, an **optional** sibling branch is not work that blocks the join. The join completes once
//! every **required** sibling has arrived, and it then retires the still-active optional siblings — marks their
//! tokens `Completed` with outcome `Skipped`, obsoletes their open tasks and cancels their open jobs — before it
//! emits `token.joined` and creates the single result token. A required sibling still active must keep the join
//! from firing; an optional sibling must never do so. The branch that is retired must be retired durably: it cannot
//! be revived as work afterwards.
//!
//! This is the production boundary, not a re-declaration of it. The real `WorkflowEngine<FlakyConnectionStore>` is
//! driven through `start_process` and `complete_task`; the `FlakyConnectionStore` delegates every byte of storage to
//! the production `MemoryStore` and composes the production transaction contract. The subject under test is
//! `WorkflowEngine::handle_join` at `rust/core/workflow/src/engine/handle_join.rs:7-119`:
//!
//! - `count_required_active_siblings` (`rust/core/workflow/src/engine/handle_join.rs:42`) is the only gate that can
//!   hold the join back, and it counts **required** siblings only (`rust/core/workflow/src/memory.rs:283-294`).
//! - `list_optional_active_siblings` (`rust/core/workflow/src/engine/handle_join.rs:46`,
//!   `rust/core/workflow/src/memory.rs:296-308`) is the set the join skips: each is completed `Skipped`, its tasks
//!   obsoleted and its jobs cancelled, with a `token.skipped` event per branch
//!   (`rust/core/workflow/src/engine/handle_join.rs:46-80`).
//! - Once no required sibling remains, exactly one `token.joined` event and one result token are produced
//!   (`rust/core/workflow/src/engine/handle_join.rs:106-118`).
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. The adversarial half is a deterministic fault: the join step
//! dies once on a broken connection and the **production** retry rule (`repeat_connection_failures`,
//! `rust/core/workflow/src/store.rs:146-165`, the same function `NeonStore::with_tx` runs) repeats the step. The
//! rolled-back attempt must leave no trace, so the run converges on **one** legal durable state — one join, one
//! skip per optional sibling.
//!
//! A ledger graph is used because the fork is where optional siblings are born (`required = transition.required
//! .unwrap_or(true)`, `rust/core/workflow/src/engine/execute_node_leave.rs:388`):
//!
//! ```text
//! start -> fan (fork)
//!            |-- main   (required) -> approve (task) -> converge (join) -> settle (end)
//!            |-- extra  (optional) -> review  (task) -> converge
//!            |-- hold   (optional) -> hold    (task) -> converge
//!            '-- wait   (optional) -> sla     (timer) -> converge
//! ```
//!
//! `review` is completed by a human (an optional branch that does arrive); `hold` and `sla` are left parked so the
//! join's retirement path is exercised for an open task and for an open job.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_join__002__optional_siblings_handled_correctly

use std::collections::BTreeMap;

use test_harness::fault::{Fault, FaultInjector};
use test_harness::TestClock;
use workflow::store::repeat_connection_failures;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, Job, JobStatus, MemoryStore, NodeDefinition,
    ProcessDefinition, ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus, Result,
    StartProcessParams, Store, Task, TaskStatus, TimerSpec, Token, TokenOutcome, TokenStatus,
    TransitionDefinition, TxStore, Value, WorkflowEngine, WorkflowError,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
/// The definition key and version registered with the engine.
const DEFINITION_KEY: &str = "TST-WF-JOIN-002";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The fork that mints the siblings, and the join that retires the optional ones.
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
/// The one required branch.
const MAIN_NODE: &str = "approve";
/// An optional branch a human completes (so it arrives, and is not skipped).
const REVIEW_NODE: &str = "review";
/// An optional branch left parked on an open task: the join must obsolete that task.
const HOLD_NODE: &str = "hold";
/// An optional branch left parked on an open timer job: the join must cancel that job.
const WAIT_NODE: &str = "sla";
const END_NODE: &str = "settle";
/// The transition each branch takes into the join.
const BRANCH_TRANSITION: &str = "go";
/// The production retry bound (`NeonStore` takes it from `db::retry::policy()`, default 3); the rule is independent
/// of where the number comes from.
const RETRY_ATTEMPTS: u32 = 3;
/// Far-future due date so the sla timer job stays open for the join to cancel.
const FAR_FUTURE_MS: &str = "4102444800000";

/// The production `MemoryStore` behind a scripted broken connection, retried by the production rule.
///
/// This is a fault interposer at the `TxStore` seam, not a second store and not a second retry: every byte of
/// storage is `MemoryStore`'s and the decision to repeat a broken step is `repeat_connection_failures`, the function
/// `NeonStore::with_tx` calls in production. On the scripted fault the step body is run against the real store and
/// then the transaction is forced to fail; `MemoryStore` restores its snapshot
/// (`rust/core/workflow/src/memory.rs:38-53`), so the repeated step sees exactly the state the first attempt saw.
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
                    panic!("the join contract is scripted with None and Error only, got {other:?}")
                }
            },
            RETRY_ATTEMPTS,
            |_attempt| {},
        )
    }
}

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

/// `start -> fan (fork: 1 required + 3 optional) -> 4 branch nodes -> converge (join) -> settle (end)`.
fn ledger_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE, None)]),
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
                // main — the only required branch; the join waits for this one and nothing else.
                transition("main", MAIN_NODE, Some(true)),
                // extra — an optional branch that does arrive (a human completes it).
                transition("extra", REVIEW_NODE, Some(false)),
                // hold — an optional branch parked on an open task the join must obsolete.
                transition("hold", HOLD_NODE, Some(false)),
                // wait — an optional branch parked on an open job the join must cancel.
                transition("wait", WAIT_NODE, Some(false)),
            ]),
            ..Default::default()
        },
    );
    for (id, name) in [
        (MAIN_NODE, "Approve"),
        (REVIEW_NODE, "Review"),
        (HOLD_NODE, "Hold"),
    ] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.to_string(),
                node_type: "task".to_string(),
                name: Some(name.to_string()),
                transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE, None)]),
                ..Default::default()
            },
        );
    }
    nodes.insert(
        WAIT_NODE.to_string(),
        NodeDefinition {
            id: WAIT_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Sla".to_string()),
            timer: Some(TimerSpec {
                due_at: Some(FAR_FUTURE_MS.to_string()),
                due_at_variable: None,
                transition: Some(BRANCH_TRANSITION.to_string()),
            }),
            transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        JOIN_NODE.to_string(),
        NodeDefinition {
            id: JOIN_NODE.to_string(),
            node_type: "join".to_string(),
            name: Some("Converge".to_string()),
            transitions: Some(vec![transition("next", END_NODE, None)]),
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
        id: "tst-wf-join-002".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 002".to_string(),
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

fn complete_task(engine: &WorkflowEngine<FlakyConnectionStore>, task_id: &str) -> Result<()> {
    engine.complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(BRANCH_TRANSITION.to_string()),
    })
}

fn tokens(store: &MemoryStore, instance: &str) -> Vec<Token> {
    store
        .with_tx(|tx| tx.tokens_for_instance(instance))
        .expect("the instance tokens are readable")
}

fn token_at(store: &MemoryStore, instance: &str, node: &str) -> Token {
    tokens(store, instance)
        .into_iter()
        .find(|token| token.node_id == node)
        .unwrap_or_else(|| panic!("a token is parked at {node}"))
}

/// A token read back by its durable id. A branch that moves into the join changes its `node_id`, so a branch under
/// motion is identified by id rather than by the node it was born at.
fn token_by_id(store: &MemoryStore, id: &str) -> Token {
    store
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
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

fn history(store: &MemoryStore, instance: &str) -> Vec<ProcessEvent> {
    store
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(store: &MemoryStore, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    history(store, instance)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn instance_status(store: &MemoryStore, instance: &str) -> ProcessStatus {
    store
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-002); the file and the assay use it.
fn wf_join_002__optional_siblings_handled_correctly() {
    let clock = TestClock::at_unix_millis(1_700_000_000_000);

    // One scripted fault on the join step (the fourth transaction: registration, start, complete `review`, then the
    // join). The first attempt rolls back; the production retry rule repeats it and the repeat commits. Every read
    // below goes through `reader.memory()`, which does not consume the script.
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
        .seed_definition(ledger_definition())
        .expect("the ledger definition registers with the engine");
    let started = engine
        .start_process(start_params())
        .expect("the fork process starts and parks its branches");
    let instance = started.process_instance_id.clone();

    // ── The required/optional split is the load-bearing input, read through the production `Store` ──────────────
    let fork = token_at(reader.memory(), &instance, FORK_NODE);
    assert_eq!(
        reader
            .memory()
            .with_tx(|tx| tx.count_required_active_siblings(&fork.id))
            .expect("the store counts required active siblings"),
        1,
        "{HARNESS}: exactly the one required branch is counted — the optional siblings are not"
    );
    let mut optional: Vec<String> = reader
        .memory()
        .with_tx(|tx| tx.list_optional_active_siblings(&fork.id))
        .expect("the store lists optional active siblings")
        .into_iter()
        .map(|token| token.node_id)
        .collect();
    optional.sort();
    let mut expected_optional = vec![
        HOLD_NODE.to_string(),
        REVIEW_NODE.to_string(),
        WAIT_NODE.to_string(),
    ];
    expected_optional.sort();
    assert_eq!(
        optional, expected_optional,
        "{HARNESS}: the optional set is exactly the three optional branches; the required branch is not in it"
    );

    // The parked optional branches exist: an open task at `hold`, an open timer job at `sla`. These are what the
    // join must retire, so their absence would make the retirement assertions vacuous.
    let hold_task = task_at(reader.memory(), &instance, HOLD_NODE);
    let hold_token_id = hold_task
        .token_id
        .clone()
        .expect("the optional hold task is linked to its token");
    let wait_token = token_at(reader.memory(), &instance, WAIT_NODE);
    let jobs_before: Vec<Job> = reader
        .memory()
        .with_tx(|tx| tx.open_jobs_for_instance(&instance))
        .expect("the instance jobs are readable");
    assert_eq!(
        jobs_before.len(),
        1,
        "{HARNESS}: the optional sla branch parked exactly one open job"
    );
    assert_eq!(
        jobs_before[0].token_id.as_deref(),
        Some(wait_token.id.as_str()),
        "{HARNESS}: the open job belongs to the optional sla branch"
    );
    assert_eq!(
        jobs_before[0].status,
        JobStatus::Pending,
        "{HARNESS}: the sla job is open before the join"
    );
    let sla_job_id = jobs_before[0].id.clone();

    // ── NEGATIVE: a required sibling still active must keep the join from firing ────────────────────────────────
    // `review` is optional, so completing it arrives at the join; the join must see the still-active required
    // `approve` and wait, not fire. If the gate counted optional siblings too, or if it fired on first arrival, this
    // is the assertion that would fail.
    let review_task = task_at(reader.memory(), &instance, REVIEW_NODE);
    let review_token_id = review_task
        .token_id
        .clone()
        .expect("the review task is linked to its token");
    complete_task(&engine, &review_task.id).expect("the optional review branch completes into the join");

    assert_eq!(
        instance_status(reader.memory(), &instance),
        ProcessStatus::Active,
        "{HARNESS}: the join did not fire while a required sibling was still active"
    );
    assert_eq!(
        events_of_type(reader.memory(), &instance, "token.joined").len(),
        0,
        "{HARNESS}: no join token is produced early"
    );
    assert_eq!(
        token_by_id(reader.memory(), &review_token_id).outcome,
        Some(TokenOutcome::Completed),
        "{HARNESS}: the arriving optional branch is completed normally, not skipped"
    );
    assert_eq!(
        reader
            .memory()
            .with_tx(|tx| tx.count_required_active_siblings(&fork.id))
            .expect("the store counts required active siblings"),
        1,
        "{HARNESS}: the required branch is still the reason the join is held"
    );
    let mut still_optional: Vec<String> = reader
        .memory()
        .with_tx(|tx| tx.list_optional_active_siblings(&fork.id))
        .expect("the store lists optional active siblings")
        .into_iter()
        .map(|token| token.node_id)
        .collect();
    still_optional.sort();
    let mut expected_still = vec![HOLD_NODE.to_string(), WAIT_NODE.to_string()];
    expected_still.sort();
    assert_eq!(
        still_optional, expected_still,
        "{HARNESS}: the remaining optionals are untouched by the held join"
    );

    // ── The join step dies once; the production retry repeats it and the run converges ──────────────────────────
    let main_task = task_at(reader.memory(), &instance, MAIN_NODE);
    let main_token_id = main_task
        .token_id
        .clone()
        .expect("the required main task is linked to its token");
    complete_task(&engine, &main_task.id)
        .expect("the production retry repeats the broken join step and the repeat commits");
    assert_eq!(
        fault_probe.fired(),
        5,
        "{HARNESS}: the broken join step was retried by the production rule, not swallowed"
    );

    // ONE legal durable state: the join fired exactly once, from the required branch, over all four branches.
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
            .map(|branches| branches.len()),
        Some(4),
        "{HARNESS}: the join accounts for every branch, required and optional"
    );
    // The roster is the four siblings BY TOKEN IDENTITY, not merely by count: the join names the required branch
    // and each optional branch — the one that arrived and the two it retired — and no other token. A join that
    // listed an unrelated token, dropped an optional sibling, or double-counted one would satisfy the count above
    // but fail here, so this clause pins `branches` to the sibling set the fork actually minted
    // (`rust/core/workflow/src/engine/handle_join.rs:82-86`, fed to the event at `:106-117`).
    let mut joined_branches: Vec<String> = joined[0]
        .data
        .get("branches")
        .and_then(Value::as_array)
        .expect("the token.joined event carries its branch roster")
        .iter()
        .map(|branch| {
            branch
                .as_str()
                .expect("every entry in the join roster is a token id")
                .to_string()
        })
        .collect();
    joined_branches.sort();
    let mut expected_branches = vec![
        main_token_id,
        review_token_id.clone(),
        hold_token_id.clone(),
        wait_token.id.clone(),
    ];
    expected_branches.sort();
    assert_eq!(
        joined_branches, expected_branches,
        "{HARNESS}: the join roster is exactly the four siblings the fork minted, by token id — required and optional alike"
    );
    let settle = token_at(reader.memory(), &instance, END_NODE);
    assert_eq!(
        joined[0].data.get("resultTokenId").and_then(Value::as_str),
        Some(settle.id.as_str()),
        "{HARNESS}: the join names the single result token"
    );
    assert_eq!(
        settle.parent_token_id.as_deref(),
        Some(fork.id.as_str()),
        "{HARNESS}: the result token belongs to the fork that was joined"
    );
    // The join event is attributed to the join node it actually fired at, not merely to a token: both the event's
    // `node_id` and its `joinNodeId` datum must name `converge`. A join that recorded the wrong node — or dropped
    // the datum — would still carry the right roster and result token, so it fails only here
    // (`rust/core/workflow/src/engine/handle_join.rs:111-114`).
    assert_eq!(
        joined[0].node_id.as_deref(),
        Some(JOIN_NODE),
        "{HARNESS}: the token.joined event names the join node it fired at"
    );
    assert_eq!(
        joined[0].data.get("joinNodeId").and_then(Value::as_str),
        Some(JOIN_NODE),
        "{HARNESS}: the token.joined payload names the join node"
    );

    // The optional branches that never arrived were retired: one `token.skipped` each, no more. The event carries the
    // branch's own token id, so the durable log names which token was retired — not just which node it sat at. A join
    // that skipped a different token, or emitted the event for a token it did not actually complete, fails here.
    let mut skipped: Vec<(String, String)> = events_of_type(reader.memory(), &instance, "token.skipped")
        .into_iter()
        .map(|event| {
            (
                event
                    .node_id
                    .expect("every token.skipped event names the skipped branch's node"),
                event
                    .token_id
                    .expect("every token.skipped event names the token that was skipped"),
            )
        })
        .collect();
    skipped.sort();
    let mut expected_skipped = vec![
        (HOLD_NODE.to_string(), hold_token_id.clone()),
        (WAIT_NODE.to_string(), wait_token.id.clone()),
    ];
    expected_skipped.sort();
    assert_eq!(
        skipped, expected_skipped,
        "{HARNESS}: exactly the two parked optional branches were skipped once each, each naming its own token; the arrived one was not"
    );
    for node in [HOLD_NODE, WAIT_NODE] {
        let token = token_at(reader.memory(), &instance, node);
        assert_eq!(
            token.status,
            TokenStatus::Completed,
            "{HARNESS}: the skipped optional token at {node} is durably concluded"
        );
        assert_eq!(
            token.outcome,
            Some(TokenOutcome::Skipped),
            "{HARNESS}: the optional token at {node} is concluded as Skipped, not Completed"
        );
    }
    assert_eq!(
        token_by_id(reader.memory(), &review_token_id).outcome,
        Some(TokenOutcome::Completed),
        "{HARNESS}: the optional branch that actually arrived stays Completed"
    );

    // The retired branches took their work with them: the open task is obsolete, the open job is cancelled.
    let hold_after = task_at(reader.memory(), &instance, HOLD_NODE);
    assert_eq!(
        hold_after.status,
        TaskStatus::Obsolete,
        "{HARNESS}: the open task on the skipped optional branch is obsoleted"
    );
    let sla_job = reader
        .memory()
        .with_tx(|tx| tx.get_job(&sla_job_id))
        .expect("the sla job is readable");
    assert_eq!(
        sla_job.status,
        JobStatus::Cancelled,
        "{HARNESS}: the open timer job on the skipped optional branch is cancelled"
    );
    assert_eq!(
        sla_job.locked_by, None,
        "{HARNESS}: a cancelled job holds no worker"
    );
    assert_eq!(
        reader
            .memory()
            .with_tx(|tx| tx.open_jobs_for_instance(&instance))
            .expect("the instance jobs are readable")
            .len(),
        0,
        "{HARNESS}: no optional branch leaves an open job behind"
    );

    // The cancel is announced on the durable log, not only left in the job row: one `job.cancelled`, naming the
    // optional branch's own job and token with the skip reason. Cancelling the row but dropping the event — or
    // cancelling another branch's job — would still satisfy the status assertions above, so this is the clause
    // that pins the production retirement path (`rust/core/workflow/src/engine/handle_join.rs:62-79`).
    let cancelled = events_of_type(reader.memory(), &instance, "job.cancelled");
    assert_eq!(
        cancelled.len(),
        1,
        "{HARNESS}: the join emits exactly one job.cancelled, for the one open job an optional sibling left"
    );
    assert_eq!(
        cancelled[0].job_id.as_deref(),
        Some(sla_job_id.as_str()),
        "{HARNESS}: the cancelled event names the optional sla branch's own timer job"
    );
    assert_eq!(
        cancelled[0].token_id.as_deref(),
        Some(wait_token.id.as_str()),
        "{HARNESS}: the cancellation is attributed to the skipped optional token, not the required branch"
    );
    assert_eq!(
        cancelled[0].data.get("reason").and_then(Value::as_str),
        Some("branch skipped"),
        "{HARNESS}: the cancellation carries the join's skip reason"
    );

    // The obsoletion of the skipped branch's open task is announced on the durable log too, not only left in the
    // task row: exactly one `task.obsoleted`, naming the optional hold task, carrying the join's skip reason. A join
    // that flipped the row Obsolete but dropped the event — or obsoleted a task on the required branch or the
    // arrived optional branch — would satisfy the task-status assertions above and fail here, so this clause pins
    // the task half of the same retirement path the job cancellation clause pins
    // (`rust/core/workflow/src/engine/handle_join.rs:59-61`, event at `execute_node_leave.rs:160-170`).
    let obsoleted = events_of_type(reader.memory(), &instance, "task.obsoleted");
    assert_eq!(
        obsoleted.len(),
        1,
        "{HARNESS}: the join obsoletes exactly one task, the open task an optional sibling left"
    );
    assert_eq!(
        obsoleted[0].task_id.as_deref(),
        Some(hold_task.id.as_str()),
        "{HARNESS}: the obsoleted event names the optional hold branch's own task, not the required or arrived one"
    );
    assert_eq!(
        obsoleted[0].data.get("reason").and_then(Value::as_str),
        Some("branch skipped"),
        "{HARNESS}: the obsoletion carries the join's skip reason"
    );

    // The retirement is durable and ordered BEFORE the join. Event ids are assigned in insertion order and the
    // test's clock never moves, so history cannot reorder events by time: every retirement event committed with
    // the join (`token.skipped`, `task.obsoleted`, `job.cancelled`) must carry a lower id than the `token.joined`
    // event. A join that announced itself before it retired the still-active optional branches would leave those
    // branches open behind a join that had already fired; the row-state and count checks above cannot see that
    // ordering, so this clause pins it (`rust/core/workflow/src/engine/handle_join.rs:46-80` before `:106-117`).
    let retirement_ids: Vec<i64> = history(reader.memory(), &instance)
        .into_iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "token.skipped" | "task.obsoleted" | "job.cancelled"
            )
        })
        .map(|event| event.id)
        .collect();
    assert_eq!(
        retirement_ids.len(),
        4,
        "{HARNESS}: the join retires the optional siblings in exactly four durable events"
    );
    for id in &retirement_ids {
        assert!(
            *id < joined[0].id,
            "{HARNESS}: retirement event {id} is announced before the join event {}",
            joined[0].id
        );
    }

    // No optional token leaks active into the terminal state: the process converges to its declared end.
    assert_eq!(
        reader
            .memory()
            .with_tx(|tx| tx.count_active_tokens(&instance))
            .expect("the active token count is readable"),
        0,
        "{HARNESS}: every branch is concluded; none leaks active"
    );
    let finished = reader
        .memory()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    assert_eq!(
        finished.status,
        ProcessStatus::Completed,
        "{HARNESS}: the join drove the process to completion"
    );
    assert_eq!(
        finished.outcome,
        Some(ProcessOutcome::Completed),
        "{HARNESS}: completion carries the declared end outcome"
    );

    // ── NEGATIVE: a retired optional branch is durably closed and cannot be revived as work ─────────────────────
    let revived = complete_task(&engine, &hold_task.id)
        .expect_err("an obsoleted optional branch must not accept work after the join");
    assert_eq!(
        revived.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the join's obsolete is what closes the skipped branch"
    );
    assert_eq!(
        task_at(reader.memory(), &instance, HOLD_NODE).status,
        TaskStatus::Obsolete,
        "{HARNESS}: the refused attempt did not reopen the branch"
    );

    // NEGATIVE: the join fired once, so the required branch's task is spent — a second completion is refused.
    let replay = complete_task(&engine, &main_task.id)
        .expect_err("the required branch is spent once the join fired");
    assert_eq!(
        replay.code(),
        "TASK_ALREADY_COMPLETED",
        "{HARNESS}: the join is not re-enterable through the branch it already consumed"
    );
    assert_eq!(
        events_of_type(reader.memory(), &instance, "token.joined").len(),
        1,
        "{HARNESS}: the refused replay produced no second join"
    );
}

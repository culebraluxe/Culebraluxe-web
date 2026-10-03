//! FORGE.JOB — a completion write is repeated; the paid turn never is, DRIVEN THROUGH THE REAL PATH.
//!
//! `forge_job__014` pins the classifier (which error is the engine's plumbing) and the rule's unit rails pin the
//! disposition (repeat while the budget lasts, and only for plumbing). Neither shows that the rule is REACHED from a
//! driver: that the failure arrives from a real store transaction inside a real Workflow completion, that the repeat
//! writes again WITHOUT paying for a second role turn, and that a completion which never lands leaves a durable row
//! that stops the next run from paying for that turn a second time.
//!
//! This is that rail, and it uses the production seams rather than doubles of them: `ForgeRuntime::from_store` (the
//! XML definition, a named `MemoryLedger`), the production `MemoryStore` behind a `TxStore` fault interposer, the
//! production services over a counting runner, and the production durable driver `drive_forge_story_with_jobs`.
//!
//!   * CASE 1 — one failed completion transaction: the turn runs once, the completion transaction dies mid-commit
//!     once, the write is repeated, the Workflow task settles and the durable job completes. The turn count stays 1.
//!   * CASE 2 — every completion transaction fails: the write is repeated to its bound, then the durable job is
//!     Failed with the paid-turn reason while the Workflow task stays unresolved. THE SAME STORY IS DRIVEN AGAIN:
//!     `enqueue` reuses the existing row (Forge keys the durable job by the Workflow task id, and
//!     `create_job_with_id` adopts a row written for the same request), the Failed row is not claimable, so the second
//!     drive executes NO second turn and surfaces the terminal condition as a human hold.
//!
//! The interposer is CONTENT-based, not call-order-based. It fails exactly the transaction in which the watched task
//! becomes `Completed` — which is the whole of `WorkflowEngine::complete_task`'s transaction — and it raises the fault
//! AFTER the step body has run, so `MemoryStore` restores its snapshot and the repeated step re-reads the state the
//! first attempt saw (`wf_command__003` composes the same seam, for the command node).
//!
//! Level: L1, production engine on the in-memory store, with fault injection at the `TxStore` seam.

use std::sync::{Arc, Mutex};

use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution, ForgeRoleOutcome, ForgeRoleRunner,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::{JobService, WorkflowJobService};
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::RecordingWriter;
use forge::roles::architect::ArchitectService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use test_harness::fault::{Fault, FaultInjector};
use workflow::{
    JobStatus, MemoryStore, Result as WfResult, Store, TaskStatus, TxStore, WorkflowError,
};

const STORY: &str = "TST-FORGE-JOB-COMPLETION-RAIL";
const WORK_TYPE: &str = "FEATURE";
const WORKER: &str = "forge-worker-a";
const SECOND_WORKER: &str = "forge-worker-b";
/// The node a FEATURE story wakes on, so this is the one turn both cases are about. The registry below registers
/// exactly this lane's service: a node bound to a service the process did not register fails the job binding before
/// any of this is tested.
const FIRST_TURN_NODE: &str = "architect";
/// The database seam's own words for a session taken away mid-statement — the shape `engine_fault` exists for, and
/// the fault that must be classified as plumbing rather than as a verdict about the work.
const UNREACHABLE_DB: &str =
    "db: DatabaseUnavailable during workflow.complete_task (sqlstate 25P03)";

/// The evidence a FEATURE story is woken with.
fn evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some(WORK_TYPE.into()),
        scout_required: Some(false),
        ..Default::default()
    }
}

/// The in-memory store with a scripted failure on exactly one transaction: the one that COMPLETES the watched task.
///
/// A `TxStore` seam cannot be told "fail the completion write" by call index, and counting calls would make this rail a
/// test about the driver's call order rather than about the completion. So the failure is decided by CONTENT: the step
/// body runs against the real store, and when the watched task has become `Completed` inside that same transaction —
/// which is what "the completion write committed" means — the scripted fault is raised and the store restores its
/// snapshot, exactly as a socket that died mid-commit leaves nothing behind.
#[derive(Clone)]
struct CompletionFaultStore {
    memory: MemoryStore,
    faults: FaultInjector,
    watch: Arc<Mutex<Option<String>>>,
    completion_writes: Arc<Mutex<usize>>,
    faults_fired: Arc<Mutex<usize>>,
}

impl CompletionFaultStore {
    fn new(memory: MemoryStore, faults: FaultInjector) -> Self {
        Self {
            memory,
            faults,
            watch: Arc::new(Mutex::new(None)),
            completion_writes: Arc::new(Mutex::new(0)),
            faults_fired: Arc::new(Mutex::new(0)),
        }
    }

    /// Point the interposer at the Workflow task whose completion is the subject of the contract.
    fn watch(&self, task_id: &str) {
        *self.watch.lock().expect("watch lock") = Some(task_id.to_string());
    }

    /// How many transactions the completion actually reached this store: one per attempted WRITE, including the
    /// attempts a fault rolled back. This is the count that says the write was repeated.
    fn completion_writes(&self) -> usize {
        *self.completion_writes.lock().expect("completion lock")
    }

    /// How many scripted faults were raised.
    fn faults_fired(&self) -> usize {
        *self.faults_fired.lock().expect("faults lock")
    }

    /// The next scripted fault, or `None` when the script is exhausted.
    fn next_fault(&self) -> Option<WorkflowError> {
        match self.faults.next_fault() {
            Fault::None => None,
            Fault::Error { message, .. } => {
                *self.faults_fired.lock().expect("faults lock") += 1;
                Some(WorkflowError::unavailable(message))
            }
            other => panic!("this contract is scripted with None and Error only, got {other:?}"),
        }
    }
}

impl TxStore for CompletionFaultStore {
    fn with_tx<R, F>(&self, mut f: F) -> WfResult<R>
    where
        F: FnMut(&mut dyn Store) -> WfResult<R>,
    {
        let watch = self.watch.lock().expect("watch lock").clone();
        self.memory.with_tx(|tx| {
            // Read the watched task on BOTH sides of the step: what this interposer is about is the transaction that
            // MOVES the task into `Completed`, not every later transaction that happens to find it there.
            let before = watch
                .as_deref()
                .map(|task_id| is_completed(tx, task_id))
                .unwrap_or(false);
            let committed = f(tx)?;
            let after = watch
                .as_deref()
                .map(|task_id| is_completed(tx, task_id))
                .unwrap_or(false);
            if before || !after {
                return Ok(committed);
            }
            *self.completion_writes.lock().expect("completion lock") += 1;
            match self.next_fault() {
                // Raised INSIDE the transaction the body just ran in, so the whole completion rolls back and the
                // repeated attempt finds the task exactly as actionable as it left it.
                Some(error) => Err(error),
                None => Ok(committed),
            }
        })
    }
}

/// Whether this transaction currently holds `task_id` as `Completed`.
fn is_completed(tx: &mut dyn Store, task_id: &str) -> bool {
    matches!(
        tx.get_task(task_id).map(|task| task.status),
        Ok(TaskStatus::Completed)
    )
}

/// A role turn that records the node it was asked to run and succeeds.
///
/// It answers no envelope (`turn_ports()` defaults to `None`), which is the seam `run_lane_turn` reads: with no ports
/// the service boundary delegates the whole turn to the runner, so one turn here is one turn and nothing else. It is
/// the observation point for "was the paid turn executed again".
#[derive(Default)]
struct CountingRunner {
    calls: Mutex<Vec<String>>,
}

impl CountingRunner {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls lock").clone()
    }
}

impl ForgeRoleRunner for CountingRunner {
    fn run(&self, node_id: &str, _task: &ActiveForgeRoleTask) -> WfResult<ForgeRoleOutcome> {
        self.calls
            .lock()
            .expect("calls lock")
            .push(node_id.to_string());
        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence: ForgeGateEvidence::default(),
        })
    }
}

/// Everything a case needs, wired the way production wires it: the XML definition over the fault-interposable store,
/// a canonical Story Board writer, and the store's untouched handle for observation.
///
/// `memory` is a second handle on the SAME in-memory store (`MemoryStore` is a shared handle): observation reads go
/// through it so they neither consume a scripted fault nor perturb the state the engine's step is being retried
/// against.
///
/// The counting runner and the service built over it are NOT here: a registry borrows the service it points at, and a
/// service borrows its runner, so a value owning all three would be borrowed by one of its own fields.
struct Fixture {
    rt: ForgeRuntime<CompletionFaultStore>,
    memory: MemoryStore,
    store: CompletionFaultStore,
    writer: Arc<RecordingWriter>,
}

fn forge_fixture(faults: FaultInjector) -> Fixture {
    let memory = MemoryStore::new();
    let store = CompletionFaultStore::new(memory.clone(), faults);
    let writer = Arc::new(RecordingWriter::default());
    let rt = ForgeRuntime::from_store(
        store.clone(),
        writer.clone(),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    )
    .expect("the XML Forge definition seeds");
    Fixture {
        rt,
        memory,
        store,
        writer,
    }
}

/// One durable generation: exactly one role turn (`max_steps: 1`), so "the turn count" is unambiguous in both cases.
fn drive_one_turn(
    fixture: &Fixture,
    jobs: &dyn JobService,
    registry: &ForgeServiceRegistry<'_>,
    worker_id: &str,
) -> WfResult<DriveForgeStoryResult> {
    drive_forge_story_with_jobs(
        &fixture.rt,
        STORY,
        DriveForgeStoryOptions {
            work_type: WORK_TYPE,
            evidence: evidence(),
            // The durable branch resolves concrete services through the registry, not through this field.
            runner: None,
            max_steps: 1,
            worker_id,
            split_concurrency: 1,
            stop_after: None,
            turn_cap: 8,
        },
        DurableForgeExecution { jobs, registry },
    )
}

/// The durable job row, read through the untouched handle.
fn read_job(memory: &MemoryStore, id: &str) -> workflow::Job {
    memory
        .with_tx(|tx| tx.get_job(id))
        .expect("the job row reads")
}

/// The Workflow task row, read through the untouched handle.
fn read_task(memory: &MemoryStore, id: &str) -> workflow::Task {
    memory
        .with_tx(|tx| tx.get_task(id))
        .expect("the task row reads")
}

/// How many `task.completed` events the durable log holds for `task_id`.
///
/// A rolled-back completion leaves none: this is what makes "the failed attempt produced nothing" a statement about
/// the store rather than about a return value.
fn completion_events(memory: &MemoryStore, instance_id: &str, task_id: &str) -> usize {
    memory
        .with_tx(|tx| tx.history(instance_id, 256))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| {
            event.event_type == "task.completed" && event.task_id.as_deref() == Some(task_id)
        })
        .count()
}

/// Wake the story and name the task whose completion transaction the interposer must fail.
///
/// The wake is done here rather than left to the drive so the contract can point at the exact Workflow task it is
/// about; the drive's own wake is idempotent on the same story, so it finds this very task.
fn wake_and_watch(fixture: &Fixture) -> (String, String) {
    let wake = fixture
        .rt
        .wake_story(STORY, WORK_TYPE, evidence())
        .expect("the story wakes");
    let open = wake.open.expect("a role task is open");
    assert_eq!(
        open.node_id.as_deref(),
        Some(FIRST_TURN_NODE),
        "the story must wake on the lane this contract registers"
    );
    fixture.store.watch(&open.task_id);
    (wake.instance_id, open.task_id)
}

/// CASE 1 — one failed completion transaction: the write is repeated, the turn is not.
#[test]
fn a_transient_completion_fault_repeats_the_write_and_never_the_turn() {
    let fixture = forge_fixture(FaultInjector::scripted(vec![Fault::error(
        "DB_UNAVAILABLE",
        UNREACHABLE_DB,
    )]));
    let runner = CountingRunner::default();
    let service = ArchitectService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&service as &dyn AbstractForgeService)
        .expect("register forge.architect");
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let (instance_id, task_id) = wake_and_watch(&fixture);

    let out = drive_one_turn(&fixture, &jobs, &registry, WORKER)
        .expect("the completion is repeated until it lands, so the drive succeeds");

    assert_eq!(
        out.steps,
        vec![FIRST_TURN_NODE.to_string()],
        "one turn was dispatched"
    );
    assert!(
        !out.needs_human,
        "an engine fault is not a human decision: {:?}",
        out.blocked_reason
    );
    assert_eq!(
        runner.calls(),
        vec![FIRST_TURN_NODE.to_string()],
        "the role turn executed exactly ONCE: the repeat is the write, never the turn"
    );
    assert_eq!(
        fixture.store.completion_writes(),
        2,
        "the completion WRITE was attempted twice — the first died mid-transaction, the retry committed"
    );
    assert_eq!(
        fixture.store.faults_fired(),
        1,
        "exactly one engine fault was injected"
    );

    let job = read_job(&fixture.memory, &task_id);
    assert_eq!(
        job.id, task_id,
        "Forge keys the durable job by the Workflow task id"
    );
    assert_eq!(
        job.status,
        JobStatus::Completed,
        "the durable job closes once the Workflow commit lands: {job:?}"
    );
    assert_eq!(
        job.attempts, 1,
        "the repeat happened inside one claim, so it is not a second attempt at the work"
    );

    let task = read_task(&fixture.memory, &task_id);
    assert_eq!(
        task.status,
        TaskStatus::Completed,
        "the Workflow task settled"
    );
    assert_eq!(
        completion_events(&fixture.memory, &instance_id, &task_id),
        1,
        "the rolled-back attempt left no trace: the task is completed exactly once in the durable log"
    );
    assert!(
        fixture.writer.holds.lock().expect("holds").is_empty(),
        "a repeatable write failure is not a hold"
    );
}

/// CASE 2 — every completion transaction fails: the write is bounded, the row is terminal, and the next drive pays
/// NOTHING.
///
/// This is the case the ordering exists for. The turn has been executed and paid for; the Workflow still says the task
/// is READY; and the only thing that stops a later run of the same story from buying a second turn is the durable row
/// that was failed with the reason an operator must read.
#[test]
fn a_completion_that_never_lands_is_terminal_and_the_second_drive_pays_no_second_turn() {
    let fixture = forge_fixture(FaultInjector::always(Fault::error(
        "DB_UNAVAILABLE",
        UNREACHABLE_DB,
    )));
    let runner = CountingRunner::default();
    let service = ArchitectService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&service as &dyn AbstractForgeService)
        .expect("register forge.architect");
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let (instance_id, task_id) = wake_and_watch(&fixture);

    let err = drive_one_turn(&fixture, &jobs, &registry, WORKER).expect_err(
        "a completion that never lands must reach the caller rather than be reported as a settled story",
    );
    assert!(
        err.is_connection_failure(),
        "the engine's own fault is what comes back, unwrapped: {err}"
    );

    assert_eq!(
        runner.calls(),
        vec![FIRST_TURN_NODE.to_string()],
        "one turn was paid for"
    );
    assert_eq!(
        fixture.store.completion_writes(),
        3,
        "the WRITE is repeated to its bound (`COMPLETION_WRITE_ATTEMPTS`) and then given up on"
    );
    assert_eq!(
        fixture.store.faults_fired(),
        3,
        "every attempt of the completion write failed, and only the write was attempted again"
    );

    let job = read_job(&fixture.memory, &task_id);
    assert_eq!(
        job.id, task_id,
        "Forge keys the durable job by the Workflow task id"
    );
    assert_eq!(
        job.status,
        JobStatus::Failed,
        "the durable row is the record of a turn that was paid for: {job:?}"
    );
    assert_eq!(
        job.attempts, 1,
        "one claim, one turn — the repeats were completion writes, not attempts at the work"
    );
    let recorded = job.last_error.clone().unwrap_or_default();
    assert!(
        recorded.contains("PAID_TURN_NOT_REDISPATCHED"),
        "the row names the one thing an operator must not do: {recorded}"
    );

    let task = read_task(&fixture.memory, &task_id);
    assert_eq!(
        task.status,
        TaskStatus::Ready,
        "the Workflow task is unresolved — the completion never committed"
    );
    assert_eq!(
        completion_events(&fixture.memory, &instance_id, &task_id),
        0,
        "and every failed write was rolled back, so the durable log shows no completion at all"
    );

    // THE SECOND DRIVE. Nothing was reconciled and nothing was requeued: the story is driven again exactly as a later
    // run would drive it, with a different worker so a stuck lease cannot be what makes this pass.
    assert!(
        jobs.claim(SECOND_WORKER, 10).expect("claim").is_empty(),
        "a terminal durable row is not claimable by any worker"
    );

    let second = drive_one_turn(&fixture, &jobs, &registry, SECOND_WORKER)
        .expect("the second drive reports the terminal condition instead of failing again");
    assert!(
        second.steps.is_empty(),
        "the second drive dispatched no step at all: {:?}",
        second.steps
    );
    assert!(
        second.needs_human,
        "the story is held for a human: {:?}",
        second.blocked_reason
    );
    let reason = second.blocked_reason.clone().unwrap_or_default();
    assert!(
        reason.contains(&task_id),
        "the second drive names the SAME durable job id — the id IS the Workflow task id: {reason}"
    );
    assert!(
        reason.contains("Failed") && reason.contains("PAID_TURN_NOT_REDISPATCHED"),
        "the reason carries the terminal condition and the paid turn: {reason}"
    );

    assert_eq!(
        runner.calls().len(),
        1,
        "TOTAL role executions across both drives: the second drive paid no second turn"
    );
    assert_eq!(
        fixture.store.completion_writes(),
        3,
        "the second drive did not even attempt the completion write"
    );
    assert_eq!(
        read_job(&fixture.memory, &task_id).attempts,
        1,
        "and no replacement claim happened behind the terminal row"
    );
    assert!(
        jobs.claim(WORKER, 10).expect("claim").is_empty(),
        "no replacement `forge.role` job was enqueued for the same task: `enqueue` adopted the existing row"
    );

    let holds = fixture.writer.holds.lock().expect("holds").clone();
    assert!(
        holds
            .iter()
            .any(|(story, held)| story == STORY && held.contains("PAID_TURN_NOT_REDISPATCHED")),
        "the story surfaces the terminal condition as a human hold: {holds:?}"
    );
}

//! SEAM-005 — a transient completion-write fault repeats only the write, never the paid Smith role.

#[path = "support/forge_seam.rs"]
mod support;

use std::sync::{Arc, Mutex};

use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DurableForgeExecution,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::WorkflowJobService;
use forge::engine::runner::ProductionRoleRunner;
use forge::engine::runtime::ForgeRuntime;
use forge::roles::ForgeLaneServices;
use support::{SeamHarness, SeamWriter, CANDIDATE_SHA};
use test_harness::fault::{Fault, FaultInjector};
use workflow::{
    JobStatus, MemoryStore, Result as WfResult, Store, TaskStatus, TxStore, WorkflowError,
};

const STORY: &str = "TST-FORGE-SEAM-005";
const WORKER: &str = "forge-seam-005";
const UNREACHABLE_DB: &str =
    "db: DatabaseUnavailable during workflow.complete_task (sqlstate 25P03)";

fn fast_evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FAST".into()),
        ..Default::default()
    }
}

#[derive(Clone)]
struct CompletionFaultStore {
    memory: MemoryStore,
    faults: FaultInjector,
    watch: Arc<Mutex<Option<String>>>,
    completion_writes: Arc<Mutex<usize>>,
}

impl CompletionFaultStore {
    fn new(memory: MemoryStore, faults: FaultInjector) -> Self {
        Self {
            memory,
            faults,
            watch: Arc::new(Mutex::new(None)),
            completion_writes: Arc::new(Mutex::new(0)),
        }
    }

    fn watch(&self, task_id: &str) {
        *self.watch.lock().expect("watch") = Some(task_id.to_string());
    }

    fn completion_writes(&self) -> usize {
        *self.completion_writes.lock().expect("writes")
    }

    fn next_fault(&self) -> Option<WorkflowError> {
        match self.faults.next_fault() {
            Fault::None => None,
            Fault::Error { message, .. } => Some(WorkflowError::unavailable(message)),
            other => panic!("unexpected scripted fault {other:?}"),
        }
    }
}

impl TxStore for CompletionFaultStore {
    fn with_tx<R, F>(&self, mut f: F) -> WfResult<R>
    where
        F: FnMut(&mut dyn Store) -> WfResult<R>,
    {
        let watch = self.watch.lock().expect("watch").clone();
        self.memory.with_tx(|tx| {
            let before = watch
                .as_deref()
                .map(|id| is_completed(tx, id))
                .unwrap_or(false);
            let result = f(tx)?;
            let after = watch
                .as_deref()
                .map(|id| is_completed(tx, id))
                .unwrap_or(false);
            if before || !after {
                return Ok(result);
            }
            *self.completion_writes.lock().expect("writes") += 1;
            match self.next_fault() {
                Some(error) => Err(error),
                None => Ok(result),
            }
        })
    }
}

fn is_completed(tx: &mut dyn Store, task_id: &str) -> bool {
    matches!(
        tx.get_task(task_id).map(|task| task.status),
        Ok(TaskStatus::Completed)
    )
}

#[test]
fn completion_fault_does_not_repeat_smith_or_change_candidate_identity() {
    let memory = MemoryStore::new();
    let store = CompletionFaultStore::new(
        memory.clone(),
        FaultInjector::scripted(vec![Fault::error("DB_UNAVAILABLE", UNREACHABLE_DB)]),
    );
    let writer = Arc::new(SeamWriter::default());
    let rt = ForgeRuntime::from_store(
        store.clone(),
        writer.clone(),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    )
    .expect("production XML seeds");

    let wake = rt
        .wake_story(STORY, "FAST", fast_evidence())
        .expect("FAST story wakes");
    let smith = wake.open.expect("FAST opens Smith directly");
    assert_eq!(smith.node_id.as_deref(), Some("fast_smith"));
    store.watch(&smith.task_id);

    let harness = Arc::new(SeamHarness::default());
    let runner = ProductionRoleRunner::new(harness.clone(), fast_evidence())
        .with_writer(writer.clone())
        .with_story_run(Some("run-forge-seam-005".into()));
    let lanes = ForgeLaneServices::new(&runner);
    let registry = lanes.registry().expect("all production services register");
    let jobs = WorkflowJobService::new(rt.engine());

    let out = drive_forge_story_with_jobs(
        &rt,
        STORY,
        DriveForgeStoryOptions {
            work_type: "FAST",
            evidence: fast_evidence(),
            runner: None,
            max_steps: 1,
            worker_id: WORKER,
            split_concurrency: 1,
            stop_after: None,
            turn_cap: 4,
        },
        DurableForgeExecution {
            jobs: &jobs,
            registry: &registry,
        },
    )
    .expect("the transient completion write is retried until it lands");

    assert_eq!(out.steps, vec!["fast_smith".to_string()]);
    assert_eq!(
        harness.count("fast_smith"),
        1,
        "the paid Smith role executed once"
    );
    assert!(
        store.completion_writes() > 1,
        "the completion WRITE, not the role, was retried"
    );
    let job = rt.engine().get_job(&smith.task_id).expect("durable job");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.attempts, 1, "one job claim paid for one role turn");

    let stamps = writer.candidates.lock().expect("candidates");
    assert_eq!(
        stamps.len(),
        1,
        "retrying the write does not create a second candidate"
    );
    assert_eq!(
        stamps[0].1, CANDIDATE_SHA,
        "candidate identity is unchanged"
    );
}

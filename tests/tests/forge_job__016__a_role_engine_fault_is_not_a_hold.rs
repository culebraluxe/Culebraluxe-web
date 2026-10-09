//! FORGE.JOB — an engine fault inside a role turn goes back to the queue; a role's own failure is held.
//!
//! The job layer already classifies a role error (`execute_claimed_job_unsettled`, pinned by `forge_job__014` and
//! ARCH-SEAM-007): plumbing is settled RETRYABLE, a verdict is settled permanent. Until 2026-10-03 the durable driver
//! then held the story for a human on EITHER kind, so a dropped session parked the story while its job sat Pending —
//! the queue saying "retry" and the board saying "human" about the same turn, against the captain's rule that an engine
//! fault is not the story's verdict (2026-09-29).
//!
//!   * CASE 1 — the turn dies on the database seam: the driver returns the error UNWRAPPED (so the lane's exit path
//!     clears the claim into the queue), opens no hold, and the job is Pending inside its retry budget.
//!   * CASE 2 — the role itself fails: the story is held with the reason, and the job is Failed, not retried.
//!
//! Level: L1, production engine and durable driver on the in-memory store.

use std::sync::{Arc, Mutex};

use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution, ForgeRoleOutcome, ForgeRoleRunner,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::WorkflowJobService;
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::RecordingWriter;
use forge::roles::architect::ArchitectService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use workflow::{JobStatus, MemoryStore, Result as WfResult, TxStore, WorkflowError};

const STORY: &str = "TST-FORGE-JOB-ROLE-ENGINE-FAULT";
const WORK_TYPE: &str = "FEATURE";
const WORKER: &str = "forge-worker-a";
/// The node a FEATURE story wakes on; the registry registers exactly this lane's service.
const FIRST_TURN_NODE: &str = "architect";

/// A role turn that fails with one scripted error and counts how often it was paid for.
struct FailingRunner {
    make: fn() -> WorkflowError,
    turns: Mutex<usize>,
}

impl ForgeRoleRunner for FailingRunner {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> WfResult<ForgeRoleOutcome> {
        *self.turns.lock().expect("turns lock") += 1;
        Err((self.make)())
    }
}

/// The database seam's own words for a session taken away mid-statement.
fn database_unavailable() -> WorkflowError {
    WorkflowError::unavailable("DatabaseUnavailable during workflow.step (sqlstate 25P03)")
}

/// A failure that is about the work, not the plumbing.
fn role_refusal() -> WorkflowError {
    WorkflowError::generic("architect refused: the packet names no acceptance criteria")
}

struct Outcome {
    drive: WfResult<DriveForgeStoryResult>,
    holds: Vec<(String, String)>,
    job: workflow::Job,
    turns: usize,
}

/// Drive one durable turn of a FEATURE story whose first role fails with `make()`.
fn drive_failing_turn(make: fn() -> WorkflowError) -> Outcome {
    let memory = MemoryStore::new();
    let writer = Arc::new(RecordingWriter::default());
    let rt = ForgeRuntime::from_store(
        memory.clone(),
        writer.clone(),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    )
    .expect("the XML Forge definition seeds");
    let evidence = ForgeGateEvidence {
        work_type: Some(WORK_TYPE.into()),
        scout_required: Some(false),
        ..Default::default()
    };
    let open = rt
        .wake_story(STORY, WORK_TYPE, evidence.clone())
        .expect("the story wakes")
        .open
        .expect("a role task is open");
    assert_eq!(open.node_id.as_deref(), Some(FIRST_TURN_NODE));

    let runner = FailingRunner {
        make,
        turns: Mutex::new(0),
    };
    let service = ArchitectService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&service as &dyn AbstractForgeService)
        .expect("register forge.architect");
    let jobs = WorkflowJobService::new(rt.engine());

    let drive = drive_forge_story_with_jobs(
        &rt,
        STORY,
        DriveForgeStoryOptions {
            work_type: WORK_TYPE,
            evidence,
            runner: None,
            max_steps: 1,
            worker_id: WORKER,
            within_story_concurrency: 1,
            stop_after: None,
            turn_cap: 8,
        },
        DurableForgeExecution {
            jobs: &jobs,
            registry: &registry,
        },
    );
    let job = memory
        .with_tx(|tx| tx.get_job(&open.task_id))
        .expect("the durable job row reads");
    let holds = writer.holds.lock().expect("holds lock").clone();
    let turns = *runner.turns.lock().expect("turns lock");
    Outcome {
        drive,
        holds,
        job,
        turns,
    }
}

#[test]
fn an_engine_fault_in_a_role_turn_returns_to_the_queue_and_is_not_held() {
    let out = drive_failing_turn(database_unavailable);

    let err = match out.drive {
        Ok(result) => {
            panic!("an engine fault must reach the caller, not settle as a result: {result:?}")
        }
        Err(err) => err,
    };
    assert!(
        err.is_connection_failure(),
        "the engine's own fault comes back unwrapped, so the exit path can clear the claim: {err}"
    );
    assert!(
        out.holds.is_empty(),
        "plumbing is not a human decision — no hold may be written: {:?}",
        out.holds
    );
    assert_eq!(out.turns, 1, "one turn was paid for");
    assert_eq!(
        out.job.status,
        JobStatus::Pending,
        "the job layer settled the fault retryable, and nothing above it overrode that: {:?}",
        out.job
    );
}

#[test]
fn a_role_failure_is_held_for_a_human_and_not_retried() {
    let out = drive_failing_turn(role_refusal);

    let result = out
        .drive
        .expect("a role failure is reported as a held result");
    assert!(result.needs_human, "the story is held: {result:?}");
    let reason = result.blocked_reason.unwrap_or_default();
    assert!(
        reason.contains("architect refused"),
        "the hold carries the role's own reason: {reason}"
    );
    assert_eq!(out.holds.len(), 1, "exactly one hold: {:?}", out.holds);
    assert_eq!(out.turns, 1, "one turn was paid for");
    assert_eq!(
        out.job.status,
        JobStatus::Failed,
        "a verdict is permanent; repeating it only spends the same money again"
    );
}

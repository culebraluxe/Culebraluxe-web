//! FORGE.RERUN — flipping a story back to Ready runs it again.
//!
//! The Captain reruns a story by flipping it to Ready on the board. Before 2026-10-03 the engine resumed the story's
//! live instance whatever state it was in; an instance whose open task's durable job was already terminal could
//! never move, so every rerun refused that job and held again quoting the PREVIOUS run's error (the scheduler test of
//! ENG-FORGE-C1-BUILD-INFO-01 read last night's fixed hold bug as a live one).
//!
//!   * a story whose turn FAILED is run again on a fresh instance — the dead one is cancelled, the turn runs;
//!   * a story whose turn is only RETRYING (an engine fault) is resumed, never restarted — a requeue pays nothing twice.
//!
//! Level: L1, production engine + XML + durable driver on the in-memory store.

use std::sync::{Arc, Mutex};

use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DurableForgeExecution, ForgeRoleOutcome,
    ForgeRoleRunner,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::WorkflowJobService;
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::RecordingWriter;
use forge::roles::architect::ArchitectService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use workflow::{MemoryStore, ProcessOutcome, Result as WfResult, WorkflowError};

const STORY: &str = "TST-FORGE-RERUN-001";

/// The first FEATURE turn (architect), failing with a scripted error and counting what it was paid for.
struct Architect {
    fail: fn() -> WorkflowError,
    turns: Mutex<usize>,
}

impl ForgeRoleRunner for Architect {
    fn run(&self, _: &str, _: &ActiveForgeRoleTask) -> WfResult<ForgeRoleOutcome> {
        *self.turns.lock().unwrap() += 1;
        Err((self.fail)())
    }
}

fn evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        scout_required: Some(false),
        ..Default::default()
    }
}

/// Drive the story twice — the second drive is the board flip — and report both instances and the turns paid.
fn drive_twice(fail: fn() -> WorkflowError) -> (String, String, usize, Option<ProcessOutcome>) {
    let rt = ForgeRuntime::from_store(
        MemoryStore::new(),
        Arc::new(RecordingWriter::default()),
        None,
        None,
        Arc::new(MemoryLedger::new()),
        forge_sdlc_definition(),
    )
    .expect("the production definition seeds");
    let runner = Architect {
        fail,
        turns: Mutex::new(0),
    };
    let architect = ArchitectService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&architect as &dyn AbstractForgeService)
        .expect("register forge.architect");
    let jobs = WorkflowJobService::new(rt.engine());
    let drive = |worker: &str| {
        drive_forge_story_with_jobs(
            &rt,
            STORY,
            DriveForgeStoryOptions {
                work_type: "FEATURE",
                evidence: evidence(),
                runner: None,
                max_steps: 1,
                worker_id: worker,
                split_concurrency: 1,
                stop_after: None,
                turn_cap: 8,
            },
            DurableForgeExecution {
                jobs: &jobs,
                registry: &registry,
            },
        )
    };
    let first = match drive("forge:run-1") {
        Ok(out) => out.instance_id,
        Err(_) => {
            rt.wake_story(STORY, "FEATURE", evidence())
                .expect("the live instance")
                .instance_id
        }
    };
    let second = match drive("forge:run-2") {
        Ok(out) => out.instance_id,
        Err(_) => {
            rt.wake_story(STORY, "FEATURE", evidence())
                .expect("the live instance")
                .instance_id
        }
    };
    let outcome = rt
        .engine()
        .get_process_instance(&first)
        .expect("the first instance reads")
        .outcome;
    let turns = *runner.turns.lock().unwrap();
    (first, second, turns, outcome)
}

#[test]
fn a_failed_story_flipped_to_ready_runs_again_on_a_fresh_instance() {
    let (first, second, turns, first_outcome) = drive_twice(|| {
        WorkflowError::generic("architect refused: the packet names no acceptance criteria")
    });
    assert_ne!(first, second, "a rerun of a dead instance starts a new one");
    assert_eq!(turns, 2, "the rerun actually runs the turn again");
    assert_eq!(
        first_outcome,
        Some(ProcessOutcome::Cancelled),
        "the dead instance is retired, not left active beside the new one"
    );
}

#[test]
fn a_story_whose_turn_is_retrying_is_resumed_never_restarted() {
    let (first, second, turns, first_outcome) = drive_twice(|| {
        WorkflowError::unavailable("DatabaseUnavailable during workflow.step (sqlstate 25P03)")
    });
    assert_eq!(
        first, second,
        "an engine fault's retry resumes the same instance"
    );
    assert_eq!(
        turns, 1,
        "the retry waits out its backoff; nothing is paid twice"
    );
    assert_eq!(first_outcome, None, "the instance is still live");
}

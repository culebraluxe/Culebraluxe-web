//! FORGE.FAILURE — database disconnect is classified correctly and leaves legal queue, board, run, receipt, and retry state (TST-FORGE-FAILURE-001).
//!
//! Contract: a database disconnect mid-turn is the engine's own plumbing, never the story's verdict. The
//! production classifier (`forge::engine::engine_fault::is_engine_fault_error`, shared by the job layer and the
//! lane's exit path) must read "DatabaseUnavailable"/"sqlstate 57P02" as engine fault, and
//! `execute_claimed_job_unsettled` must settle the durable job RETRYABLE (`JobStatus::Pending`, the same
//! `max_attempts` budget, no turn spent beyond the one that failed) — the legal retry state. A disconnect
//! recorded as the story's failure would falsely convict the work; a disconnect retried as work would be
//! a permanent loss of the paid turn.
//!
//! Level: L1 on the in-memory engine — production JobService over the production in-memory engine.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_failure__001__database_disconnect_is_classified_correctly_and_leaves_legal_queue_board_run_receipt_and

#[path = "support/forge_job.rs"]
mod support;

use std::sync::Mutex;

use forge::engine::engine_fault::is_engine_fault_error;
use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::job::{execute_claimed_job_unsettled, JobService};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::smith::SmithService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use support::{enqueue_ready, harness, jobs, ready, WORKER_A};
use workflow::{JobStatus, WorkflowError};

/// A Smith turn that always fails with the database-disconnect error, counting paid turns.
struct Disconnecting {
    turns: Mutex<usize>,
}

impl ForgeRoleRunner for Disconnecting {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        *self.turns.lock().expect("turns") += 1;
        Err(WorkflowError::unavailable(
            "db: DatabaseUnavailable during workflow.step (sqlstate 57P02 idle_in_transaction_session_timeout)",
        ))
    }
}

/// Drive one READY Smith job whose turn fails with the disconnect, and report how the production job
/// layer settled it.
fn settle_disconnect() -> (JobStatus, i32, usize) {
    let harness = harness();
    let engine = harness.engine();
    let runner = Disconnecting {
        turns: Mutex::new(0),
    };
    let smith = SmithService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&smith as &dyn AbstractForgeService)
        .expect("register smith");
    let task = ready("t-smith", "smith");
    let id = enqueue_ready(engine, &registry, &task);
    let service = jobs(engine);
    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    let outcome = execute_claimed_job_unsettled(&service, WORKER_A, &lease, &task, &registry);
    assert!(outcome.is_err(), "the failure reaches the caller");
    let state = service.inspect(&id).expect("inspect");
    let turns = *runner.turns.lock().expect("turns");
    (state.status, state.attempts, turns)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-FAILURE-001).
fn forge_failure_001__database_disconnect_is_classified_correctly_and_leaves_legal_queue_board_run_receipt_and(
) {
    // 1. The classifier's verdict: the disconnect is the engine's, never the story's.
    assert!(
        is_engine_fault_error(&WorkflowError::unavailable(
            "db: DatabaseUnavailable during workflow.step (sqlstate 57P02)"
        )),
        "DatabaseUnavailable / sqlstate 57P02 must classify as engine fault"
    );

    // 2. The durable consequence: the job goes back to Pending (retryable) inside the same budget,
    //    with exactly one turn paid for the error that produced it.
    let (status, attempts, turns) = settle_disconnect();
    assert_eq!(
        status,
        JobStatus::Pending,
        "an engine fault is retried inside the budget, never recorded as the story's failure"
    );
    assert_eq!((attempts, turns), (1, 1), "one attempt, one turn spent");

    // 3. The verdict keeps a legal shape for the retry lane: the same classifier must NOT clear a
    //    real work failure just for naming a database word in prose.
    assert!(
        !is_engine_fault_error(&WorkflowError::generic(
            "Smith could not build: error[E0308] mismatched types (mentions database later)"
        )),
        "a compile error that merely talks about the database stays the story's failure"
    );
}

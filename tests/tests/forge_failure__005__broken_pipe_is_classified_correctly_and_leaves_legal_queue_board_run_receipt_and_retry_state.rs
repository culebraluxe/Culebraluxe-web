//! FORGE.FAILURE — broken pipe is classified correctly and leaves legal queue, board, run, receipt, and retry state (TST-FORGE-FAILURE-005).
//!
//! Contract: the durable job layer must classify `broken pipe` with the SAME production rule the lane's exit
//! path uses (`forge::engine::engine_fault::is_engine_fault_error`), and `execute_claimed_job_unsettled`
//! must settle it into legal state — never silently dropped, never a story verdict invented.
//!
//! `broken pipe` is the engine's own plumbing: the classifier must return `true`, and the durable
//! job must go back to `JobStatus::Pending` — retryable inside the same `max_attempts` budget,
//! with exactly one turn paid for the error that produced it.
//!
//! Level: L1 on the in-memory engine — production JobService over the production in-memory engine.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_failure__005__broken_pipe_is_classified_correctly_and_leaves_legal_queue_board_run_receipt_and_retry_state

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

/// A Smith turn that always fails with the broken pipe error, counting paid turns.
struct Failing {
    turns: Mutex<usize>,
}

impl ForgeRoleRunner for Failing {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        *self.turns.lock().expect("turns") += 1;
        Err(WorkflowError::generic(
            "io error: Broken pipe (os error 32)",
        ))
    }
}

/// Drive one READY Smith job whose turn fails with broken pipe, and report how the production job layer
/// settled it.
fn settle_failure() -> (JobStatus, i32, usize) {
    let harness = harness();
    let engine = harness.engine();
    let runner = Failing {
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-FAILURE-005).
fn forge_failure_005__broken_pipe_is_classified_correctly_and_leaves_legal_queue_board_run_receipt_and_retry_state(
) {
    // 1. The classifier's verdict.
    assert_eq!(
        is_engine_fault_error(&WorkflowError::generic(
            "io error: Broken pipe (os error 32)"
        )),
        true,
        "broken pipe must classify as engine fault"
    );

    // 2. The durable consequence.
    let (status, attempts, turns) = settle_failure();
    assert_eq!(status, JobStatus::Pending, "legal durable state");
    assert_eq!((attempts, turns), (1, 1), "one attempt, one turn spent");

    // 3. The classifier agrees with the production split on the paired case.
    assert_eq!(
        is_engine_fault_error(&WorkflowError::generic(
            "QA verdict: FAIL - weak test, no negative case"
        )),
        false,
        "the production split is the law on both sides"
    );
}

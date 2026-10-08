//! FORGE.FAILURE — hold write failure is classified correctly and leaves legal queue, board, run, receipt, and retry state (TST-FORGE-FAILURE-011).
//!
//! Contract: the durable job layer must classify `hold write failure` with the SAME production rule the lane's exit
//! path uses (`forge::engine::engine_fault::is_engine_fault_error`), and `execute_claimed_job_unsettled`
//! must settle it into legal state — never silently dropped, never a story verdict invented.
//!
//! `hold write failure` is NOT the engine's plumbing vocabulary: the classifier must return `false`, and the
//! durable job must settle permanently `JobStatus::Failed` — recorded against the story, never
//! silently retried as infrastructure.
//!
//! Level: L4 Adversarial — production JobService over the production in-memory engine with fault injection.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_failure__011__hold_write_failure_is_classified_correctly_and_leaves_legal_queue_board_run_receipt_and
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

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

/// A Smith turn that always fails with the hold write failure error, counting paid turns.
struct FailingHoldWrite {
    turns: Mutex<usize>,
}

impl ForgeRoleRunner for FailingHoldWrite {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        *self.turns.lock().expect("turns") += 1;
        Err(WorkflowError::generic(
            "hold write failed: db error: insert forge_hold_record (deadlock detected)",
        ))
    }
}

/// Drive one READY Smith job whose turn fails with hold write failure, and report how the production job layer
/// settled it.
fn settle_hold_write_failure() -> (JobStatus, i32, usize) {
    let harness = harness();
    let engine = harness.engine();
    let runner = FailingHoldWrite {
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
    let outcome = execute_claimed_job_unsettled(&service, WORKER_A, &lease, &task, &registry, None, None);
    assert!(outcome.is_err(), "the failure reaches the caller");
    let state = service.inspect(&id).expect("inspect");
    let turns = *runner.turns.lock().expect("turns");
    (state.status, state.attempts, turns)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-FAILURE-011).
fn forge_failure_011__hold_write_failure_is_classified_correctly_and_leaves_legal_queue_board_run_receipt_and(
) {
    // 1. The classifier's verdict.
    assert_eq!(
        is_engine_fault_error(&WorkflowError::generic(
            "hold write failed: db error: insert forge_hold_record (deadlock detected)"
        )),
        false,
        "hold write failure must classify as work failure"
    );

    // 2. The durable consequence.
    let (status, attempts, turns) = settle_hold_write_failure();
    assert_eq!(status, JobStatus::Failed, "legal durable state");
    assert_eq!((attempts, turns), (1, 1), "one attempt, one turn spent");

    // 3. The classifier agrees with the production split on the paired case.
    assert_eq!(
        is_engine_fault_error(&WorkflowError::generic(
            "sqlstate 57P02 terminating connection"
        )),
        true,
        "the production split is the law on both sides"
    );
}
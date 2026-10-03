//! FORGE.JOB — a READY role task enqueues, and enqueues once.
//!
//! Contract 1 (READY is required): only a workflow role task in `Ready` becomes a durable `forge.role` job. A task
//! that is created, reserved, in progress, completed, failed, exited or obsolete is refused by `ForgeJobBridge::job_for_ready_task` BEFORE
//! anything durable is written, and so is a task whose node carries no service binding (a human task).
//!
//! Contract 1b (enqueue is idempotent per workflow task) — **was GAP-1.** Before the fix, `WorkflowJobService::enqueue`
//! always inserted a new row; nothing keyed the durable job to the workflow task it was made from, so a driver that
//! re-scanned READY tasks after a restart (crash window A/D) wrote a second job for the same task, and both
//! were claimable: the role ran twice and a paid model turn was spent twice. CLOSED 2026-10-02 by 6fb6c659 (enqueue
//! idempotent by workflow task) on 2a0277bb (stable job ids); the contract below now runs in the suite.
//!
//! Level: L1 Unit-of-infrastructure, harness EngineHarness (in-memory store, fixed clock).

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{ForgeJobBridge, JobService, FORGE_ROLE_JOB_TYPE};
use support::*;
use workflow::{JobStatus, TaskStatus};

#[test]
fn only_a_ready_task_becomes_a_job_and_nothing_durable_is_written_otherwise() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let bridge = ForgeJobBridge::new(&registry);

    for status in [
        TaskStatus::Reserved,
        TaskStatus::InProgress,
        TaskStatus::Created,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Exited,
        TaskStatus::Obsolete,
    ] {
        let refused = bridge.job_for_ready_task(&task("t-not-ready", "fast_smith", status));
        assert!(refused.is_err(), "{status:?} must not become a job");
    }
    assert!(
        bridge
            .job_for_ready_task(&ready("t-human", "hold"))
            .is_err(),
        "a human task has no service binding and must not become an agent job"
    );
    assert!(
        engine.jobs_for_instance(INSTANCE).expect("jobs").is_empty(),
        "a refused task leaves no durable trace"
    );
}

#[test]
fn a_ready_task_becomes_exactly_one_pending_forge_role_job() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();

    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let job = job(engine, &id);
    assert_eq!(job.job_type, FORGE_ROLE_JOB_TYPE);
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.attempts, 0, "enqueue is not a claim");
    assert_eq!(job.process_instance_id.as_deref(), Some(INSTANCE));
    assert_eq!(open_jobs_for_task(engine, "t-1").len(), 1);
    assert!(runners.all_calls().is_empty(), "enqueue executes nothing");
}

/// Crash window A: enqueue, then the process dies before any claim. The job is durable, so a worker started later
/// claims it — nothing about the job depended on the process that wrote it.
#[test]
fn a_job_enqueued_by_a_process_that_died_is_claimed_by_the_next_worker() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();

    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    // "Process dies": nothing but the store survives. A fresh JobService over the same store claims it.
    let leases = jobs(engine).claim(WORKER_B, 10).expect("claim");
    assert_eq!(leases.len(), 1);
    assert_eq!(leases[0].job_id, id);
}

/// GAP-1 (duplicate enqueue / replay) — CLOSED by 6fb6c659 (role-job enqueue idempotent by task) on 2a0277bb (stable job ids).
#[test]
fn enqueueing_the_same_ready_task_twice_leaves_one_job() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let t = ready("t-1", "fast_smith");

    let first = enqueue_ready(engine, &registry, &t);
    // The driver restarts and re-scans the same READY task (crash window A, or a plain second tick).
    let second = enqueue_ready(engine, &registry, &t);

    assert_eq!(
        first, second,
        "the same workflow task must map to the same durable job"
    );
    assert_eq!(
        open_jobs_for_task(engine, "t-1").len(),
        1,
        "two claimable jobs for one task is a double execution"
    );
}

/// The paid-execution half of GAP-1: however many times one READY task is enqueued, and however many workers then
/// claim, the role runs once.
#[test]
fn one_workflow_task_is_never_two_paid_executions() {
    use forge::engine::job::execute_claimed_job;

    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let t = ready("t-1", "fast_smith");
    let service = jobs(engine);

    for _ in 0..3 {
        enqueue_ready(engine, &registry, &t);
    }
    for worker in [WORKER_A, WORKER_B, "worker-c"] {
        for lease in service.claim(worker, 10).expect("claim") {
            execute_claimed_job(&service, worker, &lease, &t, &registry).expect("executes");
        }
    }
    // A restart after completion re-scans the same task once more.
    enqueue_ready(engine, &registry, &t);
    for lease in service.claim("worker-d", 10).expect("claim") {
        execute_claimed_job(&service, "worker-d", &lease, &t, &registry).expect("executes");
    }

    assert_eq!(
        runners.all_calls(),
        vec!["smith:fast_smith".to_string()],
        "one workflow task, one paid execution"
    );
}

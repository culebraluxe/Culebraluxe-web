//! FORGE.JOB — completion is terminal, and the crash windows around it.
//!
//! Contract 8: once a job is `Completed` it is never claimed again (batch or exact), stale recovery never revives it,
//! a repeated completion is a no-op, and handing the same lease to the worker again does not execute the role a
//! second time (`execute_claimed_job` renews the lease before executing, and a completed job has no lease to renew).
//!
//! Crash window C (service executed → process dies before `complete`): PINNED, AT-LEAST-ONCE. The job is still
//! `Locked`, the lease lapses, `recover_stale` returns it to `Pending`, and the next worker executes the role AGAIN.
//! Nothing in the job layer can know the first execution happened, because job completion and the role's effects are
//! not one transaction. The guard against a second paid turn is the role itself (or the workflow task having moved
//! on, which `execute_claimed_job` checks). This is reported as a risk, not a defect of the claim mechanics.
//!
//! Crash window D (job completed → process dies before the workflow task completes) — **was GAP-1, CLOSED by 6fb6c659.** The
//! workflow task is still READY and the driver re-scans it; before the fix `enqueue` wrote a new claimable job and the
//! role ran again. Enqueue is now idempotent by workflow task, so the re-scan finds the settled job; see `forge_job__001`.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{execute_claimed_job, JobService};
use forge::roles::AbstractForgeService;
use support::*;
use workflow::{JobStatus, JOB_LEASE_MS};

#[test]
fn a_completed_job_is_never_claimed_recovered_or_executed_again() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let t = ready("t-1", "fast_smith");
    let id = enqueue_ready(engine, &registry, &t);
    let service = jobs(engine);

    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    execute_claimed_job(&service, WORKER_A, &lease, &t, &registry).expect("executes");
    assert_eq!(runners.smith_runner.calls(), vec!["fast_smith".to_string()]);
    assert_status(engine, &id, JobStatus::Completed);

    assert!(service.claim(WORKER_B, 10).expect("claim").is_empty());
    assert!(service.claim_one(&id, WORKER_B).is_err());
    harness.clock().advance_millis(10 * JOB_LEASE_MS);
    assert_eq!(service.recover_stale(100).expect("recover"), 0);

    service
        .complete(&id, WORKER_A)
        .expect("a repeated completion is a no-op");
    service
        .complete(&id, WORKER_B)
        .expect("a stranger's completion of a settled job is a no-op");
    assert!(
        execute_claimed_job(&service, WORKER_A, &lease, &t, &registry).is_err(),
        "the old lease cannot execute the role again"
    );
    assert_eq!(
        runners.all_calls(),
        vec!["smith:fast_smith".to_string()],
        "the role ran exactly once"
    );
    assert_status(engine, &id, JobStatus::Completed);
}

/// Crash window C, pinned: the job layer guarantees AT-LEAST-ONCE execution, not exactly-once.
#[test]
fn a_crash_between_execute_and_complete_re_executes_the_role() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let t = ready("t-1", "fast_smith");
    let id = enqueue_ready(engine, &registry, &t);
    let service = jobs(engine);

    // Worker A runs the role, then dies before `complete` (the role's effects happened; the job never heard).
    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    owned
        .smith
        .execute(&lease.node_id, &t)
        .expect("the role executed");

    harness.clock().advance_millis(JOB_LEASE_MS + 1);
    assert_eq!(service.recover_stale(10).expect("recover"), 1);
    let retry = service.claim(WORKER_B, 1).expect("claim").remove(0);
    assert_eq!(retry.job_id, id);
    execute_claimed_job(&service, WORKER_B, &retry, &t, &registry).expect("executes");

    assert_eq!(
        runners.smith_runner.calls().len(),
        2,
        "PINNED: the role runs twice; idempotency of the effect belongs to the role/workflow, not the queue"
    );
    assert_status(engine, &id, JobStatus::Completed);
}

/// Crash window D — CLOSED with GAP-1 by 6fb6c659 (idempotent by task).
#[test]
fn a_task_whose_job_completed_is_not_enqueued_again() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let t = ready("t-1", "fast_smith");
    enqueue_ready(engine, &registry, &t);
    let service = jobs(engine);
    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    execute_claimed_job(&service, WORKER_A, &lease, &t, &registry).expect("executes");

    // The process dies before the workflow task is completed: the driver restarts and sees `t-1` READY again.
    enqueue_ready(engine, &registry, &t);

    assert!(
        open_jobs_for_task(engine, "t-1").is_empty(),
        "a task whose job already completed must not get a second claimable job"
    );
}

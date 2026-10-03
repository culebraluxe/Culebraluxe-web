//! FORGE.JOB — cancellation is terminal, whether the job was pending or already leased.
//!
//! Contract 9: a cancelled job is never claimed, never recovered, never requeued, and — the case that matters — a
//! worker that already holds a lease when the job is cancelled does NOT go on to execute the role:
//! `execute_claimed_job` renews the lease immediately before executing, and the renewal of a cancelled job is refused.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{execute_claimed_job, JobService};
use support::*;
use workflow::{JobStatus, JOB_LEASE_MS};

#[test]
fn a_cancelled_pending_job_is_never_claimed_recovered_or_requeued() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);

    service.cancel(&id, "operator").expect("cancel");
    assert_status(engine, &id, JobStatus::Cancelled);
    assert!(service.claim(WORKER_A, 10).expect("claim").is_empty());
    assert!(service.claim_one(&id, WORKER_A).is_err());
    harness.clock().advance_millis(10 * JOB_LEASE_MS);
    assert_eq!(service.recover_stale(10).expect("recover"), 0);
    assert!(
        service.requeue(&id, "operator").is_err(),
        "only a failed job is requeueable"
    );
    service
        .cancel(&id, "operator")
        .expect("cancelling twice is a no-op");
    assert_status(engine, &id, JobStatus::Cancelled);
    assert!(runners.all_calls().is_empty());
}

#[test]
fn a_job_cancelled_while_leased_is_not_executed_by_its_lease_holder() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let t = ready("t-1", "fast_smith");
    let id = enqueue_ready(engine, &registry, &t);
    let service = jobs(engine);

    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    service
        .cancel(&id, "operator")
        .expect("cancel a locked job");

    assert!(
        execute_claimed_job(&service, WORKER_A, &lease, &t, &registry).is_err(),
        "the lease holder must not run a cancelled job"
    );
    assert!(runners.all_calls().is_empty(), "no role executed");
    assert_status(engine, &id, JobStatus::Cancelled);
    let row = job(engine, &id);
    assert_eq!(row.locked_by, None, "cancellation released the lease");

    harness.clock().advance_millis(JOB_LEASE_MS + 1);
    assert_eq!(service.recover_stale(10).expect("recover"), 0);
    assert!(service.claim(WORKER_B, 10).expect("claim").is_empty());
}

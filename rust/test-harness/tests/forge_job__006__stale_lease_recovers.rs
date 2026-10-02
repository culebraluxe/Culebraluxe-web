//! FORGE.JOB — an expired lease is recovered as the SAME job, and a live one is left alone.
//!
//! Contract 6: when a lease expires, `recover_stale` moves the job `Locked → Pending` and it becomes claimable by
//! another worker. It is the same durable job id — recovery does not insert a copy — and the original worker can no
//! longer complete or fail it. A lease that has not expired is never recovered.
//!
//! Crash window B (claim → process dies before execute) is exactly this path: nothing executed, the lease lapses, the
//! next worker claims the same job and the attempt count says it is the second attempt. Owner: `recover_stale`,
//! which the worker host must run; nothing recovers a lease by itself.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::JobService;
use support::*;
use workflow::{JobStatus, JOB_LEASE_MS};

#[test]
fn an_expired_lease_is_recovered_as_the_same_job_and_reclaimed_by_another_worker() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);

    // Crash window B: A claims, then dies before executing anything.
    service.claim(WORKER_A, 1).expect("A claims");
    harness.clock().advance_millis(JOB_LEASE_MS + 1);

    assert_eq!(service.recover_stale(10).expect("recover"), 1);
    let row = job(engine, &id);
    assert_eq!(row.status, JobStatus::Pending);
    assert_eq!(row.locked_by, None);
    assert_eq!(row.locked_until, None);
    assert_eq!(
        open_jobs_for_task(engine, "t-1").len(),
        1,
        "recovered, not duplicated"
    );

    let lease = service.claim(WORKER_B, 10).expect("B claims").remove(0);
    assert_eq!(lease.job_id, id, "the same durable job");
    assert_eq!(lease.attempts, 2, "the second attempt");
    assert!(
        runners.all_calls().is_empty(),
        "nothing executed during any of this"
    );

    // The dead worker's late completion or failure cannot settle B's attempt.
    assert!(service.complete(&id, WORKER_A).is_err());
    assert!(service.fail(&id, WORKER_A, "late", true).is_err());
    assert_eq!(job(engine, &id).locked_by.as_deref(), Some(WORKER_B));
}

#[test]
fn a_lease_that_has_not_expired_is_never_recovered() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);
    service.claim(WORKER_A, 1).expect("claim");

    harness.clock().advance_millis(JOB_LEASE_MS - 1);
    assert_eq!(service.recover_stale(10).expect("recover"), 0);
    harness.clock().advance_millis(1);
    assert_eq!(
        service.recover_stale(10).expect("recover"),
        0,
        "at exactly locked_until the lease is still alive"
    );
    let row = job(engine, &id);
    assert_eq!(row.status, JobStatus::Locked);
    assert_eq!(row.locked_by.as_deref(), Some(WORKER_A));
}

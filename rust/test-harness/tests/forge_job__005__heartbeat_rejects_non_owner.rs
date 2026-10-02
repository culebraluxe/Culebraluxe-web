//! FORGE.JOB — a heartbeat is an ownership check, not an advisory timestamp.
//!
//! Contract 5 (refusals): a heartbeat from a worker that does not hold the lease is refused and moves nothing; a
//! heartbeat on a job that is not locked (pending) or is settled (completed, failed, cancelled) is refused and cannot
//! revive it; and a worker whose stale lease was recovered and re-claimed by another worker cannot heartbeat its way
//! back into ownership.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::JobService;
use support::*;
use workflow::{JobStatus, JOB_LEASE_MS};

#[test]
fn a_non_owner_heartbeat_is_refused_and_moves_nothing() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);
    service.claim(WORKER_A, 1).expect("claim");
    let before = job(engine, &id);

    harness.clock().advance_millis(1_000);
    assert!(
        service.heartbeat(&id, WORKER_B).is_err(),
        "B does not own the lease"
    );
    let after = job(engine, &id);
    assert_eq!(
        after.locked_until, before.locked_until,
        "the lease did not move"
    );
    assert_eq!(after.locked_by.as_deref(), Some(WORKER_A));
}

#[test]
fn a_heartbeat_on_a_pending_or_settled_job_is_refused_and_revives_nothing() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let service = jobs(engine);

    let pending = enqueue_ready(engine, &registry, &ready("t-pending", "fast_smith"));
    assert!(
        service.heartbeat(&pending, WORKER_A).is_err(),
        "pending is not leased"
    );
    assert_status(engine, &pending, JobStatus::Pending);

    let completed = enqueue_ready(engine, &registry, &ready("t-completed", "fast_smith"));
    service.claim_one(&completed, WORKER_A).expect("claim");
    service.complete(&completed, WORKER_A).expect("complete");

    let failed = enqueue_ready(engine, &registry, &ready("t-failed", "fast_smith"));
    service.claim_one(&failed, WORKER_A).expect("claim");
    service.fail(&failed, WORKER_A, "boom", true).expect("fail");

    let cancelled = enqueue_ready(engine, &registry, &ready("t-cancelled", "fast_smith"));
    service.claim_one(&cancelled, WORKER_A).expect("claim");
    service.cancel(&cancelled, "operator").expect("cancel");

    for (id, status) in [
        (&completed, JobStatus::Completed),
        (&failed, JobStatus::Failed),
        (&cancelled, JobStatus::Cancelled),
    ] {
        assert!(
            service.heartbeat(id, WORKER_A).is_err(),
            "{status:?} must refuse a heartbeat"
        );
        let row = job(engine, id);
        assert_eq!(row.status, status, "a refused heartbeat revives nothing");
        assert_eq!(row.locked_by, None);
    }
}

#[test]
fn a_worker_whose_lease_was_recovered_cannot_heartbeat_back_into_ownership() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);
    service.claim(WORKER_A, 1).expect("A claims");

    harness.clock().advance_millis(JOB_LEASE_MS + 1);
    assert_eq!(service.recover_stale(10).expect("recover"), 1);
    assert!(
        service.heartbeat(&id, WORKER_A).is_err(),
        "after recovery A owns nothing (job is pending)"
    );
    service.claim(WORKER_B, 1).expect("B claims");
    assert!(
        service.heartbeat(&id, WORKER_A).is_err(),
        "after B's claim A still owns nothing"
    );
    assert_eq!(job(engine, &id).locked_by.as_deref(), Some(WORKER_B));
}

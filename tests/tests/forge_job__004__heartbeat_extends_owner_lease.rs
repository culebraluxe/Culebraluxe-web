//! FORGE.JOB — the owner's heartbeat extends its lease, and changes nothing else.
//!
//! Contract 5 (owner side): a heartbeat from the worker holding the lease moves `locked_until` to `now + lease` and
//! returns that instant, while ownership, status and the attempt count stay exactly as the claim left them. A lease
//! kept alive by heartbeats is not recovered as stale. The non-owner and settled-job refusals are `forge_job__005`.
//!
//! Level: L1, harness EngineHarness (fixed clock, advanced explicitly).

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::JobService;
use support::*;
use workflow::{JobStatus, JOB_LEASE_MS};

#[test]
fn the_owners_heartbeat_moves_the_lease_and_nothing_else() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);

    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    assert_eq!(lease.locked_until, Some(START_MS + JOB_LEASE_MS));

    harness.clock().advance_millis(60_000);
    let renewed = service.heartbeat(&id, WORKER_A).expect("owner heartbeat");
    assert_eq!(renewed, START_MS + 60_000 + JOB_LEASE_MS);

    let row = job(engine, &id);
    assert_eq!(
        row.locked_until,
        Some(renewed),
        "the returned instant is the stored lease"
    );
    assert_eq!(
        row.locked_by.as_deref(),
        Some(WORKER_A),
        "ownership unchanged"
    );
    assert_eq!(row.status, JobStatus::Locked);
    assert_eq!(row.attempts, 1, "a heartbeat is not a claim");
}

#[test]
fn a_lease_kept_alive_by_heartbeats_is_never_recovered_as_stale() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);
    service.claim(WORKER_A, 1).expect("claim");

    // Ten lease-lengths of wall time, heartbeating at half a lease.
    for _ in 0..20 {
        harness.clock().advance_millis(JOB_LEASE_MS / 2);
        service.heartbeat(&id, WORKER_A).expect("heartbeat");
        assert_eq!(service.recover_stale(100).expect("recover"), 0);
    }
    let row = job(engine, &id);
    assert_eq!(row.locked_by.as_deref(), Some(WORKER_A));
    assert_eq!(row.attempts, 1);
}

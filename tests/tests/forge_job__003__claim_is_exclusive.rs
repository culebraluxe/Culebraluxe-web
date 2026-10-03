//! FORGE.JOB — exactly one worker owns a claim.
//!
//! Contract 4: given one pending role job and two workers racing for it, exactly one receives a lease and the other
//! receives no executable copy — through the batch claim (`claim`) and through the exact claim (`claim_one`). The
//! race is real: two OS threads released together by a barrier against one engine, repeated many times so an
//! interleaving that hands out two leases has many chances to appear. The in-memory store serializes transactions the
//! way Neon's `FOR UPDATE SKIP LOCKED` does; the Neon predicate itself is covered by the workflow crate's store tests.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use std::sync::Barrier;

use forge::engine::job::JobService;
use support::*;
use workflow::JobStatus;

const ROUNDS: usize = 200;

#[test]
fn two_workers_racing_the_batch_claim_get_one_lease_between_them() {
    for round in 0..ROUNDS {
        let harness = harness();
        let engine = harness.engine();
        let runners = Services::new();
        let owned = Owned::new(&runners);
        let registry = owned.registry();
        let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
        let service = jobs(engine);
        let barrier = Barrier::new(2);

        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                service.claim(WORKER_A, 10).expect("claim A")
            });
            let b = scope.spawn(|| {
                barrier.wait();
                service.claim(WORKER_B, 10).expect("claim B")
            });
            (a.join().expect("A"), b.join().expect("B"))
        });

        assert_eq!(a.len() + b.len(), 1, "round {round}: exactly one lease");
        let winner = if a.len() == 1 { WORKER_A } else { WORKER_B };
        let row = job(engine, &id);
        assert_eq!(row.status, JobStatus::Locked);
        assert_eq!(row.locked_by.as_deref(), Some(winner), "round {round}");
        assert_eq!(row.attempts, 1, "round {round}: one claim, one attempt");
    }
}

#[test]
fn two_workers_racing_the_exact_claim_get_one_lease_between_them() {
    for round in 0..ROUNDS {
        let harness = harness();
        let engine = harness.engine();
        let runners = Services::new();
        let owned = Owned::new(&runners);
        let registry = owned.registry();
        let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
        let service = jobs(engine);
        let barrier = Barrier::new(2);

        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                service.claim_one(&id, WORKER_A)
            });
            let b = scope.spawn(|| {
                barrier.wait();
                service.claim_one(&id, WORKER_B)
            });
            (a.join().expect("A"), b.join().expect("B"))
        });

        assert_eq!(
            usize::from(a.is_ok()) + usize::from(b.is_ok()),
            1,
            "round {round}: exactly one exact claim succeeds"
        );
        assert_eq!(job(engine, &id).attempts, 1, "round {round}");
    }
}

#[test]
fn a_locked_job_is_not_offered_to_a_second_worker() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let id = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let service = jobs(engine);

    assert_eq!(service.claim(WORKER_A, 10).expect("A").len(), 1);
    assert!(service.claim(WORKER_B, 10).expect("B").is_empty());
    assert!(service.claim_one(&id, WORKER_B).is_err());
    assert_eq!(job(engine, &id).locked_by.as_deref(), Some(WORKER_A));
}

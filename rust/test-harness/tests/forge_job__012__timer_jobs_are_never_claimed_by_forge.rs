//! FORGE.JOB — timer jobs and Forge role jobs never cross, in either direction.
//!
//! Contract 3 (both sides of the shared `jobs` table):
//!   * a Forge worker never claims, completes, fails or heartbeats a `timer` job (`forge_job__002` covers the claim
//!     filter against arbitrary types);
//!   * the typed timer claim (`claim_jobs_by_type("timer")`) never returns a `forge.role` job;
//!   * stale recovery is type-blind (it returns both kinds to `Pending`), but afterwards each side still claims only
//!     its own type.
//!
//! **GAP-3 (the timer worker is NOT type-isolated) — ignored test.** `WorkflowEngine::run_due_jobs` — the timer
//! worker, called by `forge::engine::re_runtime::run_due_jobs` — claims with the UNTYPED `claim_jobs`. Given a pending
//! `forge.role` job it claims it, finds no executor ("no executor registered for job type 'forge.role'"), and fails it
//! as transient: the Forge job loses an attempt and sits behind a backoff on every timer tick until its budget is gone
//! (and then GAP-2 strands it). Wherever the timer worker and Forge share the jobs table, the timer side steals Forge
//! work.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::JobService;
use support::*;
use workflow::{JobStatus, Value, JOB_LEASE_MS};

const TIMER_WORKER: &str = "timer-worker";

#[test]
fn the_typed_timer_claim_never_returns_a_forge_role_job() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let forge_job = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let timer = raw_job(engine, "timer", Value::object());

    let claimed = engine
        .claim_jobs_by_type(TIMER_WORKER, "timer", 100)
        .expect("timer claim");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, timer);
    let row = job(engine, &forge_job);
    assert_eq!(
        (row.status, row.attempts),
        (JobStatus::Pending, 0),
        "forge job untouched"
    );
}

#[test]
fn a_forge_worker_cannot_settle_or_renew_a_timer_job() {
    let harness = harness();
    let engine = harness.engine();
    let timer = raw_job(engine, "timer", Value::object());
    let service = jobs(engine);

    assert!(
        service.heartbeat(&timer, WORKER_A).is_err(),
        "not locked by the forge worker"
    );
    assert!(service.complete(&timer, WORKER_A).is_err());
    assert!(service.fail(&timer, WORKER_A, "x", true).is_err());

    engine
        .claim_jobs_by_type(TIMER_WORKER, "timer", 1)
        .expect("timer worker claims");
    assert!(
        service.heartbeat(&timer, WORKER_A).is_err(),
        "owned by the timer worker"
    );
    assert!(service.complete(&timer, WORKER_A).is_err());
    let row = job(engine, &timer);
    assert_eq!(row.status, JobStatus::Locked);
    assert_eq!(row.locked_by.as_deref(), Some(TIMER_WORKER));
}

#[test]
fn after_type_blind_recovery_each_side_still_claims_only_its_own_type() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let forge_job = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));
    let timer = raw_job(engine, "timer", Value::object());
    let service = jobs(engine);

    service.claim(WORKER_A, 10).expect("forge claim");
    engine
        .claim_jobs_by_type(TIMER_WORKER, "timer", 10)
        .expect("timer claim");
    harness.clock().advance_millis(JOB_LEASE_MS + 1);
    assert_eq!(
        service.recover_stale(10).expect("recover"),
        2,
        "recovery is type-blind"
    );

    let forge_leases = service.claim(WORKER_B, 10).expect("forge claim");
    assert_eq!(forge_leases.len(), 1);
    assert_eq!(forge_leases[0].job_id, forge_job);
    let timers = engine
        .claim_jobs_by_type(TIMER_WORKER, "timer", 10)
        .expect("timer claim");
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].id, timer);
}

/// GAP-3. Ignored so the suite stays green; it FAILS today.
#[test]
#[ignore = "GAP-3: WorkflowEngine::run_due_jobs claims untyped and fails forge.role jobs as 'no executor'"]
fn the_timer_worker_never_touches_a_forge_role_job() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let forge_job = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));

    engine
        .run_due_jobs(TIMER_WORKER, 10)
        .expect("timer worker tick");

    let row = job(engine, &forge_job);
    assert_eq!(row.status, JobStatus::Pending, "still pending");
    assert_eq!(
        row.attempts, 0,
        "the timer worker must not spend a Forge attempt"
    );
    assert_eq!(
        row.last_error, None,
        "and must not record a failure against it"
    );
    assert_eq!(
        jobs(engine).claim(WORKER_A, 1).expect("claim").len(),
        1,
        "claimable now"
    );
}

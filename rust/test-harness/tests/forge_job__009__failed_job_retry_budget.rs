//! FORGE.JOB — attempts are counted once per claim, and retry stops at `max_attempts`.
//!
//! Contract 7:
//!   * every successful claim increments `attempts` by exactly one, and nothing else does (not a heartbeat, not a
//!     failure, not recovery);
//!   * a transient failure (`permanent = false`) returns the job to `Pending` behind a backoff, so it is not claimable
//!     again at once; a permanent failure terminalizes it at once;
//!   * on the `max_attempts`-th failure the job is `Failed` and never claimed again — no unlimited retry;
//!   * a ROLE error is a verdict, not an infrastructure retry: `execute_claimed_job` fails the job permanently;
//!   * only an operator `requeue` of a `Failed` job resets the budget.
//!
//! **GAP-2 (budget exhausted by crashes is never terminalized) — ignored test.** When the attempts run out through
//! lease expiry rather than through `fail`, `recover_stale` puts the job back to `Pending` with
//! `attempts == max_attempts`, and every claim path skips it (`attempts < max_attempts`). The job is then pending
//! forever: not executable, but never `Failed`, so nothing reports it and no operator `requeue` (Failed-only) applies.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{execute_claimed_job, JobService, WorkflowJobService};
use forge::engine::runtime::ActiveForgeRoleTask;
use support::*;
use workflow::{JobStatus, MemoryStore, JOB_LEASE_MS};

const HOUR: i64 = 60 * 60 * 1000;

fn budget_of_three(
    engine: &test_harness::engine::TestEngine,
) -> WorkflowJobService<'_, MemoryStore> {
    WorkflowJobService::new(engine).with_max_attempts(3)
}

#[test]
fn transient_failures_retry_behind_a_backoff_until_the_budget_is_spent() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let service = budget_of_three(engine);
    let request = forge::engine::job::ForgeJobBridge::new(&registry)
        .job_for_ready_task(&ready("t-1", "fast_smith"))
        .expect("request");
    let id = service.enqueue(&request).expect("enqueue");
    assert_eq!(job(engine, &id).max_attempts, 3);

    for attempt in 1..=3 {
        let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
        assert_eq!(lease.attempts, attempt, "one claim, one attempt");
        service.heartbeat(&id, WORKER_A).expect("heartbeat");
        assert_eq!(
            job(engine, &id).attempts,
            attempt,
            "a heartbeat is not an attempt"
        );
        service
            .fail(&id, WORKER_A, "transient", false)
            .expect("fail");
        let row = job(engine, &id);
        assert_eq!(row.attempts, attempt, "a failure is not an attempt");
        if attempt < 3 {
            assert_eq!(row.status, JobStatus::Pending);
            assert!(
                row.due_at > harness.now_millis(),
                "retry waits behind a backoff"
            );
            assert!(
                service.claim(WORKER_A, 1).expect("claim").is_empty(),
                "not claimable before the backoff elapses"
            );
            harness.clock().advance_millis(HOUR);
        }
    }

    let row = job(engine, &id);
    assert_eq!(row.status, JobStatus::Failed, "the third failure ends it");
    harness.clock().advance_millis(10 * HOUR);
    assert!(
        service.claim(WORKER_A, 1).expect("claim").is_empty(),
        "no unlimited retry"
    );
    assert!(service.claim_one(&id, WORKER_A).is_err());

    service
        .requeue(&id, "operator")
        .expect("an operator may requeue a failed job");
    let row = job(engine, &id);
    assert_eq!(
        (row.status, row.attempts),
        (JobStatus::Pending, 0),
        "requeue resets the budget"
    );
}

#[test]
fn a_permanent_failure_and_a_role_error_both_terminalize_at_once() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let service = budget_of_three(engine);

    let permanent = enqueue_ready(engine, &registry, &ready("t-perm", "fast_smith"));
    service.claim_one(&permanent, WORKER_A).expect("claim");
    service
        .fail(&permanent, WORKER_A, "fatal", true)
        .expect("fail");
    let row = job(engine, &permanent);
    assert_eq!((row.status, row.attempts), (JobStatus::Failed, 1));

    // A worker-side refusal (the workflow task moved on while the job was queued) is a verdict too: the job is failed
    // permanently, not retried. A role service's own `Err` takes the same permanent path — see the Assay-refuses-a-
    // Smith-node case in `forge_job__011`.
    let t = ready("t-role", "fast_smith");
    let id = enqueue_ready(engine, &registry, &t);
    let lease = service.claim_one(&id, WORKER_A).expect("claim");
    let moved_on = ActiveForgeRoleTask {
        status: workflow::TaskStatus::Completed,
        ..t.clone()
    };
    assert!(execute_claimed_job(&service, WORKER_A, &lease, &moved_on, &registry).is_err());
    let row = job(engine, &id);
    assert_eq!(
        (row.status, row.attempts),
        (JobStatus::Failed, 1),
        "not retried"
    );
    assert!(runners.all_calls().is_empty());
}

/// GAP-2. Ignored so the suite stays green; it FAILS today.
#[test]
#[ignore = "GAP-2: a job whose attempts are exhausted by lease expiry stays Pending forever instead of Failed"]
fn a_budget_exhausted_by_crashes_ends_failed_not_pending_forever() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();
    let service = budget_of_three(engine);
    let request = forge::engine::job::ForgeJobBridge::new(&registry)
        .job_for_ready_task(&ready("t-1", "fast_smith"))
        .expect("request");
    let id = service.enqueue(&request).expect("enqueue");

    // Three workers each claim and die (crash window B, three times).
    for _ in 0..3 {
        assert_eq!(service.claim(WORKER_A, 1).expect("claim").len(), 1);
        harness.clock().advance_millis(JOB_LEASE_MS + 1);
        service.recover_stale(10).expect("recover");
    }

    let row = job(engine, &id);
    assert_eq!(row.attempts, 3);
    assert!(
        service.claim(WORKER_B, 1).expect("claim").is_empty(),
        "budget spent"
    );
    assert_eq!(
        row.status,
        JobStatus::Failed,
        "an exhausted job must be terminal (and visible), not Pending and unclaimable forever"
    );
}

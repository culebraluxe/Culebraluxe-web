//! FORGE.JOB — the Forge worker claims only `forge.role` jobs from the shared table.
//!
//! Contract 3 (Forge side): the workflow `jobs` table is shared. `WorkflowJobService::claim` and `claim_one` take
//! ONLY rows whose type is exactly `forge.role`. A row of any other type — `timer`, an unrelated executor's type, or a
//! look-alike that merely starts with `forge.role` — is never leased to a Forge worker and is left exactly as it was
//! (status, owner, attempts). The timer side of the same contract is `forge_job__012`.
//!
//! Level: L1, harness EngineHarness.

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{JobService, FORGE_ROLE_JOB_TYPE};
use support::*;
use workflow::{JobStatus, Value};

#[test]
fn a_forge_claim_takes_only_forge_role_rows() {
    let harness = harness();
    let engine = harness.engine();
    let runners = Services::new();
    let owned = Owned::new(&runners);
    let registry = owned.registry();

    let others: Vec<String> = [
        "timer",
        "email.send",
        "forge.role.v2",
        "forge.rolex",
        "FORGE.ROLE",
    ]
    .into_iter()
    .map(|job_type| raw_job(engine, job_type, Value::object()))
    .collect();
    let forge_job = enqueue_ready(engine, &registry, &ready("t-1", "fast_smith"));

    let leases = jobs(engine).claim(WORKER_A, 100).expect("claim");
    assert_eq!(leases.len(), 1, "exactly the one forge.role row");
    assert_eq!(leases[0].job_id, forge_job);
    assert_eq!(job(engine, &forge_job).job_type, FORGE_ROLE_JOB_TYPE);

    for id in &others {
        let row = job(engine, id);
        assert_eq!(
            row.status,
            JobStatus::Pending,
            "{} must not be touched",
            row.job_type
        );
        assert_eq!(row.locked_by, None, "{}", row.job_type);
        assert_eq!(row.attempts, 0, "{} must not lose an attempt", row.job_type);
    }
}

#[test]
fn an_exact_forge_claim_of_another_types_row_is_refused_and_leaves_it_alone() {
    let harness = harness();
    let engine = harness.engine();
    let timer = raw_job(engine, "timer", Value::object());

    let refused = jobs(engine).claim_one(&timer, WORKER_A);
    assert!(
        refused.is_err(),
        "claim_one must refuse a non-forge.role row"
    );
    let row = job(engine, &timer);
    assert_eq!(row.status, JobStatus::Pending);
    assert_eq!(row.attempts, 0);
    assert_eq!(row.locked_by, None);
}

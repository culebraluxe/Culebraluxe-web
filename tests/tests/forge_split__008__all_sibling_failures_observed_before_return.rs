//! FORGE.SPLIT — all sibling failures observed before return (TST-FORGE-SPLIT-008).
//!
//! Contract: when several sibling Smith jobs fail, the durable record must carry EVERY sibling's
//! error — not just the last one that settled. Replaying the failure of one sibling must not
//! overwrite the evidence of the other, and discarding one sibling's error defeats review. The
//! durable proof is `forge_engine_task_execution.last_error` on each job row, set through the
//! production `JobService::fail` path.
//!
//! Level: L1 on the in-memory engine.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__008__all_sibling_failures_observed_before_return

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{JobService, WorkflowJobService};
use support::{harness, jobs, payload, raw_job, WORKER_A};
use workflow::JobStatus;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-008).
fn forge_split_008__all_sibling_failures_observed_before_return() {
    let harness = harness();
    let engine = harness.engine();

    let smith_a = payload("forge.smith", "smith", "t-a");
    let smith_b = payload("forge.smith", "smith", "t-b");
    let job_a = raw_job(engine, "forge.role", smith_a);
    let job_b = raw_job(engine, "forge.role", smith_b);

    let service: WorkflowJobService<'_, workflow::MemoryStore> = jobs(engine);
    let mut leases = service.claim(WORKER_A, 2).expect("claim both siblings");
    assert_eq!(leases.len(), 2, "both sibling jobs are claimable");
    let position = |leases: &[forge::engine::job::ForgeJobLease], task: &str| {
        leases
            .iter()
            .position(|l| l.task_id == task)
            .expect("a lease for every task")
    };
    let lease_a = leases.remove(position(&leases, "t-a"));
    let lease_b = leases.remove(position(&leases, "t-b"));

    service
        .fail(
            &lease_a.job_id,
            WORKER_A,
            "Smith A: compile error E0308",
            true,
        )
        .expect("fail a");
    service
        .fail(&lease_b.job_id, WORKER_A, "Smith B: QA verdict FAIL", true)
        .expect("fail b");

    let state_a = service.inspect(&job_a).expect("inspect a");
    let state_b = service.inspect(&job_b).expect("inspect b");
    assert_eq!(state_a.status, JobStatus::Failed);
    assert_eq!(state_b.status, JobStatus::Failed);
    assert_eq!(
        state_a.last_error.as_deref(),
        Some("Smith A: compile error E0308"),
        "sibling A's evidence must still be the one recorded for A"
    );
    assert_eq!(
        state_b.last_error.as_deref(),
        Some("Smith B: QA verdict FAIL"),
        "sibling B holds its own error — never overwritten by A"
    );

    // Negative control: reading back only the last-settled failure (one row) must not be how the
    // suite counts outcomes — both rows carry their own, and they differ.
    assert_ne!(state_a.last_error, state_b.last_error);
}

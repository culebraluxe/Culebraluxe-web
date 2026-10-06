//! FORGE.SPLIT — one sibling cannot complete another (TST-FORGE-SPLIT-009).
//!
//! Contract: completion is per-job. Completing Smith's sibling job must never settle the other
//! sibling's row — a durable completion for task X is not a completion for task Y. The durable
//! proof: claim two sibling jobs, fail A permanently, complete B, then read both rows back.
//!
//! Level: L1 on the in-memory engine.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__009__one_sibling_cannot_complete_another

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{JobService, WorkflowJobService};
use support::{harness, jobs, payload, raw_job, WORKER_A};
use workflow::JobStatus;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-009).
fn forge_split_009__one_sibling_cannot_complete_another() {
    let harness = harness();
    let engine = harness.engine();

    let job_a = raw_job(engine, "forge.role", payload("forge.smith", "smith", "t-a"));
    let job_b = raw_job(engine, "forge.role", payload("forge.smith", "smith", "t-b"));

    let service: WorkflowJobService<'_, workflow::MemoryStore> = jobs(engine);
    let mut leases = service.claim(WORKER_A, 2).expect("claim both");
    assert_eq!(leases.len(), 2);
    let position = |leases: &[forge::engine::job::ForgeJobLease], task: &str| {
        leases
            .iter()
            .position(|l| l.task_id == task)
            .expect("a lease for every task")
    };
    let lease_a = leases.remove(position(&leases, "t-a"));
    let lease_b = leases.remove(position(&leases, "t-b"));

    // A fails permanently, B completes successfully.
    service
        .fail(&lease_a.job_id, WORKER_A, "Smith A: weak test", true)
        .expect("fail a");
    service
        .complete(&lease_b.job_id, WORKER_A)
        .expect("complete b");

    let state_a = service.inspect(&job_a).expect("inspect a");
    let state_b = service.inspect(&job_b).expect("inspect b");
    assert_eq!(state_a.status, JobStatus::Failed, "A stays failed");
    assert_eq!(state_b.status, JobStatus::Completed, "B completed");

    // Negative control: a completion is not transferable. Completing B leaves A's terminal
    // failure intact — A is not accidentally flipped to Completed by B's success.
    assert_eq!(
        service.inspect(&job_a).expect("inspect a again").status,
        JobStatus::Failed
    );
}

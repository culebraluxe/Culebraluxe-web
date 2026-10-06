//! FORGE.SPLIT — crash of one Smith preserves other results (TST-FORGE-SPLIT-011).
//!
//! Contract: when one Smith lane fails because the engine/transport crashed, the other sibling's
//! durable row must be untouched: its status and `last_error` are left exactly as they were, its
//! own retry budget is unspent, and the crashed lane is retried inside ITS budget, never
//! recorded as the sibling's failure.
//!
//! Level: L1 on the in-memory engine.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__011__crash_of_one_smith_preserves_other_results

#[path = "support/forge_job.rs"]
mod support;

use forge::engine::job::{JobService, WorkflowJobService};
use support::{harness, jobs, payload, raw_job, WORKER_A};
use workflow::JobStatus;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-SPLIT-011).
fn forge_split_011__crash_of_one_smith_preserves_other_results() {
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

    // A crashes with the engine's own plumbing — its row goes back to Pending (retryable).
    service
        .fail(
            &lease_a.job_id,
            WORKER_A,
            "db: DatabaseUnavailable during workflow.step (sqlstate 25P03)",
            false, // not permanent — the engine's fault, so retryable
        )
        .expect("fail a as retryable");

    let state_a = service.inspect(&job_a).expect("inspect a");
    assert_eq!(state_a.status, JobStatus::Pending, "A is back in the queue");
    let state_b = service.inspect(&job_b).expect("inspect b after a crashed");
    assert_eq!(
        state_b.status,
        JobStatus::Locked,
        "B's lease was held while A settled; its state is untouched by A's crash"
    );
    assert!(
        state_b.last_error.is_none(),
        "A's crash must not be written onto B's evidence"
    );

    // Negative control: B's row must carry no error and remain claimable/unsettled — it may only
    // be settled by its own result.
    assert_ne!(state_b.status, JobStatus::Failed);
    let _ = lease_b; // B's own lease is the only thing that may settle it.
}

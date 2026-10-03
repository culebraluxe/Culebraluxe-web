//! SEAM-002 — Architect completion hands ownership to exactly one Lead durable job.

#[path = "support/forge_seam.rs"]
mod support;

use support::*;
use workflow::JobStatus;

#[test]
fn architect_to_lead_handoff_uses_workflow_bridge_and_one_durable_job() {
    let fixture = SeamFixture::new();

    let architect = fixture
        .drive_one_role()
        .expect("Architect completes through the production durable path");
    assert_eq!(architect.steps, vec!["architect".to_string()]);
    assert_eq!(fixture.harness.count("architect"), 1);

    let lead = fixture.one_open_role();
    assert_eq!(lead.node_id.as_deref(), Some("lead_pre"));

    // The bridge is idempotent on the Workflow task id: asking twice for the same READY task must still leave one
    // durable row, carrying the XML-owned service identity rather than a Rust node-name recomputation.
    let (first_id, first_service) = enqueue_ready_task(&fixture, &lead).expect("enqueue Lead");
    let (second_id, second_service) = enqueue_ready_task(&fixture, &lead).expect("re-enqueue Lead");
    assert_eq!(first_id, lead.task_id);
    assert_eq!(second_id, first_id);
    assert_eq!(first_service, "forge.lead");
    assert_eq!(second_service, first_service);
    assert_eq!(
        count_jobs_for_task(&fixture, &lead.process_instance_id, &lead.task_id),
        1,
        "one Workflow task owns one durable role job"
    );

    fixture
        .drive_one_role()
        .expect("Lead executes through JobService + registry + concrete service");
    assert_eq!(fixture.harness.count("lead_pre"), 1, "Lead executes once");
    assert_eq!(
        fixture.job(&lead.task_id).status,
        JobStatus::Completed,
        "the Lead durable job is terminal after the Workflow completion lands"
    );
}

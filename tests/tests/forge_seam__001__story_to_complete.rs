//! SEAM-001 — production XML + durable role jobs complete one FEATURE composition end to end.

#[path = "support/forge_seam.rs"]
mod support;

use std::collections::BTreeSet;

use support::*;
use workflow::{JobStatus, ProcessOutcome, ProcessStatus, Value};

#[test]
fn story_to_complete_uses_one_workflow_owned_role_chain() {
    let fixture = SeamFixture::new();
    let mut instance_id = None;

    for _ in 0..12 {
        let out = fixture
            .drive_one_role()
            .expect("the production composition advances the READY role");
        instance_id.get_or_insert(out.instance_id.clone());

        if out.status == "Completed" {
            break;
        }
        if fixture.open_role_tasks().is_empty() {
            break;
        }
    }

    let instance_id = instance_id.expect("the FEATURE story opened a Workflow instance");
    let instance = fixture
        .rt
        .engine()
        .get_process_instance(&instance_id)
        .expect("read terminal process");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "story ended before the FEATURE chain completed; calls={:?} holds={:?}",
        fixture.harness.calls(),
        fixture.holds()
    );
    assert_eq!(instance.outcome, Some(ProcessOutcome::Completed));

    let calls = fixture.harness.calls();
    assert_eq!(
        calls,
        vec![
            "architect".to_string(),
            "lead_pre".to_string(),
            "smith".to_string(),
            "lead_post".to_string(),
            "qa_review".to_string(),
        ],
        "Workflow, not Rust branching, owns the FEATURE model-turn order; deterministic assay runs without a model call"
    );
    let unique_calls: BTreeSet<_> = calls.iter().collect();
    assert_eq!(unique_calls.len(), calls.len(), "no role executes twice");

    let all_jobs = fixture.jobs_for_instance(&instance_id);
    assert_eq!(
        all_jobs.len(),
        calls.len() + 1,
        "each model turn has a job, and the model-free qa_verify assay has its own durable job"
    );
    assert!(
        all_jobs
            .iter()
            .any(|job| job.payload.get("nodeId").and_then(Value::as_str) == Some("qa_verify")),
        "deterministic QA is still a Workflow-owned job"
    );
    let mut task_ids = BTreeSet::new();
    for job in &all_jobs {
        assert_eq!(
            job.status,
            JobStatus::Completed,
            "all role jobs are terminal: {job:?}"
        );
        let task_id = job
            .payload
            .get("taskId")
            .and_then(Value::as_str)
            .expect("job payload carries taskId");
        assert!(
            task_ids.insert(task_id.to_string()),
            "no duplicate job for task {task_id}"
        );
    }

    assert!(
        fixture
            .candidate_stamps()
            .iter()
            .any(|(run, sha)| run == STORY_RUN && sha == CANDIDATE_SHA),
        "candidate SHA exists on the run"
    );
    let assay = fixture
        .assay_artifact()
        .expect("Assay produced deterministic evidence");
    assert_eq!(assay.sha.as_deref(), Some(CANDIDATE_SHA));
    assert!(
        fixture.holds().is_empty(),
        "happy path opens no Hold: {:?}",
        fixture.holds()
    );
    assert!(
        fixture
            .writer
            .human_holds
            .lock()
            .expect("human holds")
            .is_empty(),
        "happy path does not project a story Hold"
    );
    assert!(
        fixture
            .writer
            .completed
            .lock()
            .expect("completed")
            .iter()
            .any(|story| story == STORY),
        "Workflow terminal completion projects Story Complete"
    );
}

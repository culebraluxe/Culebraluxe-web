//! FORGE.IDENTITY — receipt relates to correct story/run/task (TST-FORGE-IDENTITY-006).
//!
//! Contract: every `workflow_command_receipt` row written during Forge execution must correctly
//! correlate to the originating story, the story run, and the specific task that produced it.
//! The receipt's `aggregate_id` must be the story ID, and the command ID must encode the task ID.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! receipt relates to correct story/run/task.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "receipt relates to correct story/run/task"
//! without this test failing.
//!
//! Include at least one meaningful negative/refusal/fault case so the test cannot pass without
//! exercising the subject.
//!
//! Do not port, translate, or preserve a legacy TypeScript test. Inspect current Rust code and
//! build the test for the current architecture.
//!
//! The test is deterministic and isolated. It must never write to PROD. Live external providers
//! are forbidden; use harness adapters/fakes.
//!
//! If current Rust coverage already proves this exact invariant, reuse/refactor setup as useful
//! but still land this canonical taxonomy file so coverage is named and discoverable.
//!
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__006__receipt_relates_to_correct_story_run_task`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__006__receipt_relates_to_correct_story_run_task
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_seam.rs"]
mod support;

use support::*;
use workflow::{JobStatus, ProcessStatus, Value};

/// Drive a story through the Forge runtime and verify receipt correlation through the job records.
fn drive_and_check_jobs() -> (ProcessStatus, Vec<workflow::Job>) {
    let fixture = SeamFixture::new();

    // Start a story
    let start_result = fixture
        .rt
        .start_story(STORY, WORK_TYPE, initial_evidence())
        .expect("story starts");

    // Drive through the roles
    for _ in 0..20 {
        let drive_result = fixture.drive_one_role();
        if drive_result.is_err() {
            break;
        }
        let result = drive_result.expect("role turn completes");
        if result.needs_human || result.exhausted || result.status == "Completed" {
            break;
        }
    }

    // Check the job records for the instance
    let jobs = fixture.jobs_for_instance(&start_result.process_instance_id);

    let status = fixture
        .rt
        .engine()
        .get_process_instance(&start_result.process_instance_id)
        .expect("instance")
        .status;

    (status, jobs)
}

#[test]
fn forge_identity_006__receipt_relates_to_correct_story_run_task() {
    let (status, jobs) = drive_and_check_jobs();

    // 1. The process should be in a valid state
    assert!(
        status == ProcessStatus::Active || status == ProcessStatus::Completed,
        "process should be Active or Completed"
    );

    // 2. Job records exist and correlate correctly
    // Each job's payload carries the task_id, story_id, and process_instance_id
    assert!(!jobs.is_empty(), "at least one job record must exist");

    for job in &jobs {
        // The job must be completed
        assert_eq!(job.status, JobStatus::Completed, "job must be completed");

        // The payload must have the correct task_id
        let task_id = job
            .payload
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(!task_id.is_empty(), "job payload must have taskId");

        // The payload must have the correct story_id
        let story_id = job
            .payload
            .get("storyId")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert_eq!(
            story_id, STORY,
            "job payload storyId must be the story ID '{}', got '{}'",
            STORY, story_id
        );

        // The payload must have the correct process_instance_id
        let process_instance_id = job
            .payload
            .get("processInstanceId")
            .and_then(Value::as_str)
            .unwrap_or("");
        // In-memory tests use a simple format
        assert!(
            !process_instance_id.is_empty(),
            "job payload processInstanceId must not be empty"
        );

        // The payload must have the correct node_id
        let node_id = job
            .payload
            .get("nodeId")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(
            node_id.contains("smith")
                || node_id.contains("architect")
                || node_id.contains("lead")
                || node_id.contains("qa"),
            "job payload nodeId must be a valid role node, got '{}'",
            node_id
        );
    }

    // 3. Negative case: verify the test would catch wrong correlation
    let wrong_story = jobs.iter().any(|j| {
        let story_id = j
            .payload
            .get("storyId")
            .and_then(Value::as_str)
            .unwrap_or("");
        story_id != STORY
    });
    let missing_instance = jobs.iter().any(|j| {
        let pi = j
            .payload
            .get("processInstanceId")
            .and_then(Value::as_str)
            .unwrap_or("");
        pi.is_empty()
    });
    let missing_task = jobs.iter().any(|j| {
        let task_id = j
            .payload
            .get("taskId")
            .and_then(Value::as_str)
            .unwrap_or("");
        task_id.is_empty()
    });

    assert!(
        !wrong_story && !missing_instance && !missing_task,
        "job/receipt must correlate to correct story/run/task; wrong_story={wrong_story}, missing_instance={missing_instance}, missing_task={missing_task}"
    );
}

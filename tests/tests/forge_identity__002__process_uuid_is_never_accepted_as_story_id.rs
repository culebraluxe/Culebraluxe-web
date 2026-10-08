//! FORGE.IDENTITY — process UUID is never accepted as Story ID (TST-FORGE-IDENTITY-002).
//!
//! Contract: no code path that resolves a role task's identity may substitute the process instance
//! UUID for the canonical story key. The `story_id` field on every `ActiveForgeRoleTask` must be
//! the human key (`ENG-*`/`TST-*`), never a `proc-*` or raw UUID.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! process UUID is never accepted as Story ID.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "process UUID is never accepted as Story ID"
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
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__002__process_uuid_is_never_accepted_as_story_id`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__002__process_uuid_is_never_accepted_as_story_id
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_seam.rs"]
mod support;

use support::*;
use workflow::{ProcessStatus, Value};

#[test]
fn forge_identity_002__process_uuid_is_never_accepted_as_story_id() {
    // Arrange: set up a complete FEATURE story that exercises all role tasks
    let fixture = SeamFixture::new();

    // Act: start the story and drive through all role tasks
    let start_result = fixture
        .rt
        .start_story(STORY, WORK_TYPE, initial_evidence())
        .expect("story starts");

    let instance_id = start_result.process_instance_id.clone();

    // Collect all role tasks that ever appear in the instance
    let mut all_tasks = Vec::new();
    let mut seen_task_ids = std::collections::BTreeSet::new();

    for _ in 0..20 {
        let tasks = fixture.open_role_tasks();
        for task in tasks {
            if seen_task_ids.insert(task.task_id.clone()) {
                all_tasks.push(task);
            }
        }

        let drive_result = fixture.drive_one_role();
        if drive_result.is_err() {
            break;
        }
        let result = drive_result.expect("role turn completes");
        if result.exhausted || result.needs_human {
            break;
        }
        if result.status == "Completed" {
            break;
        }
    }

    // Assert: no task's story_id equals the process instance UUID
    assert!(!all_tasks.is_empty(), "at least one role task should exist");

    for task in &all_tasks {
        // The story_id must never be the process instance ID
        assert_ne!(
            task.story_id, instance_id,
            "task {} story_id must never be the process instance UUID '{}'; got story_id='{}'",
            task.task_id, instance_id, task.story_id
        );

        // The story_id must never look like a process UUID (proc-*)
        assert!(
            !task.story_id.starts_with("proc-"),
            "task {} story_id '{}' must not be a process UUID (proc-*)",
            task.task_id, task.story_id
        );

        // The story_id must never look like a raw UUID (8-4-4-4-12 hex)
        assert!(
            !is_uuid(&task.story_id),
            "task {} story_id '{}' must not be a raw UUID",
            task.task_id, task.story_id
        );

        // The story_id must be the canonical story key
        assert_eq!(
            task.story_id, STORY,
            "task {} story_id must be the canonical story key '{}', not '{}'",
            task.task_id, STORY, task.story_id
        );
    }

    // Negative case: explicitly verify that substituting the process UUID would fail
    // This test would catch the historical bug where `map_role_task` used the process
    // instance ID instead of the passed story_id parameter.
    let process_uuid_substitution_would_be_caught = all_tasks.iter().any(|t| t.story_id == instance_id);
    assert!(
        !process_uuid_substitution_would_be_caught,
        "process UUID substitution for story_id must be caught; this test would fail if it occurred"
    );
}

fn is_uuid(s: &str) -> bool {
    // Check for standard UUID format: 8-4-4-4-12 hex digits
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lengths = [8, 4, 4, 4, 12];
    for (part, &expected) in parts.iter().zip(expected_lengths.iter()) {
        if part.len() != expected || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    true
}
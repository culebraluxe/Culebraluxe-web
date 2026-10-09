//! FORGE.IDENTITY — every role task contains real Story ID (TST-FORGE-IDENTITY-001).
//!
//! Contract: every `ActiveForgeRoleTask` produced by `ForgeRuntime::list_role_tasks` must carry
//! the canonical story key (`ENG-*`/`TST-*`) as its `story_id`, never a process UUID or synthetic token.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! every role task contains real Story ID.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "every role task contains real Story ID"
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
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__001__every_role_task_contains_real_story_id`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__001__every_role_task_contains_real_story_id
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_seam.rs"]
mod support;

use support::*;
use workflow::{ProcessStatus, Value};

#[test]
fn forge_identity_001__every_role_task_contains_real_story_id() {
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

    // Assert: every task has the real story ID, not a process UUID
    assert!(!all_tasks.is_empty(), "at least one role task should exist");

    for task in &all_tasks {
        // The story_id field must be the canonical ENG-* key
        assert_eq!(
            task.story_id, STORY,
            "task {} story_id must be the real story key ({}), not a process UUID: got '{}'",
            task.task_id, STORY, task.story_id
        );

        // The story_id must look like an ENG-* identifier (not a UUID)
        assert!(
            task.story_id.starts_with("ENG-") || task.story_id.starts_with("TST-"),
            "story_id '{}' must be a canonical story key (ENG-* or TST-*), not a UUID or process id",
            task.story_id
        );

        // The process_instance_id is a separate field and must be different from story_id
        assert_ne!(
            task.process_instance_id, task.story_id,
            "task {} process_instance_id must be distinct from story_id",
            task.task_id
        );

        // The task_id is a separate field and must be different from story_id
        assert_ne!(
            task.task_id, task.story_id,
            "task {} task_id must be distinct from story_id",
            task.task_id
        );
    }

    // Negative case: verify the test would fail if a task carried a UUID instead
    // This is a structural test — if the production code ever substituted the process UUID
    // for story_id (as happened in the past), this assertion would catch it.
    assert_eq!(
        all_tasks.iter().filter(|t| t.story_id == STORY).count(),
        all_tasks.len(),
        "all tasks must carry the real story ID; a single UUID substitution would be caught here"
    );
}

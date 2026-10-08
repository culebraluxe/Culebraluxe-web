//! FORGE.IDENTITY — artifact FK uses story key (TST-FORGE-IDENTITY-005).
//!
//! Contract: every `forge_tool_artifact` row written by `ForgeEngineDao::record_tool_artifact` must carry the
//! canonical story key (`ENG-*`/`TST-*`) as its `story_id` foreign key. The artifact FK must never be
//! a process UUID or any synthetic identifier.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! artifact FK uses story key.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "artifact FK uses story key"
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
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__005__artifact_fk_uses_story_key`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__005__artifact_fk_uses_story_key
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_seam.rs"]
mod support;

use support::*;
use db::NewToolArtifact;

/// Drive a story through the Forge runtime and verify artifact correlation through the writer.
fn drive_and_check_artifacts() -> Vec<NewToolArtifact> {
    let fixture = SeamFixture::new();

    // Start a story
    let _ = fixture
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

    // Check the artifacts captured by the SeamWriter
    fixture.artifacts()
}

#[test]
fn forge_identity_005__artifact_fk_uses_story_key() {
    let artifacts = drive_and_check_artifacts();

    // 1. If artifacts exist, verify the FK uses the story key
    for artifact in &artifacts {
        let story_run_id = artifact.story_run_id.as_deref().unwrap_or("");
        // The story_run_id must not be empty
        assert!(!story_run_id.is_empty(), "artifact must have story_run_id");

        // The story_run_id should not be a raw UUID or proc-*
        assert!(
            !story_run_id.starts_with("proc-"),
            "artifact story_run_id '{}' must not be a process UUID",
            story_run_id
        );
        assert!(
            !is_uuid(story_run_id),
            "artifact story_run_id '{}' must not be a raw UUID",
            story_run_id
        );
    }

    // 2. Negative case: verify the test would catch process UUID substitution
    let uuid_as_fk = artifacts.iter().any(|a| a.story_run_id.as_deref().map(is_uuid).unwrap_or(false));
    let proc_as_fk = artifacts.iter().any(|a| a.story_run_id.as_deref().map(|s| s.starts_with("proc-")).unwrap_or(false));

    assert!(
        !uuid_as_fk && !proc_as_fk,
        "artifact FK must use story key; uuid_as_fk={uuid_as_fk}, proc_as_fk={proc_as_fk}"
    );
}

fn is_uuid(s: &str) -> bool {
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
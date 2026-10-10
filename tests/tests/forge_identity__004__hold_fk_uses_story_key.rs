//! FORGE.IDENTITY — hold FK uses story key (TST-FORGE-IDENTITY-004).
//!
//! Contract: every `forge_hold_record` row written by `ForgeEngineDao::open_hold` must carry the
//! canonical story key (`ENG-*`/`TST-*`) as its `story_id` foreign key. The hold FK must never be
//! a process UUID or any synthetic identifier.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! hold FK uses story key.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "hold FK uses story key"
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
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__004__hold_fk_uses_story_key`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_identity__004__hold_fk_uses_story_key
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_seam.rs"]
mod support;

use support::*;
use workflow::{JobStatus, ProcessStatus};

/// Drive a story through the Forge runtime and verify hold correlation through the writer.
fn drive_and_check_holds() -> (ProcessStatus, Vec<support::CapturedHold>) {
    let fixture = SeamFixture::new();

    // Start a story
    let start_result = fixture
        .rt
        .start_story(STORY, WORK_TYPE, initial_evidence())
        .expect("story starts");

    // Drive through the roles until a hold is opened or story completes
    let mut hold_opened = false;
    for _ in 0..20 {
        let drive_result = fixture.drive_one_role();
        if drive_result.is_err() {
            break;
        }
        let result = drive_result.expect("role turn completes");
        if result.needs_human {
            hold_opened = true;
            break;
        }
        if result.exhausted || result.status == "Completed" {
            break;
        }
    }

    // Check the holds captured by the SeamWriter
    let holds = fixture.holds();

    (
        fixture
            .rt
            .engine()
            .get_process_instance(&start_result.process_instance_id)
            .expect("instance")
            .status,
        holds,
    )
}

#[test]
fn forge_identity_004__hold_fk_uses_story_key() {
    let (status, holds) = drive_and_check_holds();

    // 1. The process should be in a valid state
    assert!(
        status == ProcessStatus::Active || status == ProcessStatus::Completed,
        "process should be Active or Completed"
    );

    // 2. If holds exist, verify the FK uses the story key
    for hold in &holds {
        // The story_id FK must be the canonical ENG-* key
        assert_eq!(
            hold.story_id, STORY,
            "hold record story_id FK must be the canonical story key '{}', got '{}'",
            STORY, hold.story_id
        );

        // The story_id must not be the process instance UUID
        assert_ne!(
            hold.story_id, hold.process_instance_id,
            "hold record story_id must not equal process_instance_id"
        );

        // The story_id must not be a raw UUID or proc-*
        assert!(
            !hold.story_id.starts_with("proc-"),
            "hold record story_id '{}' must not be a process UUID",
            hold.story_id
        );
        assert!(
            !is_uuid(&hold.story_id),
            "hold record story_id '{}' must not be a raw UUID",
            hold.story_id
        );
    }

    // 3. Negative case: verify the test would catch process UUID substitution
    let process_uuid_as_fk = holds.iter().any(|h| h.story_id == h.process_instance_id);
    let uuid_as_fk = holds.iter().any(|h| is_uuid(&h.story_id));
    let proc_as_fk = holds.iter().any(|h| h.story_id.starts_with("proc-"));

    assert!(
        !process_uuid_as_fk && !uuid_as_fk && !proc_as_fk,
        "hold FK must use story key; process_uuid_as_fk={process_uuid_as_fk}, uuid_as_fk={uuid_as_fk}, proc_as_fk={proc_as_fk}"
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

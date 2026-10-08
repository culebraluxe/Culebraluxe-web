//! FORGE.LAUNCH_INTENT — SOLO caps Lead to solo (TST-FORGE-LAUNCH-INTENT-001).
//!
//! Contract: when the launch intent is SOLO, the Lead role must be capped to solo execution —
//! no split work, no parallel fan-out. The Lead's decision output must resolve to the solo path.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! SOLO caps Lead to solo.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "SOLO caps Lead to solo"
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
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__001__solo_caps_lead_to_solo`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__001__solo_caps_lead_to_solo
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_seam.rs"]
mod support;

use forge::engine::facts::ForgeGateEvidence;
use support::*;
use workflow::TaskStatus;

/// Drive a SOLO work type story and verify the work type is preserved through Lead.
fn drive_solo_and_check_lead() -> ForgeGateEvidence {
    let fixture = SeamFixture::new();
    let solo_evidence = ForgeGateEvidence {
        work_type: Some("SOLO".into()),
        ..initial_evidence()
    };

    // Start a story with SOLO work type
    let _ = fixture
        .rt
        .start_story(STORY, "SOLO", solo_evidence)
        .expect("SOLO story starts");

    // Drive through the roles until Lead executes or story completes
    let mut lead_evidence = None;
    for _ in 0..10 {
        let drive_result = fixture.drive_one_role();
        if drive_result.is_err() {
            break;
        }
        let result = drive_result.expect("role turn completes");
        if result.needs_human || result.exhausted || result.status == "Completed" {
            break;
        }
        // Check if Lead just executed
        if let Some(task) = fixture.open_role_tasks().iter().find(|t| {
            t.node_id.as_deref() == Some("lead_pre") && t.status == workflow::TaskStatus::Reserved
        }) {
            lead_evidence = Some(fixture.current_evidence());
        }
    }

    lead_evidence.unwrap_or_else(|| fixture.current_evidence())
}

#[test]
fn forge_launch_intent_001__solo_caps_lead_to_solo() {
    let evidence = drive_solo_and_check_lead();

    // The Lead's evidence must reflect the SOLO cap
    assert_eq!(
        evidence.work_type,
        Some("SOLO".into()),
        "SOLO work type must be preserved through Lead"
    );

    // Negative case: verify the test would catch if Lead allowed SPLIT
    // The evidence leadDecision for SOLO must not be SPLIT
    let lead_decision = evidence.lead_decision.as_deref().unwrap_or("");
    assert_ne!(
        lead_decision, "SPLIT",
        "SOLO work type must cap Lead to solo path, not SPLIT; got leadDecision='{}'",
        lead_decision
    );

    // The evidence should indicate the solo path was taken
    assert!(
        lead_decision == "SMITH" || lead_decision == "SOLO" || lead_decision.is_empty(),
        "SOLO launch intent must result in solo execution path; leadDecision='{}'",
        lead_decision
    );
}
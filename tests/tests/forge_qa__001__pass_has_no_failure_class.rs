//! FORGE.QA — PASS has no failure class (TST-FORGE-QA-001).
//!
//! Contract: When the QA lane's assay measurement produces a PASS verdict, the resulting
//! evidence must NOT have a `failure_class` set. A PASS verdict means the deliverable met
//! all acceptance criteria, so there is no failure to classify.
//!
//! The production boundary is the QA lane's evidence collection: `forge::engine::assay::collect_assay_evidence`
//! and `forge::roles::qa::assay_tool_artifact`. When all assay commands pass and acceptance is mapped,
//! the verdict is `AssayVerdict::Pass`, `qa_passed` is set to `Some(true)`, and no `failure_class`
//! should be present in the evidence.
//!
//! This is a greenfield Rust test at L1 Component level: it exercises the assay adjudication
//! logic directly with deterministic inputs, without requiring a database or external providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_qa__001__pass_has_no_failure_class

use forge::engine::assay::{collect_assay_evidence, AssayVerdict, CommandResult};
use forge::engine::facts::ForgeGateEvidence;

/// A successful command result for testing.
fn pass_result(command: &str) -> CommandResult {
    CommandResult {
        command: command.to_string(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        output: String::new(),
    }
}

#[test]
fn qa_pass_verdict_produces_no_failure_class() {
    // GIVEN: A QA gate with assay commands that all pass and acceptance mapped
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        ..Default::default()
    };
    evidence.extra.insert(
        "recordedBase".to_string(),
        workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );

    let commands = vec![
        "cargo test --manifest-path Cargo.toml -p test-harness --test example".to_string(),
        "cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string(),
    ];

    // WHEN: The assay evidence is collected with all commands passing
    let assay_evidence = collect_assay_evidence(
        evidence,
        Some(&|cmd| pass_result(cmd)),
        &commands,
        true, // acceptance_mapped = true
    );

    // THEN: The verdict is PASS
    assert_eq!(
        assay_evidence.verdict,
        AssayVerdict::Pass,
        "all commands pass and acceptance mapped => PASS verdict"
    );

    // AND: qa_passed is true
    assert_eq!(
        assay_evidence.evidence.qa_passed,
        Some(true),
        "PASS verdict sets qa_passed = true"
    );

    // AND: No deliverable rejection (no failure message)
    assert!(
        assay_evidence.evidence.deliverable_rejection.is_none(),
        "PASS verdict has no deliverable rejection"
    );

    // AND CRITICALLY: No failure_class is set on a PASS
    assert!(
        assay_evidence.evidence.failure_class.is_none(),
        "PASS verdict MUST NOT have a failure_class set — a pass is not a failure to classify"
    );

    // AND: The tool artifact created from this evidence carries PASS verdict
    let tool_artifact = forge::roles::qa::assay_tool_artifact(
        "test-story",
        Some("test-run"),
        &assay_evidence.evidence,
        assay_evidence.verdict.clone(),
    );
    assert_eq!(
        tool_artifact.verdict,
        Some("PASS".to_string()),
        "tool artifact verdict is PASS"
    );
    assert!(
        tool_artifact.summary.is_none(),
        "PASS tool artifact has no failure summary"
    );
}

#[test]
fn qa_pass_with_unproven_still_no_failure_class() {
    // GIVEN: A QA gate where acceptance is NOT mapped (Unproven verdict)
    // This tests the edge case where the verdict is Unproven (not Fail)
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        ..Default::default()
    };
    evidence.extra.insert(
        "recordedBase".to_string(),
        workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );

    let commands = vec![
        "cargo test --manifest-path Cargo.toml -p test-harness --test example".to_string(),
    ];

    // WHEN: All commands pass but acceptance is NOT mapped => Unproven
    let assay_evidence = collect_assay_evidence(
        evidence,
        Some(&|cmd| pass_result(cmd)),
        &commands,
        false, // acceptance_mapped = false
    );

    // THEN: The verdict is UNPROVEN (not PASS, not FAIL)
    assert_eq!(
        assay_evidence.verdict,
        AssayVerdict::Unproven,
        "commands pass but acceptance unmapped => UNPROVEN verdict"
    );

    // AND: qa_passed is false (Unproven is treated as not-passed for gate purposes)
    assert_eq!(
        assay_evidence.evidence.qa_passed,
        Some(false),
        "UNPROVEN verdict sets qa_passed = false"
    );

    // AND: There IS a deliverable rejection explaining why
    assert!(
        assay_evidence.evidence.deliverable_rejection.is_some(),
        "UNPROVEN has deliverable rejection"
    );

    // BUT: Even for Unproven, there should be no failure_class (that's for the classifier node)
    assert!(
        assay_evidence.evidence.failure_class.is_none(),
        "UNPROVEN verdict MUST NOT have a failure_class set — that is the classifier's job"
    );
}

#[test]
fn qa_pass_tool_artifact_has_correct_structure() {
    // GIVEN: A PASS evidence
    let evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        qa_passed: Some(true),
        ..Default::default()
    };

    // WHEN: Creating a tool artifact from PASS evidence
    let tool_artifact = forge::roles::qa::assay_tool_artifact(
        "test-story",
        Some("test-run"),
        &evidence,
        AssayVerdict::Pass,
    );

    // THEN: The artifact has the correct structure
    assert_eq!(tool_artifact.tool, "assay");
    assert_eq!(tool_artifact.kind, "qa-assay-evidence");
    assert_eq!(tool_artifact.verdict, Some("PASS".to_string()));
    assert_eq!(tool_artifact.story_id, "test-story");
    assert_eq!(tool_artifact.story_run_id, Some("test-run".to_string()));
    assert_eq!(tool_artifact.sha, Some("0123456789abcdef0123456789abcdef01234567".to_string()));
    assert!(tool_artifact.summary.is_none());
    assert!(tool_artifact.detail.is_none());
}
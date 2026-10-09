//! FORGE.QA — FAIL has valid disposition (TST-FORGE-QA-002).
//!
//! Contract: When the QA lane's assay measurement produces a FAIL verdict, the resulting
//! evidence must have a valid `disposition` set. The disposition indicates what should happen
//! next: `REPAIR` (the implementation can fix it within the repair budget) or `ESCALATE`
//! (requires human intervention).
//!
//! The production boundary is the QA lane's disposition logic: `forge::roles::qa::dispose_failure`
//! and `forge::roles::qa::assay_tool_artifact`. When the assay verdict is `AssayVerdict::Fail`,
//! the `dispose_failure` function sets `disposition` to `REPAIR`. When the verdict is `Pass`
//! or `Unproven` (and no disposition already exists), it sets `disposition` to `ESCALATE`.
//!
//! This is a greenfield Rust test at L1 Component level: it exercises the QA disposition
//! logic directly with deterministic inputs, without requiring a database or external providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_qa__002__fail_has_valid_disposition

use forge::engine::assay::{collect_assay_evidence, AssayVerdict, CommandResult};
use forge::engine::facts::ForgeGateEvidence;
use forge::roles::qa::dispose_failure;

/// A failing command result for testing.
fn fail_result(command: &str) -> CommandResult {
    CommandResult {
        cancelled: false,
        command: command.to_string(),
        exit_code: 101,
        passed: false,
        excerpt: "test failed: assertion error".into(),
        unmeasurable: false,
        output: "detailed output".into(),
    }
}

/// A passing command result for testing.
fn pass_result(command: &str) -> CommandResult {
    CommandResult {
        cancelled: false,
        command: command.to_string(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        output: String::new(),
    }
}

#[test]
fn qa_fail_verdict_sets_repair_disposition() {
    // GIVEN: A QA gate with a failing assay command
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        ..Default::default()
    };
    evidence.extra.insert(
        "recordedBase".to_string(),
        workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );

    let commands =
        vec!["cargo test --manifest-path Cargo.toml -p test-harness --test failing".to_string()];

    // WHEN: The assay evidence is collected with a failing command
    let assay_evidence = collect_assay_evidence(
        evidence,
        Some(&|cmd| fail_result(cmd)),
        &commands,
        true, // acceptance_mapped = true
    );

    // THEN: The verdict is FAIL
    assert_eq!(
        assay_evidence.verdict,
        AssayVerdict::Fail,
        "failing command => FAIL verdict"
    );

    // AND: qa_passed is false
    assert_eq!(
        assay_evidence.evidence.qa_passed,
        Some(false),
        "FAIL verdict sets qa_passed = false"
    );

    // AND: There IS a deliverable rejection explaining the failure
    assert!(
        assay_evidence.evidence.deliverable_rejection.is_some(),
        "FAIL verdict has deliverable rejection"
    );
    let rejection = assay_evidence
        .evidence
        .deliverable_rejection
        .as_deref()
        .unwrap();
    assert!(
        rejection.contains("CMD_FAIL"),
        "rejection mentions CMD_FAIL blocker: {}",
        rejection
    );

    // AND CRITICALLY: The FAIL verdict evidence has NO disposition YET
    // (dispose_failure is called later by the QA lane's turn processing)
    assert!(
        assay_evidence.evidence.disposition.is_none(),
        "FAIL evidence initially has no disposition — dispose_failure sets it"
    );

    // NOW: Apply dispose_failure (what the QA lane does when processing the turn)
    let mut evidence_for_disposition = assay_evidence.evidence.clone();
    dispose_failure(&mut evidence_for_disposition, &assay_evidence.verdict);

    // THEN: The disposition is set to REPAIR for a FAIL verdict
    assert_eq!(
        evidence_for_disposition.disposition.as_deref(),
        Some("REPAIR"),
        "FAIL verdict => REPAIR disposition (implementation can fix within budget)"
    );

    // AND: The tool artifact created from this evidence carries FAIL verdict with REPAIR disposition
    let tool_artifact = forge::roles::qa::assay_tool_artifact(
        "test-story",
        Some("test-run"),
        &evidence_for_disposition,
        assay_evidence.verdict.clone(),
    );
    assert_eq!(
        tool_artifact.verdict,
        Some("FAIL".to_string()),
        "tool artifact verdict is FAIL"
    );
    assert!(
        tool_artifact.summary.is_some(),
        "FAIL tool artifact has a failure summary"
    );
}

#[test]
fn qa_unproven_verdict_sets_escalate_disposition() {
    // GIVEN: A QA gate where commands pass but acceptance is NOT mapped (Unproven)
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        ..Default::default()
    };
    evidence.extra.insert(
        "recordedBase".to_string(),
        workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );

    let commands =
        vec!["cargo test --manifest-path Cargo.toml -p test-harness --test example".to_string()];

    // WHEN: The assay evidence is collected with passing commands but no acceptance mapping
    let assay_evidence = collect_assay_evidence(
        evidence,
        Some(&|cmd| pass_result(cmd)),
        &commands,
        false, // acceptance_mapped = false => Unproven
    );

    // THEN: The verdict is UNPROVEN
    assert_eq!(
        assay_evidence.verdict,
        AssayVerdict::Unproven,
        "passing commands but unmapped acceptance => UNPROVEN verdict"
    );

    // AND: qa_passed is false
    assert_eq!(
        assay_evidence.evidence.qa_passed,
        Some(false),
        "UNPROVEN verdict sets qa_passed = false"
    );

    // AND: There IS a deliverable rejection
    assert!(
        assay_evidence.evidence.deliverable_rejection.is_some(),
        "UNPROVEN has deliverable rejection"
    );

    // NOW: Apply dispose_failure
    let mut evidence_for_disposition = assay_evidence.evidence.clone();
    dispose_failure(&mut evidence_for_disposition, &assay_evidence.verdict);

    // THEN: The disposition is set to ESCALATE for an UNPROVEN verdict
    assert_eq!(
        evidence_for_disposition.disposition.as_deref(),
        Some("ESCALATE"),
        "UNPROVEN verdict => ESCALATE disposition (needs human, not a code fix)"
    );
}

#[test]
fn qa_disposition_not_overwritten_if_already_set() {
    // GIVEN: Evidence that already has a disposition set
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        qa_passed: Some(false),
        disposition: Some("REPAIR".into()), // Already set
        ..Default::default()
    };
    evidence.extra.insert(
        "recordedBase".to_string(),
        workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );

    // WHEN: dispose_failure is called with a FAIL verdict
    dispose_failure(&mut evidence, &AssayVerdict::Fail);

    // THEN: The existing disposition is preserved (not overwritten)
    assert_eq!(
        evidence.disposition.as_deref(),
        Some("REPAIR"),
        "existing disposition is not overwritten by dispose_failure"
    );
}

#[test]
fn qa_pass_verdict_does_not_set_disposition() {
    // GIVEN: Evidence with a PASS verdict
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        qa_passed: Some(true),
        ..Default::default()
    };
    evidence.extra.insert(
        "recordedBase".to_string(),
        workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
    );

    // WHEN: dispose_failure is called with a PASS verdict
    dispose_failure(&mut evidence, &AssayVerdict::Pass);

    // THEN: No disposition is set (PASS doesn't need a disposition)
    assert!(
        evidence.disposition.is_none(),
        "PASS verdict does not set a disposition — it's not a failure"
    );
}

#[test]
fn qa_fail_tool_artifact_has_correct_structure() {
    // GIVEN: Evidence with a FAIL verdict and REPAIR disposition
    let evidence = ForgeGateEvidence {
        candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
        qa_passed: Some(false),
        disposition: Some("REPAIR".into()),
        deliverable_rejection: Some("QA FAIL: test failed".into()),
        ..Default::default()
    };

    // WHEN: Creating a tool artifact from FAIL evidence
    let tool_artifact = forge::roles::qa::assay_tool_artifact(
        "test-story",
        Some("test-run"),
        &evidence,
        AssayVerdict::Fail,
    );

    // THEN: The artifact has the correct structure
    assert_eq!(tool_artifact.tool, "assay");
    assert_eq!(tool_artifact.kind, "qa-assay-evidence");
    assert_eq!(tool_artifact.verdict, Some("FAIL".to_string()));
    assert_eq!(tool_artifact.story_id, "test-story");
    assert_eq!(tool_artifact.story_run_id, Some("test-run".to_string()));
    assert_eq!(
        tool_artifact.sha,
        Some("0123456789abcdef0123456789abcdef01234567".to_string())
    );
    assert!(tool_artifact.summary.is_some());
    assert_eq!(
        tool_artifact.summary.as_deref(),
        Some("QA FAIL: test failed")
    );
}

//! FORGE.ASSAY-009 — assay arithmetic.
//!
//! CONTRACT. The assay's counts are exact and its verdict follows them: `evidence_detail_line`
//! reports `<passed> passed, <failed> failed of <total>` with `passed + failed == total`.
//! `assay_artifact` records observations only: without an approved frozen plan, mixed, green,
//! and empty runs all remain `UNPROVEN`. The per-command observations travel on the artifact
//! row so a later reader can audit them without confusing them with an accepted verdict.
//!
//! Level: L3 Composition — the production evidence builders, results faked.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__009__assay_arithmetic

use forge::engine::assay::CommandResult;
use forge::pianola::worker_exec::{assay_artifact, evidence_detail_line};

fn result(command: &str, passed: bool) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: if passed { 0 } else { 101 },
        cancelled: false,
        passed,
        excerpt: String::new(),
        unmeasurable: false,
        output: String::new(),
    }
}

#[test]
fn forge_assay_009__assay_arithmetic() {
    // ── 1. THE COUNTS ADD UP. ────────────────────────────────────────────────
    let results = vec![
        result("cargo test -p probe --test one", true),
        result("cargo test -p probe --test two", true),
        result("cargo check --workspace", false),
    ];
    let line = evidence_detail_line(&results, "Tests: 2 passed, 1 failed");
    assert!(
        line.contains("(assay: 2 passed, 1 failed of 3)"),
        "passed + failed must equal the total, stated outright: {line}"
    );
    assert!(
        line.starts_with("Tests:"),
        "the line keeps the packet-contract summary marker: {line}"
    );

    // ── 2. OBSERVATIONS DO NOT AWARD ACCEPTANCE WITHOUT A FROZEN PLAN. ────────
    let mixed = assay_artifact("TST-9", "run-1", &results, "2 passed, 1 failed");
    assert_eq!(mixed.tool, "pianola");
    assert_eq!(mixed.kind, "assay");
    assert_eq!(
        mixed.verdict.as_deref(),
        Some("UNPROVEN"),
        "the helper cannot award a verdict without an approved frozen plan"
    );
    let detail = mixed.detail.expect("the artifact carries its arithmetic");
    assert_eq!(
        detail["commands"].as_array().map(Vec::len),
        Some(3),
        "every measured command travels on the row: {detail}"
    );
    assert_eq!(detail["test_artifact_judgment"], "UNPROVEN");
    assert_eq!(detail["product_judgment"], "UNPROVEN");
    let all_green = assay_artifact(
        "TST-9",
        "run-1",
        &[result("cargo test -p probe", true)],
        "all green",
    );
    assert_eq!(
        all_green.verdict.as_deref(),
        Some("UNPROVEN"),
        "even a green observation needs an approved frozen plan"
    );

    // ── 3. NEGATIVE: AN EMPTY RUN REMAINS UNPROVEN, NOT A PASS OR FAIL. ──────
    let empty = assay_artifact("TST-9", "run-1", &[], "nothing ran");
    assert_eq!(
        empty.verdict.as_deref(),
        Some("UNPROVEN"),
        "measuring nothing cannot produce a verdict"
    );
    let empty_line = evidence_detail_line(&[], "Tests: nothing ran");
    assert!(
        empty_line.contains("(assay: 0 passed, 0 failed of 0)"),
        "the empty counts must say zero, not hide: {empty_line}"
    );
}

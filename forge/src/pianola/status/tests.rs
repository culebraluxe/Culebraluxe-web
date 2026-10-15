// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use db::{ForgeStoryHoldRow, ForgeStoryReceiptRow, ToolArtifactRow};

fn receipt(
    id: &str,
    status: Option<&str>,
    summary: Option<&str>,
    commit: Option<&str>,
) -> ForgeStoryReceiptRow {
    ForgeStoryReceiptRow {
        run_id: format!("run-{id}"),
        story_id: id.to_string(),
        result_status: status.map(str::to_string),
        commit_hash: commit.map(str::to_string),
        tests_summary: summary.map(str::to_string),
        completion: None,
        started_at: None,
        ended_at: status.map(|_| "2026-10-04T00:00:00.000Z".to_string()),
        created_at: None,
    }
}

fn hold(story: &str, reason: Option<&str>, failure_class: Option<&str>) -> ForgeStoryHoldRow {
    ForgeStoryHoldRow {
        hold_id: format!("hold-{story}"),
        story_id: story.to_string(),
        reason: reason.map(str::to_string),
        originating_node: None,
        failure_class: failure_class.map(str::to_string),
        resume_target: None,
        created_at: None,
    }
}

fn artifact(
    story: &str,
    kind: &str,
    verdict: Option<&str>,
    summary: Option<&str>,
) -> ToolArtifactRow {
    ToolArtifactRow {
        id: format!("artifact-{story}-{kind}"),
        story_id: story.to_string(),
        story_run_id: None,
        tool: "pianola".to_string(),
        kind: kind.to_string(),
        verdict: verdict.map(str::to_string),
        summary: summary.map(str::to_string),
        sha: None,
        created_at: None,
    }
}

fn four_clean_receipts() -> Vec<ForgeStoryReceiptRow> {
    vec![
        receipt(
            "TST-1",
            Some("Pass"),
            Some("Tests: 4 passed"),
            Some("a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4"),
        ),
        receipt(
            "TST-2",
            Some("Pass"),
            Some("Tests: 3 passed"),
            Some("b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5"),
        ),
        receipt(
            "TST-3",
            Some("Pass"),
            Some("Tests: 5 passed"),
            Some("c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f6"),
        ),
        receipt(
            "TST-4",
            Some("Pass"),
            Some("Tests: 2 passed"),
            Some("d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f607"),
        ),
    ]
}

#[test]
fn counts_attempted_completed_pass_and_compiling_per_story() {
    let receipts = four_clean_receipts();
    let report = summarize_from_receipts(&receipts, &[], &[]);
    assert_eq!(report.stories_attempted, 4);
    assert_eq!(report.stories_completed, 4);
    assert_eq!(report.tests_passing, 4);
    assert_eq!(report.tests_failing, 0);
    assert_eq!(report.tests_compiling.len(), 4);
    assert!(report.tests_compiling.values().all(|compiling| *compiling));
    assert!(report.clean_commits_verified);
}

#[test]
fn template_placeholder_hashes_are_not_verified() {
    // FORGE-FIX-004: the old gate accepted any non-blank hash, so the
    // "aaa"/"bbb" placeholders read as verified commits. Plausible shape
    // (hex, 7..=64 chars) is now required: short placeholders fail it,
    // and so does "same" (not hex).
    let receipts = vec![
        receipt("TST-1", Some("Pass"), Some("Tests: 4 passed"), Some("aaa")),
        receipt("TST-2", Some("Pass"), Some("Tests: 3 passed"), Some("bbb")),
        receipt("TST-3", Some("Pass"), Some("Tests: 5 passed"), Some("same")),
        receipt(
            "TST-4",
            Some("Pass"),
            Some("Tests: 2 passed"),
            Some("e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718"),
        ),
    ];
    let report = summarize_from_receipts(&receipts, &[], &[]);
    assert_eq!(report.stories_completed, 4);
    assert!(
        !report.clean_commits_verified,
        "template hashes must never read as verified commits"
    );
}

const REAL_SHA: &str = "f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293";

fn closure_evidence() -> ClosureEvidence {
    ClosureEvidence {
        commit_resolves: true,
        diff_non_empty: true,
        prod_already_closed: false,
        prod_complete: 4,
        prod_failed: 0,
        batch_size: 4,
    }
}

#[test]
fn closure_gate_rejects_template_hash() {
    // Criterion 1: hash "aaa" → rejected, batch stays open/requeued.
    let row = receipt("TST-1", Some("Pass"), Some("Tests: 4 passed"), Some("aaa"));
    let verdict = verify_receipt_for_closure(&row, &closure_evidence());
    assert!(
        matches!(verdict, ClosureVerdict::Rejected { .. }),
        "unexpected: {verdict:?}"
    );
    assert!(!verdict.closes_batch());
}

#[test]
fn closure_gate_rejects_unresolvable_commit() {
    // Well-formed but never committed: shape passes, existence fails.
    let row = receipt(
        "TST-1",
        Some("Pass"),
        Some("Tests: 4 passed"),
        Some(REAL_SHA),
    );
    let evidence = ClosureEvidence {
        commit_resolves: false,
        ..closure_evidence()
    };
    let verdict = verify_receipt_for_closure(&row, &evidence);
    assert!(
        matches!(verdict, ClosureVerdict::Rejected { .. }),
        "unexpected: {verdict:?}"
    );
    assert!(!verdict.closes_batch());
}

#[test]
fn closure_gate_holds_follow_up_when_prod_counts_short() {
    // Criterion 2: real SHA but PROD short → NOT closed; follow-up.
    let row = receipt(
        "TST-1",
        Some("Pass"),
        Some("Tests: 4 passed"),
        Some(REAL_SHA),
    );
    let evidence = ClosureEvidence {
        prod_complete: 2,
        prod_failed: 0,
        ..closure_evidence()
    };
    let verdict = verify_receipt_for_closure(&row, &evidence);
    assert!(
        matches!(verdict, ClosureVerdict::FollowUp { .. }),
        "unexpected: {verdict:?}"
    );
    assert!(!verdict.closes_batch());
}

#[test]
fn closure_gate_closes_on_real_sha_matching_counts() {
    // Criterion 3: real SHA + matching PROD counts → CLOSED, commit recorded.
    let row = receipt(
        "TST-1",
        Some("Pass"),
        Some("Tests: 4 passed"),
        Some(REAL_SHA),
    );
    let verdict = verify_receipt_for_closure(&row, &closure_evidence());
    assert_eq!(
        verdict,
        ClosureVerdict::Closed {
            commit: REAL_SHA.to_string()
        }
    );
    assert!(verdict.closes_batch());
}

#[test]
fn closure_gate_records_no_change_explicitly() {
    // Empty diff with PROD already closed: closed, but recorded as
    // no-change rather than mistaken for work.
    let row = receipt(
        "TST-1",
        Some("Pass"),
        Some("Tests: 4 passed"),
        Some(REAL_SHA),
    );
    let evidence = ClosureEvidence {
        diff_non_empty: false,
        prod_already_closed: true,
        ..closure_evidence()
    };
    let verdict = verify_receipt_for_closure(&row, &evidence);
    assert_eq!(verdict, ClosureVerdict::NothingToClose);
    assert!(verdict.closes_batch());
}

#[test]
fn closure_gate_refuses_empty_diff_without_prod_already_closed() {
    let row = receipt(
        "TST-1",
        Some("Pass"),
        Some("Tests: 4 passed"),
        Some(REAL_SHA),
    );
    let evidence = ClosureEvidence {
        diff_non_empty: false,
        prod_already_closed: false,
        ..closure_evidence()
    };
    let verdict = verify_receipt_for_closure(&row, &evidence);
    assert!(
        matches!(verdict, ClosureVerdict::FollowUp { .. }),
        "unexpected: {verdict:?}"
    );
    assert!(!verdict.closes_batch());
}

#[test]
fn closure_gate_requires_summary_status_agreement() {
    // Pass word with a failure in the summary: not closed.
    let row = receipt(
        "TST-1",
        Some("Pass"),
        Some("Tests: 4 passed, 1 failed"),
        Some(REAL_SHA),
    );
    let verdict = verify_receipt_for_closure(&row, &closure_evidence());
    assert!(
        matches!(verdict, ClosureVerdict::FollowUp { .. }),
        "unexpected: {verdict:?}"
    );
    // Fail word with matching failure count: close-worthy on this axis.
    let row = receipt(
        "TST-1",
        Some("Fail"),
        Some("Tests: 4 passed, 1 failed"),
        Some(REAL_SHA),
    );
    let evidence = ClosureEvidence {
        prod_complete: 3,
        prod_failed: 1,
        ..closure_evidence()
    };
    let verdict = verify_receipt_for_closure(&row, &evidence);
    assert!(verdict.closes_batch(), "unexpected: {verdict:?}");
    // Indecisive word (Unproven): never closed.
    let row = receipt(
        "TST-1",
        Some("Unproven"),
        Some("Tests: ran"),
        Some(REAL_SHA),
    );
    let verdict = verify_receipt_for_closure(&row, &closure_evidence());
    assert!(!verdict.closes_batch(), "unexpected: {verdict:?}");
}

#[test]
fn prod_decisive_counts_cover_the_batch() {
    let receipts = vec![
        receipt(
            "TST-1",
            Some("Pass"),
            Some("Tests: 1 passed"),
            Some(REAL_SHA),
        ),
        receipt(
            "TST-2",
            Some("Fail"),
            Some("Tests: 1 failed"),
            Some(REAL_SHA),
        ),
        receipt(
            "TST-3",
            Some("Unproven"),
            Some("Tests: ran"),
            Some(REAL_SHA),
        ),
        receipt("TST-4", None, None, None),
    ];
    assert_eq!(count_prod_decisive(&receipts), (1, 1));
}

#[test]
fn failing_and_unproven_statuses_count_honestly() {
    let receipts = vec![
        receipt("TST-1", Some("Pass"), Some("Tests: 1 passed"), Some("aaa")),
        receipt("TST-2", Some("Fail"), Some("Tests: 1 failed"), Some("bbb")),
        receipt("TST-3", Some("Unproven"), Some("Tests: ran"), Some("ccc")),
        receipt("TST-4", None, None, None),
    ];
    let report = summarize_from_receipts(&receipts, &[], &[]);
    assert_eq!(report.stories_attempted, 4);
    // The verdict-less receipt has neither status nor ended_at: attempted, not completed.
    assert_eq!(report.stories_completed, 3);
    assert_eq!(report.tests_passing, 1);
    assert_eq!(report.tests_failing, 1);
    assert_eq!(report.tests_compiling.get("TST-4"), Some(&false));
    assert!(!report.clean_commits_verified);
}

#[test]
fn compile_failure_marker_reads_as_not_compiling() {
    let receipts = vec![receipt(
        "TST-1",
        Some("Fail"),
        Some("error[E0308]: mismatched types, failed to compile"),
        Some("aaa"),
    )];
    let report = summarize_from_receipts(&receipts, &[], &[]);
    assert_eq!(report.tests_compiling.get("TST-1"), Some(&false));
}

#[test]
fn arch_duplicate_and_regression_signals_counted() {
    let receipts = four_clean_receipts();
    let artifacts = vec![
        artifact(
            "TST-1",
            "escalation",
            Some("escalate"),
            Some("ArchConflict: touches Abstract Service"),
        ),
        artifact(
            "TST-2",
            "escalation",
            Some("escalate"),
            Some("OverlappingTarget: TST-1 and TST-2 claim forge/src/x.rs"),
        ),
        artifact(
            "TST-3",
            "qa-assay-evidence",
            Some("FAIL"),
            Some("regression in buyers filter"),
        ),
    ];
    let holds = vec![hold(
        "TST-4",
        Some("waiting on captain"),
        Some("regression"),
    )];
    let report = summarize_from_receipts(&receipts, &holds, &artifacts);
    assert_eq!(report.architecture_violations, 1);
    assert_eq!(report.duplicate_conflicting_work, 1);
    assert_eq!(report.regressions_detected, 2);
    assert!(!report.worker_lane_isolation_verified);
    assert!(report.neon_canonical_preserved);
}

#[test]
fn shared_commit_hash_is_cross_lane_interference() {
    let receipts = vec![
        receipt("TST-1", Some("Pass"), Some("Tests: 1 passed"), Some("same")),
        receipt("TST-2", Some("Pass"), Some("Tests: 1 passed"), Some("same")),
    ];
    let report = summarize_from_receipts(&receipts, &[], &[]);
    assert!(report.cross_lane_interference);
    assert!(!report.worker_lane_isolation_verified);
}

#[test]
fn canonical_threat_clears_neon_flag() {
    let receipts = four_clean_receipts();
    let artifacts = vec![artifact(
        "TST-1",
        "escalation",
        Some("escalate"),
        Some("model proposes CREATE TABLE pianola_queue"),
    )];
    let report = summarize_from_receipts(&receipts, &[], &artifacts);
    assert!(!report.neon_canonical_preserved);
}

#[test]
fn turns_and_cost_parse_from_evidence_text() {
    let receipts = four_clean_receipts();
    let artifacts = vec![
        artifact(
            "TST-1",
            "run-verdict",
            Some("PASS"),
            Some("turns: 8, cost $1.25"),
        ),
        artifact(
            "TST-2",
            "run-verdict",
            Some("PASS"),
            Some("turns: 12, cost $2.75"),
        ),
    ];
    let report = summarize_from_receipts(&receipts, &[], &artifacts);
    assert!((report.average_agent_turns - 10.0).abs() < 0.01);
    assert!((report.approximate_cost_usd - 4.0).abs() < 0.01);
}

#[test]
fn markdown_carries_front_matter_fields_and_wiki_links() {
    let report = summarize_from_receipts(&four_clean_receipts(), &[], &[]);
    let body = render_markdown_report(&report);
    assert!(body.contains("type: report"));
    assert!(body.contains("title: Pianola Status"));
    assert!(body.contains("tags: [pianola, status, experiment]"));
    for field in [
        "stories_attempted",
        "stories_completed",
        "tests_passing",
        "tests_failing",
        "regressions_detected",
        "architecture_violations",
        "duplicate_conflicting_work",
        "average_agent_turns",
        "approximate_cost_usd",
        "worker_lane_isolation_verified",
        "neon_canonical_preserved",
        "cross_lane_interference",
        "clean_commits_verified",
    ] {
        assert!(body.contains(field), "missing field {field}");
    }
    assert!(body.contains("[[Ground-Truth]]"));
    assert!(body.contains("[[Phase-04-Report]]"));
}

#[test]
fn sanitize_story_id_strips_sql_breakout() {
    assert_eq!(sanitize_story_id("TST-1"), "TST-1");
    assert_eq!(
        sanitize_story_id("TST-1'; DROP TABLE x;--"),
        "TST-1DROPTABLEx--"
    );
    assert_eq!(sanitize_story_id("   "), "");
}

//! FORGE-FIX-004 — template/placeholder receipts never close a batch.
//!
//! The fleet supervisor caught workers submitting template receipts (`"aaa"`
//! hashes, counts never cross-checked). This assay pins the closure gate
//! (`forge::pianola::status::verify_receipt_for_closure`) against a real
//! fixture git repo:
//!
//! 1. Receipt with hash `"aaa"` → rejected, batch stays open/requeued.
//! 2. Receipt with a real SHA but PROD counts short → NOT closed; follow-up.
//! 3. Receipt with a real SHA + matching PROD counts → CLOSED, commit recorded.
//!
//! No database is involved: the git probes run against a temp fixture repo
//! and the PROD counts are injected as the evidence the gate requires.

use std::path::{Path, PathBuf};
use std::process::Command;

use db::ForgeStoryReceiptRow;
use forge::pianola::status::{
    commit_diff_non_empty, commit_resolves_in_lane, verify_receipt_for_closure, ClosureEvidence,
    ClosureVerdict,
};

fn receipt(story: &str, status: &str, summary: &str, commit: &str) -> ForgeStoryReceiptRow {
    ForgeStoryReceiptRow {
        run_id: format!("run-{story}"),
        story_id: story.to_string(),
        result_status: Some(status.to_string()),
        commit_hash: Some(commit.to_string()),
        tests_summary: Some(summary.to_string()),
        completion: None,
        started_at: None,
        ended_at: Some("2026-10-08T00:00:00.000Z".to_string()),
        created_at: None,
    }
}

/// A two-commit fixture repo: `base` (first commit) then `sha` (second commit
/// touching a new file, so `base..sha` carries a diff). Returns
/// `(repo_path, base, sha)` with both SHAs resolved from git itself — the
/// only "real" hashes in this assay.
fn fixture_repo() -> (PathBuf, String, String) {
    let dir = std::env::temp_dir().join(format!(
        "forge-receipt-004-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    // Parallel tests in one binary share the clock: start from a clean,
    // unique-per-thread directory.
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("fixture repo dir");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .output()
            .expect("git runs in the fixture repo");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "assay@example.test"]);
    git(&["config", "user.name", "assay"]);
    std::fs::write(dir.join("base.txt"), "base\n").expect("base file");
    git(&["add", "base.txt"]);
    git(&["commit", "-qm", "base"]);
    let base = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_string();
    std::fs::write(dir.join("work.txt"), "work\n").expect("work file");
    git(&["add", "work.txt"]);
    git(&["commit", "-qm", "work"]);
    let sha = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_string();
    assert_ne!(base, sha, "fixture repo owes two distinct commits");
    (dir, base, sha)
}

fn full_batch() -> ClosureEvidence {
    ClosureEvidence {
        commit_resolves: true,
        diff_non_empty: true,
        prod_already_closed: false,
        prod_complete: 4,
        prod_failed: 0,
        batch_size: 4,
    }
}

fn lane_evidence(repo: &Path, base: &str, sha: &str, prod_complete: usize) -> ClosureEvidence {
    ClosureEvidence {
        commit_resolves: commit_resolves_in_lane(repo, sha),
        diff_non_empty: commit_diff_non_empty(repo, base, sha),
        prod_already_closed: false,
        prod_complete,
        prod_failed: 0,
        batch_size: 4,
    }
}

#[test]
fn template_receipt_is_rejected_before_closure() {
    let (repo, base, sha) = fixture_repo();
    // The probes themselves work against the fixture repo: the real commit
    // resolves with a non-empty diff, while "aaa" resolves nowhere.
    assert!(commit_resolves_in_lane(&repo, &sha));
    assert!(commit_diff_non_empty(&repo, &base, &sha));
    assert!(!commit_resolves_in_lane(&repo, "aaa"));

    // Criterion 1: hash "aaa" → rejected, batch stays open/requeued.
    let row = receipt("TST-1", "Pass", "Tests: 4 passed", "aaa");
    let verdict = verify_receipt_for_closure(&row, &lane_evidence(&repo, &base, &sha, 4));
    assert!(
        matches!(verdict, ClosureVerdict::Rejected { .. }),
        "template receipt must be rejected, got: {verdict:?}"
    );
    assert!(
        !verdict.closes_batch(),
        "rejected receipt never closes the batch"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn real_sha_with_short_prod_counts_is_not_closed() {
    let (repo, base, sha) = fixture_repo();

    // Criterion 2: real SHA but PROD counts short → NOT closed; follow-up.
    let row = receipt("TST-1", "Pass", "Tests: 4 passed", &sha);
    let verdict = verify_receipt_for_closure(&row, &lane_evidence(&repo, &base, &sha, 2));
    assert!(
        matches!(verdict, ClosureVerdict::FollowUp { .. }),
        "short PROD counts must hold a follow-up, got: {verdict:?}"
    );
    assert!(!verdict.closes_batch());

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn real_sha_with_matching_prod_counts_is_closed_with_commit() {
    let (repo, base, sha) = fixture_repo();

    // Criterion 3: real SHA + matching PROD counts → CLOSED, commit recorded.
    let row = receipt("TST-1", "Pass", "Tests: 4 passed", &sha);
    let verdict = verify_receipt_for_closure(&row, &lane_evidence(&repo, &base, &sha, 4));
    assert_eq!(
        verdict,
        ClosureVerdict::Closed {
            commit: sha.clone()
        },
        "matching receipt must close the batch with the commit recorded"
    );
    assert!(verdict.closes_batch());

    // The injected full-batch evidence agrees with the lane-probed evidence.
    let direct = verify_receipt_for_closure(&row, &full_batch());
    assert_eq!(direct, verdict);

    std::fs::remove_dir_all(&repo).ok();
}

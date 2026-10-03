//! FORGE.PUBLISH — an integration commit is proven before it reaches `origin/main`.
//!
//! When `origin/main` moved after QA, the publish path builds a merge of the latest main and the QA-approved
//! candidate. That merged tree was never built or tested by anyone, and until 2026-10-03 it was pushed as-is: two
//! candidates that each passed QA could break main together, and only CI — after the fact — would say so.
//!
//! Driven against a real bare remote, never a mock: the claim is about what `origin/main` holds afterwards.
//!
//!   * a proof that passes on the merged tree — the integration commit lands, and it contains both changes;
//!   * a proof that fails on the merged tree — `IntegrationUnverified`, and main does not move;
//!   * no proofs at all — `IntegrationUnverified`, and main does not move (fail closed, never "untested is fine").
//!
//! The remote is advanced by a `fetch` INTO it, not by a client sending to it: `git_publish.rs` is the one file
//! permitted to name that verb (arch_boundary__011), and a fixture is not an exception.
//!
//! Level: L2, real git, temporary repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use forge::engine::git_publish::{publish_candidate, publish_switch_off};
use forge::engine::release::PublishOutcome;
use forge::engine::worktree;

fn run(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(worktree::git_binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A work checkout and its bare `origin`, where `origin/main` has moved past the candidate's base: the candidate
/// adds `candidate.rs` on the old base, and main has since gained `sibling.rs`.
struct Diverged {
    root: PathBuf,
    work: PathBuf,
    remote: PathBuf,
    candidate: String,
    main_before: String,
}

impl Drop for Diverged {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn diverged(name: &str) -> Diverged {
    let switch = std::env::var("FORGE_ALLOW_PUBLISH").ok();
    assert!(
        !publish_switch_off(switch.as_deref()),
        "this test lands commits, so it needs the publish switch unset or on; it is {switch:?}"
    );
    let root = std::env::temp_dir().join(format!("forge-publish-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let work = root.join("work");
    let remote = root.join("origin.git");
    fs::create_dir_all(&work).expect("temp work");
    run(&work, &["init", "--initial-branch=main", "."]);
    run(&work, &["config", "user.email", "forge@test.invalid"]);
    run(&work, &["config", "user.name", "forge test"]);
    run(&work, &["config", "commit.gpgsign", "false"]);
    fs::write(work.join("README.md"), "base\n").expect("seed");
    run(&work, &["add", "."]);
    run(&work, &["commit", "-m", "base"]);
    let work_url = work.to_string_lossy().to_string();
    let remote_url = remote.to_string_lossy().to_string();
    run(&root, &["clone", "--bare", "-q", &work_url, &remote_url]);
    run(&work, &["remote", "add", "origin", &remote_url]);

    // A sibling story lands first: main gains sibling.rs.
    run(&work, &["checkout", "-q", "-b", "sibling"]);
    fs::write(work.join("sibling.rs"), "// landed first\n").expect("sibling");
    run(&work, &["add", "."]);
    run(&work, &["commit", "-m", "sibling story"]);
    run(&remote, &["fetch", "-q", &work_url, "sibling:main"]);

    // The candidate was cut from the old base, as a Smith that started earlier leaves it.
    run(&work, &["checkout", "-q", "main"]);
    run(&work, &["checkout", "-q", "-b", "agent/tst-story/run-1"]);
    fs::write(work.join("candidate.rs"), "// the smith's work\n").expect("candidate");
    run(&work, &["add", "."]);
    run(&work, &["commit", "-m", "smith: candidate"]);
    let candidate = run(&work, &["rev-parse", "HEAD"]);
    let main_before = run(&remote, &["rev-parse", "main"]);
    Diverged {
        root,
        work,
        remote,
        candidate,
        main_before,
    }
}

#[test]
fn a_proven_integration_commit_lands_with_both_changes() {
    let repo = diverged("proven");
    let proofs = vec!["test -f candidate.rs && test -f sibling.rs".to_string()];
    let landed = match publish_candidate(&repo.work, &repo.candidate, &proofs) {
        PublishOutcome::IntegratedAndPublished {
            published_main_hash,
        } => published_main_hash,
        other => panic!("a proven integration must land: {other:?}"),
    };
    assert_eq!(run(&repo.remote, &["rev-parse", "main"]), landed);
    assert_eq!(
        run(&repo.remote, &["show", "main:candidate.rs"]),
        "// the smith's work"
    );
    assert_eq!(
        run(&repo.remote, &["show", "main:sibling.rs"]),
        "// landed first"
    );
}

#[test]
fn an_integration_commit_that_fails_its_proof_never_reaches_main() {
    let repo = diverged("refused");
    let proofs = vec!["test -f candidate.rs".to_string(), "exit 3".to_string()];
    match publish_candidate(&repo.work, &repo.candidate, &proofs) {
        PublishOutcome::IntegrationUnverified { reason, .. } => {
            assert!(
                reason.contains("exit 3"),
                "the failing proof is named: {reason}"
            );
        }
        other => panic!("a failed proof must refuse the publish: {other:?}"),
    }
    assert_eq!(
        run(&repo.remote, &["rev-parse", "main"]),
        repo.main_before,
        "main must not move"
    );
}

#[test]
fn an_integration_commit_with_nothing_to_prove_it_is_refused() {
    let repo = diverged("unprovable");
    match publish_candidate(&repo.work, &repo.candidate, &[]) {
        PublishOutcome::IntegrationUnverified { reason, .. } => {
            assert!(reason.contains("untested merge"), "{reason}");
        }
        other => panic!("no proof is not a pass: {other:?}"),
    }
    assert_eq!(run(&repo.remote, &["rev-parse", "main"]), repo.main_before);
    assert!(
        !run(&repo.work, &["worktree", "list"]).contains("integration-"),
        "no checkout is left behind"
    );
}

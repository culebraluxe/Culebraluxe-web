//! Worker commit identity — the lane's commit is stamped with the forge identity, never the machine
//! placeholder (ported from `legacy/workflow_app/tests/worker-commit-identity.test.ts`).
//!
//! The incident this pins: 228 commits on the real repository (2026-08-23..28) were authored by
//! `Your Name <you@example.com>` — git's fallback when a repository carries no real identity. The old
//! TypeScript fence configured `user.email = eng21@test` in its fixture, so it proved a setup the engine
//! never had and the defect survived a green suite. This test refuses that mistake: the fixture repository
//! is configured with the PLACEHOLDER identity, and the salvage commit must still be stamped with the forge
//! identity because the engine passes it per commit (`rust/forge/src/engine/worktree.rs:364-375`,
//! `-c user.name=forge-salvage -c user.email=forge-salvage@localhost`).
//!
//! Nothing here is mocked. A real repository, a real `origin`, a real disposable worktree, and the same
//! `salvage_worker_workspace` the teardown runs — then `git log` reads the committed truth back.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The forge commit identity the engine stamps, mirrored from `rust/forge/src/engine/worktree.rs:368,370`.
/// Written as literals so this test fails loudly if the seam is re-cut to the machine placeholder.
const FORGE_NAME: &str = "forge-salvage";
const FORGE_EMAIL: &str = "forge-salvage@localhost";
/// git's fallback identity when no real one is configured — the placeholder that must never author a commit.
const PLACEHOLDER_EMAIL: &str = "you@example.com";

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} in {} failed: {}",
        cwd.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A scratch root that cleans itself up, without a `tempfile` dependency (the forge crate's tests carry none).
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!("forge-identity-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("scratch root");
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A repository whose CONFIG IS THE BUG: it carries the placeholder identity git falls back to, on `main`
/// with a bare `origin` it has pushed that commit to, so the engine can provision a worktree off `origin/main`.
fn repo_with_placeholder_identity(scratch: &Path) -> PathBuf {
    let repo = scratch.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.name", "Your Name"]);
    git(&repo, &["config", "user.email", PLACEHOLDER_EMAIL]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("README.md"), "base\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);

    let origin = scratch.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    repo
}

#[test]
fn the_worker_commit_is_stamped_with_the_forge_identity_not_the_machine_placeholder() {
    let scratch = Scratch::new("stamp");
    let repo = repo_with_placeholder_identity(scratch.path());
    let worktrees = scratch.path().join("worktrees");
    let story = "TST-FORGE-IDENTITY-001";
    let run = "run-identity-0001";

    let ws = forge::engine::provision_worker_workspace(
        Some(&repo),
        story,
        Some(run),
        Some("origin/main"),
        Some(&worktrees),
    )
    .expect("the disposable worktree is provisioned off origin/main");
    let worktree = PathBuf::from(&ws.worktree_path);

    // The turn changes a file and dies before it commits — the state the teardown salvage exists for.
    std::fs::write(worktree.join("work.txt"), "the lane changed this\n").unwrap();

    let saved = forge::engine::salvage_worker_workspace(Some(&repo), story, run, Some(&worktrees))
        .expect("salvage reads the sandbox")
        .expect("a dirty sandbox holds unsaved work");
    assert!(saved.made_commit, "the salvage had to create the commit");

    // The committed truth: author and committer are both the forge identity, never the machine placeholder.
    let stamped = git(
        &worktree,
        &["log", "-1", "--format=%an|%ae|%cn|%ce", &saved.commit_sha],
    );
    let fields: Vec<&str> = stamped.split('|').collect();
    assert_eq!(fields.len(), 4, "unexpected commit format: {stamped}");
    let (author_name, author_email, committer_name, committer_email) =
        (fields[0], fields[1], fields[2], fields[3]);
    assert_eq!(author_name, FORGE_NAME, "author name");
    assert_eq!(author_email, FORGE_EMAIL, "author email");
    assert_eq!(committer_name, FORGE_NAME, "committer name");
    assert_eq!(committer_email, FORGE_EMAIL, "committer email");
    assert_ne!(
        author_email, PLACEHOLDER_EMAIL,
        "the placeholder machine identity must never author a commit"
    );
}

#[test]
fn a_clean_turn_writes_no_commit_so_nothing_is_stamped() {
    let scratch = Scratch::new("clean");
    let repo = repo_with_placeholder_identity(scratch.path());
    let worktrees = scratch.path().join("worktrees");
    let story = "TST-FORGE-IDENTITY-002";
    let run = "run-identity-0002";

    let ws = forge::engine::provision_worker_workspace(
        Some(&repo),
        story,
        Some(run),
        Some("origin/main"),
        Some(&worktrees),
    )
    .expect("the disposable worktree is provisioned");
    let worktree = PathBuf::from(&ws.worktree_path);

    // The lane committed and pushed its own branch: there is nothing to salvage, and nothing to stamp.
    git(
        &worktree,
        &[
            "push",
            "-q",
            "origin",
            &format!("HEAD:refs/heads/{}", ws.branch_name),
        ],
    );
    let before = git(&worktree, &["rev-parse", "HEAD"]);

    let saved =
        forge::engine::salvage_worker_workspace(Some(&repo), story, run, Some(&worktrees)).unwrap();
    assert!(
        saved.is_none(),
        "a clean tree already on origin has nothing to save"
    );
    assert_eq!(
        git(&worktree, &["rev-parse", "HEAD"]),
        before,
        "nothing to commit is reported as nothing, and writes no commit"
    );
}

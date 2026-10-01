//! The teardown salvage, end to end — "it does not get to die without giving me code" (captain, 2026-10-01).
//!
//! Nothing here is mocked. The test builds a real repository and a real `origin`, provisions a real disposable
//! worktree the way the engine provisions one, writes a file the way a dying turn does, and then performs the
//! exact `git worktree remove --force` that used to be the end of the code. The assertion is the whole point of
//! the fix: after the sandbox is gone, the code is still reachable from the branch and the patch still replays.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// A scratch root that cleans itself up, without a `tempfile` dependency: the forge crate's tests carry none,
/// and one directory under the system temp root is enough for one repository, its `origin` and its worktrees.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!("forge-salvage-{tag}-{}", std::process::id()));
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

/// A repository with one commit on `main` and a bare `origin` it has pushed that commit to, so `origin/main` is
/// a real ref — which is what the engine provisions a worktree from.
fn repo_with_origin(scratch: &Path) -> PathBuf {
    let repo = scratch.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.name", "test"]);
    git(&repo, &["config", "user.email", "test@localhost"]);
    std::fs::write(repo.join("README.md"), "base\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);

    let origin = scratch.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--bare", "-b", "main"]);
    git(
        &repo,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&repo, &["push", "-q", "-u", "origin", "main"]);
    repo
}

#[test]
fn a_sandbox_that_died_mid_edit_hands_its_code_over_before_it_is_deleted() {
    let scratch = Scratch::new("dirty");
    let repo = repo_with_origin(scratch.path());
    let worktrees = scratch.path().join("worktrees");
    let story = "TST-SALVAGE-001";
    let run = "run-dead-0001";

    let ws = forge::engine::provision_worker_workspace(
        Some(&repo),
        story,
        Some(run),
        Some("origin/main"),
        Some(&worktrees),
    )
    .expect("the disposable worktree is provisioned off origin/main");
    let worktree = PathBuf::from(&ws.worktree_path);

    // The turn writes code and dies before it commits: exactly the state `--force` used to destroy.
    std::fs::write(worktree.join("the-code.rs"), "fn paid() {}\n").unwrap();
    assert!(
        !git(&worktree, &["status", "--porcelain"]).is_empty(),
        "the fixture has to be dirty or this test proves nothing"
    );

    let saved = forge::engine::salvage_worker_workspace(Some(&repo), story, run, Some(&worktrees))
        .expect("salvage reads the sandbox")
        .expect("a dirty sandbox holds unsaved work");

    assert!(saved.made_commit, "the salvage had to create the commit");
    assert!(
        saved.changed_files.iter().any(|f| f == "the-code.rs"),
        "the salvaged diff names the file the turn wrote: {:?}",
        saved.changed_files
    );
    assert!(
        saved.patch.contains("fn paid()"),
        "the patch carries the code itself, so it replays out of the record"
    );

    // Perform the deletion that used to lose the code, and prove the code is not in the sandbox but in the
    // branch: this is the whole requirement, checked against the real removal command.
    git(&repo, &["worktree", "remove", "--force", worktree.to_str().unwrap()]);
    assert!(
        !worktree.exists(),
        "the sandbox is gone, as it is after every run"
    );
    assert_eq!(
        git(&repo, &["show", &format!("{}:the-code.rs", saved.branch_name)]),
        "fn paid() {}",
        "the code survives the sandbox: it is on the branch"
    );
    assert_eq!(
        git(&repo, &["rev-parse", &saved.branch_name]),
        saved.commit_sha,
        "the branch points at the salvage commit"
    );
}

#[test]
fn a_clean_sandbox_already_on_the_remote_has_nothing_to_salvage() {
    let scratch = Scratch::new("clean");
    let repo = repo_with_origin(scratch.path());
    let worktrees = scratch.path().join("worktrees");
    let story = "TST-SALVAGE-002";
    let run = "run-done-0002";

    let ws = forge::engine::provision_worker_workspace(
        Some(&repo),
        story,
        Some(run),
        Some("origin/main"),
        Some(&worktrees),
    )
    .expect("the disposable worktree is provisioned");
    let worktree = PathBuf::from(&ws.worktree_path);

    // The normal path: the turn committed and pushed its own branch. There is nothing left to save, and saving
    // again would only write a second copy of the same facts.
    git(
        &worktree,
        &["push", "-q", "origin", &format!("HEAD:refs/heads/{}", ws.branch_name)],
    );
    let saved =
        forge::engine::salvage_worker_workspace(Some(&repo), story, run, Some(&worktrees)).unwrap();
    assert!(
        saved.is_none(),
        "a clean tree already on origin is already saved"
    );
}

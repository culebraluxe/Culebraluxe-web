//! Disposable Git repositories and worktrees for tests.
//!
//! Tests that exercise a Git boundary — a commit, a publish, a worktree, a dirty tree — must not touch the operator's
//! repository. `DisposableRepo` creates a throwaway repository under the system temp directory, configures it so a
//! commit cannot prompt for a signature, and removes it when dropped. It uses the `git` binary on `PATH`, because the
//! contracts under test are contracts *with Git*, and a re-implementation would prove something Git does not do.
//!
//! If `git` is not on `PATH`, [`git_available`] is false and a test can skip rather than fail: the absence of a tool
//! is an environment fact, not a contract failure.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Whether the `git` binary can be run on this machine.
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A path under the system temp root that no other test shares. Not created; the caller decides.
pub fn unique_temp_path(prefix: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{nanos:x}-{counter}",
        std::process::id()
    ))
}

/// A directory under the system temp root that no other test shares and that this process can remove.
pub fn unique_temp_dir(prefix: &str) -> PathBuf {
    let path = unique_temp_path(prefix);
    std::fs::create_dir_all(&path).expect("a temp directory must be creatable");
    path
}

/// A throwaway Git repository that is removed when dropped.
pub struct DisposableRepo {
    root: PathBuf,
}

impl DisposableRepo {
    /// Create a repository with one initial commit on `main` (branch creation is environment-independent).
    pub fn init() -> io::Result<Self> {
        let root = unique_temp_dir("clx-harness-repo");
        let repo = Self { root };
        repo.git(&["init", "-q", "-b", "main"])?;
        repo.git(&["config", "user.email", "harness@example.test"])?;
        repo.git(&["config", "user.name", "CulebraLuxe Test Harness"])?;
        // A machine with global commit signing must not make a test fixture prompt or fail.
        repo.git(&["config", "commit.gpgsign", "false"])?;
        repo.git(&["config", "tag.gpgsign", "false"])?;
        repo.write("README.md", "# disposable\n")?;
        repo.commit_all("initial")?;
        Ok(repo)
    }

    /// The repository root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Run `git` in the repository, returning stdout on success and an error carrying stderr otherwise.
    pub fn git(&self, args: &[&str]) -> io::Result<String> {
        run_git(&self.root, args)
    }

    /// Write a file relative to the repository root, creating parent directories.
    pub fn write(&self, relative: &str, contents: &str) -> io::Result<()> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }

    /// Stage everything and commit, returning the new commit id.
    pub fn commit_all(&self, message: &str) -> io::Result<String> {
        self.git(&["add", "-A"])?;
        self.git(&["commit", "-q", "-m", message])?;
        self.head_sha()
    }

    /// The current `HEAD` commit id.
    pub fn head_sha(&self) -> io::Result<String> {
        self.git(&["rev-parse", "HEAD"])
            .map(|sha| sha.trim().to_owned())
    }

    /// Whether the working tree has no uncommitted changes.
    pub fn is_clean(&self) -> io::Result<bool> {
        Ok(self.git(&["status", "--porcelain"])?.trim().is_empty())
    }

    /// Add a linked worktree at a detached `HEAD`, owned and removed by the returned handle.
    pub fn add_worktree(&self, name: &str) -> io::Result<DisposableWorktree> {
        let path = unique_temp_path(&format!("clx-harness-wt-{name}"));
        self.git(&[
            "worktree",
            "add",
            "--detach",
            path.to_str().expect("a temp path is UTF-8"),
            "HEAD",
        ])?;
        Ok(DisposableWorktree {
            repo_root: self.root.clone(),
            path,
        })
    }
}

impl Drop for DisposableRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A linked worktree of a [`DisposableRepo`], removed when dropped.
pub struct DisposableWorktree {
    repo_root: PathBuf,
    path: PathBuf,
}

impl DisposableWorktree {
    /// The worktree's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Run `git` in the worktree.
    pub fn git(&self, args: &[&str]) -> io::Result<String> {
        run_git(&self.path, args)
    }

    /// The commit the worktree is checked out at.
    pub fn head_sha(&self) -> io::Result<String> {
        self.git(&["rev-parse", "HEAD"])
            .map(|sha| sha.trim().to_owned())
    }

    /// Write a file relative to the worktree root.
    pub fn write(&self, relative: &str, contents: &str) -> io::Result<()> {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }
}

impl Drop for DisposableWorktree {
    fn drop(&mut self) {
        let _ = run_git(
            &self.repo_root,
            &[
                "worktree",
                "remove",
                "--force",
                self.path.to_str().expect("a temp path is UTF-8"),
            ],
        );
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_git(directory: &Path, args: &[&str]) -> io::Result<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .map_err(|error| {
            io::Error::new(error.kind(), format!("could not run git {args:?}: {error}"))
        })?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {args:?} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disposable_repository_commits_and_cleans_up() {
        if !git_available() {
            eprintln!("skipping: git is not on PATH");
            return;
        }
        let repo = DisposableRepo::init().expect("init");
        assert!(repo.is_clean().unwrap(), "the initial state is clean");
        let first = repo.head_sha().unwrap();

        repo.write("src/lib.rs", "// fixture\n").unwrap();
        let second = repo.commit_all("add a file").unwrap();
        assert_ne!(first, second);
        assert!(repo.is_clean().unwrap());

        let root = repo.path().to_path_buf();
        assert!(root.exists());
        drop(repo);
        assert!(!root.exists(), "the repository is removed with the guard");
    }

    #[test]
    fn a_linked_worktree_is_checked_out_and_removed() {
        if !git_available() {
            eprintln!("skipping: git is not on PATH");
            return;
        }
        let repo = DisposableRepo::init().expect("init");
        let worktree = repo.add_worktree("probe").expect("add worktree");
        assert_eq!(worktree.head_sha().unwrap(), repo.head_sha().unwrap());
        let path = worktree.path().to_path_buf();
        drop(worktree);
        assert!(!path.exists(), "the worktree is removed with the guard");
    }
}

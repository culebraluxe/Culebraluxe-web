//! Shared fixture for the FORGE.WORKSPACE_GIT suite (`forge_workspace_git__0NN__*.rs`).
//!
//! These rails bind the workspace-git boundary — lane provisioning (`engine::worktree`), candidate judgment
//! (`roles::smith`), publish (`engine::git_publish`) and cleanup (`engine::worktree::cleanup_worker_workspace`) —
//! so a later change cannot silently disconnect them.
//!
//! The composition is PRODUCTION: `provision_worker_workspace`, `judge_delivered_candidate`, `publish_candidate`
//! and `cleanup_worker_workspace` are the same functions the engine calls. Only the git repository is disposable
//! (a throwaway repo under the system temp directory, removed on drop), because the contracts under test are
//! contracts *with Git* and a re-implementation would prove something Git does not do.
//!
//! Included with `#[path = "support/forge_workspace_git.rs"] mod workspace_git;` — a directory under `tests/` is
//! not a target.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use forge::engine::assay::CommandResult;
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use test_harness::git::DisposableRepo;

/// The story id used across the suite. Any valid story id works; the lane branch derives from it.
pub const STORY_ID: &str = "TST-FORGE-WORKSPACE-GIT";
/// The run id used across the suite.
pub const RUN_ID: &str = "run-001";

/// A disposable repository plus the lane provisioned inside it, with the `RoleHarness` implementation
/// that lets production git-boundary functions run against it.
pub struct LaneFixture {
    repo: DisposableRepo,
    /// The lane's branch name, `agent/<story>/<run>`.
    pub branch_name: String,
    /// The lane's worktree path.
    pub worktree_path: PathBuf,
    /// The base commit the lane was provisioned from.
    pub base_commit: String,
}

impl LaneFixture {
    /// Create a disposable repo with one initial commit on `main`, then provision a TST lane from it.
    ///
    /// The lane is provisioned by the production `provision_worker_workspace`, so the branch name, worktree
    /// path and base commit are the engine's own decisions, not a re-declaration of them.
    pub fn provision() -> Self {
        let repo = DisposableRepo::init().expect("a disposable repository is created");
        let base_commit = repo.head_sha().expect("the initial commit resolves");
        let worktrees_root = test_harness::git::unique_temp_path("clx-wsgit-worktrees");
        let lane = forge::engine::worktree::provision_worker_workspace(
            Some(repo.path()),
            STORY_ID,
            Some(RUN_ID),
            Some("main"),
            Some(&worktrees_root),
        )
        .expect("the lane is provisioned");
        Self {
            repo,
            branch_name: lane.branch_name,
            worktree_path: lane.worktree_path,
            base_commit,
        }
    }

    /// The disposable repository, for git operations the fixture does not wrap.
    pub fn repo(&self) -> &DisposableRepo {
        &self.repo
    }

    /// The lane's worktree path.
    pub fn worktree(&self) -> &Path {
        &self.worktree_path
    }

    /// Write a file in the lane worktree and commit it, returning the new commit id.
    pub fn commit_in_lane(&self, relative: &str, contents: &str) -> String {
        let worktree = self.worktree();
        let path = worktree.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory is created");
        }
        std::fs::write(&path, contents).expect("a file is written in the lane");
        let add = forge::engine::worktree::git_binary();
        let status = std::process::Command::new(&add)
            .args(["add", "-A"])
            .current_dir(worktree)
            .output()
            .expect("git add runs");
        assert!(status.status.success(), "git add succeeds in the lane");
        let status = std::process::Command::new(&add)
            .args(["commit", "-q", "-m", "lane work"])
            .current_dir(worktree)
            .output()
            .expect("git commit runs");
        assert!(status.status.success(), "git commit succeeds in the lane");
        let sha = std::process::Command::new(&add)
            .args(["rev-parse", "HEAD"])
            .current_dir(worktree)
            .output()
            .expect("git rev-parse runs");
        assert!(sha.status.success(), "git rev-parse succeeds in the lane");
        String::from_utf8_lossy(&sha.stdout).trim().to_string()
    }

    /// The commit id the lane's branch currently points at.
    pub fn lane_head(&self) -> String {
        let sha = std::process::Command::new(forge::engine::worktree::git_binary())
            .args(["rev-parse", &self.branch_name])
            .current_dir(self.repo.path())
            .output()
            .expect("git rev-parse runs");
        assert!(sha.status.success(), "the lane branch resolves");
        String::from_utf8_lossy(&sha.stdout).trim().to_string()
    }

    /// A `RoleHarness` whose git runs inside the lane worktree, so production judgment and publish
    /// functions see the lane as their workspace.
    pub fn harness(&self) -> LaneHarness {
        LaneHarness {
            worktree: self.worktree_path.clone(),
            base_commit: self.base_commit.clone(),
        }
    }
}

/// A `RoleHarness` that runs git inside the lane worktree. This is the seam production uses: the
/// candidate judgment and publish path receive git facts from the harness, never from a second
/// implementation of their own.
pub struct LaneHarness {
    worktree: PathBuf,
    base_commit: String,
}

impl RoleHarness for LaneHarness {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        panic!("the workspace-git suite judges candidates directly; no model turn is run")
    }

    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        false
    }

    fn assay_cwd(&self) -> &Path {
        &self.worktree
    }

    fn execution_base_commit(&self) -> Option<&str> {
        Some(&self.base_commit)
    }

    fn run_command(&self, command: &str) -> CommandResult {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.worktree)
            .output();
        match output {
            Ok(out) => CommandResult {
                command: command.to_string(),
                exit_code: out.status.code().unwrap_or(-1),
                passed: out.status.success(),
                excerpt: String::from_utf8_lossy(&out.stderr).trim().to_string(),
                unmeasurable: false,
                output: String::from_utf8_lossy(&out.stdout).into_owned(),
            },
            Err(error) => CommandResult {
                command: command.to_string(),
                exit_code: -1,
                passed: false,
                excerpt: error.to_string(),
                unmeasurable: false,
                output: String::new(),
            },
        }
    }

    fn candidate_probe(&self) -> Option<&dyn forge::engine::runner::CandidateProbe> {
        Some(self)
    }
}

impl forge::engine::runner::CandidateProbe for LaneHarness {
    fn git(&self, args: &[&str]) -> Option<String> {
        let output = std::process::Command::new(forge::engine::worktree::git_binary())
            .args(args)
            .current_dir(&self.worktree)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn declared_test_mode(&self) -> Option<&str> {
        None
    }
}

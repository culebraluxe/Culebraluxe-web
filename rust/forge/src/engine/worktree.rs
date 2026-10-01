//! Port of lib/worker-workspace/provisioner.ts naming + provision.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const GIT_BRANCH_PREFIX: &str = "agent/";
pub const DEFAULT_WORKTREES_DIRNAME: &str = "culebraluxe-forge-worktrees";

#[derive(Debug, Clone)]
pub struct WorkerWorkspace {
    pub repo_root: PathBuf,
    pub worktree_path: PathBuf,
    pub branch_name: String,
    pub base_ref: String,
    pub base_commit: String,
    pub run_id: String,
}

pub fn git_binary() -> String {
    if let Ok(explicit) = std::env::var("FORGE_GIT_BIN") {
        let t = explicit.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    for c in [
        "/usr/bin/git",
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
    ] {
        if Path::new(c).exists() {
            return c.to_string();
        }
    }
    "git".into()
}

pub fn resolve_approved_base_ref() -> String {
    std::env::var("AGENT_WORKSPACE_BASE_REF")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "origin/main".into())
}

pub fn sanitize_branch_segment(input: &str, max: usize) -> String {
    let mut cleaned = String::new();
    for ch in input.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            cleaned.push(ch);
        } else {
            cleaned.push('-');
        }
    }
    while cleaned.contains("--") {
        cleaned = cleaned.replace("--", "-");
    }
    let cleaned = cleaned.trim_matches('-');
    let base = if cleaned.is_empty() { "story" } else { cleaned };
    base.chars().take(max).collect()
}

pub fn derive_run_id(run_id: Option<&str>) -> String {
    if let Some(raw) = run_id.map(str::trim).filter(|s| !s.is_empty()) {
        return sanitize_branch_segment(raw, 60);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{:x}", d.as_millis()))
        .unwrap_or_else(|_| "0".into());
    format!("run-{stamp}")
}

pub fn derive_branch_name(story_id: &str, run_id: &str) -> String {
    format!(
        "{GIT_BRANCH_PREFIX}{}/{}",
        sanitize_branch_segment(story_id, 60),
        sanitize_branch_segment(run_id, 60)
    )
}

pub fn derive_worktree_path(worktrees_root: &Path, story_id: &str, run_id: &str) -> PathBuf {
    worktrees_root.join(format!(
        "{}-{}",
        sanitize_branch_segment(story_id, 60),
        sanitize_branch_segment(run_id, 60)
    ))
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(git_binary())
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Resolve `base_ref` to a 40-char commit. Does not create a worktree.
/// The live engine stamps this onto `storyboard_story_run.base_commit_hash` even when
/// `FORGE_PROVISION` is off (NO TREES): the receipt still has to name the base the run read.
pub fn resolve_base_commit(repo_root: &Path, base_ref: &str) -> Result<String, String> {
    let base_commit = git(
        repo_root,
        &["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")],
    )
    .map_err(|e| format!("base ref {base_ref:?} could not be resolved to a commit: {e}"))?;
    if base_commit.len() != 40 || !base_commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "base ref {base_ref:?} could not be resolved to a commit."
        ));
    }
    Ok(base_commit)
}

pub fn resolve_repo_root(repo_root: Option<&Path>) -> Result<PathBuf, String> {
    let cwd = repo_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    git(&cwd, &["rev-parse", "--git-dir"])?;
    let common = git(&cwd, &["rev-parse", "--git-common-dir"])?;
    let common_dir = if Path::new(&common).is_absolute() {
        PathBuf::from(common)
    } else {
        cwd.join(common)
    };
    Ok(common_dir.parent().unwrap_or(&cwd).to_path_buf())
}

pub fn provision_worker_workspace(
    repo_root: Option<&Path>,
    story_id: &str,
    run_id: Option<&str>,
    base_ref: Option<&str>,
    worktrees_root: Option<&Path>,
) -> Result<WorkerWorkspace, String> {
    let repo_root = resolve_repo_root(repo_root)?;
    let base_ref = base_ref
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    if base_ref.is_empty() {
        return Err("baseRef is required: pass an explicit approved integration base.".into());
    }
    let base_commit = resolve_base_commit(&repo_root, &base_ref)?;
    let run = derive_run_id(run_id);
    let branch_name = derive_branch_name(story_id, &run);
    let root = worktrees_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(DEFAULT_WORKTREES_DIRNAME));
    if root == repo_root || root.starts_with(&repo_root) {
        return Err(format!(
            "worktreesRoot must be OUTSIDE the primary checkout: {}",
            root.display()
        ));
    }
    let worktree_path = derive_worktree_path(&root, story_id, &run);
    if worktree_path.starts_with(&repo_root) {
        return Err(format!(
            "worktree path must be outside the primary checkout: {}",
            worktree_path.display()
        ));
    }
    if git(
        &repo_root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ],
    )
    .is_ok()
    {
        return Err(format!("branch already exists unexpectedly: {branch_name}"));
    }
    if worktree_path.exists() {
        return Err(format!(
            "worktree path already exists unexpectedly: {}",
            worktree_path.display()
        ));
    }
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    git(
        &repo_root,
        &[
            "worktree",
            "add",
            "-b",
            &branch_name,
            worktree_path.to_str().unwrap_or(""),
            &base_commit,
        ],
    )?;
    Ok(WorkerWorkspace {
        repo_root,
        worktree_path,
        branch_name,
        base_ref,
        base_commit,
        run_id: run,
    })
}

/// Remove the disposable execution worktree for one scheduled story. The branch is deleted only after its
/// candidate is contained in origin/main; an unpublished/held candidate keeps its branch so paid code is not lost.
pub fn cleanup_worker_workspace(
    repo_root: Option<&Path>,
    story_id: &str,
    run_id: &str,
) -> Result<(), String> {
    let repo_root = resolve_repo_root(repo_root)?;
    let run = derive_run_id(Some(run_id));
    let branch_name = derive_branch_name(story_id, &run);
    let root = std::env::temp_dir().join(DEFAULT_WORKTREES_DIRNAME);
    let worktree_path = derive_worktree_path(&root, story_id, &run);

    if worktree_path.exists() {
        git(
            &repo_root,
            &[
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap_or(""),
            ],
        )?;
    }
    let _ = git(&repo_root, &["worktree", "prune"]);

    // Refresh only the tracking ref used to decide whether the candidate is safely reachable from main.
    let _ = git(&repo_root, &["fetch", "origin", "main"]);
    let branch_exists = git(
        &repo_root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ],
    )
    .is_ok();
    if branch_exists {
        let candidate = git(&repo_root, &["rev-parse", &branch_name])?;
        if git(
            &repo_root,
            &["merge-base", "--is-ancestor", &candidate, "origin/main"],
        )
        .is_ok()
        {
            git(&repo_root, &["branch", "-D", &branch_name])?;
        }
    }

    if root.exists()
        && std::fs::read_dir(&root)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(&root);
    }
    Ok(())
}

/// The code a teardown pulled out of a sandbox that was about to be deleted.
///
/// This is the record of a lane that did **not** finish the normal way. Every normal exit commits, and the
/// committed path records the diff itself; this is what survives when the turn did not get that far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SalvagedWork {
    pub branch_name: String,
    pub commit_sha: String,
    pub base_sha: String,
    pub changed_files: Vec<String>,
    pub patch: String,
    /// True when the salvage had to *create* the commit — the turn died mid-edit and `worktree remove --force`
    /// was about to erase the only copy. False when it kept a commit the turn had already made but never
    /// recorded. Both save the code; the difference is what a reader is told.
    pub made_commit: bool,
}

/// The whole policy of the salvage, in one place: does this sandbox hold work that is not already safely out?
///
/// A dirty tree says yes — that is code a `--force` delete would erase. A clean tree whose HEAD is not the head
/// `origin` already has says yes too — that is a commit the normal path never got to record. Only a clean tree
/// sitting on the head `origin` already has is safe to delete unsaved: the committed path already saved it, and
/// saving again would only write a second copy of the same facts.
///
/// Its own function because it is the entire decision, and a decision is the part a unit test can hold without
/// a repository.
fn sandbox_holds_unsaved_work(dirty: &str, head: &str, remote_head: Option<&str>) -> bool {
    if !dirty.trim().is_empty() {
        return true;
    }
    match remote_head {
        Some(remote) => remote.trim() != head.trim(),
        None => true,
    }
}

/// Take the last way for paid code to die inside a sandbox out of the picture, before the sandbox is deleted.
///
/// `cleanup_worker_workspace` removes the worktree with `--force`, which erases an uncommitted working tree and,
/// with it, the only copy of whatever a lane wrote before it died — a crash, a kill, a context that ran out
/// mid-edit. That is the one exit the normal path does not cover, because the normal path *is* the commit. This
/// commits what is dirty, on the lane's own `agent/*` branch, so the branch carries the code instead of the
/// doomed tree, and hands the diff back so the caller can write the same fail-safe row the committed path
/// writes. A worker branch with no commit is one `cleanup_worker_workspace` has no reason to keep; a branch that
/// carries a salvage commit is kept, because its candidate is not yet contained in `origin/main`.
///
/// Returns `None` when there is nothing that is not already out: a clean tree on an already-pushed head.
///
/// `worktrees_root` names where the sandbox was provisioned; `None` uses the same temp root
/// `provision_worker_workspace` defaults to. It is a parameter for the same reason it is there: a caller that
/// provisioned into a chosen root has to be able to salvage out of it, and a test needs a root of its own.
pub fn salvage_worker_workspace(
    repo_root: Option<&Path>,
    story_id: &str,
    run_id: &str,
    worktrees_root: Option<&Path>,
) -> Result<Option<SalvagedWork>, String> {
    let repo_root = resolve_repo_root(repo_root)?;
    let run = derive_run_id(Some(run_id));
    let branch_name = derive_branch_name(story_id, &run);
    let root = worktrees_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(DEFAULT_WORKTREES_DIRNAME));
    let worktree_path = derive_worktree_path(&root, story_id, &run);
    if !worktree_path.exists() {
        return Ok(None);
    }

    let dirty = git(&worktree_path, &["status", "--porcelain"])?;
    let head = git(&worktree_path, &["rev-parse", "HEAD"])?;
    // The tracking ref the decision reads. Best effort: a `fetch` that cannot reach the remote must not stop the
    // salvage, because the code in the tree is the thing that cannot be fetched again. With no `origin/<branch>`
    // the check reads as unsaved — the safe direction.
    let _ = git(&repo_root, &["fetch", "origin", "main"]);
    let remote_head = git(
        &repo_root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("origin/{branch_name}"),
        ],
    )
    .ok();
    if !sandbox_holds_unsaved_work(&dirty, &head, remote_head.as_deref()) {
        return Ok(None);
    }

    // A half-finished commit beats no commit: it can be fixed later, and the alternative is `--force` deleting
    // it. `-c user.name/email` because a sandbox may carry no configured identity, and a commit that cannot be
    // made is code that cannot be saved.
    let made_commit = !dirty.trim().is_empty();
    if made_commit {
        git(&worktree_path, &["add", "-A"])?;
        git(
            &worktree_path,
            &[
                "-c",
                "user.name=forge-salvage",
                "-c",
                "user.email=forge-salvage@localhost",
                "commit",
                "-m",
                &format!("salvage({story_id}): uncommitted work at teardown"),
            ],
        )?;
    }
    let commit_sha = git(&worktree_path, &["rev-parse", "HEAD"])?;

    // The deliverable is the whole branch against the approved integration base, not just the last commit: a
    // salvage that saved only the final commit would hand back a fragment.
    let base_sha =
        git(&repo_root, &["merge-base", "origin/main", &commit_sha]).unwrap_or_else(|_| commit_sha.clone());
    let patch = git(
        &worktree_path,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--binary",
            &format!("{base_sha}..{commit_sha}"),
        ],
    )
    .unwrap_or_default();
    let changed_files: Vec<String> = git(
        &worktree_path,
        &["diff", "--name-only", &format!("{base_sha}..{commit_sha}")],
    )
    .unwrap_or_default()
    .lines()
    .map(|line| line.trim().to_string())
    .filter(|line| !line.is_empty())
    .collect();

    Ok(Some(SalvagedWork {
        branch_name,
        commit_sha,
        base_sha,
        changed_files,
        patch,
        made_commit,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Ported from `legacy/workflow_app/tests/worker-workspace-branch-naming.test.ts`.
    ///
    /// REGRESSION (2026-09-10 "mangled branch" incident, ENG-FORGE-SPLIT-01): a SPLIT child's run id is
    /// `<uuid 36>-e<generation>-split-<slot>`. The TypeScript branch name truncated the run id to 40 chars,
    /// which cut the `-split-N` suffix off, so BOTH siblings derived the SAME branch (`…-e0-`) and the
    /// second child was refused as an attempt to steal another workspace. Two children must never share a
    /// branch. The bound here is 60, so the suffix survives — and this test fails if it is ever lowered
    /// back under it.
    #[test]
    fn split_siblings_derive_different_branch_names() {
        let uuid = "f8aa02a6-6cf7-4506-9a58-b8b648df7fe8"; // 36 chars
        let a = derive_branch_name("eng-forge-split-dogfood-01", &format!("{uuid}-e0-split-0"));
        let b = derive_branch_name("eng-forge-split-dogfood-01", &format!("{uuid}-e0-split-1"));
        assert_ne!(a, b, "two split children must never share a branch");
        assert!(a.ends_with("-split-0"), "{a}");
        assert!(b.ends_with("-split-1"), "{b}");
    }

    /// `<uuid>-e0` is 39 chars: it was never truncated and must not change shape.
    #[test]
    fn a_serial_run_id_is_unaffected_by_the_split_fix() {
        let uuid = "f8aa02a6-6cf7-4506-9a58-b8b648df7fe8";
        let serial = derive_branch_name("some-story", &format!("{uuid}-e0"));
        assert_eq!(serial, format!("agent/some-story/{uuid}-e0"));
    }

    #[test]
    fn a_replan_generation_survives() {
        let uuid = "f8aa02a6-6cf7-4506-9a58-b8b648df7fe8";
        let g0 = derive_branch_name("s", &format!("{uuid}-e0-split-0"));
        let g1 = derive_branch_name("s", &format!("{uuid}-e1-split-0"));
        assert_ne!(g0, g1);
    }

    #[test]
    fn sanitization_still_bounds_the_segment() {
        assert_eq!(sanitize_branch_segment("A/B C", 10), "a-b-c");
    }

    /// The salvage's whole decision, without a repository: dirty is always unsaved, a clean tree at an unpushed
    /// head is unsaved, and only a clean tree on the head `origin` already has is safe to delete.
    #[test]
    fn only_a_clean_tree_on_a_pushed_head_is_safe_to_delete() {
        // Dirty always has something to save, even if the head matches the remote.
        assert!(sandbox_holds_unsaved_work(
            " M rust/forge/src/x.rs\n",
            "abc",
            Some("abc")
        ));
        // Clean but never pushed: the commit exists only here.
        assert!(sandbox_holds_unsaved_work("", "abc", None));
        assert!(sandbox_holds_unsaved_work("", "abc", Some("def")));
        // Clean and already on the remote: the committed path already saved it.
        assert!(!sandbox_holds_unsaved_work("", "abc", Some("abc")));
        assert!(!sandbox_holds_unsaved_work("  \n", "abc", Some("abc")));
    }
}

pub fn git_changed_files(repo: &std::path::Path, base: &str, sha: &str) -> Vec<String> {
    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["diff", "--name-only", &format!("{base}...{sha}")])
        .output()
        .ok();
    out.map(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

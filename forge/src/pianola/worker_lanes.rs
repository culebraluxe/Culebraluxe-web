//! TST worker lane provisioning wrapper.
//!
//! Thin wrapper over `crate::engine::worktree` for the Pianola TST
//! experiment. Each worker operates in its own Forge-provisioned
//! worktree/lane; Neon (`storyboard_story_run`, `forge_tool_artifact`)
//! remains the only workflow authority.
//!
//! Lane layout note: the task spec names a `Culebraluxe-worktrees`
//! sibling of the repo root, outside the primary checkout. That matches
//! the captain exception (2026-10-01) allowing one disposable Git
//! worktree per story as an isolation sandbox: the worktree is removed
//! when the child run ends, no lane reads another story's worktree, and
//! only an unpublished/held candidate retains its branch. This module
//! enforces the outside-checkout guard itself and never invents a new
//! queue, table, or release authority.
//!
//! Path note: the playbook names this file `rust/forge/src/pianola/...`.
//! The workspace root is the repo root (`members = ["forge", ...]`), so
//! the canonical path is `forge/src/pianola/worker_lanes.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::worktree::{
    derive_branch_name, git_binary, provision_worker_workspace, resolve_approved_base_ref,
    resolve_repo_root,
};

/// Sibling directory name for TST lanes, placed beside the repo root so it
/// can never live inside the primary checkout.
pub const TST_WORKTREES_DIRNAME: &str = "Culebraluxe-worktrees";

/// Worker workspace for one TST story lane. Mirrors the engine
/// `WorkerWorkspace` fields the worker contract needs.
#[derive(Debug, Clone)]
pub struct WorkerWorkspace {
    pub repo_root: PathBuf,
    pub worktree_path: PathBuf,
    pub branch_name: String,
    pub base_ref: String,
    pub base_commit: String,
    pub run_id: String,
}

/// Sibling worktrees root for `repo_root`: `<parent-of-repo>/Culebraluxe-worktrees`.
pub fn worktrees_root_for_repo(repo_root: &Path) -> PathBuf {
    match repo_root.parent() {
        Some(parent) => parent.join(TST_WORKTREES_DIRNAME),
        None => PathBuf::from(TST_WORKTREES_DIRNAME),
    }
}

/// Reject a worktrees root that sits inside (or is) the primary checkout.
pub fn check_worktrees_root_outside_checkout(
    repo_root: &Path,
    worktrees_root: &Path,
) -> Result<(), String> {
    if worktrees_root == repo_root || worktrees_root.starts_with(repo_root) {
        return Err(format!(
            "worktrees root must be OUTSIDE the primary checkout (repo={}, root={}): refusing to nest lanes inside the checkout",
            repo_root.display(),
            worktrees_root.display()
        ));
    }
    Ok(())
}

/// Provision one TST worker lane.
///
/// * `base_ref`: explicit approved base. When empty, falls back to
///   `AGENT_WORKSPACE_BASE_REF` or `origin/main` via
///   `resolve_approved_base_ref()`.
/// * Branch name comes from `derive_branch_name(story_id, run_id)` and is
///   therefore `agent/<sanitized-story>/<sanitized-run>`.
/// * The worktrees root is the `Culebraluxe-worktrees` sibling of the
///   resolved repo root, never a directory inside the checkout.
pub fn provision_tst_lane(
    repo_root: &Path,
    story_id: &str,
    run_id: &str,
    base_ref: &str,
) -> Result<WorkerWorkspace, String> {
    let repo_root = resolve_repo_root(Some(repo_root))?;
    let effective_base = {
        let trimmed = base_ref.trim();
        if trimmed.is_empty() {
            resolve_approved_base_ref()
        } else {
            trimmed.to_string()
        }
    };
    let worktrees_root = worktrees_root_for_repo(&repo_root);
    check_worktrees_root_outside_checkout(&repo_root, &worktrees_root)?;
    let branch_name = derive_branch_name(story_id, run_id);
    if !branch_name.starts_with("agent/") {
        return Err(format!(
            "derived branch {branch_name:?} must live under agent/"
        ));
    }
    let engine_ws = provision_worker_workspace(
        Some(&repo_root),
        story_id,
        Some(run_id),
        Some(&effective_base),
        Some(&worktrees_root),
    )?;
    Ok(WorkerWorkspace {
        repo_root: engine_ws.repo_root,
        worktree_path: engine_ws.worktree_path,
        branch_name: engine_ws.branch_name,
        base_ref: engine_ws.base_ref,
        base_commit: engine_ws.base_commit,
        run_id: engine_ws.run_id,
    })
}

/// True when two lanes are isolated: canonicalized paths are disjoint
/// (neither prefixes the other) and no file reachable at depth <= 1 under
/// both lanes shares the same `(dev, ino)`.
pub fn verify_lane_isolation(lane_a: &Path, lane_b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| absolutize(p));
    let a = canon(lane_a);
    let b = canon(lane_b);
    if a == b || a.starts_with(&b) || b.starts_with(&a) {
        return false;
    }
    // Inode-level check: the lane roots plus their direct children must not
    // share an identity. A shared inode means the "two" lanes are the same
    // directory (symlink, bind mount) or share files.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Re-scan per lane and compare sets to avoid intra-lane hardlink
        // false positives.
        let set_for = |lane: &PathBuf| {
            let mut set = std::collections::HashSet::new();
            let mut candidates = vec![lane.clone()];
            if let Ok(entries) = std::fs::read_dir(lane) {
                for entry in entries.flatten().take(200) {
                    candidates.push(entry.path());
                }
            }
            for path in candidates {
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_symlink() {
                        continue;
                    }
                    set.insert((meta.dev(), meta.ino()));
                }
            }
            set
        };
        let set_a = set_for(&a);
        let set_b = set_for(&b);
        if !set_a.is_disjoint(&set_b) {
            return false;
        }
    }
    true
}

fn absolutize(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

/// True when `origin/main` has moved past `base_commit`.
///
/// Runs `git rev-list <base>..origin/main --count` in the resolved repo and
/// reports whether the count is non-zero. A fetch refusal or unknown ref is
/// an `Err`, never a silent "not moved".
pub fn detect_mainline_movement(base_commit: &str) -> Result<bool, String> {
    let base = base_commit.trim();
    if base.is_empty() {
        return Err("detect_mainline_movement: base_commit must not be empty".into());
    }
    let repo_root = resolve_repo_root(None)?;
    detect_mainline_movement_in(&repo_root, base)
}

fn detect_mainline_movement_in(repo_root: &Path, base_commit: &str) -> Result<bool, String> {
    let out = Command::new(git_binary())
        .args([
            "rev-list",
            &format!("{base_commit}..origin/main"),
            "--count",
        ])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("git rev-list base..origin/main: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "git rev-list base..origin/main failed: {}",
            err.trim()
        ));
    }
    let count: u64 = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(|e| format!("git rev-list answered a non-count: {e}"))?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::worktree::sanitize_branch_segment;

    #[test]
    fn branch_name_is_agent_scoped_and_sanitized() {
        let branch = derive_branch_name("TST-ABC 01", "RUN 02/x");
        assert!(branch.starts_with("agent/"), "{branch}");
        assert!(!branch.contains(' '), "{branch}");
        // Uppercase is lowered by the sanitizer.
        assert_eq!(branch, branch.to_ascii_lowercase(), "{branch}");
    }

    #[test]
    fn sanitize_branch_segment_collapses_and_trims() {
        assert_eq!(sanitize_branch_segment("TST Foo__Bar!!", 60), "tst-foo-bar");
        assert_eq!(sanitize_branch_segment("---", 60), "story");
        assert_eq!(sanitize_branch_segment("", 60), "story");
        let long = "a".repeat(100);
        assert_eq!(sanitize_branch_segment(&long, 60).len(), 60);
    }

    #[test]
    fn worktrees_root_is_sibling_outside_checkout() {
        let repo = Path::new("/tmp/repo-check/Culebraluxe-web");
        let root = worktrees_root_for_repo(repo);
        assert_eq!(root, Path::new("/tmp/repo-check/Culebraluxe-worktrees"));
        assert!(check_worktrees_root_outside_checkout(repo, &root).is_ok());
    }

    #[test]
    fn rejects_worktrees_root_inside_checkout() {
        let repo = Path::new("/tmp/repo-check/Culebraluxe-web");
        let inside = repo.join("Culebraluxe-worktrees");
        let err = check_worktrees_root_outside_checkout(repo, &inside).unwrap_err();
        assert!(err.contains("OUTSIDE"), "unexpected: {err}");
        let same = check_worktrees_root_outside_checkout(repo, repo).unwrap_err();
        assert!(same.contains("OUTSIDE"), "unexpected: {same}");
    }

    #[test]
    fn lane_isolation_rejects_nested_and_same_paths() {
        let dir = std::env::temp_dir().join(format!("pianola-lane-iso-{}", std::process::id()));
        let a = dir.join("lane-a");
        let b = dir.join("lane-a").join("nested");
        std::fs::create_dir_all(&b).expect("fixture");
        assert!(!verify_lane_isolation(&a, &b));
        assert!(!verify_lane_isolation(&a, &a));
        let c = dir.join("lane-c");
        std::fs::create_dir_all(&c).expect("fixture");
        assert!(verify_lane_isolation(&a, &c));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_mainline_movement_rejects_empty_base() {
        assert!(detect_mainline_movement("").is_err());
        assert!(detect_mainline_movement("   ").is_err());
    }
}

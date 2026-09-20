//! Port of lib/worker-workspace/provisioner.ts naming + provision.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const GIT_BRANCH_PREFIX: &str = "agent/";
pub const DEFAULT_WORKTREES_DIRNAME: &str = "Culebraluxe-worktrees";

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
    for c in ["/usr/bin/git", "/opt/homebrew/bin/git", "/usr/local/bin/git"] {
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
    let base_commit = git(&repo_root, &["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")])
        .map_err(|e| format!("base ref {base_ref:?} could not be resolved to a commit: {e}"))?;
    if base_commit.len() != 40 || !base_commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("base ref {base_ref:?} could not be resolved to a commit."));
    }
    let run = derive_run_id(run_id);
    let branch_name = derive_branch_name(story_id, &run);
    let root = worktrees_root
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.parent().unwrap_or(&repo_root).join(DEFAULT_WORKTREES_DIRNAME));
    if root == repo_root || root.starts_with(&repo_root) {
        return Err(format!("worktreesRoot must be OUTSIDE the primary checkout: {}", root.display()));
    }
    let worktree_path = derive_worktree_path(&root, story_id, &run);
    if worktree_path.starts_with(&repo_root) {
        return Err(format!("worktree path must be outside the primary checkout: {}", worktree_path.display()));
    }
    if git(&repo_root, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch_name}")]).is_ok() {
        return Err(format!("branch already exists unexpectedly: {branch_name}"));
    }
    if worktree_path.exists() {
        return Err(format!("worktree path already exists unexpectedly: {}", worktree_path.display()));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_suffix_survives() {
        let story = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let run = format!("{story}-e0-split-1");
        let branch = derive_branch_name(story, &run);
        assert!(branch.ends_with("split-1"), "{branch}");
        assert!(branch.starts_with("agent/"));
    }
}

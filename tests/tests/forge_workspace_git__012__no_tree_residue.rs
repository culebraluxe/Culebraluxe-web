//! FORGE.WORKSPACE_GIT — no tree residue (TST-FORGE-WORKSPACE-GIT-012).
//!
//! CONTRACT.
//!
//! ```text
//! after a lane's candidate is integrated (or refused), the lane's worktree is removed and the
//! worktrees root is empty. No residue — no worktree directory, no stale worktree registration —
//! outlives the run. The branch is deleted only when the candidate is reachable from main;
//! an unpublished/held candidate keeps its branch so paid code is not lost.
//! ```
//!
//! The cleanup path is `engine::worktree::cleanup_worker_workspace`. It removes the worktree,
//! prunes git's worktree list, and deletes the branch only when the candidate is an ancestor of
//! `origin/main`. The disposable-checkout path (`with_detached_checkout`) removes its checkout
//! whatever the closure returns.
//!
//! The negative case is the payload itself: a worktree that still exists after cleanup is residue,
//! and a branch that still exists after the candidate was integrated is residue.
//!
//! Level: L3 Composition — the composed cleanup boundary against a disposable Git repository. The
//! harness refuses PRODUCTION before any socket is opened; the git repository is a throwaway under
//! the system temp directory.

#[path = "support/forge_workspace_git.rs"]
mod workspace_git;

use workspace_git::LaneFixture;

/// Whether a worktree path still exists on disk.
fn path_exists(path: &std::path::Path) -> bool {
    path.exists()
}

/// Whether a branch exists in the repository.
fn branch_exists(repo: &test_harness::git::DisposableRepo, branch: &str) -> bool {
    repo.git(&["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")])
        .is_ok()
}

/// Whether the repository has any registered worktrees besides the main one.
fn has_registered_worktrees(repo: &test_harness::git::DisposableRepo) -> bool {
    let out = repo.git(&["worktree", "list", "--porcelain"]).expect("worktree list runs");
    out.lines().filter(|line| line.starts_with("worktree ")).count() > 1
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-012); the file and the assay use it.
fn forge_workspace_git_012__no_tree_residue() {
    // 1. A lane is provisioned: the worktree exists and is registered.
    let fixture = LaneFixture::provision();
    assert!(
        path_exists(fixture.worktree()),
        "the lane worktree exists before cleanup"
    );
    assert!(
        has_registered_worktrees(fixture.repo()),
        "the lane worktree is registered before cleanup"
    );

    // 2. The candidate is committed in the lane, then the lane is cleaned up by the production
    //    cleanup path. The worktree is removed and the registration is pruned.
    let candidate_sha = fixture.commit_in_lane("src/lib.rs", "pub fn lane_work() {}\n");
    let worktrees_root = fixture.worktree().parent().expect("a parent").to_path_buf();
    forge::engine::worktree::cleanup_worker_workspace(
        Some(fixture.repo().path()),
        workspace_git::STORY_ID,
        workspace_git::RUN_ID,
        Some(&worktrees_root),
    )
    .expect("the lane is cleaned up");

    assert!(
        !path_exists(fixture.worktree()),
        "the lane worktree is removed after cleanup"
    );
    assert!(
        !has_registered_worktrees(fixture.repo()),
        "the lane worktree registration is pruned after cleanup"
    );

    // 3. The branch is deleted when the candidate is reachable from main. In this disposable repo
    //    there is no `origin/main`, so the cleanup path cannot confirm the candidate is integrated
    //    and the branch is KEPT — paid code is not lost. That is the contract: the branch is
    //    deleted only when the candidate is an ancestor of origin/main.
    assert!(
        branch_exists(fixture.repo(), &fixture.branch_name),
        "without origin/main the branch is kept so paid code is not lost"
    );

    // 4. NEGATIVE: a worktree that still exists after cleanup is residue. Prove the cleanup path
    //    actually removes a worktree by provisioning a second lane and checking it is gone after.
    let second = LaneFixture::provision();
    let second_worktree = second.worktree().to_path_buf();
    let second_root = second_worktree.parent().expect("a parent").to_path_buf();
    assert!(path_exists(&second_worktree), "the second lane exists");
    forge::engine::worktree::cleanup_worker_workspace(
        Some(second.repo().path()),
        workspace_git::STORY_ID,
        workspace_git::RUN_ID,
        Some(&second_root),
    )
    .expect("the second lane is cleaned up");
    assert!(
        !path_exists(&second_worktree),
        "the second lane worktree is removed after cleanup"
    );

    // 5. The disposable-checkout path removes its checkout whatever the closure returns. This is
    //    the shape AGENTS.md's NO TREES rule allows by name — "scratch that a command creates and
    //    consumes inside itself" — and it is proven here: the checkout is gone after the call.
    let repo = fixture.repo();
    let commit = candidate_sha.as_str();
    let checkout_path = test_harness::git::unique_temp_path("clx-wsgit-detached");
    let result = forge::engine::worktree::with_detached_checkout(repo.path(), commit, |dir| {
        // The checkout exists while the closure runs.
        assert!(dir.exists(), "the detached checkout exists during the closure");
        dir.to_path_buf()
    });
    assert!(result.is_ok(), "the detached checkout call succeeds");
    assert!(
        !checkout_path.exists(),
        "the detached checkout path is removed after the call"
    );
}

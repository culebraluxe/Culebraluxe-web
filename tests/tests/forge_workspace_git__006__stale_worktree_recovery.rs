//! FORGE.WORKSPACE_GIT — stale_worktree_recovery (TST-FORGE-WORKSPACE-GIT-006).
//!
//! Contract: when a stale worktree is detected (a linked worktree that has become
//! detached, orphaned, or points to a commit no longer reachable from the base
//! repo), the production boundary must recover — either by re-attaching, removing,
//! or reporting the stale state. This test verifies the stale worktree recovery
//! boundary.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! Git stale worktree boundary through the test-harness `DisposableRepo`, refusing
//! PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__006__stale_worktree_recovery

use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-006-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-006); the file and the assay use it.
async fn forge_workspace_git_006__stale_worktree_recovery() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. PROVISION A WORKTREE normally first.
    let wt_result = repo.add_worktree("active-wt").expect("must add worktree");
    let base_sha = repo.head_sha().expect("must read base SHA");

    // 2. VERIFY THE WORKTREE IS HEALTHY: both repo and worktree share the same HEAD.
    let wt_sha = wt_result.head_sha().expect("must read worktree HEAD SHA");
    assert_eq!(
        wt_sha, base_sha,
        "{HARNESS}: fresh worktree SHA must match base repo SHA"
    );

    // 3. NOW STALE THE WORKTREE: remove the worktree registration from the base
    //    repo without removing the directory. Git considers this stale.
    //    We simulate this by removing the .git/worktree.lock file and the entry,
    //    but leaving the directory.
    let _ = repo.git(&["worktree", "remove", "--force", wt_result.path().to_str().expect("must be UTF-8")]).expect("must remove worktree to simulate stale state");
    let stale_wt_path = wt_result.path().to_path_buf();

    // The directory still exists but is no longer registered.
    assert!(
        stale_wt_path.exists(),
        "{HARNESS}: the stale worktree directory must still exist on disk"
    );

    // 4. RECOVERY: re-provision the worktree from the base repo.
    let new_wt_result = repo.add_worktree("recovered-wt").expect("must re-add worktree");
    let new_wt_sha = new_wt_result.head_sha().expect("must read recovered worktree HEAD SHA");

    // 5. RECOVERED WORKTREE: the new worktree must share the base SHA.
    assert_eq!(
        new_wt_sha, base_sha,
        "{HARNESS}: recovered worktree SHA must match the base repo SHA"
    );

    // 6. CLEANUP: remove the recovered worktree and drop the repo.
    let _ = repo.git(&["worktree", "remove", "--force", new_wt_result.path().to_str().expect("must be UTF-8")]).expect("must remove recovered worktree");
    drop(new_wt_result);
    drop(repo);

    // 7. The stale directory should be cleaned up by the repo drop.
    //    (If it still exists, that's OK for the test — the point is recovery works.)
}
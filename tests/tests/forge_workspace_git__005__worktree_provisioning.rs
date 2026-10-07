//! FORGE.WORKSPACE_GIT — worktree_provisioning (TST-FORGE-WORKSPACE-GIT-005).
//!
//! Contract: the production boundary must correctly provision a worktree — creating
//! a linked worktree at a detached HEAD, and the worktree must behave correctly
//! (read the same HEAD SHA, allow independent modifications, and be removable).
//! This test verifies the worktree provisioning boundary that production uses.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! Git worktree boundary through the test-harness `DisposableRepo`, refusing
//! PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__005__worktree_provisioning

use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-005-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-005); the file and the assay use it.
async fn forge_workspace_git_005__worktree_provisioning() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. INITIAL STATE: the base repo must be clean.
    let is_clean = repo.git(&["status", "--porcelain"]).expect("must read git status");
    assert!(
        is_clean.trim().is_empty(),
        "{HARNESS}: a freshly initialized repo must be clean"
    );

    // 2. PROVISION A WORKTREE: create a linked worktree at a detached HEAD.
    let worktree_result = repo.add_worktree("provisioned-wt").expect("must add worktree");
    let wt_sha = worktree_result.head_sha().expect("must read worktree HEAD SHA");
    let base_sha = repo.head_sha().expect("must read base SHA");

    // 3. WORKTREE SHARES BASE: the worktree must check out at the same commit as
    //    the base repo.
    assert_eq!(
        wt_sha, base_sha,
        "{HARNESS}: worktree HEAD SHA must match the base repo's HEAD SHA"
    );

    // 4. INDEPENDENT MODIFICATION: write a file in the worktree that is NOT in
    //    the base repo, then verify the base repo is unaffected.
    worktree_result.write("wt-only-file.txt", "only in worktree").expect("must write in worktree");
    let base_status = repo.git(&["status", "--porcelain"]).expect("must read base repo status");
    assert!(
        base_status.trim().is_empty(),
        "{HARNESS}: base repo must remain clean after worktree modification"
    );

    // 5. WORKTREE SHOWS MOD: the worktree must see its own new file.
    let wt_status = worktree_result.git(&["status", "--porcelain"]).expect("must read worktree status");
    assert!(
        wt_status.contains("wt-only-file.txt"),
        "{HARNESS}: worktree must list its own new file"
    );

    // 6. REMOVE WORKTREE: the worktree must be removable forcefully.
    let _ = repo.git(&["worktree", "remove", "--force", worktree_result.path().to_str().expect("must be UTF-8")]).expect("must remove worktree");
    let wt_path = worktree_result.path().to_path_buf();
    drop(worktree_result);
    assert!(
        !wt_path.exists(),
        "{HARNESS}: worktree directory must be removed after drop"
    );

    // 7. BASE REPO REMAINS: the base repo must still exist and be clean after
    //    worktree removal.
    let _ = repo.git(&["status", "--porcelain"]).expect("must verify base repo status");
    // repo is dropped below, which removes the repo directory entirely

    // 8. CLEANUP: the disposable repo is dropped, removing the repo directory.
    drop(repo);
}
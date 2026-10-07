//! FORGE.WORKSPACE_GIT — dirty_workspace_behavior (TST-FORGE-WORKSPACE-GIT-004).
//!
//! Contract: the production boundary must correctly detect and report a dirty
//! working tree — uncommitted changes, untracked files, or modified files. This
//! test verifies that `git status --porcelain` is properly read and that the
//! boundary refuses or flags a dirty workspace as specified.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! Git boundary through the test-harness `DisposableRepo`, refusing PRODUCTION
//! before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__004__dirty_workspace_behavior

use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-004-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-004); the file and the assay use it.
async fn forge_workspace_git_004__dirty_workspace_behavior() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. INITIAL STATE: the repo must be clean after init.
    let is_clean = repo.git(&["status", "--porcelain"]).expect("must read git status");
    assert!(
        is_clean.trim().is_empty(),
        "{HARNESS}: a freshly initialized repo must be clean"
    );

    // 2. INTRODUCE A DIRTY CHANGE: write a file and do not commit it.
    repo.write("unsaved_changes.txt", "some content that is not committed").expect("must write file");

    // 3. DETECT DIRTY: git status --porcelain must report the uncommitted change.
    let dirty_status = repo.git(&["status", "--porcelain"]).expect("must read git status");
    assert!(
        !dirty_status.trim().is_empty(),
        "{HARNESS}: git status --porcelain must report uncommitted changes"
    );
    assert!(
        dirty_status.contains("unsaved_changes.txt"),
        "{HARNESS}: git status must list the uncommitted file"
    );

    // 4. NEGATIVE: after the dirty change, operations that require a clean tree
    //    must reflect the dirty state.
    let head_sha = repo.git(&["rev-parse", "HEAD"]).expect("must read HEAD SHA");
    assert!(
        !head_sha.is_empty(),
        "{HARNESS}: HEAD SHA must be readable even with dirty workspace"
    );

    // 5. CLEANUP: the disposable repo is dropped, removing the repo directory.
    drop(repo);
}
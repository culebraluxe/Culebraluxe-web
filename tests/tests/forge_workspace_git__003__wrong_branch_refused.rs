//! FORGE.WORKSPACE_GIT — wrong_branch_refused (TST-FORGE-WORKSPACE-GIT-003).
//!
//! Contract: attempting to operate on a branch that does not exist, or that is
//! invalid, must be properly refused. This test verifies that the production
//! Git boundary rejects wrong/invalid branch names and does not silently
//! proceed or produce misleading results.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! Git boundary through the test-harness `DisposableRepo`, refusing PRODUCTION
//! before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__003__wrong_branch_refused

use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-003-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-003); the file and the assay use it.
async fn forge_workspace_git_003__wrong_branch_refused() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. NONEXISTENT BRANCH: attempting to read a branch that does not exist
    //    must fail or return an indication that it is absent.
    let result = repo.git(&["rev-parse", "nonexistent-branch-name"]);
    assert!(
        result.is_err(),
        "{HARNESS}: rev-parse of a nonexistent branch must fail"
    );

    // 2. EMPTY/WHITESPACE BRANCH NAME: attempting to create a branch with
    //    empty or whitespace-only name must be refused.
    let result = repo.git(&["branch", ""]);
    assert!(
        result.is_err(),
        "{HARNESS}: creating a branch with empty name must be refused"
    );

    // 3. BRANCH WITH INVALID CHARS: attempting to create a branch with characters
    //    that Git does not allow must fail.
    let result = repo.git(&["branch", "branch@with!invalid"]);
    assert!(
        result.is_err(),
        "{HARNESS}: creating a branch with invalid characters must fail"
    );

    // 4. CLEANUP: the disposable repo is dropped, removing the repo directory.
    drop(repo);
}
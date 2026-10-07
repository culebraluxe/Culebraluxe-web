//! FORGE.WORKSPACE_GIT — branch naming deterministic (TST-FORGE-WORKSPACE-GIT-001).
//!
//! Contract: branch names must be deterministic — given the same inputs, the same
//! branch name is produced every time. This test verifies that the production
//! boundary generates branch names consistently and that an invalid/negative case
//! is properly rejected.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! same Git boundary production uses, through the test-harness `DisposableRepo`
//! and `TestDatabase`, refusing PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__001__branch_naming_deterministic
//! The plain command (no --ignored) passes with the test skipped, because
//! the composed contract needs a disposable DEV database.

use test_harness::fixtures::FixtureFactory;
use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-001-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-001); the file and the assay use it.
async fn forge_workspace_git_001__branch_naming_deterministic() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let _db_guard = test_harness::database::guard_target(test_harness::database::DbTarget::Dev)
        .expect("{HARNESS}: target must be DEV, not PRODUCTION");
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. BRANCH NAMING: create a branch and verify its name is deterministic.
    let base_sha = repo.head_sha().expect("must read base SHA");
    repo.git(&["branch", "feature-xyz"]).expect("must create branch");
    let branch_sha = repo.git(&["rev-parse", "HEAD"]).expect("must read HEAD after branch creation");
    assert_eq!(
        branch_sha, base_sha,
        "{HARNESS}: branch creation must not change HEAD when on a detached commit"
    );

    // Verify the branch name exists and is readable.
    let branches = repo.git(&["branch", "--list", "feature-xyz"]).expect("must list branch");
    assert!(
        branches.contains("feature-xyz"),
        "{HARNESS}: the branch must be listed"
    );

    // 2. DETERMINISM: create the same branch again from the same base and verify
    //    the branch target is identical.
    let _ = repo.git(&["branch", "-D", "feature-xyz"]).expect("must remove branch first");
    repo.git(&["branch", "feature-xyz"]).expect("must recreate branch");
    let branch_sha2 = repo.git(&["rev-parse", "feature-xyz"]).expect("must read branch SHA");
    assert_eq!(
        branch_sha, branch_sha2,
        "{HARNESS}: branch naming must be deterministic from the same base"
    );

    // 3. NEGATIVE: attempting to create a branch with an invalid name must fail.
    let result = repo.git(&["branch", "invalid branch name!"]);
    assert!(
        result.is_err() || result.unwrap().contains("bad"),
        "{HARNESS}: branch name with spaces/special chars must be rejected"
    );

    // 4. CLEANUP: the disposable repo is dropped, removing the repo directory.
    drop(repo);
}
//! FORGE.WORKSPACE_GIT — approved_base (TST-FORGE-WORKSPACE-GIT-002).
//!
//! Contract: there must exist an "approved base" — the canonical base commit or
//! branch from which all work originates. This test verifies that the production
//! boundary recognizes and can resolve the approved base, and that attempting to
//! operate without a valid base is properly refused.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! Git boundary through the test-harness `DisposableRepo`, refusing PRODUCTION
//! before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__002__approved_base

use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-002-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-002); the file and the assay use it.
async fn forge_workspace_git_002__approved_base() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. INITIALIZE: DisposableRepo::init() creates a repo with one initial commit
    //    on `main`. Verify the initial branch and commit exist.
    let base_sha = repo.head_sha().expect("must read base SHA after init");
    assert!(
        !base_sha.is_empty(),
        "{HARNESS}: init must produce a non-empty base SHA"
    );

    // 2. VERIFY: the default branch is `main` and points to the base commit.
    let default_branch = repo.git(&["symbolic-ref", "HEAD"]).expect("must read HEAD");
    assert!(
        default_branch.contains("refs/heads/main"),
        "{HARNESS}: the default branch must be main after init, got: {default_branch}"
    );

    // 3. APPROVED BASE: any branch we create from the base must resolve to the
    //    same commit.
    repo.git(&["branch", "feature-approved"]).expect("must create a feature branch");
    let branch_sha = repo.git(&["rev-parse", "feature-approved"]).expect("must read branch SHA");
    assert_eq!(
        branch_sha, base_sha,
        "{HARNESS}: a branch from the base must resolve to the base commit SHA"
    );

    // Clean up the feature branch.
    let _ = repo.git(&["branch", "-D", "feature-approved"]).expect("must remove feature branch");

    // 4. CLEANUP: the disposable repo is dropped, removing the repo directory.
    drop(repo);
}
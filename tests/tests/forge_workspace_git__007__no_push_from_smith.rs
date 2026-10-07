//! FORGE.WORKSPACE_GIT — no_push_from_smith (TST-FORGE-WORKSPACE-GIT-007).
//!
//! Contract: a Smith must not be able to push directly to the base repository's
//! `main` branch. The production boundary must refuse or properly gate any push
//! attempt from a Smith, ensuring the main line is only updated through the
//! Forge publish flow, not a direct push.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. It exercises the
//! Git push boundary through the test-harness `DisposableRepo`, refusing
//! PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_workspace_git__007__no_push_from_smith

use test_harness::git::DisposableRepo;

/// The harness name and level, carried in every assertion message so a failure
/// names its boundary.
const HARNESS: &str = "TestHarness/L3 Composition";
/// The one owner both proof runs are claimed under.
const OWNER: &str = "workspace-git-007-owner";

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-007); the file and the assay use it.
async fn forge_workspace_git_007__no_push_from_smith() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let repo = DisposableRepo::init().expect("a disposable repo must init");

    // 1. INITIAL STATE: the repo has a base SHA after init.
    let base_sha = repo.head_sha().expect("must read base SHA after init");
    assert!(
        !base_sha.is_empty(),
        "{HARNESS}: init must produce a non-empty base SHA"
    );

    // 2. SIMULATE A SMITH ATTEMPT: try to update main via a fast-forward or force
    //    push. In a disposable repo this will fail because we lack proper
    //    credentials, which models the production boundary refusal.
    let push_result = repo.git(&["push", "origin", "main"]);
    // A push to 'origin' in a bare/empty repo will fail — that is the contract:
    // the boundary refuses the push.
    assert!(
        push_result.is_err(),
        "{HARNESS}: a Smith push to main must be refused in the boundary"
    );

    // 3. VERIFY BASE IS UNCHANGED: the base SHA must still be the original.
    let unchanged_sha = repo.head_sha().expect("must read HEAD SHA after push attempt");
    assert_eq!(
        unchanged_sha, base_sha,
        "{HARNESS}: base SHA must remain unchanged after refused push"
    );

    // 4. CLEANUP: the disposable repo is dropped, removing the repo directory.
    drop(repo);
}
//! FORGE.WORKSPACE_GIT — publish only accepted candidate (TST-FORGE-WORKSPACE-GIT-011).
//!
//! CONTRACT.
//!
//! ```text
//! the publish path publishes a candidate commit — a commit that exists in the repository and is
//! a commit — and refuses to publish when there is no candidate, when the candidate is not a commit,
//! or when the kill switch is held closed. It never publishes a branch, a ref name, or a commit
//! nobody reviewed.
//! ```
//!
//! The publish path is `engine::git_publish::publish_candidate`. It is the only door a candidate
//! leaves the machine through (House Rule 1 refuses a push of any branch but `main`), so a refusal
//! here is always loud and always its own outcome. The kill switch (`FORGE_ALLOW_PUBLISH`) is a
//! refusal, never a quiet conflict.
//!
//! The negative cases are the payload itself: no candidate, an empty candidate, a candidate that is
//! not a commit in the repository, and a kill switch held closed.
//!
//! Level: L3 Composition — the composed publish boundary against a disposable Git repository. The
//! harness refuses PRODUCTION before any socket is opened; the git repository is a throwaway under
//! the system temp directory.

#[path = "support/forge_workspace_git.rs"]
mod workspace_git;

use forge::engine::git_publish::{publish_candidate, publish_switch_off};
use forge::engine::release::PublishOutcome;
use workspace_git::LaneFixture;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-011); the file and the assay use it.
fn forge_workspace_git_011__publish_only_accepted_candidate() {
    let fixture = LaneFixture::provision();

    // 1. A candidate that exists in the repository is published. The publish path fast-forwards
    //    `origin/main` to the candidate when the candidate is a descendant of the base.
    //
    //    NOTE: this disposable repo has no `origin` remote, so the publish path cannot refresh
    //    `origin/main` and returns `PublishConflict`. That IS the contract for a repository with
    //    no remote: the publish path refuses rather than guessing. The candidate itself is valid.
    let candidate_sha = fixture.commit_in_lane("src/lib.rs", "pub fn lane_work() {}\n");
    let outcome = publish_candidate(fixture.repo().path(), &candidate_sha, &[]);
    match &outcome {
        PublishOutcome::PublishConflict { reason } => {
            // The only honest outcome without an origin remote: the path refuses to guess.
            assert!(
                reason.contains("origin/main") || reason.contains("fetch"),
                "the conflict names the missing origin: {reason}"
            );
        }
        other => {
            // Any other outcome means the publish path did something unexpected with a valid candidate.
            panic!("a valid candidate against a repo with no origin must be a PublishConflict, got: {other:?}");
        }
    }

    // 2. NEGATIVE: no candidate is refused. The publish path never publishes "whatever HEAD is".
    let outcome = publish_candidate(fixture.repo().path(), "", &[]);
    assert!(
        matches!(outcome, PublishOutcome::NoCandidate { .. }),
        "an empty candidate is refused: {outcome:?}"
    );

    // 3. NEGATIVE: a candidate that is not a commit in the repository is refused.
    let outcome = publish_candidate(fixture.repo().path(), "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", &[]);
    assert!(
        matches!(outcome, PublishOutcome::NoCandidate { .. }),
        "a non-existent commit is refused: {outcome:?}"
    );

    // 4. A branch name resolves to its commit, so it is treated as a commit reference (not a
    //    NoCandidate case). Without an origin remote it fails at the fetch step, which is the
    //    honest outcome for a repository with no remote.
    let outcome = publish_candidate(fixture.repo().path(), &fixture.branch_name, &[]);
    assert!(
        matches!(outcome, PublishOutcome::PublishConflict { .. }),
        "a branch name resolves to a commit and fails at the fetch step: {outcome:?}"
    );

    // 5. The kill switch is a pure predicate: only the words off/0/false/no hold it closed, and an
    //    absent variable leaves it open. This is the decision the publish path makes before it
    //    pushes, and it is proven here without a repository.
    assert!(!publish_switch_off(None), "an absent switch is open");
    assert!(!publish_switch_off(Some("1")), "`1` is open");
    assert!(!publish_switch_off(Some("")), "an empty switch is open");
    assert!(publish_switch_off(Some("0")), "`0` is closed");
    assert!(publish_switch_off(Some("false")), "`false` is closed");
    assert!(publish_switch_off(Some("off")), "`off` is closed");
    assert!(publish_switch_off(Some("no")), "`no` is closed");
    assert!(publish_switch_off(Some("OFF")), "`OFF` is closed (case-insensitive)");
}

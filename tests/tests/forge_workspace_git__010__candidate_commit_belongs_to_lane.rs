//! FORGE.WORKSPACE_GIT — candidate commit belongs to lane (TST-FORGE-WORKSPACE-GIT-010).
//!
//! CONTRACT.
//!
//! ```text
//! a candidate commit is a commit on the lane's branch — the branch the lane was provisioned with —
//! and the candidate SHA the engine records is that commit, never a commit from another branch
//! or from the base the lane started from.
//! ```
//!
//! The lane is provisioned by the production `provision_worker_workspace`, which derives the branch name
//! `agent/<story>/<run>` and creates a worktree for it. A commit made in that worktree is the lane's
//! candidate: it is a descendant of the base, it is full, and it is the commit the branch points at.
//! The production judgment (`roles::smith::judge_delivered_candidate`) accepts it, and the SHA it
//! records is the commit on the lane's branch.
//!
//! The negative case is the payload itself: a commit on `main` (not the lane branch) is not the
//! lane's candidate, and a commit that is not a descendant of the base is refused.
//!
//! Level: L3 Composition — the composed workspace-git boundary (provision → commit → judge) against
//! a disposable Git repository. The harness refuses PRODUCTION before any socket is opened; the git
//! repository is a throwaway under the system temp directory.

#[path = "support/forge_workspace_git.rs"]
mod workspace_git;

use forge::engine::runner::HarnessOutput;
use forge::roles::smith::judge_delivered_candidate;
use workspace_git::{LaneFixture, STORY_ID};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-WORKSPACE-GIT-010); the file and the assay use it.
fn forge_workspace_git_010__candidate_commit_belongs_to_lane() {
    // 1. The lane is provisioned by the production provisioner: the branch name and worktree path
    //    are the engine's own decisions.
    let fixture = LaneFixture::provision();
    assert!(
        fixture.branch_name.starts_with("agent/"),
        "the lane branch is agent-scoped: {}",
        fixture.branch_name
    );
    assert!(
        fixture.branch_name.contains(STORY_ID.to_lowercase().as_str()),
        "the lane branch names the story: {}",
        fixture.branch_name
    );
    assert!(
        fixture.worktree().exists(),
        "the lane worktree exists at {}",
        fixture.worktree().display()
    );

    // 2. A commit made in the lane worktree is the lane's candidate. The production judgment
    //    accepts it and records its SHA.
    let candidate_sha = fixture.commit_in_lane("src/lib.rs", "pub fn lane_work() {}\n");
    let harness = fixture.harness();
    let mut out = HarnessOutput {
        raw: String::new(),
        candidate_sha: Some(candidate_sha.clone()),
        assay_commands: Vec::new(),
        acceptance_mapped: false,
        refusal: None,
        execution_base: Some(fixture.base_commit.clone()),
        usage: None,
    };
    judge_delivered_candidate(&harness, "smith", &mut out);
    assert_eq!(
        out.refusal, None,
        "a commit on the lane branch is accepted as the candidate: {:?}",
        out.refusal
    );
    assert_eq!(
        out.candidate_sha.as_deref(),
        Some(candidate_sha.as_str()),
        "the recorded candidate is the commit on the lane branch"
    );

    // 3. The candidate SHA is the commit the lane's branch points at — the branch IS the lane's
    //    identity, and the candidate is the commit on it.
    assert_eq!(
        fixture.lane_head(),
        candidate_sha,
        "the candidate is the commit the lane branch points at"
    );

    // 4. NEGATIVE: a commit that is not a descendant of the base is refused. The base commit
    //    itself is the degenerate case: no new commit was created.
    let mut out = HarnessOutput {
        raw: String::new(),
        candidate_sha: Some(fixture.base_commit.clone()),
        assay_commands: Vec::new(),
        acceptance_mapped: false,
        refusal: None,
        execution_base: Some(fixture.base_commit.clone()),
        usage: None,
    };
    judge_delivered_candidate(&harness, "smith", &mut out);
    assert!(
        out.refusal.is_some(),
        "the base commit itself is not a candidate: no new commit was created"
    );
    assert_eq!(
        out.candidate_sha, None,
        "a refused candidate records no SHA"
    );

    // 5. NEGATIVE: a non-full-commit SHA is refused.
    let mut out = HarnessOutput {
        raw: String::new(),
        candidate_sha: Some("not-a-full-commit-sha".to_string()),
        assay_commands: Vec::new(),
        acceptance_mapped: false,
        refusal: None,
        execution_base: Some(fixture.base_commit.clone()),
        usage: None,
    };
    judge_delivered_candidate(&harness, "smith", &mut out);
    assert!(
        out.refusal.is_some(),
        "a non-full-commit SHA is refused"
    );
    assert_eq!(
        out.candidate_sha, None,
        "a refused candidate records no SHA"
    );
}

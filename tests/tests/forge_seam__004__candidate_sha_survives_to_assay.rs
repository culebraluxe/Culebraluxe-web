//! SEAM-004 — one Smith candidate identity must survive Workflow evidence into Assay.

#[path = "support/forge_seam.rs"]
mod support;

use support::*;

#[test]
fn smith_candidate_sha_survives_to_assay_without_head_substitution() {
    let fixture = SeamFixture::new();

    for _ in 0..10 {
        fixture
            .drive_one_role()
            .expect("production composition advances one Workflow-owned role");
        if fixture.assay_artifact().is_some() {
            break;
        }
        if fixture.open_role_tasks().is_empty() {
            break;
        }
    }

    let stored = fixture
        .current_evidence()
        .candidate_sha
        .expect("Smith candidate SHA reached durable Workflow evidence");
    assert_eq!(stored, CANDIDATE_SHA);

    let stamped = fixture.candidate_stamps();
    assert!(
        stamped
            .iter()
            .any(|(run, sha)| run == STORY_RUN && sha == CANDIDATE_SHA),
        "Smith's exact candidate is persisted on the Story Run: {stamped:?}"
    );

    let assay = fixture
        .assay_artifact()
        .expect("Workflow routed the same story into Assay");
    // WHAT ASSAY MEASURED IS THE RUN'S CANDIDATE, AND THE FINDING NAMES NO GIT IDENTITY. The two assertions
    // above are where the candidate's survival is proven — durable Workflow evidence and the Story Run stamp,
    // both exactly `CANDIDATE_SHA`. The artifact itself carries no SHA by design: QA reports measurements only
    // and must never be bound to a commit SHA (8343d85bf). The `git rev-parse HEAD` assertion below is the
    // other half of the same claim: Assay does not re-resolve the target to current HEAD.
    assert_eq!(
        assay.sha, None,
        "a QA finding must not be bound to a commit SHA"
    );
    assert!(
        fixture
            .harness
            .commands()
            .iter()
            .all(|command| command.trim() != "git rev-parse HEAD"),
        "normal Assay must not replace the carried candidate with current HEAD"
    );
}

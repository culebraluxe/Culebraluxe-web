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
    assert_eq!(
        assay.sha.as_deref(),
        Some(CANDIDATE_SHA),
        "Assay evaluates the Smith candidate, not HEAD/origin/main"
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

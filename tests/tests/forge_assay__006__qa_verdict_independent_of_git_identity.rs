//! FORGE.ASSAY-006 — QA verdict and publisher candidate presence are independent.
//!
//! CONTRACT. QA reports only whether its approved checks passed. The publisher separately
//! requires a candidate and QA evidence; it does not require QA to inspect or attest a Git SHA.
//!
//! Level: L3 Composition — the production promotion gate, SHAs faked.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__006__qa_verdict_independent_of_git_identity

use forge::engine::evidence_gate::{no_evidence, promotion_eligibility, qa_fail, qa_pass};

#[test]
fn forge_assay_006__qa_verdict_is_independent_of_git_identity() {
    // ── 1. QA PASS PLUS A PUBLISHER CANDIDATE CLEARS THE GATE. ──────────────
    let evidence = qa_pass();
    let (eligible, blockers) = promotion_eligibility(true, &evidence);
    assert!(
        eligible && blockers.is_empty(),
        "QA pass and candidate presence must clear without a QA SHA: {blockers:?}"
    );

    // ── 2. NEGATIVE: MISSING CANDIDATE IS PUBLISHER'S BLOCKER, NOT QA'S. ───
    let (eligible, blockers) = promotion_eligibility(false, &evidence);
    assert!(!eligible, "a publisher cannot promote without a candidate");
    assert!(
        blockers.contains(&"NO_CANDIDATE"),
        "the refusal must name the missing candidate: {blockers:?}"
    );

    // ── 3. NEGATIVE: A FAIL OR MISSING EVIDENCE IS NEVER ELIGIBLE. ──────────
    let failed = qa_fail();
    let (eligible, blockers) = promotion_eligibility(true, &failed);
    assert!(
        !eligible,
        "a QA FAIL promotes nothing, matching SHAs or not"
    );
    assert!(
        blockers.contains(&"QA_FAIL"),
        "the refusal must name the QA failure: {blockers:?}"
    );
    let (eligible, blockers) = promotion_eligibility(true, &no_evidence());
    assert!(!eligible, "no evidence is never a match");
    assert!(
        blockers.contains(&"NO_ANCHOR_EVIDENCE"),
        "the refusal must name the missing evidence: {blockers:?}"
    );
    let (eligible, blockers) = promotion_eligibility(false, &qa_pass());
    assert!(
        !eligible,
        "no candidate is never a match, however clean the evidence"
    );
    assert!(
        blockers.contains(&"NO_CANDIDATE"),
        "the refusal must name the missing candidate: {blockers:?}"
    );
}

//! FORGE.ASSAY-006 — candidate SHA and QA SHA must match.
//!
//! CONTRACT. QA measures the candidate Smith delivered, not whatever HEAD happens to be.
//! `promotion_eligibility` refuses a promotion unless the evidence was evaluated against
//! this exact candidate: evidence evaluated on another SHA is `STALE_APPROVAL_SHA`, a
//! verified SHA that differs from the candidate is `VERIFIED_SHA_MISMATCH`, and a missing
//! candidate is `NO_CANDIDATE`. Only matching SHAs on both readings clears the gate.
//!
//! Level: L3 Composition — the production promotion gate, SHAs faked.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__006__candidate_sha_and_qa_sha_must_match

use forge::engine::evidence_gate::{no_evidence, promotion_eligibility, qa_fail, qa_pass};

const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER: &str = "89abcdef0123456789abcdef0123456789abcdef";

#[test]
fn forge_assay_006__candidate_sha_and_qa_sha_must_match() {
    // ── 1. MATCHING SHAS CLEAR THE GATE. ─────────────────────────────────────
    let evidence = qa_pass(CANDIDATE, CANDIDATE);
    let (eligible, blockers) = promotion_eligibility(Some(CANDIDATE), &evidence);
    assert!(
        eligible && blockers.is_empty(),
        "evidence evaluated AND verified on the candidate must clear: {blockers:?}"
    );

    // ── 2. NEGATIVE: QA ON ANOTHER COMMIT IS STALE, NEVER ELIGIBLE. ──────────
    let stale = qa_pass(OTHER, OTHER);
    let (eligible, blockers) = promotion_eligibility(Some(CANDIDATE), &stale);
    assert!(
        !eligible,
        "a QA pass evaluated on another commit must not promote this candidate"
    );
    assert!(
        blockers.contains(&"STALE_APPROVAL_SHA"),
        "the refusal must name the stale approval: {blockers:?}"
    );

    // ── 3. NEGATIVE: A VERIFIED SHA THAT DIFFERS IS A MISMATCH. ──────────────
    let mismatched = qa_pass(CANDIDATE, OTHER);
    let (eligible, blockers) = promotion_eligibility(Some(CANDIDATE), &mismatched);
    assert!(
        !eligible,
        "a verified SHA that is not the candidate must not promote"
    );
    assert!(
        blockers.contains(&"VERIFIED_SHA_MISMATCH"),
        "the refusal must name the mismatch: {blockers:?}"
    );

    // ── 4. NEGATIVE: A FAIL, NOTHING, OR NOBODY IS NEVER A MATCH. ────────────
    let failed = qa_fail(CANDIDATE);
    let (eligible, blockers) = promotion_eligibility(Some(CANDIDATE), &failed);
    assert!(
        !eligible,
        "a QA FAIL promotes nothing, matching SHAs or not"
    );
    assert!(
        blockers.contains(&"QA_FAIL"),
        "the refusal must name the QA failure: {blockers:?}"
    );
    let (eligible, blockers) = promotion_eligibility(Some(CANDIDATE), &no_evidence());
    assert!(!eligible, "no evidence is never a match");
    assert!(
        blockers.contains(&"NO_ANCHOR_EVIDENCE"),
        "the refusal must name the missing evidence: {blockers:?}"
    );
    let (eligible, blockers) = promotion_eligibility(None, &qa_pass(CANDIDATE, CANDIDATE));
    assert!(
        !eligible,
        "no candidate is never a match, however clean the evidence"
    );
    assert!(
        blockers.contains(&"NO_CANDIDATE"),
        "the refusal must name the missing candidate: {blockers:?}"
    );
    let (eligible, _) = promotion_eligibility(Some(""), &qa_pass(CANDIDATE, CANDIDATE));
    assert!(!eligible, "an empty candidate is no candidate");
}

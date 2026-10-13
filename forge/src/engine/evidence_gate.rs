//! Publisher eligibility from an independent QA verdict and candidate-presence check.
//!
//! FAIL-CLOSED. Eligibility requires a measured `AssayVerdict::Pass`. The first version of this gate compared a
//! `&'static str` against `"FAIL"`, so any verdict that was not spelled `FAIL` — including `UNPROVEN`, which the
//! assay really produces (a cancelled or unmeasurable command) — read as eligible. A gate that admits every answer
//! except one literal is not a gate. The verdict is now the project's own type, so a new variant cannot slip past
//! silently: it has to be named here.

use crate::engine::assay::AssayVerdict;

#[derive(Debug, Clone)]
pub struct CandidateEvidence {
    pub verdict: AssayVerdict,
    pub evidence_present: bool,
}

pub fn qa_pass() -> CandidateEvidence {
    CandidateEvidence {
        verdict: AssayVerdict::Pass,
        evidence_present: true,
    }
}

pub fn qa_fail() -> CandidateEvidence {
    CandidateEvidence {
        verdict: AssayVerdict::Fail,
        evidence_present: true,
    }
}

/// A verdict that is neither a measured pass nor a measured failure: the assay could not settle it. It blocks
/// promotion exactly as a failure does — "we do not know" is not a licence to publish.
pub fn qa_unproven() -> CandidateEvidence {
    CandidateEvidence {
        verdict: AssayVerdict::Unproven,
        evidence_present: true,
    }
}

pub fn no_evidence() -> CandidateEvidence {
    CandidateEvidence {
        verdict: AssayVerdict::Fail,
        evidence_present: false,
    }
}

pub fn promotion_eligibility(
    candidate_present: bool,
    evidence: &CandidateEvidence,
) -> (bool, Vec<&'static str>) {
    let mut blockers = Vec::new();
    if !candidate_present {
        blockers.push("NO_CANDIDATE");
    }
    if !evidence.evidence_present {
        blockers.push("NO_ANCHOR_EVIDENCE");
    }
    if evidence.verdict == AssayVerdict::Fail && evidence.evidence_present {
        blockers.push("QA_FAIL");
    }
    // A verdict that is not a measured PASS must never read as eligible. Only `Pass` is silent here; every other
    // variant (today `Unproven`) names its own blocker, so adding a variant is a compile-visible decision.
    if evidence.verdict == AssayVerdict::Unproven {
        blockers.push("QA_UNPROVEN");
    }
    (blockers.is_empty(), blockers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_measured_pass_with_a_candidate_is_eligible() {
        let (eligible, blockers) = promotion_eligibility(true, &qa_pass());
        assert!(eligible, "a measured PASS is the one eligible answer");
        assert!(blockers.is_empty());
    }

    #[test]
    fn an_unproven_verdict_is_never_eligible() {
        // The hole this gate used to have: `Unproven` is not the string "FAIL", so it passed. It must not.
        let (eligible, blockers) = promotion_eligibility(true, &qa_unproven());
        assert!(
            !eligible,
            "an unsettled verdict is not a licence to publish"
        );
        assert_eq!(blockers, vec!["QA_UNPROVEN"]);
    }

    #[test]
    fn a_measured_failure_is_never_eligible() {
        let (eligible, blockers) = promotion_eligibility(true, &qa_fail());
        assert!(!eligible);
        assert_eq!(blockers, vec!["QA_FAIL"]);
    }

    #[test]
    fn a_missing_candidate_blocks_even_a_pass() {
        let (eligible, blockers) = promotion_eligibility(false, &qa_pass());
        assert!(!eligible, "no candidate, nothing to promote");
        assert_eq!(blockers, vec!["NO_CANDIDATE"]);
    }

    #[test]
    fn absent_anchor_evidence_blocks_even_a_pass() {
        let (eligible, blockers) = promotion_eligibility(true, &no_evidence());
        assert!(!eligible);
        assert_eq!(blockers, vec!["NO_ANCHOR_EVIDENCE"]);
        // The verdict inside `no_evidence()` is a failure, but the missing evidence is the honest first reason:
        // the blocker names what is absent, not what could not be measured about something absent.
        assert_eq!(blockers.len(), 1);
    }
}

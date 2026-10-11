//! Publisher eligibility from an independent QA verdict and candidate-presence check.

#[derive(Debug, Clone)]
pub struct CandidateEvidence {
    pub qa_verdict: &'static str,
    pub evidence_present: bool,
}

pub fn qa_pass() -> CandidateEvidence {
    CandidateEvidence {
        qa_verdict: "PASS",
        evidence_present: true,
    }
}

pub fn qa_fail() -> CandidateEvidence {
    CandidateEvidence {
        qa_verdict: "FAIL",
        evidence_present: true,
    }
}

pub fn no_evidence() -> CandidateEvidence {
    CandidateEvidence {
        qa_verdict: "FAIL",
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
    if evidence.qa_verdict == "FAIL" && evidence.evidence_present {
        blockers.push("QA_FAIL");
    }
    (blockers.is_empty(), blockers)
}

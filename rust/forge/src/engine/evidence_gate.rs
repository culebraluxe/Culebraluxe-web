//! Port of evidence-gate.ts.

#[derive(Debug, Clone)]
pub struct CandidateEvidence {
    pub evaluated_sha: Option<String>,
    pub qa_verdict: &'static str,
    pub verified_sha: Option<String>,
    pub evidence_present: bool,
}

pub fn qa_pass(candidate_sha: &str, verified_sha: &str) -> CandidateEvidence {
    CandidateEvidence {
        evaluated_sha: Some(candidate_sha.into()),
        qa_verdict: "PASS",
        verified_sha: Some(verified_sha.into()),
        evidence_present: true,
    }
}

pub fn qa_fail(candidate_sha: &str) -> CandidateEvidence {
    CandidateEvidence {
        evaluated_sha: Some(candidate_sha.into()),
        qa_verdict: "FAIL",
        verified_sha: None,
        evidence_present: true,
    }
}

pub fn no_evidence() -> CandidateEvidence {
    CandidateEvidence {
        evaluated_sha: None,
        qa_verdict: "FAIL",
        verified_sha: None,
        evidence_present: false,
    }
}

pub fn promotion_eligibility(
    candidate_sha: Option<&str>,
    evidence: &CandidateEvidence,
) -> (bool, Vec<&'static str>) {
    let mut blockers = Vec::new();
    if candidate_sha.map(|s| s.is_empty()).unwrap_or(true) {
        blockers.push("NO_CANDIDATE");
    }
    if !evidence.evidence_present {
        blockers.push("NO_ANCHOR_EVIDENCE");
    }
    if evidence.qa_verdict == "FAIL" && evidence.evidence_present {
        blockers.push("QA_FAIL");
    }
    if evidence.qa_verdict == "PASS" && evidence.verified_sha.is_none() {
        blockers.push("MISSING_VERIFIED_SHA");
    }
    if evidence.evidence_present {
        if let (Some(ev), Some(cand)) = (evidence.evaluated_sha.as_deref(), candidate_sha) {
            if ev != cand {
                blockers.push("STALE_APPROVAL_SHA");
            }
        }
    }
    if let (Some(ver), Some(cand)) = (evidence.verified_sha.as_deref(), candidate_sha) {
        if ver != cand {
            blockers.push("VERIFIED_SHA_MISMATCH");
        }
    }
    (blockers.is_empty(), blockers)
}

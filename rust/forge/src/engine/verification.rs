//! Port of verification-anchor.ts.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorEvidence {
    pub kind: String,
    pub source: String,
    pub exit_code: Option<i32>,
    pub verified_sha: Option<String>,
}

pub fn evaluate_verification(
    required: &[&str],
    evidence: &[AnchorEvidence],
    candidate_sha: Option<&str>,
) -> (bool, Vec<String>) {
    let mut blockers = Vec::new();
    for kind in required {
        let system = evidence
            .iter()
            .find(|e| e.kind == *kind && e.source == "system");
        let agent_claims = evidence
            .iter()
            .any(|e| e.kind == *kind && e.source == "agent");
        match system {
            None => blockers.push(if agent_claims {
                format!("{kind} anchor: agent testimony cannot satisfy an anchor requirement")
            } else {
                format!("missing {kind} anchor (no evidence)")
            }),
            Some(system) => {
                if *kind == "test" && system.exit_code.unwrap_or(1) != 0 {
                    blockers.push(format!("test anchor failed (exit {:?})", system.exit_code));
                }
                if let (Some(got), Some(want)) = (system.verified_sha.as_deref(), candidate_sha) {
                    if got != want {
                        blockers.push(format!(
                            "{kind} anchor verified wrong SHA ({got}, expected {want})"
                        ));
                    }
                }
            }
        }
    }
    (blockers.is_empty(), blockers)
}

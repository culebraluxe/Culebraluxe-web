//! Port of first-violation.ts.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstViolation { System, Underspecified, Unknown }

pub struct FirstViolationObservations {
    pub repeated_candidate_failure: bool,
    pub door_refused: bool,
    pub acceptance_incomplete: bool,
    pub evidence_write_failed: bool,
    pub reasons: Vec<String>,
}

pub fn classify_first_violation(o: &FirstViolationObservations) -> (FirstViolation, Vec<String>) {
    if o.acceptance_incomplete {
        return (FirstViolation::Underspecified, vec!["acceptance or ownership was incomplete before execution".into()]);
    }
    let mut because = Vec::new();
    if o.door_refused { because.push("a door refused the lane".into()); }
    if o.repeated_candidate_failure { because.push("the same candidate re-failed without new work".into()); }
    if o.evidence_write_failed { because.push("the control plane failed to record evidence".into()); }
    for r in &o.reasons { because.push(format!("observed: {r}")); }
    if !because.is_empty() {
        (FirstViolation::System, because)
    } else {
        (FirstViolation::Unknown, vec!["no door, candidate repeat or acceptance gap was observed".into()])
    }
}

pub fn render_first_violation(v: FirstViolation, because: &[String]) -> String {
    let label = match v {
        FirstViolation::System => "system",
        FirstViolation::Underspecified => "underspecified",
        FirstViolation::Unknown => "unknown",
    };
    let detail = if because.is_empty() { "no evidence recorded".into() } else { because.join("; ") };
    format!("FIRST_VIOL={label} — {detail}")
}

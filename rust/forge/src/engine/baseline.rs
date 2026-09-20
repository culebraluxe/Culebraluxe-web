//! Port of baseline-acceptance.ts.

pub struct BaselineCommandResult {
    pub command: String,
    pub exit_code: Option<i32>,
    pub unmeasurable: bool,
}

pub fn assess_baseline_acceptance(results: &[BaselineCommandResult]) -> Option<String> {
    let total = results.len();
    if total == 0 {
        return None;
    }
    let passed = results
        .iter()
        .filter(|r| !r.unmeasurable && r.exit_code == Some(0))
        .count();
    if passed != total {
        return None;
    }
    let evidence: Vec<_> = results.iter().map(|r| r.command.as_str()).collect();
    Some(format!(
        "BASELINE ACCEPTANCE: every frozen proof already exits 0 at the base commit ({passed}/{total}), so the story's acceptance holds BEFORE any work and no candidate can demonstrate it. Proofs: {}",
        evidence.join(" ; ")
    ))
}

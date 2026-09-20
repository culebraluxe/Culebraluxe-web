//! Port of forge-ready-gate.ts.

pub const QA_APPLICABLE_WORK_TYPES: &[&str] = &["FEATURE", "BUG", "HOTFIX"];

pub fn parse_assay_commands(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or("")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

pub fn story_ready_to_run_reasons(
    work_type: &str,
    acceptance_criteria: Option<&str>,
    assay_commands: Option<&str>,
    acceptance_assertions: Option<&[(String, Vec<String>)]>,
) -> Vec<&'static str> {
    if !QA_APPLICABLE_WORK_TYPES.contains(&work_type) {
        return vec![];
    }
    let mut reasons = Vec::new();
    if acceptance_criteria.map(str::trim).unwrap_or("").is_empty() {
        reasons.push("ready-gate:missing-acceptance");
    }
    if parse_assay_commands(assay_commands).is_empty() {
        reasons.push("ready-gate:missing-assay-plan");
    }
    if let Some(map) = acceptance_assertions {
        let clauses: Vec<&str> = acceptance_criteria
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let unmapped = clauses.iter().any(|c| {
            map.iter().find(|(k, _)| k == c).map(|(_, v)| v.is_empty()).unwrap_or(true)
        });
        if unmapped {
            reasons.push("ready-gate:unmapped-acceptance");
        }
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feature_without_assay_is_not_ready() {
        let r = story_ready_to_run_reasons("FEATURE", Some("it works"), None, None);
        assert!(r.contains(&"ready-gate:missing-assay-plan"));
    }
    #[test]
    fn research_is_not_gated() {
        assert!(story_ready_to_run_reasons("RESEARCH", None, None, None).is_empty());
    }
}

//! The lane vocabulary and the evidence marker. Originally the port of `workflow_app/forge/forge-role-mapping.ts`.
//!
//! WHAT IS NO LONGER HERE (2026-10-02): the node → lane match (`forge_role_node_plan`). `FORGE_SDLC-v6.xml` binds
//! every executable task-node to its service, and a service is a lane, so a node's lane is read from the definition
//! ([`crate::engine::service_binding::lane_for_node`]). The Lead's phases, which the definition does not carry, are
//! the Lead lane's own (`roles::lead`).

use crate::engine::facts::ForgeGateEvidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneId {
    Scout,
    Architect,
    Lead,
    Smith,
    Assay,
    Inspector,
    DevOps,
}

impl LaneId {
    /// Every lane, in the order the enum declares them.
    pub const ALL: [LaneId; 7] = [
        LaneId::Scout,
        LaneId::Architect,
        LaneId::Lead,
        LaneId::Smith,
        LaneId::Assay,
        LaneId::Inspector,
        LaneId::DevOps,
    ];

    /// The service key this lane is registered under — the key `FORGE_SDLC-v6.xml` writes as `service="…"`.
    ///
    /// A lane IS a service (the registry refuses a second owner for one), so this is the identity between the two
    /// names, not a node table: which NODE belongs to which service is the definition's to say
    /// ([`crate::engine::service_binding::lane_for_node`]).
    pub fn service_key(self) -> &'static str {
        match self {
            LaneId::Scout => crate::roles::scout::SCOUT_SERVICE_ID,
            LaneId::Architect => crate::roles::architect::ARCHITECT_SERVICE_ID,
            LaneId::Lead => crate::roles::lead::LEAD_SERVICE_ID,
            LaneId::Smith => crate::roles::smith::SMITH_SERVICE_ID,
            LaneId::Assay => crate::roles::qa::ASSAY_SERVICE_ID,
            LaneId::Inspector => crate::roles::inspector::INSPECTOR_SERVICE_ID,
            LaneId::DevOps => crate::roles::dev_ops::DEVOPS_SERVICE_ID,
        }
    }

    /// The lane a service key names, or `None` for a key no lane is registered under.
    pub fn for_service_key(key: &str) -> Option<LaneId> {
        Self::ALL.into_iter().find(|lane| lane.service_key() == key)
    }
}

const PREFIX: &str = "FORGE_EVIDENCE_JSON:";

fn allowed_enum(key: &str, value: &str) -> bool {
    match key {
        "researchDisposition" => matches!(value, "IMPLEMENT" | "ARCHIVE" | "HOLD"),
        "leadDecision" => matches!(value, "SOLO" | "SMITH" | "SPLIT" | "HOLD" | "ASSAY"),
        "disposition" => matches!(value, "REPAIR" | "REPLAN" | "ESCALATE"),
        "failureClass" => matches!(
            value,
            "CODE_DEFECT"
                | "TEST_DEFECT"
                | "ARCHITECTURE_GAP"
                | "REQUIREMENTS_GAP"
                | "UNKNOWN_CAUSE"
                | "ENVIRONMENT"
                | "MIGRATION"
                | "PUBLISH_CONFLICT"
                | "DEPLOYMENT"
                | "PRODUCTION_SMOKE"
                | "HOLD"
        ),
        "failedReleaseStage" => matches!(
            value,
            "PUBLISH" | "DEV_MIGRATION" | "PROD_MIGRATION" | "DERIVED_REFRESH" | "DEPLOY" | "SMOKE"
        ),
        "resumeTarget" => matches!(
            value,
            "SCOUT"
                | "DIAGNOSE"
                | "ARCHITECT"
                | "LEAD"
                | "SMITH"
                | "QA"
                | "DEV_OPS"
                | "PUBLISH"
                | "DEPLOY"
                | "SMOKE"
                | "CANCEL"
        ),
        _ => false,
    }
}

/// Parse one `FORGE_EVIDENCE_JSON:` line. Prose never becomes routing.
pub fn parse_forge_evidence_marker(text: &str) -> ForgeGateEvidence {
    let line = text.lines().find(|l| l.trim().starts_with(PREFIX));
    let Some(line) = line else {
        return ForgeGateEvidence::default();
    };
    let raw = line.trim().trim_start_matches(PREFIX).trim();
    parse_marker_object(raw)
}

fn parse_marker_object(raw: &str) -> ForgeGateEvidence {
    let mut ev = ForgeGateEvidence::default();
    // The payload IS JSON, so it is parsed as JSON: a hand-split on commas cannot see that a
    // key appears twice (last-wins lets prose choose the routing) and cannot tell `"true"` the
    // string from `true` the boolean. Anything that is not one JSON object is refused whole.
    let raw = raw.trim();
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => return ev,
    };
    let obj = match value.as_object() {
        Some(obj) => obj,
        None => return ev,
    };
    // `serde_json` itself takes last-wins on duplicates, so the top-level keys are scanned for a
    // repeat first: a duplicated decision key refuses the whole marker rather than routing on
    // whichever copy won.
    {
        let mut seen = std::collections::HashSet::new();
        for key in top_level_keys(raw) {
            if !seen.insert(key) {
                return ForgeGateEvidence::default();
            }
        }
    }
    for (key, val) in obj {
        match key.as_str() {
            "scoutRequired"
            | "rootCauseKnown"
            | "diagnosisBlocked"
            | "architectureSuspect"
            | "architectureReviewRequired"
            | "qaReviewRequired"
            | "qaReviewPassed"
            | "qaPassed"
            | "migrationRequired"
            | "derivedRefreshRequired"
            | "deploymentRequired" => {
                // Only an unquoted boolean counts: a quoted `"true"` is a string, and a string
                // is refused (left `None`) rather than misread as `false`.
                let Some(b) = val.as_bool() else {
                    continue;
                };
                match key.as_str() {
                    "scoutRequired" => ev.scout_required = Some(b),
                    "rootCauseKnown" => ev.root_cause_known = Some(b),
                    "diagnosisBlocked" => ev.diagnosis_blocked = Some(b),
                    "architectureSuspect" => ev.architecture_suspect = Some(b),
                    "architectureReviewRequired" => ev.architecture_review_required = Some(b),
                    "qaReviewRequired" => ev.qa_review_required = Some(b),
                    "qaReviewPassed" => ev.qa_review_passed = Some(b),
                    "qaPassed" => ev.qa_passed = Some(b),
                    "migrationRequired" => ev.migration_required = Some(b),
                    "derivedRefreshRequired" => ev.derived_refresh_required = Some(b),
                    "deploymentRequired" => ev.deployment_required = Some(b),
                    _ => {}
                }
            }
            "splitCount" => {
                if let Some(n) = val.as_i64() {
                    if (2..=8).contains(&n) {
                        ev.split_count = Some(n);
                    }
                }
            }
            "researchDisposition"
            | "leadDecision"
            | "disposition"
            | "failureClass"
            | "failedReleaseStage"
            | "resumeTarget" => {
                let Some(s) = val.as_str() else {
                    continue;
                };
                if allowed_enum(key, s) {
                    match key.as_str() {
                        "researchDisposition" => ev.research_disposition = Some(s.into()),
                        "leadDecision" => ev.lead_decision = Some(s.into()),
                        "disposition" => ev.disposition = Some(s.into()),
                        "failureClass" => ev.failure_class = Some(s.into()),
                        "failedReleaseStage" => ev.failed_release_stage = Some(s.into()),
                        "resumeTarget" => ev.resume_target = Some(s.into()),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    ev
}

/// Every top-level key of one JSON object, in order. Runs only on input `serde_json` already
/// accepted, so this is a duplicate scan, not a second parser: strings (with `\` escapes) are
/// skipped whole so a `:` inside a value is never mistaken for a key, and braces inside strings
/// never change the depth.
fn top_level_keys(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut keys = Vec::new();
    let mut i = 0;
    while i < chars.len() && chars[i] != '{' {
        i += 1;
    }
    if i >= chars.len() {
        return keys;
    }
    i += 1;
    let mut depth: u32 = 1;
    while i < chars.len() && depth > 0 {
        match chars[i] {
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < chars.len() {
                    match chars[i] {
                        '\\' => {
                            if i + 1 < chars.len() {
                                s.push(chars[i + 1]);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        '"' => {
                            i += 1;
                            break;
                        }
                        c => {
                            s.push(c);
                            i += 1;
                        }
                    }
                }
                if depth == 1 {
                    let mut j = i;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j < chars.len() && chars[j] == ':' {
                        keys.push(s);
                    }
                }
            }
            '{' | '[' => {
                depth += 1;
                i += 1;
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ASSAY is the Lead's decision for a story whose work already exists on the base
    /// (FORGE-VERIFY-EXISTING-COMPLETE-01). It is one of the engine's `LEAD_DECISIONS`
    /// (`phase.rs:14`) and one of the `execution_shape` conditions
    /// (`FORGE_SDLC-v6.xml:257`), so the marker parser must record it rather than drop it —
    /// an unparsed ASSAY is reported as a missing `lead-decision` and holds the run.
    #[test]
    fn lead_decision_assay_is_parsed() {
        let ev = parse_forge_evidence_marker(
            "prose\nFORGE_EVIDENCE_JSON: {\"leadDecision\":\"ASSAY\"}\n",
        );
        assert_eq!(ev.lead_decision.as_deref(), Some("ASSAY"));
    }

    /// The enum still refuses anything the engine cannot route.
    #[test]
    fn lead_decision_unknown_value_is_refused() {
        let ev = parse_forge_evidence_marker("FORGE_EVIDENCE_JSON: {\"leadDecision\":\"MAYBE\"}");
        assert!(ev.lead_decision.is_none());
    }

    /// The bug these tests exist for: the marker was split on commas by hand, so a second
    /// `leadDecision` silently overrode the first (last-wins) and prose chose the routing.
    /// The payload is JSON, and JSON with a duplicated key is refused whole — in BOTH
    /// orderings, so neither value can smuggle itself in as "the second one".
    #[test]
    fn duplicate_lead_decision_hold_then_smith_is_refused() {
        let ev = parse_forge_evidence_marker(
            "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"HOLD\",\"leadDecision\":\"SMITH\"}",
        );
        assert!(
            ev.lead_decision.is_none(),
            "duplicate keys are refused, got {:?}",
            ev.lead_decision
        );
    }

    #[test]
    fn duplicate_lead_decision_smith_then_hold_is_refused() {
        let ev = parse_forge_evidence_marker(
            "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"SMITH\",\"leadDecision\":\"HOLD\"}",
        );
        assert!(
            ev.lead_decision.is_none(),
            "duplicate keys are refused, got {:?}",
            ev.lead_decision
        );
    }

    /// The bug this test exists for: only unquoted `true`/`false` are booleans. A quoted
    /// `"true"` is a string, and the old hand-splitter read it as `false` — a confident
    /// wrong answer where silence (None) is the honest one.
    #[test]
    fn quoted_booleans_are_refused() {
        let ev = parse_forge_evidence_marker("FORGE_EVIDENCE_JSON: {\"qaPassed\":\"true\"}");
        assert!(
            ev.qa_passed.is_none(),
            "quoted booleans are refused, got {:?}",
            ev.qa_passed
        );
        let ev = parse_forge_evidence_marker("FORGE_EVIDENCE_JSON: {\"qaPassed\":\"false\"}");
        assert!(
            ev.qa_passed.is_none(),
            "quoted booleans are refused, got {:?}",
            ev.qa_passed
        );
        let ev = parse_forge_evidence_marker("FORGE_EVIDENCE_JSON: {\"qaPassed\":true}");
        assert_eq!(ev.qa_passed, Some(true));
    }
}

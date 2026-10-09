//! The lane vocabulary and the evidence marker. Originally the port of `workflow_app/forge/forge-role-mapping.ts`.
//!
//! WHAT IS NO LONGER HERE (2026-10-02): the node → lane match (`forge_role_node_plan`). `FORGE_SDLC-v6.xml` binds
//! every executable task-node to its service, and a service is a lane, so a node's lane is read from the definition
//! ([`crate::engine::service_binding::lane_for_node`]). The Lead's phases, which the definition does not carry, are
//! the Lead lane's own (`roles::lead`).

use crate::engine::facts::ForgeGateEvidence;
use serde::Deserialize;

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
const MAX_MARKER_BYTES: usize = 8 * 1024;
const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub enum RoleOutputParse {
    NoMarker,
    Valid {
        schema_version: u32,
        evidence: ForgeGateEvidence,
    },
    Malformed(String),
    UnsupportedSchema(u32),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoleEvidenceDto {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    scout_required: Option<bool>,
    #[serde(default)]
    root_cause_known: Option<bool>,
    #[serde(default)]
    diagnosis_blocked: Option<bool>,
    #[serde(default)]
    architecture_suspect: Option<bool>,
    #[serde(default)]
    architecture_review_required: Option<bool>,
    #[serde(default)]
    qa_review_required: Option<bool>,
    #[serde(default)]
    qa_review_passed: Option<bool>,
    #[serde(default)]
    qa_passed: Option<bool>,
    #[serde(default)]
    migration_required: Option<bool>,
    #[serde(default)]
    derived_refresh_required: Option<bool>,
    #[serde(default)]
    deployment_required: Option<bool>,
    #[serde(default)]
    split_count: Option<i64>,
    #[serde(default)]
    research_disposition: Option<String>,
    #[serde(default)]
    lead_decision: Option<String>,
    #[serde(default)]
    disposition: Option<String>,
    #[serde(default)]
    failure_class: Option<String>,
    #[serde(default)]
    failed_release_stage: Option<String>,
    #[serde(default)]
    resume_target: Option<String>,
}

impl RoleEvidenceDto {
    fn into_evidence(self) -> Result<ForgeGateEvidence, String> {
        if let Some(n) = self.split_count {
            if !(2..=8).contains(&n) {
                return Err("splitCount must be between 2 and 8".into());
            }
        }
        for (key, value) in [
            ("researchDisposition", self.research_disposition.as_deref()),
            ("leadDecision", self.lead_decision.as_deref()),
            ("disposition", self.disposition.as_deref()),
            ("failureClass", self.failure_class.as_deref()),
            ("failedReleaseStage", self.failed_release_stage.as_deref()),
            ("resumeTarget", self.resume_target.as_deref()),
        ] {
            if value.is_some_and(|value| !allowed_enum(key, value)) {
                return Err(format!("{key} has an unsupported value"));
            }
        }
        Ok(ForgeGateEvidence {
            scout_required: self.scout_required,
            root_cause_known: self.root_cause_known,
            diagnosis_blocked: self.diagnosis_blocked,
            architecture_suspect: self.architecture_suspect,
            architecture_review_required: self.architecture_review_required,
            qa_review_required: self.qa_review_required,
            qa_review_passed: self.qa_review_passed,
            qa_passed: self.qa_passed,
            migration_required: self.migration_required,
            derived_refresh_required: self.derived_refresh_required,
            deployment_required: self.deployment_required,
            split_count: self.split_count,
            research_disposition: self.research_disposition,
            lead_decision: self.lead_decision,
            disposition: self.disposition,
            failure_class: self.failure_class,
            failed_release_stage: self.failed_release_stage,
            resume_target: self.resume_target,
            ..ForgeGateEvidence::default()
        })
    }
}

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

/// Fields a model-authored role marker may propose at the shared authority boundary.
/// Requirements and measured/release outcomes intentionally have no model owner.
pub fn unauthorized_role_fields(
    lane: LaneId,
    node_id: &str,
    evidence: &ForgeGateEvidence,
) -> Vec<&'static str> {
    let allowed: &[&str] = match lane {
        LaneId::Scout => &["rootCauseKnown", "diagnosisBlocked"],
        LaneId::Architect if node_id == crate::roles::architect::RESEARCH_ARCHITECT_NODE => &[
            "researchDisposition",
            "architectureSuspect",
            "architectureReviewRequired",
        ],
        LaneId::Architect => &["architectureSuspect", "architectureReviewRequired"],
        LaneId::Lead if crate::roles::lead::is_failure_classifier(node_id) => {
            &["failureClass", "failedReleaseStage"]
        }
        LaneId::Lead if node_id == crate::roles::lead::LEAD_DECISION_NODE => {
            &["leadDecision", "splitCount"]
        }
        LaneId::Inspector => &["qaReviewPassed"],
        LaneId::Smith | LaneId::Assay | LaneId::DevOps | LaneId::Lead => &[],
    };
    let supplied = [
        ("scoutRequired", evidence.scout_required.is_some()),
        ("rootCauseKnown", evidence.root_cause_known.is_some()),
        ("diagnosisBlocked", evidence.diagnosis_blocked.is_some()),
        (
            "architectureSuspect",
            evidence.architecture_suspect.is_some(),
        ),
        (
            "architectureReviewRequired",
            evidence.architecture_review_required.is_some(),
        ),
        ("qaReviewRequired", evidence.qa_review_required.is_some()),
        ("qaReviewPassed", evidence.qa_review_passed.is_some()),
        ("qaPassed", evidence.qa_passed.is_some()),
        ("migrationRequired", evidence.migration_required.is_some()),
        (
            "derivedRefreshRequired",
            evidence.derived_refresh_required.is_some(),
        ),
        ("deploymentRequired", evidence.deployment_required.is_some()),
        ("splitCount", evidence.split_count.is_some()),
        (
            "researchDisposition",
            evidence.research_disposition.is_some(),
        ),
        ("leadDecision", evidence.lead_decision.is_some()),
        ("disposition", evidence.disposition.is_some()),
        ("failureClass", evidence.failure_class.is_some()),
        (
            "failedReleaseStage",
            evidence.failed_release_stage.is_some(),
        ),
        ("resumeTarget", evidence.resume_target.is_some()),
    ];
    supplied
        .into_iter()
        .filter_map(|(field, is_set)| (is_set && !allowed.contains(&field)).then_some(field))
        .collect()
}

/// Parse the role's complete output marker. Exactly one bounded, single-line JSON object is
/// accepted. The unversioned shape is retained as a strict legacy decoder (schema version 0).
pub fn parse_forge_evidence_marker(text: &str) -> RoleOutputParse {
    let mut in_fence = false;
    let mut markers = Vec::new();
    let mut last_nonempty_line = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.is_empty() {
            last_nonempty_line = Some(index);
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        if !in_fence && trimmed.starts_with(PREFIX) {
            markers.push((index, line));
        }
    }
    if markers.is_empty() {
        return RoleOutputParse::NoMarker;
    }
    if markers.len() != 1 {
        return RoleOutputParse::Malformed("expected exactly one evidence marker".into());
    }
    if last_nonempty_line != Some(markers[0].0) {
        return RoleOutputParse::Malformed(
            "evidence marker must be the final nonempty, unquoted line".into(),
        );
    }
    let Some(raw) = markers[0]
        .1
        .trim_start()
        .strip_prefix(PREFIX)
        .map(str::trim)
    else {
        return RoleOutputParse::Malformed("invalid evidence marker framing".into());
    };
    if raw.len() > MAX_MARKER_BYTES {
        return RoleOutputParse::Malformed("evidence marker exceeds the 8 KiB limit".into());
    }
    let dto: RoleEvidenceDto = match serde_json::from_str(raw) {
        Ok(dto) => dto,
        Err(error) => return RoleOutputParse::Malformed(format!("invalid evidence JSON: {error}")),
    };
    let schema_version = dto.schema_version.unwrap_or(0);
    if schema_version > CURRENT_SCHEMA_VERSION {
        return RoleOutputParse::UnsupportedSchema(schema_version);
    }
    let evidence = match dto.into_evidence() {
        Ok(evidence) => evidence,
        Err(error) => return RoleOutputParse::Malformed(error),
    };
    RoleOutputParse::Valid {
        schema_version,
        evidence,
    }
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
        let RoleOutputParse::Valid { evidence, .. } = ev else {
            panic!("expected a valid marker");
        };
        assert_eq!(evidence.lead_decision.as_deref(), Some("ASSAY"));
    }

    /// The enum still refuses anything the engine cannot route.
    #[test]
    fn lead_decision_unknown_value_is_refused() {
        let ev = parse_forge_evidence_marker("FORGE_EVIDENCE_JSON: {\"leadDecision\":\"MAYBE\"}");
        assert!(matches!(ev, RoleOutputParse::Malformed(_)));
    }

    #[test]
    fn typed_marker_preserves_both_boolean_values_and_json_strings() {
        let parsed = parse_forge_evidence_marker(
            "FORGE_EVIDENCE_JSON: {\"schemaVersion\":1,\"qaPassed\":false,\"leadDecision\":\"ASSAY\"}",
        );
        let RoleOutputParse::Valid {
            schema_version,
            evidence,
        } = parsed
        else {
            panic!("expected typed role output");
        };
        assert_eq!(schema_version, 1);
        assert_eq!(evidence.qa_passed, Some(false));
        assert_eq!(evidence.lead_decision.as_deref(), Some("ASSAY"));
    }

    #[test]
    fn punctuation_inside_json_strings_is_parsed_before_enum_validation() {
        let parsed = parse_forge_evidence_marker(
            "FORGE_EVIDENCE_JSON: {\"schemaVersion\":1,\"researchDisposition\":\"HOLD: needs, review\"}",
        );
        assert!(matches!(
            parsed,
            RoleOutputParse::Malformed(reason) if reason.contains("researchDisposition has an unsupported value")
        ));
    }

    #[test]
    fn wrong_boolean_type_duplicate_key_and_duplicate_markers_are_rejected() {
        for raw in [
            "FORGE_EVIDENCE_JSON: {\"schemaVersion\":1,\"qaPassed\":\"false\"}",
            "FORGE_EVIDENCE_JSON: {\"schemaVersion\":1,\"qaPassed\":true,\"qaPassed\":false}",
            "FORGE_EVIDENCE_JSON: {\"schemaVersion\":1}\nFORGE_EVIDENCE_JSON: {\"schemaVersion\":1}",
        ] {
            assert!(matches!(
                parse_forge_evidence_marker(raw),
                RoleOutputParse::Malformed(_)
            ));
        }
    }

    #[test]
    fn quoted_or_code_fenced_marker_text_is_not_authoritative() {
        assert!(matches!(
            parse_forge_evidence_marker(
                "```text\nFORGE_EVIDENCE_JSON: {\"leadDecision\":\"HOLD\"}\n```"
            ),
            RoleOutputParse::NoMarker
        ));
        assert!(matches!(
            parse_forge_evidence_marker("> FORGE_EVIDENCE_JSON: {\"leadDecision\":\"HOLD\"}"),
            RoleOutputParse::NoMarker
        ));
        assert!(matches!(
            parse_forge_evidence_marker(
                "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"HOLD\"}\nquoted after"
            ),
            RoleOutputParse::Malformed(_)
        ));
    }

    #[test]
    fn unknown_fields_and_future_schemas_are_rejected_explicitly() {
        assert!(matches!(
            parse_forge_evidence_marker(
                "FORGE_EVIDENCE_JSON: {\"schemaVersion\":1,\"qaPas\":true}"
            ),
            RoleOutputParse::Malformed(_)
        ));
        assert!(matches!(
            parse_forge_evidence_marker("FORGE_EVIDENCE_JSON: {\"schemaVersion\":2}"),
            RoleOutputParse::UnsupportedSchema(2)
        ));
    }

    #[test]
    fn model_cannot_claim_qa_success_or_clear_specification_gates() {
        let evidence = ForgeGateEvidence {
            qa_passed: Some(true),
            qa_review_required: Some(false),
            migration_required: Some(false),
            ..ForgeGateEvidence::default()
        };
        assert_eq!(
            unauthorized_role_fields(LaneId::Scout, "scout", &evidence),
            vec!["qaReviewRequired", "qaPassed", "migrationRequired"]
        );
    }

    #[test]
    fn role_fields_are_node_scoped_and_false_remains_an_explicit_patch() {
        let evidence = ForgeGateEvidence {
            lead_decision: Some("HOLD".into()),
            split_count: Some(3),
            ..ForgeGateEvidence::default()
        };
        assert!(unauthorized_role_fields(LaneId::Lead, "lead_pre", &evidence).is_empty());
        assert_eq!(
            unauthorized_role_fields(LaneId::Lead, "lead_post", &evidence),
            vec!["splitCount", "leadDecision"]
        );
    }
}

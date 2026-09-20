//! Port of `workflow_app/forge/forge-role-mapping.ts` (plan + evidence marker).

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadPhase {
    Pre,
    Implement,
    Post,
}

#[derive(Debug, Clone)]
pub struct ForgeRoleNodePlan {
    pub lane: LaneId,
    pub lead_phase: Option<LeadPhase>,
}

pub fn forge_role_node_plan(node_id: &str) -> Result<ForgeRoleNodePlan, String> {
    Ok(match node_id {
        "research_scout" | "feature_scout" | "diagnose_scout" | "repair_scout" => {
            ForgeRoleNodePlan {
                lane: LaneId::Scout,
                lead_phase: None,
            }
        }
        "research_architect" | "architect" | "repair_architect" => ForgeRoleNodePlan {
            lane: LaneId::Architect,
            lead_phase: None,
        },
        "lead_pre" => ForgeRoleNodePlan {
            lane: LaneId::Lead,
            lead_phase: Some(LeadPhase::Pre),
        },
        "lead_solo_implement" => ForgeRoleNodePlan {
            lane: LaneId::Lead,
            lead_phase: Some(LeadPhase::Implement),
        },
        "lead_post" => ForgeRoleNodePlan {
            lane: LaneId::Lead,
            lead_phase: Some(LeadPhase::Post),
        },
        "failure_classifier" => ForgeRoleNodePlan {
            lane: LaneId::Lead,
            lead_phase: Some(LeadPhase::Pre),
        },
        "smith" | "smith_split_work" | "repair_smith" | "fast_smith" | "fast_repair_smith" => {
            ForgeRoleNodePlan {
                lane: LaneId::Smith,
                lead_phase: None,
            }
        }
        "qa_review" => ForgeRoleNodePlan {
            lane: LaneId::Inspector,
            lead_phase: None,
        },
        "qa_verify" | "fast_qa_verify" => ForgeRoleNodePlan {
            lane: LaneId::Assay,
            lead_phase: None,
        },
        "repair_devops" | "deploy" | "production_smoke" => ForgeRoleNodePlan {
            lane: LaneId::DevOps,
            lead_phase: None,
        },
        other => {
            return Err(format!(
                "No Forge agent-runtime mapping for engine node '{other}'"
            ))
        }
    })
}

const PREFIX: &str = "FORGE_EVIDENCE_JSON:";

fn allowed_enum(key: &str, value: &str) -> bool {
    match key {
        "researchDisposition" => matches!(value, "IMPLEMENT" | "ARCHIVE" | "HOLD"),
        "leadDecision" => matches!(value, "SOLO" | "SMITH" | "SPLIT" | "HOLD"),
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
    let body = raw.trim().trim_start_matches('{').trim_end_matches('}');
    for part in body.split(',') {
        let mut kv = part.splitn(2, ':');
        let key = kv.next().unwrap_or("").trim().trim_matches('"');
        let val = kv.next().unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        match key {
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
                let b = val == "true";
                match key {
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
                if let Ok(n) = val.parse::<i64>() {
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
                let s = val.trim_matches('"');
                if allowed_enum(key, s) {
                    match key {
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

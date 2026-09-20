//! Port of `workflow_app/forge/agents/role-agents.ts` collect() hooks.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::phase::{ForgePhaseAgent, RoleEffectPorts};
use crate::engine::role_mapping::{parse_forge_evidence_marker, LeadPhase};

pub fn forge_agent_collect(
    node_id: &str,
    evidence: ForgeGateEvidence,
    raw: &str,
    ports: &RoleEffectPorts,
) -> Result<ForgeGateEvidence, String> {
    let agent = ForgePhaseAgent::new(node_id)?;
    let marked = parse_forge_evidence_marker(raw);
    match node_id {
        "failure_classifier" => {
            let mut next = evidence.merge_over(&ForgeGateEvidence::default());
            next = marked.merge_over(&next);
            if next.failed_release_stage.is_some() {
                if let Some(stage) = next
                    .stage_failure_class
                    .clone()
                    .or_else(|| next.failure_class.clone())
                {
                    next.classifier_failure_class = next.failure_class.clone();
                    next.failure_class = Some(stage);
                }
            }
            Ok(next)
        }
        "research_architect" => Ok(marked.merge_over(&evidence)),
        n if agent.is_architect => {
            let mut next = marked.merge_over(&evidence);
            if next.findings.is_none() && next.research_disposition.is_none() {
                next.deliverable_rejection = Some(
                    "ARCHITECT_HANDOFF_MISSING: emit FORGE_ARCHITECT_HANDOFF or FORGE_FINDINGS_JSON"
                        .into(),
                );
            }
            Ok(next)
        }
        "lead_pre" => {
            // PRE: chat JSON cannot set leadDecision.
            let mut stripped = marked;
            stripped.lead_decision = None;
            stripped.split_count = None;
            Ok(stripped.merge_over(&evidence))
        }
        n if agent.plan.lead_phase == Some(LeadPhase::Implement)
            || agent.plan.lead_phase == Some(LeadPhase::Post) =>
        {
            let mut stripped = marked;
            stripped.lead_decision = None;
            stripped.split_count = None;
            Ok(stripped.merge_over(&evidence))
        }
        n if agent.is_scout => Ok(marked.merge_over(&evidence)),
        n if matches!(
            n,
            "smith" | "smith_split_work" | "repair_smith" | "fast_smith" | "fast_repair_smith"
        ) =>
        {
            Ok(marked.merge_over(&evidence))
        }
        "qa_verify" | "fast_qa_verify" | "qa_review" => Ok(marked.merge_over(&evidence)),
        "deploy" | "production_smoke" | "repair_devops" => {
            let mut next = marked.merge_over(&evidence);
            if let Some(batch) = ports.deployment_deferred_to_batch {
                next.deployment_deferred_to_batch = Some(batch);
                return Ok(next);
            }
            if let Some(r) = &ports.deployment_receipt {
                next.deployment_receipt = Some(r.clone());
                if let Some(s) = &ports.deployed_sha {
                    next.deployed_sha = Some(s.to_ascii_lowercase());
                }
            }
            if let Some(r) = &ports.production_verification_receipt {
                next.production_verification_receipt = Some(r.clone());
                if let Some(s) = &ports.production_verified_sha {
                    next.production_verified_sha = Some(s.to_ascii_lowercase());
                }
            }
            Ok(next)
        }
        _ => Ok(marked.merge_over(&evidence)),
    }
}

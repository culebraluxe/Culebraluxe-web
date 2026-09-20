//! Port of `workflow_app/forge/agents/forge-phase-agent.ts`.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::role_mapping::{forge_role_node_plan, ForgeRoleNodePlan, LaneId, LeadPhase};

const SCOUT_NODES: &[&str] = &[
    "research_scout",
    "feature_scout",
    "diagnose_scout",
    "repair_scout",
];
const ARCHITECT_NODES: &[&str] = &["architect", "repair_architect", "research_architect"];
const RESEARCH_DISPOSITIONS: &[&str] = &["IMPLEMENT", "ARCHIVE", "HOLD"];
const LEAD_DECISIONS: &[&str] = &["SOLO", "SMITH", "SPLIT", "HOLD", "ASSAY"];
const FAILURE_CLASSES: &[&str] = &[
    "CODE_DEFECT",
    "TEST_DEFECT",
    "ARCHITECTURE_GAP",
    "REQUIREMENTS_GAP",
    "UNKNOWN_CAUSE",
    "ENVIRONMENT",
    "MIGRATION",
    "PUBLISH_CONFLICT",
    "DEPLOYMENT",
    "PRODUCTION_SMOKE",
    "HOLD",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseDeliverableKind {
    ScoutPacket,
    ArchitectPlan,
    LeadDecision,
    FailureClass,
    SmithCandidate,
    QaVerdict,
    DevopsReceipt,
    None,
}

#[derive(Debug, Clone)]
pub struct RoleEffectPorts {
    pub deployment_deferred_to_batch: Option<i64>,
    pub deployment_receipt: Option<String>,
    pub production_verification_receipt: Option<String>,
    pub deployed_sha: Option<String>,
    pub production_verified_sha: Option<String>,
}

impl Default for RoleEffectPorts {
    fn default() -> Self {
        Self {
            deployment_deferred_to_batch: None,
            deployment_receipt: None,
            production_verification_receipt: None,
            deployed_sha: None,
            production_verified_sha: None,
        }
    }
}

pub struct ForgePhaseAgent {
    pub node_id: String,
    pub plan: ForgeRoleNodePlan,
    pub is_scout: bool,
    pub is_architect: bool,
}

impl ForgePhaseAgent {
    pub fn new(node_id: &str) -> Result<Self, String> {
        Ok(Self {
            node_id: node_id.into(),
            plan: forge_role_node_plan(node_id)?,
            is_scout: SCOUT_NODES.contains(&node_id),
            is_architect: ARCHITECT_NODES.contains(&node_id),
        })
    }

    pub fn deliverable_kind(&self) -> PhaseDeliverableKind {
        if self.node_id == "failure_classifier" {
            return PhaseDeliverableKind::FailureClass;
        }
        if self.is_scout {
            return PhaseDeliverableKind::ScoutPacket;
        }
        if self.is_architect {
            return PhaseDeliverableKind::ArchitectPlan;
        }
        if self.plan.lane == LaneId::Lead
            && self.plan.lead_phase == Some(LeadPhase::Pre)
            && self.node_id != "failure_classifier"
        {
            return PhaseDeliverableKind::LeadDecision;
        }
        match self.plan.lane {
            LaneId::Smith => PhaseDeliverableKind::SmithCandidate,
            LaneId::Assay => PhaseDeliverableKind::QaVerdict,
            LaneId::DevOps => PhaseDeliverableKind::DevopsReceipt,
            _ => PhaseDeliverableKind::None,
        }
    }

    pub fn missing_deliverables(
        &self,
        evidence: &ForgeGateEvidence,
        raw: &str,
        scout_context_refs_set: bool,
        architect_brief_set: bool,
    ) -> Vec<&'static str> {
        let mut missing = Vec::new();
        match self.deliverable_kind() {
            PhaseDeliverableKind::ScoutPacket => {
                if !scout_context_refs_set && raw.trim().is_empty() {
                    missing.push("scout-packet");
                }
            }
            PhaseDeliverableKind::ArchitectPlan => {
                let has_findings = evidence
                    .findings
                    .as_ref()
                    .map(|v| !v.is_null())
                    .unwrap_or(false);
                if evidence.deliverable_rejection.is_some()
                    || (!architect_brief_set
                        && evidence.research_disposition.is_none()
                        && !has_findings)
                {
                    missing.push("architect-plan");
                }
            }
            PhaseDeliverableKind::LeadDecision => {
                if evidence.lead_decision.is_none() {
                    missing.push("lead-decision");
                }
            }
            PhaseDeliverableKind::FailureClass => {
                if !FAILURE_CLASSES.contains(&evidence.failure_class.as_deref().unwrap_or("")) {
                    missing.push("failure-class");
                }
            }
            PhaseDeliverableKind::SmithCandidate => {
                if evidence.deliverable_rejection.is_some() || evidence.candidate_sha.is_none() {
                    missing.push("smith-candidate");
                }
            }
            PhaseDeliverableKind::QaVerdict => {
                if evidence.qa_passed.is_none() {
                    missing.push("qa-verdict");
                }
            }
            PhaseDeliverableKind::DevopsReceipt => {
                if evidence.deployment_receipt.is_none()
                    && evidence.production_verification_receipt.is_none()
                    && evidence.deployment_deferred_to_batch.is_none()
                {
                    missing.push("devops-receipt");
                }
            }
            PhaseDeliverableKind::None => {}
        }
        missing
    }

    pub fn routing_decision_missing(&self, evidence: &ForgeGateEvidence) -> Option<&'static str> {
        if self.node_id == "failure_classifier" {
            return if FAILURE_CLASSES.contains(&evidence.failure_class.as_deref().unwrap_or("")) {
                None
            } else {
                Some("failure_class")
            };
        }
        if self.node_id == "research_architect" {
            return if RESEARCH_DISPOSITIONS
                .contains(&evidence.research_disposition.as_deref().unwrap_or(""))
            {
                None
            } else {
                Some("research_disposition")
            };
        }
        if self.plan.lane == LaneId::Lead
            && self.plan.lead_phase == Some(LeadPhase::Pre)
            && self.node_id != "failure_classifier"
        {
            let d = evidence.lead_decision.as_deref().unwrap_or("");
            if !LEAD_DECISIONS.contains(&d) {
                return Some("lead_decision");
            }
            if d == "SPLIT" && evidence.split_count.unwrap_or(0) <= 0 {
                return Some("lead_decision.splitCount");
            }
        }
        None
    }
}

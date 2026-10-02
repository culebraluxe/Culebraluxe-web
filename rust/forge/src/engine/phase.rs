//! The gate's own reading of a Forge turn: what a node owes, and the effect ports a turn's deliverable
//! may arrive through. The surviving half of the port of
//! `workflow_app/forge/agents/forge-phase-agent.ts` — the per-node half of that port is the lanes' now.
//!
//! WHAT IS DELIBERATELY NOT HERE, and where it went (2026-10-02, the seam closure): the *reading of a
//! reply* — the deleted `engine::agents` — is the lane's, through
//! [`crate::roles::hooks::ForgeRoleHooks::collect_evidence`]; and which node owes which kind, or
//! needs which routing decision, is the lane's too, with [`lane_deliverable_kind`] as the answer the lane
//! table gives by default. What stays is the gate's own vocabulary: the kinds, what counts as a delivered
//! one, and the failure classes the gate can route.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::role_mapping::{forge_role_node_plan, LaneId, LeadPhase};

/// The failure classes the gate can route.
///
/// Public because the lane that owns the classifier node (`roles::lead`) states its rule in these terms:
/// the gate owns the vocabulary of what it can route, the lane owns which node must produce one.
pub const FAILURE_CLASSES: &[&str] = &[
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
    /// The bench intent the dispatch carried (migration 167 `launch_intent`): the Cockpit's cap on what the Lead
    /// may decide. `None` means the Lead decides. It lives here beside the other effects because it is an envelope
    /// the run was started under, not something a lane may infer from the story.
    pub bench_intent: Option<String>,
}

impl Default for RoleEffectPorts {
    fn default() -> Self {
        Self {
            deployment_deferred_to_batch: None,
            deployment_receipt: None,
            production_verification_receipt: None,
            deployed_sha: None,
            production_verified_sha: None,
            bench_intent: None,
        }
    }
}

/// What the lane table says a node owes, for every node no lane narrows further.
///
/// The lane's own answer is `ForgeRoleHooks::deliverable_kind`; this is the default it inherits. Reading
/// the table rather than a second list of node names is the point: `engine::role_mapping` is the one place
/// a node is bound to a lane (the map `arch_boundary__011` pins), so a node added there is owed its lane's
/// deliverable with no second edit — which is exactly what the two node-name lists deleted here used to
/// re-derive by hand.
///
/// `None` for a node the table does not know (a caller that must refuse one asks the table directly) and
/// for a node whose deliverable is not this gate's to name.
pub fn lane_deliverable_kind(node_id: &str) -> PhaseDeliverableKind {
    match forge_role_node_plan(node_id) {
        Ok(plan) => match (plan.lane, plan.lead_phase) {
            (LaneId::Scout, _) => PhaseDeliverableKind::ScoutPacket,
            (LaneId::Architect, _) => PhaseDeliverableKind::ArchitectPlan,
            (LaneId::Lead, Some(LeadPhase::Pre)) => PhaseDeliverableKind::LeadDecision,
            (LaneId::Smith, _) => PhaseDeliverableKind::SmithCandidate,
            (LaneId::Assay, _) => PhaseDeliverableKind::QaVerdict,
            (LaneId::DevOps, _) => PhaseDeliverableKind::DevopsReceipt,
            _ => PhaseDeliverableKind::None,
        },
        Err(_) => PhaseDeliverableKind::None,
    }
}

/// What a turn of this kind still owes the gate, named the way the gate names it.
///
/// Pure in the kind, holding the requirement table and no node names: that is what keeps "what does this
/// node owe" a lane's answer (through `ForgeRoleHooks::deliverable_kind`, which decides the kind asked
/// about here) and "is it delivered" the gate's.
pub fn missing_deliverables(
    kind: PhaseDeliverableKind,
    evidence: &ForgeGateEvidence,
    raw: &str,
    scout_context_refs_set: bool,
    architect_brief_set: bool,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    match kind {
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

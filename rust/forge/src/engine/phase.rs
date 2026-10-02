//! The gate's own reading of a Forge turn: what a node owes, and the effect ports a turn's deliverable
//! may arrive through. The surviving half of the port of
//! `workflow_app/forge/agents/forge-phase-agent.ts` — the per-node half of that port is the lanes' now.
//!
//! WHAT IS DELIBERATELY NOT HERE, and where it went (2026-10-02, the seam closure): the *reading of a
//! reply* — the deleted `engine::agents` — is the lane's, through
//! [`crate::roles::hooks::ForgeRoleHooks::collect_evidence`]; and which node owes which kind, or
//! needs which routing decision, is the lane's too, with [`lane_deliverable_kind`] as the answer the gate gives
//! by default for a node's service. What stays is the gate's own vocabulary: the kinds, what counts as a delivered
//! one, and the failure classes the gate can route.

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::service_binding::service_for_node;

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

/// What a node's service owes the gate, for every node no lane narrows further.
///
/// The lane's own answer is `ForgeRoleHooks::deliverable_kind`; this is the default it inherits. Which service
/// owns a node is the workflow definition's (`service="forge.*"`, read through [`service_for_node`]), so a node
/// added to the definition is owed its service's deliverable with no second edit here — which is what the
/// retired node → lane table (`engine::role_mapping`) used to be consulted for, and what the two node-name
/// lists deleted from this file before it re-derived by hand.
///
/// The arms are service KEYS, spelled as `FORGE_SDLC-v6.xml` spells them, not a second node list: the keys are
/// the binding's vocabulary, and a key this match does not know answers `None` rather than a guessed deliverable.
///
/// `None` for a node no service owns (a caller that must refuse one asks the binding directly) and for a service
/// whose deliverable is not this gate's to name: Lead's phases are Lead's own answer (`roles::lead`, one service
/// across three turns) and the Inspector's semantic review names no artifact this gate can check.
pub fn lane_deliverable_kind(node_id: &str) -> PhaseDeliverableKind {
    match service_for_node(node_id) {
        Some("forge.scout") => PhaseDeliverableKind::ScoutPacket,
        Some("forge.architect") => PhaseDeliverableKind::ArchitectPlan,
        Some("forge.smith") => PhaseDeliverableKind::SmithCandidate,
        Some("forge.assay") => PhaseDeliverableKind::QaVerdict,
        Some("forge.devops") => PhaseDeliverableKind::DevopsReceipt,
        Some(_) | None => PhaseDeliverableKind::None,
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

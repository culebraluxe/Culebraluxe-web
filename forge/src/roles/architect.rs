//! Architect lane.
//!
//! The Architect's turn carries a handoff, and this service owns what is read out of it: the handoff is
//! parsed and assessed against the base it claims, a handoff that does not hold is a rejected deliverable
//! rather than a finding to debate, and one that does hold reports the count of findings it carried. The
//! lane inherits the shared execution lifecycle (see `roles::lifecycle`) instead of copying it.

use crate::engine::architect::ArchitectAssessment;
use crate::engine::executor::ForgeRoleRunner;
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::RoleEffectPorts;
use crate::engine::role_mapping::LaneId;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::lifecycle::{ForgeRoleContext, ForgeRoleTurn};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::architect::{assess_architect_handoff, parse_architect_handoff};

pub const ARCHITECT_SERVICE_ID: &str = "forge.architect";

/// The Architect nodes whose turn carries a handoff — the ones this lane assesses.
///
/// `research_architect` is deliberately not one: it answers with a disposition rather than a handoff, so it
/// owes neither the handoff nor the rejection below.
pub fn carries_architect_handoff(node_id: &str) -> bool {
    matches!(node_id, "architect" | "repair_architect")
}

/// The Architect node that answers with a research disposition instead of a handoff.
pub const RESEARCH_ARCHITECT_NODE: &str = "research_architect";

/// The dispositions the gate can route. The research node is the one that must carry one, which is what makes
/// this list and that node one rule, owned here.
pub const RESEARCH_DISPOSITIONS: &[&str] = &["IMPLEMENT", "ARCHIVE", "HOLD"];

/// The rejection a handoff node's reply earns when it carries neither a handoff nor findings, phrased once so
/// the self-heal directive and the gate read the same sentence.
pub const ARCHITECT_HANDOFF_MISSING: &str =
    "ARCHITECT_HANDOFF_MISSING: emit FORGE_ARCHITECT_HANDOFF or FORGE_FINDINGS_JSON";

/// Architect's own reading, supplied to the shared lifecycle as this lane's hooks.
///
/// It adopts no candidate: the Architect produces a handoff, not code, so the evidence's candidate stays
/// whatever the delivering lane put there.
pub struct ArchitectHooks;

impl ForgeRoleHooks for ArchitectHooks {
    /// A handoff node that replies with neither a handoff nor findings has delivered nothing, and that is read
    /// here — on the reply — rather than left to the gate's missing-deliverable check, so the same attempt can be
    /// re-asked with a directive naming the sentence it lacked.
    fn collect_evidence(
        &self,
        node_id: &str,
        evidence: ForgeGateEvidence,
        raw: &str,
        _ports: &RoleEffectPorts,
    ) -> std::result::Result<ForgeGateEvidence, String> {
        let mut next = marker_evidence(raw, &evidence);
        if !carries_architect_handoff(node_id) {
            return Ok(next);
        }
        // The handoff is the deliverable, so a reply that carries a readable one HAS delivered — on the attempt
        // that produced it. The evidence marker never carries findings for a handoff, so reading only the marker
        // (as this did) called every valid handoff missing, re-prompted the Architect and paid for a second turn
        // on every story. Whether the handoff HOLDS against its base is still assessed once, in `interpret_turn`.
        if next.findings.is_none() {
            if let Some(handoff) = crate::engine::architect::parse_architect_handoff(raw) {
                next.findings = Some(workflow::Value::from(handoff.findings.len() as i64));
            }
        }
        if next.findings.is_none() && next.research_disposition.is_none() {
            next.deliverable_rejection = Some(ARCHITECT_HANDOFF_MISSING.into());
        }
        Ok(next)
    }

    /// The research node owes a disposition the gate can route. The handoff nodes owe no routing decision: their
    /// handoff is assessed below, and an assessment that fails is a rejected deliverable, not a missing decision.
    fn routing_decision_missing(
        &self,
        node_id: &str,
        evidence: &ForgeGateEvidence,
    ) -> Option<&'static str> {
        if node_id != RESEARCH_ARCHITECT_NODE {
            return None;
        }
        if RESEARCH_DISPOSITIONS.contains(&evidence.research_disposition.as_deref().unwrap_or("")) {
            None
        } else {
            Some("research_disposition")
        }
    }

    fn interpret_turn(
        &self,
        ctx: &ForgeRoleContext<'_>,
        turn: &ForgeRoleTurn<'_>,
        evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        architect_reading(ctx, turn, evidence)
    }
}

/// Forge-internal service for every Architect workflow node.
pub struct ArchitectService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> ArchitectService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for ArchitectService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: ARCHITECT_SERVICE_ID,
            lane: LaneId::Architect,
            description: "Forge architecture service for Architect analysis lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }

    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &ArchitectHooks
    }
}

/// Architect's own reading: the handoff is parsed and assessed against the base it claims. A handoff
/// that does not hold is a rejected deliverable rather than a finding to debate; one that does hold
/// reports the count of findings it carried.
fn architect_reading(
    ctx: &ForgeRoleContext<'_>,
    turn: &ForgeRoleTurn<'_>,
    evidence: &mut ForgeGateEvidence,
) -> Result<()> {
    if carries_architect_handoff(turn.node_id) {
        let handoff = parse_architect_handoff(&turn.out.raw);
        match assess_architect_handoff(
            handoff.as_ref(),
            Some(&|base, path| ctx.harness.exists_on_base_ref(base, path)),
        ) {
            ArchitectAssessment::Ok { .. } => {
                if let Some(h) = handoff {
                    evidence.findings = Some(workflow::Value::from(h.findings.len() as i64));
                    evidence.deliverable_rejection = None;
                }
            }
            ArchitectAssessment::Fail { reasons } => {
                evidence.deliverable_rejection = Some(reasons.join("; "));
            }
        }
    }
    Ok(())
}

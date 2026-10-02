//! Architect lane.
//!
//! The Architect's turn carries a handoff, and this service owns what is read out of it: the handoff is
//! parsed and assessed against the base it claims, a handoff that does not hold is a rejected deliverable
//! rather than a finding to debate, and one that does hold reports the count of findings it carried. The
//! lane inherits the shared execution lifecycle (see `roles::lifecycle`) instead of copying it.

use crate::engine::architect::ArchitectAssessment;
use crate::engine::executor::ForgeRoleRunner;
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::role_mapping::LaneId;
use crate::roles::lifecycle::{ForgeRoleContext, ForgeRoleHooks, ForgeRoleTurn};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::architect::{assess_architect_handoff, parse_architect_handoff};

pub const ARCHITECT_SERVICE_ID: &str = "forge.architect";

/// Architect's own reading, supplied to the shared lifecycle as this lane's hooks.
///
/// It adopts no candidate: the Architect produces a handoff, not code, so the evidence's candidate stays
/// whatever the delivering lane put there.
pub struct ArchitectHooks;

impl ForgeRoleHooks for ArchitectHooks {
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
    if turn.node_id == "architect" || turn.node_id == "repair_architect" {
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

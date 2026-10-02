//! Lead lane.
//!
//! Lead owns three phases of the same turn: the pre-implementation decision, the implement (SOLO), and the
//! post decision. Only the middle one is intelligence this service has to supply, and it is not Lead's own:
//! when the Lead decides SOLO it performs Smith's act of delivering code — the candidate becomes the run's
//! candidate and the patch is captured — so the reading is inherited from [`crate::roles::smith`] rather than
//! re-implemented here. Everything else the Lead says is read out of its own evidence marker by the shared
//! lifecycle, which this lane inherits like every other (see `roles::lifecycle`).

use crate::engine::executor::ForgeRoleRunner;
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::role_mapping::LaneId;
use crate::roles::lifecycle::{ForgeRoleContext, ForgeRoleHooks, ForgeRoleTurn};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::graph::{plan_smith_layers, split_eligibility};

pub const LEAD_SERVICE_ID: &str = "forge.lead";

/// Lead's own reading, supplied to the shared lifecycle as this lane's hooks.
///
/// The pre and post decisions need nothing here: the Lead writes its decision into the evidence marker and
/// the lifecycle collects it for every lane alike. The implement phase is code delivery, and that belongs to
/// the lane that owns code delivery.
pub struct LeadHooks;

impl ForgeRoleHooks for LeadHooks {
    fn adopts_candidate_sha(&self, node_id: &str) -> bool {
        crate::roles::smith::delivers_code(node_id)
    }

    fn interpret_turn(
        &self,
        ctx: &ForgeRoleContext<'_>,
        turn: &ForgeRoleTurn<'_>,
        evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        crate::roles::smith::read_delivered_work(ctx, turn, evidence)
    }
}

/// Forge-internal service for every Lead workflow node.
pub struct LeadService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> LeadService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for LeadService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: LEAD_SERVICE_ID,
            lane: LaneId::Lead,
            description: "Forge orchestration service for Lead decision and assembly lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }

    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &LeadHooks
    }
}

//! Scout lane — control plane lives in crate::engine::{phase,runner}.
//!
//! Scout's intelligence is its own diagnosis, and it is carried in the turn's own evidence marker
//! (`researchDisposition`, `rootCauseKnown`, …), which the shared lifecycle collects for every lane alike.
//! So this service owns the lane boundary and supplies no reading: there is nothing a Scout turn needs read
//! out of it that the marker does not already say.

use crate::engine::executor::ForgeRoleRunner;
use crate::engine::role_mapping::LaneId;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};

pub use crate::engine::phase::ForgePhaseAgent as ScoutPhase;

pub const SCOUT_SERVICE_ID: &str = "forge.scout";

/// Forge-internal service for every Scout workflow node.
///
/// The service owns Scout lane identity and authorization — it is what makes "this node is Scout's" a
/// checkable claim — and inherits the shared execution lifecycle unchanged.
pub struct ScoutService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> ScoutService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for ScoutService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: SCOUT_SERVICE_ID,
            lane: LaneId::Scout,
            description: "Forge research and diagnosis service for Scout lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }
}


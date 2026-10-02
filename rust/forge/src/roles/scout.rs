//! Scout lane — control plane lives in crate::engine::{phase,runner}.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::phase::ForgePhaseAgent as ScoutPhase;

pub const SCOUT_SERVICE_ID: &str = "forge.scout";

/// Forge-internal service for every Scout workflow node.
///
/// The service owns Scout lane identity and authorization while the proven runner
/// continues to own OpenCode/session/budget/evidence execution semantics.
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

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

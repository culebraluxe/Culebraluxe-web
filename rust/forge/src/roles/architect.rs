//! Architect lane.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::architect::{assess_architect_handoff, parse_architect_handoff};

pub const ARCHITECT_SERVICE_ID: &str = "forge.architect";

/// Forge-internal service for every Architect workflow node.
///
/// The first cut delegates the proven execution lifecycle to the existing
/// `ForgeRoleRunner`. That keeps OpenCode/session/budget/evidence semantics unchanged
/// while making the Architect boundary real.
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

    fn execute(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

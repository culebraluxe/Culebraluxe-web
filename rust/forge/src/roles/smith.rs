//! Smith lane.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::graph::SmithWorkNode;

pub const SMITH_SERVICE_ID: &str = "forge.smith";

/// Forge-internal service for every Smith workflow node.
///
/// The first cut deliberately delegates the proven execution lifecycle to the existing
/// `ForgeRoleRunner`. That keeps OpenCode/session/budget/evidence semantics unchanged while
/// making the Smith boundary real. Subsequent extraction can move shared lifecycle plumbing
/// behind `AbstractForgeService` without changing Workflow or JobService contracts.
pub struct SmithService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> SmithService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for SmithService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: SMITH_SERVICE_ID,
            lane: LaneId::Smith,
            description: "Forge implementation service for Smith code-generation lanes",
        }
    }

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

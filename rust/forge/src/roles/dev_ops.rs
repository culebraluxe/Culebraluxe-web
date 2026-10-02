//! DevOps / release lane.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::git_publish::HostReleaseExecutor;
pub use crate::engine::release::DbForgeReleaseExecutor;

pub const DEVOPS_SERVICE_ID: &str = "forge.devops";

/// Forge-internal service for release and deployment lanes.
///
/// Publication and deployment authority remain in the proven release/runtime code.
/// This service establishes the canonical Forge role boundary and refuses non-DevOps work.
pub struct DevOpsService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> DevOpsService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for DevOpsService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: DEVOPS_SERVICE_ID,
            lane: LaneId::DevOps,
            description: "Forge release and deployment service for DevOps lanes",
        }
    }

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

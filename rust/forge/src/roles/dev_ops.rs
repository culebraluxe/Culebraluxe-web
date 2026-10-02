//! DevOps / release lane.
//!
//! DevOps OWNS PUBLICATION: the deploy, repair and smoke nodes are this lane's, and no other lane may claim
//! them (the registry refuses a second owner for a lane, and the router refuses a wrong-lane call). The act
//! of publishing is not a reading of a turn, though: it is performed by the release executor
//! ([`DbForgeReleaseExecutor`], [`HostReleaseExecutor`]) and recorded as evidence the shared lifecycle
//! collects. So this service owns the authority and supplies no lane reading of its own.

use crate::engine::executor::ForgeRoleRunner;
use crate::engine::role_mapping::LaneId;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};

pub use crate::engine::git_publish::HostReleaseExecutor;
pub use crate::engine::release::DbForgeReleaseExecutor;

pub const DEVOPS_SERVICE_ID: &str = "forge.devops";

/// Forge-internal service for release and deployment lanes.
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

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }
}

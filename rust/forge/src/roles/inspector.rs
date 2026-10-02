//! Inspector review lane.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub const INSPECTOR_SERVICE_ID: &str = "forge.inspector";

/// Forge-internal service for the Inspector review lane.
///
/// Inspector is deliberately distinct from Assay: Inspector owns review/adjudication
/// work, while Assay owns deterministic verification. Both continue to delegate their
/// established execution semantics to the proven runner during the strangler migration.
pub struct InspectorService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> InspectorService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for InspectorService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: INSPECTOR_SERVICE_ID,
            lane: LaneId::Inspector,
            description: "Forge review service for Inspector lanes",
        }
    }

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

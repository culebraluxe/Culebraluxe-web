//! Lead lane.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::graph::{plan_smith_layers, split_eligibility};

pub const LEAD_SERVICE_ID: &str = "forge.lead";

/// Forge-internal service for every Lead workflow node.
///
/// Lead keeps its existing phase-specific behavior in `ProductionRoleRunner`; this
/// service establishes the canonical Forge role boundary without changing those semantics.
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

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

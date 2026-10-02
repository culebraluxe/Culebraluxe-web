//! Assay/QA lane. Policy in qa_repair; provenance in qa_assert; collect in assay.

use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::assay::{adjudicate_assay, collect_assay_evidence};
pub use crate::engine::qa_assert::assertion_resolution;
pub use crate::engine::qa_repair::route_qa_result;

pub const ASSAY_SERVICE_ID: &str = "forge.assay";

/// Forge-internal service for deterministic QA/Assay verification lanes.
///
/// Assay remains read/test/report only. The proven runner still owns the existing
/// RUST_CONTRACT handling, command execution, evidence collection, and verdict semantics.
pub struct AssayService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> AssayService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for AssayService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: ASSAY_SERVICE_ID,
            lane: LaneId::Assay,
            description: "Forge verification service for Assay/QA execution lanes",
        }
    }

    fn execute(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        self.assert_supports_node(node_id)?;
        self.runner.run(node_id, task)
    }
}

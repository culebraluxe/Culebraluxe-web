//! Inspector review lane.
//!
//! Inspector is deliberately distinct from Assay: Inspector owns review/adjudication work, while Assay owns
//! deterministic verification. That distinction is enforced where it is decided — `role_mapping` gives
//! `qa_review` to this lane and the measurement nodes to Assay — and it is why this service supplies no
//! reading of its own: a reviewer's turn is read out of its own evidence marker, and its review verdict is
//! adjudicated downstream. Nothing here may measure, and Assay may not review.

use crate::engine::executor::ForgeRoleRunner;
use crate::engine::role_mapping::LaneId;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};

pub const INSPECTOR_SERVICE_ID: &str = "forge.inspector";

/// Forge-internal service for the Inspector review lane.
///
/// It points the shared lifecycle at the runner that hosts its turns and claims nothing beyond that: its
/// hooks are the defaults, which adopt no candidate and interpret nothing.
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

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }
}


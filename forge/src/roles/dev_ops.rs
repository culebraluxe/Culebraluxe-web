//! DevOps / release lane.
//!
//! DevOps OWNS PUBLICATION: the deploy, repair and smoke nodes are this lane's, and no other lane may claim
//! them (the registry refuses a second owner for a lane, and the router refuses a wrong-lane call). The act
//! of publishing is not a reading of a turn: it is performed by the release executor
//! ([`DbForgeReleaseExecutor`], [`HostReleaseExecutor`]) and recorded as evidence the shared lifecycle
//! collects.
//!
//! WHAT THIS LANE DOES READ is that evidence (`2026-10-02`, the seam closure). The capability arrives through
//! the turn's effect ports — a batch the deploy was deferred to, a receipt paired with the SHA it was issued
//! for, a production verification paired with its own SHA — and reading it is this lane's, so it lives here
//! rather than in the engine's deleted collect switch.

use crate::engine::executor::ForgeRoleRunner;
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::RoleEffectPorts;
use crate::engine::role_mapping::LaneId;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};

pub use crate::engine::git_publish::HostReleaseExecutor;
pub use crate::engine::release::DbForgeReleaseExecutor;

pub const DEVOPS_SERVICE_ID: &str = "forge.devops";

/// The nodes whose turn presents a deployment capability. Named here because the reading below is this lane's.
pub const RELEASE_NODES: &[&str] = &["deploy", "production_smoke", "repair_devops"];

/// DevOps' own reading, supplied to the shared lifecycle as this lane's hooks.
///
/// The lifecycle's default reading already takes the reply's evidence marker; what this lane adds is the
/// capability that arrives through the ports, which no reply can state and no other lane may claim.
pub struct DevOpsHooks;

impl ForgeRoleHooks for DevOpsHooks {
    fn collect_evidence(
        &self,
        node_id: &str,
        evidence: ForgeGateEvidence,
        raw: &str,
        ports: &RoleEffectPorts,
    ) -> Result<ForgeGateEvidence, String> {
        let mut next = marker_evidence(raw, &evidence);
        if !RELEASE_NODES.contains(&node_id) {
            return Ok(next);
        }
        // A batch says the obligation is out of scope for this run, and it wins over any receipt: a deploy that
        // was deferred did not publish, whatever else the envelope carries.
        if let Some(batch) = ports.deployment_deferred_to_batch {
            next.deployment_deferred_to_batch = Some(batch);
            return Ok(next);
        }
        if let Some(receipt) = &ports.deployment_receipt {
            next.deployment_receipt = Some(receipt.clone());
            if let Some(sha) = &ports.deployed_sha {
                next.deployed_sha = Some(sha.to_ascii_lowercase());
            }
        }
        if let Some(receipt) = &ports.production_verification_receipt {
            next.production_verification_receipt = Some(receipt.clone());
            if let Some(sha) = &ports.production_verified_sha {
                next.production_verified_sha = Some(sha.to_ascii_lowercase());
            }
        }
        Ok(next)
    }
}

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

    /// DevOps reads its own turns: publication's capability arrives through the ports, and no other lane may
    /// claim it (see [`DevOpsHooks`]).
    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &DevOpsHooks
    }
}

use async_trait::async_trait;
use service::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy,
};
use serde_json::Value;

use crate::engine::worker;

pub const FORGE_SERVICE_DOMAIN: &str = "forge";
pub const FORGE_SCHEDULED_PASS_OPERATION: &str = "forge.runScheduledPass";

/// Application-level parent for the Forge control plane.
///
/// This service deliberately owns no Workflow state, queue implementation, role routing,
/// prompt policy or release logic. Those remain in Workflow, JobService and the concrete
/// AbstractForgeService implementations. Its job is to give Forge one application-service
/// identity and one controlled entry into the existing orchestration stack.
#[derive(Debug, Default)]
pub struct ForgeService;

impl ForgeService {
    pub fn new() -> Self {
        Self
    }

    /// Entry used by the unattended scheduler.
    ///
    /// launchd owns process resurrection; this method owns only one bounded Forge worker
    /// pass. The existing worker continues to own stale recovery, durable Storyboard
    /// dispatch claims and bounded per-story concurrency.
    pub fn run_scheduled_pass(&self) -> Result<i32, String> {
        worker::run_worker_pass()
    }
}

fn scheduled_pass_capability() -> ServiceCapability {
    ServiceCapability {
        name: FORGE_SCHEDULED_PASS_OPERATION.into(),
        kind: OperationKind::Command,
        description: "Run one bounded unattended Forge control-plane pass.".into(),
        authorization: "forge.execute".into(),
        idempotent: false,
        execution: ServiceExecutionPolicy::ordered("forge-worker"),
    }
}

#[async_trait]
impl AbstractService for ForgeService {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: FORGE_SERVICE_DOMAIN.into(),
            version: "1".into(),
            description: "CulebraLuxe Forge application control-plane service".into(),
            capabilities: vec![scheduled_pass_capability()],
            dependencies: Vec::new(),
            invariants: vec![
                "Workflow is the only owner of process state and routing.".into(),
                "Generic JobService is the only owner of durable role execution reliability.".into(),
                "ForgeServiceRegistry resolves XML service keys to AbstractForgeService implementations.".into(),
                "ForgeService does not contain Scout/Architect/Lead/Smith/Inspector/Assay/DevOps policy.".into(),
                "The unattended scheduler executes bounded passes; launchd owns resurrection and cadence.".into(),
            ],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        _context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            FORGE_SCHEDULED_PASS_OPERATION => Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                "forge.runScheduledPass is an internal scheduler command and cannot be dispatched as an inline service call.",
                false,
            )),
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: FORGE_SERVICE_DOMAIN.into(),
                operation: operation.into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_parent_service_declares_the_architecture_c_boundaries() {
        let descriptor = ForgeService::new().descriptor();

        assert_eq!(descriptor.domain, FORGE_SERVICE_DOMAIN);
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(
            descriptor.capabilities[0].name,
            FORGE_SCHEDULED_PASS_OPERATION
        );
        assert!(descriptor
            .invariants
            .iter()
            .any(|value| value.contains("Workflow is the only owner")));
        assert!(descriptor
            .invariants
            .iter()
            .any(|value| value.contains("JobService")));
        assert!(descriptor
            .invariants
            .iter()
            .any(|value| value.contains("AbstractForgeService")));
    }

    #[tokio::test]
    async fn scheduler_command_cannot_bypass_the_control_plane_boundary() {
        let envelope = ServiceEnvelope {
            domain: FORGE_SERVICE_DOMAIN.into(),
            operation: FORGE_SCHEDULED_PASS_OPERATION.into(),
            payload: Value::Null,
        };
        let context = ServiceContext {
            actor: service::ServiceActor {
                id: Some("forge-service-test".into()),
                kind: service::ServiceActorKind::System,
            },
            correlation_id: "forge-service-test".into(),
            causation_id: None,
            principal: None,
        };

        let error = ForgeService::new()
            .dispatch(&envelope, &context)
            .await
            .expect_err("scheduled pass must not be inline-dispatchable");

        assert_eq!(error.code(), "DURABLE_COMMAND_REQUIRED");
    }
}

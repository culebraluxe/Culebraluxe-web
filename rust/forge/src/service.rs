use async_trait::async_trait;
use db::{Database, ForgeReadDao};
use domain::ForgeLiveSnapshot;
use serde_json::Value;
use service as service_kernel;
use service_kernel::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy, ServiceInfrastructure,
    ServiceOutcome, ServiceRuntime, ServiceRuntimeError,
};

use crate::engine::worker;

pub const FORGE_SERVICE_DOMAIN: &str = "forge";
pub const FORGE_SCHEDULED_PASS_OPERATION: &str = "forge.runScheduledPass";
pub const FORGE_LIVE_SNAPSHOT_OPERATION: &str = "forge.liveSnapshot";

/// Application-level parent for the Forge control plane.
///
/// This service deliberately owns no Workflow state, queue implementation, role routing,
/// prompt policy or release logic. Those remain in Workflow, JobService and the concrete
/// AbstractForgeService implementations. Its job is to give Forge one application-service
/// identity and one controlled entry into the existing orchestration stack.
#[derive(Default)]
pub struct ForgeService {
    read: Option<ForgeReadDao>,
    runtime: Option<ServiceRuntime>,
}

impl ForgeService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Application-hosted Forge service. Unlike the scheduler-only constructor, this
    /// has the sanctioned read repository and common service runtime required by the
    /// TECH Cockpit's Work in Flight query.
    pub fn for_application(db: Database, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            read: Some(ForgeReadDao::new(db)),
            runtime: Some(ServiceRuntime::new(infrastructure)),
        }
    }

    pub async fn live_snapshot(
        &self,
        selected_story_id: Option<&str>,
        context: &ServiceContext,
    ) -> Result<ForgeLiveSnapshot, ServiceDispatchError> {
        let read = self.read.as_ref().ok_or_else(|| {
            ServiceDispatchError::infrastructure(
                "FORGE_READ_UNAVAILABLE",
                "Forge live reads are available only from the application-hosted ForgeService.",
                false,
            )
        })?;
        let runtime = self.runtime.as_ref().ok_or_else(|| {
            ServiceDispatchError::infrastructure(
                "FORGE_RUNTIME_UNAVAILABLE",
                "Forge service runtime is not configured.",
                false,
            )
        })?;
        let decision = runtime
            .authorize(
                FORGE_SERVICE_DOMAIN,
                "tech.access",
                FORGE_LIVE_SNAPSHOT_OPERATION,
                OperationKind::Query,
                context,
            )
            .await
            .map_err(dispatch_runtime_error)?;

        match read.live_snapshot(selected_story_id, 50).await {
            Ok(snapshot) => {
                runtime
                    .audit(
                        FORGE_SERVICE_DOMAIN,
                        FORGE_LIVE_SNAPSHOT_OPERATION,
                        context,
                        ServiceOutcome::Success,
                        None,
                        decision,
                    )
                    .await
                    .map_err(dispatch_runtime_error)?;
                Ok(snapshot)
            }
            Err(error) => {
                runtime
                    .audit(
                        FORGE_SERVICE_DOMAIN,
                        FORGE_LIVE_SNAPSHOT_OPERATION,
                        context,
                        ServiceOutcome::Failure,
                        Some("DATABASE".into()),
                        decision,
                    )
                    .await
                    .map_err(dispatch_runtime_error)?;
                Err(ServiceDispatchError::infrastructure(
                    "DATABASE",
                    format!("Forge live read failed: {error}"),
                    error.retryable,
                ))
            }
        }
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

fn live_snapshot_capability() -> ServiceCapability {
    ServiceCapability {
        name: FORGE_LIVE_SNAPSHOT_OPERATION.into(),
        kind: OperationKind::Query,
        description: "Read Forge work currently in flight and recent engine work status.".into(),
        authorization: "tech.access".into(),
        idempotent: true,
        execution: ServiceExecutionPolicy::inline(),
    }
}

fn dispatch_runtime_error(error: ServiceRuntimeError) -> ServiceDispatchError {
    match error {
        ServiceRuntimeError::Forbidden { reason, .. } => {
            ServiceDispatchError::business("FORBIDDEN", reason, false)
        }
        ServiceRuntimeError::Authorization(message) => {
            ServiceDispatchError::infrastructure("AUTHORIZATION_UNAVAILABLE", message, true)
        }
        ServiceRuntimeError::Audit(message) => {
            ServiceDispatchError::infrastructure("AUDIT_UNAVAILABLE", message, true)
        }
        ServiceRuntimeError::Event(message) => {
            ServiceDispatchError::infrastructure("DOMAIN_EVENT_UNAVAILABLE", message, true)
        }
        ServiceRuntimeError::Router {
            code,
            message,
            retryable,
            class: _,
        } => ServiceDispatchError::infrastructure(code, message, retryable),
    }
}

fn scheduled_pass_capability() -> ServiceCapability {
    ServiceCapability {
        name: FORGE_SCHEDULED_PASS_OPERATION.into(),
        kind: OperationKind::Command,
        description: "Run one bounded unattended Forge control-plane pass.".into(),
        authorization: "tech.access".into(),
        idempotent: false,
        execution: ServiceExecutionPolicy::queued(),
    }
}

#[async_trait]
impl AbstractService for ForgeService {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: FORGE_SERVICE_DOMAIN.into(),
            version: "1".into(),
            description: "CulebraLuxe Forge application control-plane service".into(),
            capabilities: vec![scheduled_pass_capability(), live_snapshot_capability()],
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
            FORGE_LIVE_SNAPSHOT_OPERATION => {
                let selected = envelope
                    .payload
                    .get("selectedStoryId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                serde_json::to_value(self.live_snapshot(selected, _context).await?)
                    .map_err(|error| ServiceDispatchError::infrastructure(
                        "FORGE_LIVE_SERIALIZATION",
                        error.to_string(),
                        false,
                    ))
            }
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
        assert_eq!(descriptor.capabilities.len(), 2);
        assert_eq!(
            descriptor.capabilities[0].name,
            FORGE_SCHEDULED_PASS_OPERATION
        );
        assert_eq!(
            descriptor.capabilities[1].name,
            FORGE_LIVE_SNAPSHOT_OPERATION
        );
        assert_eq!(descriptor.capabilities[1].authorization, "tech.access");
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
            actor: service_kernel::ServiceActor {
                id: Some("forge-service-test".into()),
                kind: service_kernel::ServiceActorKind::System,
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

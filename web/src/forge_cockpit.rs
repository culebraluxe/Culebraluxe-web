//! App-owned Forge Cockpit read service.
//!
//! This replaces the app's in-process `forge::ForgeService` read path without linking the Forge
//! crate: the read DAO already lives in the app `db` crate, and the authorization/audit semantics
//! are the same `forge` / `tech.access` / `forge.liveSnapshot` contract. The factory engine,
//! roles, queues and release doors stay in Forge and are not reachable from here.

use db::{Database, ForgeReadDao};
use model::ForgeLiveSnapshot;
use services::{
    OperationKind, ServiceContext, ServiceDispatchError, ServiceInfrastructure, ServiceOutcome,
    ServiceRuntime, ServiceRuntimeError,
};

pub const FORGE_SERVICE_DOMAIN: &str = "forge";
pub const FORGE_LIVE_SNAPSHOT_OPERATION: &str = "forge.liveSnapshot";

pub struct ForgeCockpitService {
    read: ForgeReadDao,
    runtime: ServiceRuntime,
}

impl ForgeCockpitService {
    pub fn new(db: Database, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            read: ForgeReadDao::new(db),
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    /// Read the TECH Cockpit's Work in Flight + Engine Work Status data.
    ///
    /// Observation only: no claim, retry, settlement or Workflow transition happens here. The
    /// authorization action, operation name, success/failure audit and error codes match the old
    /// app-hosted ForgeService exactly.
    pub async fn live_snapshot(
        &self,
        selected_story_id: Option<&str>,
        context: &ServiceContext,
    ) -> Result<ForgeLiveSnapshot, ServiceDispatchError> {
        let decision = self
            .runtime
            .authorize(
                FORGE_SERVICE_DOMAIN,
                "tech.access",
                FORGE_LIVE_SNAPSHOT_OPERATION,
                OperationKind::Query,
                context,
            )
            .await
            .map_err(dispatch_runtime_error)?;

        match self.read.live_snapshot(selected_story_id, 50).await {
            Ok(snapshot) => {
                self.runtime
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
                self.runtime
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cockpit_read_keeps_the_forge_operation_contract() {
        assert_eq!(FORGE_SERVICE_DOMAIN, "forge");
        assert_eq!(FORGE_LIVE_SNAPSHOT_OPERATION, "forge.liveSnapshot");
    }

    #[test]
    fn forbidden_is_a_business_error_and_runtime_faults_are_infrastructure() {
        let forbidden = dispatch_runtime_error(ServiceRuntimeError::Forbidden {
            reason: "no tech.access".into(),
            decision: services::AuthorizationDecision {
                allowed: false,
                reason: "no tech.access".into(),
                policy_id: "test".into(),
                mode: "test",
            },
        });
        assert_eq!(forbidden.code(), "FORBIDDEN");
        assert!(!forbidden.retryable());

        let audit = dispatch_runtime_error(ServiceRuntimeError::Audit("sink down".into()));
        assert_eq!(audit.code(), "AUDIT_UNAVAILABLE");
        assert!(audit.retryable());
    }
}

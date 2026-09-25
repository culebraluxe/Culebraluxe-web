use async_trait::async_trait;
use db::SecurityAuditDao;
use serde_json::json;
use service::{AuditPort, ServiceActorKind, ServiceAuditEvent, ServiceOutcome, ServicePortError};

#[derive(Clone)]
pub struct DurableSecurityAuditPort {
    dao: SecurityAuditDao,
}

impl DurableSecurityAuditPort {
    pub fn new(dao: SecurityAuditDao) -> Self {
        Self { dao }
    }
}

#[async_trait]
impl AuditPort for DurableSecurityAuditPort {
    async fn record(&self, event: ServiceAuditEvent) -> Result<(), ServicePortError> {
        let app_user_id = match event.actor.kind {
            ServiceActorKind::User => event.actor.id.as_deref(),
            ServiceActorKind::System | ServiceActorKind::Agent => None,
        };
        let actor_kind = match event.actor.kind {
            ServiceActorKind::User => "user",
            ServiceActorKind::System => "system",
            ServiceActorKind::Agent => "agent",
        };
        let outcome = match event.outcome {
            ServiceOutcome::Success => "success",
            ServiceOutcome::Failure => "failure",
        };
        let metadata = json!({
            "domain": event.domain,
            "operation": event.operation,
            "actor": {
                "id": event.actor.id,
                "kind": actor_kind,
            },
            "correlationId": event.correlation_id,
            "causationId": event.causation_id,
            "outcome": outcome,
            "errorCode": event.error_code,
            "authorization": {
                "allowed": event.authorization.allowed,
                "reason": event.authorization.reason,
                "policyId": event.authorization.policy_id,
                "mode": event.authorization.mode,
            },
            "source": "rust-service",
        });

        self.dao
            .record(app_user_id, event.operation, "rust-service", &metadata)
            .await
            .map_err(|error| ServicePortError::new(error.to_string()))
    }
}

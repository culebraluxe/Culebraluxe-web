use crate::{AuthorizationDecision, ServiceActor, ServicePortError};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAuditEvent {
    pub domain: &'static str,
    pub operation: &'static str,
    pub actor: ServiceActor,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub outcome: ServiceOutcome,
    pub error_code: Option<String>,
    pub authorization: AuthorizationDecision,
}

#[async_trait]
pub trait AuditPort: Send + Sync {
    async fn record(&self, event: ServiceAuditEvent) -> Result<(), ServicePortError>;
}

#[derive(Debug, Clone, Default)]
pub struct CapturingAuditPort {
    events: Arc<Mutex<Vec<ServiceAuditEvent>>>,
}

impl CapturingAuditPort {
    pub fn events(&self) -> Vec<ServiceAuditEvent> {
        self.events.lock().expect("audit capture poisoned").clone()
    }
}

#[async_trait]
impl AuditPort for CapturingAuditPort {
    async fn record(&self, event: ServiceAuditEvent) -> Result<(), ServicePortError> {
        self.events
            .lock()
            .map_err(|_| ServicePortError::new("audit capture lock poisoned"))?
            .push(event);
        Ok(())
    }
}

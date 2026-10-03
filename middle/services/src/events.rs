use crate::ServicePortError;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceDomainEvent {
    pub event_type: &'static str,
    pub aggregate_id: Option<String>,
    pub payload: BTreeMap<String, Value>,
    pub correlation_id: String,
    pub causation_id: Option<String>,
}

#[async_trait]
pub trait DomainEventPort: Send + Sync {
    async fn emit(&self, event: ServiceDomainEvent) -> Result<(), ServicePortError>;
}

#[derive(Debug, Clone, Default)]
pub struct CapturingDomainEventPort {
    events: Arc<Mutex<Vec<ServiceDomainEvent>>>,
}

impl CapturingDomainEventPort {
    pub fn events(&self) -> Vec<ServiceDomainEvent> {
        self.events.lock().expect("event capture poisoned").clone()
    }
}

#[async_trait]
impl DomainEventPort for CapturingDomainEventPort {
    async fn emit(&self, event: ServiceDomainEvent) -> Result<(), ServicePortError> {
        self.events
            .lock()
            .map_err(|_| ServicePortError::new("event capture lock poisoned"))?
            .push(event);
        Ok(())
    }
}

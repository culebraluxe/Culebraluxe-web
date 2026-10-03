use async_trait::async_trait;
use db::{Database, DomainEventOutboxDao, OutboxEventInput};
use services::{DomainEventPort, ServiceDomainEvent, ServicePortError};

/// Production events are only accepted inside the same mutation transaction as
/// their business changes. Missing transaction composition is an error, not a
/// successful in-memory publish or an independent post-commit insert.
pub struct TransactionalDomainEventPort {
    db: Database,
}
impl TransactionalDomainEventPort {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}
#[async_trait]
impl DomainEventPort for TransactionalDomainEventPort {
    async fn emit(&self, event: ServiceDomainEvent) -> Result<(), ServicePortError> {
        if !self.db.in_mutation() {
            return Err(ServicePortError::new(
                "Domain events require an active service mutation transaction.",
            ));
        }
        let mut tx = self
            .db
            .begin("service.event")
            .await
            .map_err(|e| ServicePortError::new(e.to_string()))?;
        let event = OutboxEventInput {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: event.event_type.into(),
            actor_app_user_id: None,
            aggregate_type: event
                .event_type
                .split('.')
                .next()
                .unwrap_or("service")
                .into(),
            aggregate_id: event.aggregate_id.unwrap_or_default(),
            correlation_id: Some(event.correlation_id),
            causation_id: event.causation_id,
            occurred_at: chrono::Utc::now().to_rfc3339(),
            payload: serde_json::to_value(event.payload)
                .map_err(|e| ServicePortError::new(e.to_string()))?,
        };
        DomainEventOutboxDao::new(self.db.clone())
            .append_tx(&mut tx, &[event])
            .await
            .map_err(|e| ServicePortError::new(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ServicePortError::new(e.to_string()))
    }
}

use crate::{
    AuditPort, AuthorizationDecision, AuthorizationPort, AuthorizationRequest, DomainEventPort,
    OperationKind, ServiceAlert, ServiceAlertPort, ServiceAuditEvent, ServiceContext,
    ServiceDispatchError, ServiceDomainEvent, ServiceEnvelope, ServiceErrorRecord,
    ServiceErrorSink, ServiceFailureClass, ServiceFailureSeverity, ServiceOutcome, ServiceRouter,
    TracingServiceAlertPort, TracingServiceErrorSink,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct ServicePortError {
    pub message: String,
}

impl ServicePortError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ServiceRuntimeError {
    #[error("authorization port failed: {0}")]
    Authorization(String),
    #[error("forbidden: {reason}")]
    Forbidden {
        reason: String,
        decision: AuthorizationDecision,
    },
    #[error("audit port failed: {0}")]
    Audit(String),
    #[error("domain event port failed: {0}")]
    Event(String),
    #[error("service router failed: {code}: {message}")]
    Router {
        code: String,
        message: String,
        retryable: bool,
    },
}

#[derive(Clone)]
pub struct ServiceInfrastructure {
    pub authorization: Arc<dyn AuthorizationPort>,
    pub audit: Arc<dyn AuditPort>,
    pub events: Arc<dyn DomainEventPort>,
    pub errors: Arc<dyn ServiceErrorSink>,
    pub alerts: Arc<dyn ServiceAlertPort>,
    pub router: Option<Arc<dyn ServiceRouter>>,
}

impl ServiceInfrastructure {
    pub fn new(
        authorization: Arc<dyn AuthorizationPort>,
        audit: Arc<dyn AuditPort>,
        events: Arc<dyn DomainEventPort>,
    ) -> Self {
        Self {
            authorization,
            audit,
            events,
            errors: Arc::new(TracingServiceErrorSink),
            alerts: Arc::new(TracingServiceAlertPort),
            router: None,
        }
    }

    pub fn with_error_sink(mut self, errors: Arc<dyn ServiceErrorSink>) -> Self {
        self.errors = errors;
        self
    }

    pub fn with_alert_port(mut self, alerts: Arc<dyn ServiceAlertPort>) -> Self {
        self.alerts = alerts;
        self
    }

    pub fn with_router(mut self, router: Arc<dyn ServiceRouter>) -> Self {
        self.router = Some(router);
        self
    }
}

#[derive(Clone)]
pub struct ServiceRuntime {
    infrastructure: ServiceInfrastructure,
}

impl ServiceRuntime {
    pub fn new(infrastructure: ServiceInfrastructure) -> Self {
        Self { infrastructure }
    }

    pub async fn authorize(
        &self,
        domain: &'static str,
        action: &'static str,
        operation: &'static str,
        kind: OperationKind,
        context: &ServiceContext,
    ) -> Result<AuthorizationDecision, ServiceRuntimeError> {
        let decision = self
            .infrastructure
            .authorization
            .authorize(AuthorizationRequest {
                domain,
                action,
                operation,
                kind,
                actor: context.actor.clone(),
                principal: context.principal.clone(),
            })
            .await
            .map_err(|error| ServiceRuntimeError::Authorization(error.message))?;

        if !decision.allowed {
            return Err(ServiceRuntimeError::Forbidden {
                reason: decision.reason.clone(),
                decision,
            });
        }

        Ok(decision)
    }

    pub async fn audit(
        &self,
        domain: &'static str,
        operation: &'static str,
        context: &ServiceContext,
        outcome: ServiceOutcome,
        error_code: Option<String>,
        authorization: AuthorizationDecision,
    ) -> Result<(), ServiceRuntimeError> {
        self.infrastructure
            .audit
            .record(ServiceAuditEvent {
                domain,
                operation,
                actor: context.actor.clone(),
                correlation_id: context.correlation_id.clone(),
                causation_id: context.causation_id.clone(),
                outcome,
                error_code,
                authorization,
            })
            .await
            .map_err(|error| ServiceRuntimeError::Audit(error.message))
    }

    pub async fn emit(
        &self,
        event_type: &'static str,
        aggregate_id: Option<String>,
        payload: BTreeMap<String, serde_json::Value>,
        context: &ServiceContext,
    ) -> Result<(), ServiceRuntimeError> {
        self.infrastructure
            .events
            .emit(ServiceDomainEvent {
                event_type,
                aggregate_id,
                payload,
                correlation_id: context.correlation_id.clone(),
                causation_id: context.causation_id.clone(),
            })
            .await
            .map_err(|error| ServiceRuntimeError::Event(error.message))
    }

    pub async fn observe_dispatch_failure(
        &self,
        domain: &str,
        operation: &str,
        context: &ServiceContext,
        error: &ServiceDispatchError,
    ) {
        let class = error.failure_class();
        if !matches!(class, ServiceFailureClass::Infrastructure | ServiceFailureClass::Panic) {
            return;
        }

        let severity = if class == ServiceFailureClass::Panic {
            ServiceFailureSeverity::Fatal
        } else {
            ServiceFailureSeverity::Error
        };
        let record = ServiceErrorRecord {
            domain: domain.to_owned(),
            operation: operation.to_owned(),
            code: error.code().to_owned(),
            message: error.to_string(),
            retryable: error.retryable(),
            stack: None,
            correlation_id: context.correlation_id.clone(),
            causation_id: context.causation_id.clone(),
            actor_id: context.actor.id.clone(),
            severity,
        };

        if let Err(sink_error) = self.infrastructure.errors.record(record.clone()).await {
            tracing::error!(
                target: "culebraluxe::service::observer",
                domain = %record.domain,
                operation = %record.operation,
                code = %record.code,
                correlation_id = %record.correlation_id,
                observer_error = %sink_error,
                "service error sink failed"
            );
        }

        let alert = ServiceAlert {
            domain: record.domain,
            operation: record.operation,
            code: record.code,
            message: record.message,
            correlation_id: record.correlation_id,
            severity,
        };
        if let Err(alert_error) = self.infrastructure.alerts.notify(alert).await {
            tracing::error!(
                target: "culebraluxe::service::observer",
                domain,
                operation,
                correlation_id = %context.correlation_id,
                observer_error = %alert_error,
                "service alert port failed"
            );
        }
    }

    pub async fn call_service(
        &self,
        domain: impl Into<String>,
        operation: impl Into<String>,
        payload: Value,
        context: &ServiceContext,
    ) -> Result<Value, ServiceRuntimeError> {
        let router =
            self.infrastructure
                .router
                .as_ref()
                .ok_or_else(|| ServiceRuntimeError::Router {
                    code: "SERVICE_ROUTER_UNAVAILABLE".into(),
                    message: "No ServiceRouter is configured for this service.".into(),
                    retryable: true,
                })?;

        let envelope = ServiceEnvelope {
            domain: domain.into(),
            operation: operation.into(),
            payload,
        };
        router
            .dispatch(&envelope, context)
            .await
            .map_err(|error| ServiceRuntimeError::Router {
                code: error.code().to_owned(),
                message: error.to_string(),
                retryable: error.retryable(),
            })
    }
}

use crate::ServicePortError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceFailureSeverity {
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceErrorRecord {
    pub domain: String,
    pub operation: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub stack: Option<String>,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub actor_id: Option<String>,
    pub severity: ServiceFailureSeverity,
}

#[async_trait]
pub trait ServiceErrorSink: Send + Sync {
    async fn record(&self, failure: ServiceErrorRecord) -> Result<(), ServicePortError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAlert {
    pub domain: String,
    pub operation: String,
    pub code: String,
    pub message: String,
    pub correlation_id: String,
    pub severity: ServiceFailureSeverity,
}

#[async_trait]
pub trait ServiceAlertPort: Send + Sync {
    async fn notify(&self, alert: ServiceAlert) -> Result<(), ServicePortError>;
}

#[derive(Debug, Clone, Default)]
pub struct CapturingServiceErrorSink {
    records: Arc<Mutex<Vec<ServiceErrorRecord>>>,
}

impl CapturingServiceErrorSink {
    pub fn records(&self) -> Vec<ServiceErrorRecord> {
        self.records
            .lock()
            .expect("service error capture poisoned")
            .clone()
    }
}

#[async_trait]
impl ServiceErrorSink for CapturingServiceErrorSink {
    async fn record(&self, failure: ServiceErrorRecord) -> Result<(), ServicePortError> {
        self.records
            .lock()
            .map_err(|_| ServicePortError::new("service error capture lock poisoned"))?
            .push(failure);
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapturingServiceAlertPort {
    alerts: Arc<Mutex<Vec<ServiceAlert>>>,
}

impl CapturingServiceAlertPort {
    pub fn alerts(&self) -> Vec<ServiceAlert> {
        self.alerts
            .lock()
            .expect("service alert capture poisoned")
            .clone()
    }
}

#[async_trait]
impl ServiceAlertPort for CapturingServiceAlertPort {
    async fn notify(&self, alert: ServiceAlert) -> Result<(), ServicePortError> {
        self.alerts
            .lock()
            .map_err(|_| ServicePortError::new("service alert capture lock poisoned"))?
            .push(alert);
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TracingServiceErrorSink;

#[async_trait]
impl ServiceErrorSink for TracingServiceErrorSink {
    async fn record(&self, failure: ServiceErrorRecord) -> Result<(), ServicePortError> {
        match failure.severity {
            ServiceFailureSeverity::Warning => tracing::warn!(
                target: "culebraluxe::service",
                domain = %failure.domain,
                operation = %failure.operation,
                code = %failure.code,
                retryable = failure.retryable,
                correlation_id = %failure.correlation_id,
                causation_id = ?failure.causation_id,
                actor_id = ?failure.actor_id,
                message = %failure.message,
                "service failure"
            ),
            ServiceFailureSeverity::Error => tracing::error!(
                target: "culebraluxe::service",
                domain = %failure.domain,
                operation = %failure.operation,
                code = %failure.code,
                retryable = failure.retryable,
                correlation_id = %failure.correlation_id,
                causation_id = ?failure.causation_id,
                actor_id = ?failure.actor_id,
                message = %failure.message,
                "service failure"
            ),
            ServiceFailureSeverity::Fatal => tracing::error!(
                target: "culebraluxe::service",
                domain = %failure.domain,
                operation = %failure.operation,
                code = %failure.code,
                retryable = failure.retryable,
                correlation_id = %failure.correlation_id,
                causation_id = ?failure.causation_id,
                actor_id = ?failure.actor_id,
                message = %failure.message,
                "fatal service failure"
            ),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TracingServiceAlertPort;

#[async_trait]
impl ServiceAlertPort for TracingServiceAlertPort {
    async fn notify(&self, alert: ServiceAlert) -> Result<(), ServicePortError> {
        match alert.severity {
            ServiceFailureSeverity::Warning => tracing::warn!(
                target: "culebraluxe::service::alert",
                domain = %alert.domain,
                operation = %alert.operation,
                code = %alert.code,
                correlation_id = %alert.correlation_id,
                message = %alert.message,
                "service alert"
            ),
            ServiceFailureSeverity::Error | ServiceFailureSeverity::Fatal => tracing::error!(
                target: "culebraluxe::service::alert",
                domain = %alert.domain,
                operation = %alert.operation,
                code = %alert.code,
                correlation_id = %alert.correlation_id,
                message = %alert.message,
                "service alert"
            ),
        }
        Ok(())
    }
}

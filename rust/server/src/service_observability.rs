use async_trait::async_trait;
use db::AppErrorDao;
use service::{
    ServiceAlert, ServiceAlertPort, ServiceErrorRecord, ServiceErrorSink, ServiceFailureSeverity,
    ServicePortError,
};

#[derive(Clone)]
pub struct DurableServiceErrorSink {
    dao: AppErrorDao,
}

impl DurableServiceErrorSink {
    pub fn new(dao: AppErrorDao) -> Self {
        Self { dao }
    }
}

#[async_trait]
impl ServiceErrorSink for DurableServiceErrorSink {
    async fn record(&self, failure: ServiceErrorRecord) -> Result<(), ServicePortError> {
        let level = match failure.severity {
            ServiceFailureSeverity::Warning => "warn",
            ServiceFailureSeverity::Error => "error",
            ServiceFailureSeverity::Fatal => "fatal",
        };
        let operation = format!("service:{}:{}", failure.domain, failure.operation);
        let kind = format!("service:{}", failure.code);
        let meta = serde_json::json!({
            "source": "rust-service-runtime",
            "domain": failure.domain,
            "operation": failure.operation,
            "code": failure.code,
            "retryable": failure.retryable,
            "correlationId": failure.correlation_id,
            "causationId": failure.causation_id,
            "actorId": failure.actor_id,
            "severity": level,
        })
        .to_string();

        self.dao
            .record_runtime_error(
                &kind,
                &operation,
                &failure.message,
                level,
                failure.stack.as_deref(),
                &meta,
            )
            .await
            .map_err(|_| ServicePortError::new("durable service error sink write failed"))
    }
}

#[derive(Clone)]
pub struct DurableServiceAlertPort {
    dao: AppErrorDao,
}

impl DurableServiceAlertPort {
    pub fn new(dao: AppErrorDao) -> Self {
        Self { dao }
    }
}

#[async_trait]
impl ServiceAlertPort for DurableServiceAlertPort {
    async fn notify(&self, alert: ServiceAlert) -> Result<(), ServicePortError> {
        let level = match alert.severity {
            ServiceFailureSeverity::Warning => "warn",
            ServiceFailureSeverity::Error => "error",
            ServiceFailureSeverity::Fatal => "fatal",
        };
        let operation = format!("service:{}:{}", alert.domain, alert.operation);
        let kind = format!("service-alert:{}", alert.code);
        let meta = serde_json::json!({
            "source": "rust-service-runtime",
            "domain": alert.domain,
            "operation": alert.operation,
            "code": alert.code,
            "correlationId": alert.correlation_id,
            "severity": level,
        })
        .to_string();

        self.dao
            .record_runtime_error(&kind, &operation, &alert.message, level, None, &meta)
            .await
            .map_err(|_| ServicePortError::new("durable service alert write failed"))
    }
}

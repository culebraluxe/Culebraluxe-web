use crate::security::CasbinAuthorizationPort;
use crate::service_events::TransactionalDomainEventPort;
use crate::service_observability::{DurableServiceAlertPort, DurableServiceErrorSink};
use db::{AppErrorDao, Database};
use services::{NoopAuditPort, ServiceInfrastructure};
use std::sync::Arc;

pub async fn production_service_infrastructure(
    db: &Database,
) -> Result<ServiceInfrastructure, casbin::Error> {
    let app_error = AppErrorDao::new(db.clone());
    Ok(ServiceInfrastructure::new(
        Arc::new(CasbinAuthorizationPort::new().await?),
        Arc::new(NoopAuditPort),
        Arc::new(TransactionalDomainEventPort::new(db.clone())),
    )
    .with_error_sink(Arc::new(DurableServiceErrorSink::new(app_error.clone())))
    .with_alert_port(Arc::new(DurableServiceAlertPort::new(app_error))))
}

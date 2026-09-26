use crate::{ServiceContext, ServiceDispatchError, ServiceEnvelope};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, OnceLock, Weak};

#[async_trait]
pub trait ServiceRouter: Send + Sync {
    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError>;
}

#[derive(Clone, Default)]
pub struct DeferredServiceRouter {
    target: Arc<OnceLock<Weak<dyn ServiceRouter>>>,
}

impl DeferredServiceRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install(
        &self,
        target: &Arc<dyn ServiceRouter>,
    ) -> Result<(), ServiceDispatchError> {
        self.target
            .set(Arc::downgrade(target))
            .map_err(|_| {
                ServiceDispatchError::operation(
                    "SERVICE_ROUTER_ALREADY_INSTALLED",
                    "The service router can only be installed once.",
                    false,
                )
            })
    }
}

#[async_trait]
impl ServiceRouter for DeferredServiceRouter {
    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        let target = self
            .target
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| {
                ServiceDispatchError::operation(
                    "SERVICE_ROUTER_UNAVAILABLE",
                    "The service registry is not available.",
                    true,
                )
            })?;
        target.dispatch(envelope, context).await
    }
}

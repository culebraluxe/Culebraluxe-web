use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Starting,
    #[default]
    Running,
    Draining,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {
    pub status: ServiceStatus,
    pub accepting: bool,
    pub queued: usize,
    pub in_flight: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceControlCommand {
    Start,
    Status,
    Health,
    Drain,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceControlResult {
    pub domain: String,
    pub command: ServiceControlCommand,
    pub status: ServiceStatus,
    pub health: ServiceHealth,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ServiceLifecycleError {
    pub code: &'static str,
    pub message: String,
}

impl ServiceLifecycleError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[async_trait]
pub trait ServiceLifecycle: Send + Sync {
    async fn start(&self) -> Result<(), ServiceLifecycleError>;
    async fn drain(&self) -> Result<(), ServiceLifecycleError>;
    async fn stop(&self) -> Result<(), ServiceLifecycleError>;
    fn status(&self) -> ServiceStatus;
    fn health(&self) -> ServiceHealth;
}

//! Blocking access to the app's one shared database pool for the synchronous workflow kernel.
//!
//! This is the re-host equivalent of Forge's `vendor_session::with_shared`, without the Forge
//! vendor-session repository: the store is synchronous and `block_on`s on its own runtime, so the
//! caller's thread must not already be driving a runtime.
//!
//! A pool-connect failure is a **database-unavailable** failure, not a generic engine error. The
//! distinction is preserved out of this module so the HTTP route can return the database 503 with an
//! incident and the app's `app_error` capture, instead of `ENGINE_COMMAND_FAILED` 500.

use std::sync::OnceLock;

use db::Database;
use tokio::runtime::Runtime;
use workflow_core::WorkflowError;

struct SharedDb {
    db: Database,
    rt: Runtime,
}

#[derive(Debug, Clone)]
pub enum SharedInitFailure {
    Runtime(String),
    Connect(String),
}

impl SharedInitFailure {
    pub fn into_workflow_error(self) -> WorkflowError {
        match self {
            Self::Runtime(message) => WorkflowError::generic(message),
            Self::Connect(message) => WorkflowError::unavailable(message),
        }
    }
}

impl std::fmt::Display for SharedInitFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime(message) | Self::Connect(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SharedInitFailure {}

impl From<SharedInitFailure> for WorkflowError {
    fn from(error: SharedInitFailure) -> Self {
        error.into_workflow_error()
    }
}

impl From<SharedInitFailure> for String {
    fn from(error: SharedInitFailure) -> Self {
        error.to_string()
    }
}

static SHARED: OnceLock<Result<SharedDb, SharedInitFailure>> = OnceLock::new();

fn shared() -> Result<&'static SharedDb, SharedInitFailure> {
    SHARED
        .get_or_init(|| {
            let rt = Runtime::new()
                .map_err(|error| SharedInitFailure::Runtime(format!("tokio: {error}")))?;
            let db = rt
                .block_on(async {
                    match db::shared::get() {
                        Some(db) => Ok(db),
                        None => db::shared::get_or_connect().await,
                    }
                })
                .map_err(|error| SharedInitFailure::Connect(error.to_string()))?;
            Ok(SharedDb { db, rt })
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn with_shared<R>(f: impl FnOnce(&Database, &Runtime) -> R) -> Result<R, SharedInitFailure> {
    let shared = shared()?;
    Ok(f(&shared.db, &shared.rt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pool_connect_failure_is_a_connection_failure() {
        let error = SharedInitFailure::Connect("connection refused".into()).into_workflow_error();
        assert!(
            error.is_connection_failure(),
            "a pool connect failure must take the DB-unavailable path, not generic 500"
        );
        assert_eq!(error.code(), "DB_UNAVAILABLE");
    }

    #[test]
    fn a_runtime_construction_failure_is_generic() {
        let error = SharedInitFailure::Runtime("no runtime".into()).into_workflow_error();
        assert!(!error.is_connection_failure());
        assert_eq!(error.code(), "ERROR");
    }
}

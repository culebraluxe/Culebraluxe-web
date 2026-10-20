//! Error translation for the app-side workflow host.
//!
//! The engine kernel speaks `WorkflowError`. The HTTP layer needs to distinguish a database
//! connection failure (503 + incident + `app_error` capture) from an ordinary engine refusal
//! (500 `ENGINE_COMMAND_FAILED`). This type carries that distinction across the engine worker
//! thread without importing the workflow crate into `web`.

use std::fmt;

use db::DbFailure;
use workflow_core::WorkflowError;

#[derive(Debug)]
pub struct EngineFailure {
    message: String,
    db_failure: Option<DbFailure>,
}

impl EngineFailure {
    /// A database-unavailable engine failure: classify, capture through the app sink, and carry the
    /// incident id to the HTTP boundary.
    pub fn database_unavailable(message: impl Into<String>) -> Self {
        let detail = message.into();
        let db_failure = DbFailure::unavailable("workflow.engine", detail.clone());
        let message = db_failure.to_string();
        Self {
            message,
            db_failure: Some(db_failure),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_db_failure(self) -> Option<DbFailure> {
        self.db_failure
    }
}

impl From<WorkflowError> for EngineFailure {
    fn from(error: WorkflowError) -> Self {
        if error.is_connection_failure() {
            Self::database_unavailable(error.to_string())
        } else {
            let message = error.to_string();
            Self {
                message,
                db_failure: None,
            }
        }
    }
}

impl From<String> for EngineFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            db_failure: None,
        }
    }
}

impl From<&str> for EngineFailure {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_owned(),
            db_failure: None,
        }
    }
}

impl fmt::Display for EngineFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EngineFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_connection_failure_becomes_a_database_failure_with_an_incident() {
        let failure: EngineFailure =
            WorkflowError::unavailable("connection reset while stepping").into();
        let db = failure
            .into_db_failure()
            .expect("classified as a database failure");
        assert_eq!(db.kind, db::DbFailureKind::DatabaseUnavailable);
        assert!(db.retryable);
        assert!(!db.incident_id.is_nil());
    }

    #[test]
    fn an_ordinary_refusal_stays_an_engine_failure() {
        let failure: EngineFailure = WorkflowError::generic("no workflow task correlates").into();
        assert_eq!(failure.message(), "no workflow task correlates");
        assert!(failure.into_db_failure().is_none());
    }

    #[test]
    fn other_workflow_errors_also_stay_engine_failures() {
        let conflict: EngineFailure =
            WorkflowError::conflict("PROCESS_NOT_ACTIVE", "already finished").into();
        assert_eq!(conflict.message(), "already finished");
        assert!(conflict.into_db_failure().is_none());

        let missing: EngineFailure = WorkflowError::NotFound("task".into()).into();
        assert_eq!(missing.message(), "task");
        assert!(missing.into_db_failure().is_none());
    }
}

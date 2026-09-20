use db::DbFailure;
use service::{
    AuthorizationDecision, OperationKind, ServiceContext, ServiceOutcome, ServiceRuntime,
    ServiceRuntimeError,
};

#[derive(Debug, thiserror::Error)]
pub enum CoreServiceError {
    #[error("{code}: {message}")]
    Business { code: &'static str, message: String },
    #[error(transparent)]
    Database(#[from] DbFailure),
    #[error(transparent)]
    Runtime(#[from] ServiceRuntimeError),
}

impl CoreServiceError {
    pub fn business(code: &'static str, message: impl Into<String>) -> Self {
        Self::Business {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Business { code, .. } => code,
            Self::Database(_) => "DATABASE",
            Self::Runtime(ServiceRuntimeError::Authorization(_)) => "AUTHORIZATION_UNAVAILABLE",
            Self::Runtime(ServiceRuntimeError::Forbidden { .. }) => "FORBIDDEN",
            Self::Runtime(ServiceRuntimeError::Audit(_)) => "AUDIT_UNAVAILABLE",
            Self::Runtime(ServiceRuntimeError::Event(_)) => "DOMAIN_EVENT_UNAVAILABLE",
        }
    }
}

pub async fn authorize(
    runtime: &ServiceRuntime,
    domain: &'static str,
    action: &'static str,
    operation: &'static str,
    kind: OperationKind,
    context: &ServiceContext,
) -> Result<AuthorizationDecision, CoreServiceError> {
    match runtime
        .authorize(domain, action, operation, kind, context)
        .await
    {
        Ok(decision) => Ok(decision),
        Err(ServiceRuntimeError::Forbidden { reason, decision }) => {
            runtime
                .audit(
                    domain,
                    operation,
                    context,
                    ServiceOutcome::Failure,
                    Some("FORBIDDEN".into()),
                    decision.clone(),
                )
                .await?;
            Err(ServiceRuntimeError::Forbidden { reason, decision }.into())
        }
        Err(error) => Err(error.into()),
    }
}

pub async fn audit_result<T>(
    runtime: &ServiceRuntime,
    domain: &'static str,
    operation: &'static str,
    context: &ServiceContext,
    decision: AuthorizationDecision,
    result: &Result<T, CoreServiceError>,
) -> Result<(), CoreServiceError> {
    let (outcome, error_code) = match result {
        Ok(_) => (ServiceOutcome::Success, None),
        Err(error) => (ServiceOutcome::Failure, Some(error.code().to_owned())),
    };
    runtime
        .audit(domain, operation, context, outcome, error_code, decision)
        .await?;
    Ok(())
}

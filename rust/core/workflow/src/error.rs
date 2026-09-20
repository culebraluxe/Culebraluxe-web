use std::fmt;

#[derive(Debug)]
pub enum WorkflowError {
    Generic { message: String },
    Conflict { code: &'static str, message: String },
    StaleToken { message: String },
    MissingApplicationPort { message: String },
    Expression(String),
    NotFound(String),
}

impl WorkflowError {
    pub fn generic(msg: impl Into<String>) -> Self {
        Self::Generic {
            message: msg.into(),
        }
    }
    pub fn conflict(code: &'static str, msg: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: msg.into(),
        }
    }
    pub fn stale_token(msg: impl Into<String>) -> Self {
        Self::StaleToken {
            message: msg.into(),
        }
    }
    pub fn missing_port(msg: impl Into<String>) -> Self {
        Self::MissingApplicationPort {
            message: msg.into(),
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Generic { .. } => "ERROR",
            Self::Conflict { code, .. } => code,
            Self::StaleToken { .. } => "STALE_TOKEN",
            Self::MissingApplicationPort { .. } => "MISSING_APPLICATION_PORT",
            Self::Expression(_) => "EXPRESSION",
            Self::NotFound(_) => "NOT_FOUND",
        }
    }
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generic { message }
            | Self::Conflict { message, .. }
            | Self::StaleToken { message }
            | Self::MissingApplicationPort { message } => write!(f, "{message}"),
            Self::Expression(m) | Self::NotFound(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for WorkflowError {}

pub type Result<T> = std::result::Result<T, WorkflowError>;

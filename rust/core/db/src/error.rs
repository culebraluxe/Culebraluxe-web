use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbFailureKind {
    DatabaseUnavailable,
    SchemaMismatch,
    Constraint,
    Timeout,
    Unknown,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind:?} during {operation} (incident {incident_id})")]
pub struct DbFailure {
    pub kind: DbFailureKind,
    pub operation: &'static str,
    pub incident_id: Uuid,
    pub code: Option<String>,
    pub detail: Option<String>,
    pub retryable: bool,
}

pub type DbResult<T> = Result<T, DbFailure>;

impl DbFailure {
    pub fn configuration(operation: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind: DbFailureKind::DatabaseUnavailable,
            operation,
            incident_id: Uuid::new_v4(),
            code: None,
            detail: Some(detail.into()),
            retryable: false,
        }
    }

    pub fn schema_mismatch(operation: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind: DbFailureKind::SchemaMismatch,
            operation,
            incident_id: Uuid::new_v4(),
            code: None,
            detail: Some(detail.into()),
            retryable: false,
        }
    }

    pub fn from_sqlx(operation: &'static str, error: &sqlx::Error) -> Self {
        let code = error
            .as_database_error()
            .and_then(|database_error| database_error.code())
            .map(|value| value.to_string());

        if let Some(code) = code {
            let kind = classify_sqlstate(&code);
            return Self {
                retryable: matches!(
                    kind,
                    DbFailureKind::DatabaseUnavailable | DbFailureKind::Timeout
                ),
                detail: match code.as_str() {
                    "42703" | "42P01" => Some("schema mismatch (column/table missing)".into()),
                    _ => None,
                },
                kind,
                operation,
                incident_id: Uuid::new_v4(),
                code: Some(code),
            };
        }

        let message = error.to_string().to_lowercase();
        let kind = if message.contains("timed out") || message.contains("timeout") {
            DbFailureKind::Timeout
        } else if message.contains("connection")
            || message.contains("pool closed")
            || message.contains("broken pipe")
            || message.contains("dns")
            || message.contains("tls")
            || message.contains("socket")
        {
            DbFailureKind::DatabaseUnavailable
        } else {
            DbFailureKind::Unknown
        };

        Self {
            retryable: matches!(
                kind,
                DbFailureKind::DatabaseUnavailable | DbFailureKind::Timeout
            ),
            kind,
            operation,
            incident_id: Uuid::new_v4(),
            code: None,
            detail: None,
        }
    }
}

fn classify_sqlstate(code: &str) -> DbFailureKind {
    if code == "42703" || code == "42P01" {
        return DbFailureKind::SchemaMismatch;
    }
    if code == "57014" || code == "55P03" {
        return DbFailureKind::Timeout;
    }
    if code.starts_with("08") || matches!(code, "57P01" | "57P03" | "53300") {
        return DbFailureKind::DatabaseUnavailable;
    }
    if code.starts_with("23") {
        return DbFailureKind::Constraint;
    }
    DbFailureKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlstate_classification_matches_typescript_gateway() {
        assert_eq!(classify_sqlstate("42703"), DbFailureKind::SchemaMismatch);
        assert_eq!(classify_sqlstate("42P01"), DbFailureKind::SchemaMismatch);
        assert_eq!(classify_sqlstate("57014"), DbFailureKind::Timeout);
        assert_eq!(classify_sqlstate("08006"), DbFailureKind::DatabaseUnavailable);
        assert_eq!(classify_sqlstate("23505"), DbFailureKind::Constraint);
        assert_eq!(classify_sqlstate("XX000"), DbFailureKind::Unknown);
    }
}

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbFailureKind {
    DatabaseUnavailable,
    SchemaMismatch,
    Constraint,
    Timeout,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DbFailure {
    pub kind: DbFailureKind,
    pub operation: &'static str,
    pub incident_id: Uuid,
    pub code: Option<String>,
    /// The driver's own message, verbatim, plus the constraint name when the failure was a constraint.
    /// This is the only text that says WHAT failed ("column p.foo does not exist"), so it is carried and
    /// printed rather than dropped — an operator-facing tool that answers "Unknown during db.run_text"
    /// has told them nothing. See `docs/agent/HANDOFF-contacts-port-2026-09-28.md`.
    pub detail: Option<String>,
    pub retryable: bool,
}

/// `Display` is written by hand, not by `#[error(...)]`, so the detail can be appended when it exists.
impl std::fmt::Display for DbFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{:?} during {} (incident {}",
            self.kind, self.operation, self.incident_id
        )?;
        if let Some(code) = &self.code {
            write!(formatter, ", sqlstate {code}")?;
        }
        write!(formatter, ")")?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DbFailure {}

pub type DbResult<T> = Result<T, DbFailure>;

impl DbFailure {
    /// Every failure is announced from here, in the constructors, because they are the single place a `DbFailure` can
    /// come into existence. The alternative - notifying at each `map_err` in the pool - means the one call site
    /// somebody forgets is the one you needed. Constructors with a side effect is the trade: the effect is best effort,
    /// idempotent, and the only thing it can do is tell the process's sink.
    fn announced(self) -> Self {
        crate::capture::notify(&self);
        self
    }

    pub fn configuration(operation: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind: DbFailureKind::DatabaseUnavailable,
            operation,
            incident_id: Uuid::new_v4(),
            code: None,
            detail: Some(detail.into()),
            retryable: false,
        }
        .announced()
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
        .announced()
    }

    pub fn from_sqlx(operation: &'static str, error: &sqlx::Error) -> Self {
        if let Some(database_error) = error.as_database_error() {
            let code = database_error.code().map(|value| value.to_string());
            let kind = code
                .as_deref()
                .map(classify_sqlstate)
                .unwrap_or(DbFailureKind::Unknown);
            // The driver's message is kept for EVERY sqlstate, not only the schema ones: it is the
            // difference between "Unknown during db.run_text" and "relation l_person does not exist".
            let mut detail = database_error.message().to_string();
            if let Some(constraint) = database_error.constraint() {
                detail = format!("{detail} (constraint {constraint})");
            }
            return Self {
                retryable: matches!(
                    kind,
                    DbFailureKind::DatabaseUnavailable | DbFailureKind::Timeout
                ),
                detail: Some(detail),
                kind,
                operation,
                incident_id: Uuid::new_v4(),
                code,
            }
            .announced();
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
            detail: Some(error.to_string()),
        }
        .announced()
    }
}

fn classify_sqlstate(code: &str) -> DbFailureKind {
    if code == "42703" || code == "42P01" {
        return DbFailureKind::SchemaMismatch;
    }
    if code == "57014" || code == "55P03" {
        return DbFailureKind::Timeout;
    }
    // `25P03` IS A CONNECTION FAILURE, NOT A STATEMENT FAILURE (2026-09-29).
    //
    // `idle_in_transaction_session_timeout` does not cancel a statement - it TERMINATES THE SESSION. The server is
    // saying "this connection is gone", and the client is told so at whatever statement it happens to send next,
    // which for the engine is the `BEGIN` of the following workflow step. Classified as `Unknown` this was a dead
    // end: `Unknown` is not `retryable`, so nothing retried it and an eight-minute story was lost 2m45s in at a step
    // boundary, on a session that had nothing to do with the work. The same is true of `57P02` (crash shutdown) and
    // of `53300` (no free connections): the work is fine, the session is not.
    if code.starts_with("08")
        || matches!(code, "25P03" | "57P01" | "57P02" | "57P03" | "53300")
    {
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
        assert_eq!(
            classify_sqlstate("08006"),
            DbFailureKind::DatabaseUnavailable
        );
        assert_eq!(classify_sqlstate("23505"), DbFailureKind::Constraint);
        assert_eq!(classify_sqlstate("XX000"), DbFailureKind::Unknown);
    }

    /// The failure that killed a real engine run at a step boundary (2026-09-29) must be retryable, because the
    /// work was never in question — the session was. `25P03` reaching `Unknown` is what made it terminal.
    #[test]
    fn a_session_the_server_terminated_is_retryable() {
        assert_eq!(
            classify_sqlstate("25P03"),
            DbFailureKind::DatabaseUnavailable
        );
        assert_eq!(
            classify_sqlstate("57P02"),
            DbFailureKind::DatabaseUnavailable
        );
        // And the decision that follows from the kind: `retry` and `Database::begin` both read this flag.
        assert!(matches!(
            classify_sqlstate("25P03"),
            DbFailureKind::DatabaseUnavailable
        ));
        // A real statement-level fault stays where it was: 57014 is a cancelled statement, not a lost session.
        assert_eq!(classify_sqlstate("57014"), DbFailureKind::Timeout);
    }

    /// A failure that prints only its kind is a failure an operator cannot act on: the migration tool
    /// used to answer "Unknown during db.run_text (incident …)" while Postgres had said exactly what was
    /// wrong. The detail and the sqlstate must reach the text.
    #[test]
    fn the_driver_message_survives_into_the_error_text() {
        let failure = DbFailure {
            kind: DbFailureKind::Unknown,
            operation: "db.run_text",
            incident_id: Uuid::new_v4(),
            code: Some("42601".into()),
            detail: Some("syntax error at or near \"creat\"".into()),
            retryable: false,
        };
        let text = failure.to_string();
        assert!(text.contains("syntax error at or near"), "{text}");
        assert!(text.contains("sqlstate 42601"), "{text}");
        assert!(text.contains("db.run_text"), "{text}");
    }
}

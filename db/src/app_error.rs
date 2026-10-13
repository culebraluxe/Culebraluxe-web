use crate::{Database, DbFailure};

#[derive(Clone)]
pub struct AppErrorDao {
    db: Database,
}

/// One browser-edge report as the columns `record_application_event` binds them.
///
/// Seven positional arguments were seven chances to transpose two free-text `&str`s that sit next to each other in
/// the bind order, and the two the table's own comment calls load-bearing are the pair a reader has to tell apart:
/// the page that failed (`route`) and the taxonomy that has no column of its own (`meta`). Named fields make the call
/// site say which is which. Borrowed, like the other pass-through inputs here: nothing outlives the statement that
/// binds it.
#[derive(Debug, Clone)]
pub struct NewApplicationEvent<'a> {
    pub kind: &'a str,
    pub operation: &'a str,
    pub message: &'a str,
    pub route: &'a str,
    pub level: &'a str,
    pub code: Option<&'a str>,
    pub meta: &'a str,
}

impl AppErrorDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Best-effort sink: deliberately does not construct a DbFailure if this insert fails.
    pub async fn record_db_failure(
        &self,
        failure: &DbFailure,
        kind: &str,
        meta: &str,
    ) -> Result<(), ()> {
        sqlx::query(
            "insert into app_error (kind, operation, incident_id, code, message, retryable, route, level, meta)
             values ($1, $2, $3::uuid, $4, $5, $6, 'rust/db', 'error', $7::jsonb)",
        )
        .bind(kind)
        .bind(failure.operation)
        .bind(failure.incident_id.to_string())
        .bind(failure.code.as_deref())
        .bind(failure.detail.as_deref().unwrap_or("database failure"))
        .bind(failure.retryable)
        .bind(meta)
        .execute(self.db.pool())
        .await
        .map(|_| ())
        .map_err(|_| ())
    }

    /// Best-effort diagnostic event from a trusted application edge. Unlike
    /// record_runtime_error, the route/code are supplied by the caller because
    /// browser boundary reports need to preserve the page that failed.
    pub async fn record_application_event(
        &self,
        event: &NewApplicationEvent<'_>,
    ) -> Result<(), ()> {
        sqlx::query(
            "insert into app_error (kind, operation, code, message, route, level, retryable, meta)
             values ($1, $2, $3, $4, $5, $6, false, $7::jsonb)",
        )
        .bind(event.kind)
        .bind(event.operation)
        .bind(event.code)
        .bind(event.message)
        .bind(event.route)
        .bind(event.level)
        .bind(event.meta)
        .execute(self.db.pool())
        .await
        .map(|_| ())
        .map_err(|_| ())
    }

    /// Best-effort sink: deliberately does not construct a DbFailure if this insert fails.
    pub async fn record_runtime_error(
        &self,
        kind: &str,
        operation: &str,
        message: &str,
        level: &str,
        stack: Option<&str>,
        meta: &str,
    ) -> Result<(), ()> {
        sqlx::query(
            "insert into app_error (kind, operation, message, stack, route, level, retryable, meta)
             values ($1, $2, $3, $4, 'rust', $5, false, $6::jsonb)",
        )
        .bind(kind)
        .bind(operation)
        .bind(message)
        .bind(stack)
        .bind(level)
        .bind(meta)
        .execute(self.db.pool())
        .await
        .map(|_| ())
        .map_err(|_| ())
    }
}

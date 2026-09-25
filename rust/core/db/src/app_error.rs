use crate::{Database, DbFailure};

#[derive(Clone)]
pub struct AppErrorDao {
    db: Database,
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

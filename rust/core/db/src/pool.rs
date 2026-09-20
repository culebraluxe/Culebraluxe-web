use crate::error::{DbFailure, DbResult};
use crate::transaction::DbTransaction;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use futures_util::TryStreamExt;
use std::env;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbTarget {
    Dev,
    Prod,
}

impl DbTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Prod => "prod",
        }
    }

    const fn env_name(self) -> &'static str {
        match self {
            Self::Dev => "DATABASE_URL_DEV",
            Self::Prod => "DATABASE_URL_PROD",
        }
    }
}

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
    target: DbTarget,
}

impl Database {
    pub async fn connect_from_env() -> DbResult<Self> {
        let target = resolve_declared_target(
            env::var("VERCEL_ENV").ok().as_deref(),
            env::var("APP_ENV").ok().as_deref(),
        )?;
        Self::connect_target(target).await
    }

    pub async fn connect_target(target: DbTarget) -> DbResult<Self> {
        let url = env::var(target.env_name()).map_err(|_| {
            DbFailure::configuration(
                "db.connect",
                format!(
                    "{} is not configured; Rust DB refuses to fall back to another environment",
                    target.env_name()
                ),
            )
        })?;

        let normalized = normalize_ssl_mode(&url);
        let options = PgConnectOptions::from_str(&normalized)
            .map_err(|_| DbFailure::configuration("db.connect", "invalid database connection URL"))?
            .application_name(&format!("culebraluxe-rust-{}", target.as_str()));

        let max_connections = positive_u32("FORGE_DB_POOL_MAX", 5);
        let idle_ms = positive_u64("FORGE_DB_POOL_IDLE_MS", 10_000);
        let connect_ms = positive_u64("FORGE_DB_POOL_CONNECT_MS", 10_000);

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .idle_timeout(Some(Duration::from_millis(idle_ms)))
            .acquire_timeout(Duration::from_millis(connect_ms))
            .connect_with(options)
            .await
            .map_err(|error| DbFailure::from_sqlx("db.connect", &error))?;

        Ok(Self { pool, target })
    }

    pub const fn target(&self) -> DbTarget {
        self.target
    }

    pub async fn ping(&self) -> DbResult<()> {
        let _: i32 = sqlx::query_scalar("select 1::int")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| DbFailure::from_sqlx("db.ping", &error))?;
        Ok(())
    }

    pub async fn begin(&self, operation: &'static str) -> DbResult<DbTransaction> {
        let transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| DbFailure::from_sqlx(operation, &error))?;
        Ok(DbTransaction::new(transaction, operation))
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Ad-hoc SQL on the shared pool. Used by Forge to retire the `psql` CLI client.
    pub async fn run_text(&self, sql: &str) -> DbResult<String> {
        use sqlx::Either;
        use sqlx::Row;
        let mut out = Vec::new();
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(|error| DbFailure::from_sqlx("db.run_text.acquire", &error))?;
        let mut stream = sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_many(&mut *connection);
        while let Some(item) = stream.try_next().await.map_err(|error| DbFailure::from_sqlx("db.run_text", &error))? {
            let Either::Right(row) = item else { continue };
            let mut cols = Vec::new();
            for i in 0..row.len() {
                cols.push(cell_as_text(&row, i));
            }
            out.push(cols.join("|"));
        }
        Ok(out.join("\n"))
    }
}

fn cell_as_text(row: &sqlx::postgres::PgRow, i: usize) -> String {
    use sqlx::Row;
    if let Ok(v) = row.try_get::<Option<String>, _>(i) {
        return v.unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
        return v.map(|n| n.to_string()).unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(i) {
        return v.map(|n| n.to_string()).unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
        return match v {
            Some(true) => "t".into(),
            Some(false) => "f".into(),
            None => String::new(),
        };
    }
    String::new()
}

pub fn resolve_declared_target(
    vercel_env: Option<&str>,
    app_env: Option<&str>,
) -> DbResult<DbTarget> {
    match vercel_env
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .as_str()
    {
        "production" => return Ok(DbTarget::Prod),
        "preview" | "development" => return Ok(DbTarget::Dev),
        _ => {}
    }

    match app_env.unwrap_or_default().trim().to_lowercase().as_str() {
        "production" | "prod" => Ok(DbTarget::Prod),
        "development" | "dev" | "test" | "testing" => Ok(DbTarget::Dev),
        _ => Err(DbFailure::configuration(
            "db.resolve_target",
            "database target is undeclared; set APP_ENV or use VERCEL_ENV",
        )),
    }
}

fn normalize_ssl_mode(url: &str) -> String {
    url.replace("sslmode=prefer", "sslmode=verify-full")
        .replace("sslmode=require", "sslmode=verify-full")
        .replace("sslmode=verify-ca", "sslmode=verify-full")
}

fn positive_u32(name: &str, fallback: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn positive_u64(name: &str, fallback: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vercel_environment_wins() {
        assert_eq!(
            resolve_declared_target(Some("production"), Some("dev")).unwrap(),
            DbTarget::Prod
        );
        assert_eq!(
            resolve_declared_target(Some("preview"), Some("prod")).unwrap(),
            DbTarget::Dev
        );
    }

    #[test]
    fn silence_is_refused() {
        assert!(resolve_declared_target(None, None).is_err());
    }

    #[test]
    fn ssl_modes_are_pinned_to_verify_full() {
        assert_eq!(
            normalize_ssl_mode("postgres://x/db?sslmode=require"),
            "postgres://x/db?sslmode=verify-full"
        );
    }
}

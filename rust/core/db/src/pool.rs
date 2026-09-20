use crate::error::{DbFailure, DbResult};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
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

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }
}

pub fn resolve_declared_target(
    vercel_env: Option<&str>,
    app_env: Option<&str>,
) -> DbResult<DbTarget> {
    match vercel_env.unwrap_or_default().trim().to_lowercase().as_str() {
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

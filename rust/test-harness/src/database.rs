//! Isolated, disposable database support that refuses the PRODUCTION target.
//!
//! THE REFUSAL IS THE POINT. `Database::connect_from_env()` deliberately resolves PROD from `VERCEL_ENV` /
//! `APP_ENV` and connects with no confirmation step (see `db::pool::resolve_declared_target`). A test harness that
//! called it directly could therefore open a production connection from a stray environment variable. This module
//! resolves the declared target itself, refuses [`DbTarget::Prod`] *before any socket is opened*, and only then
//! connects. The refusal is a pure function ([`guard_target`]), which is what lets the self-tests prove it without a
//! database.
//!
//! OWNERSHIP OF CLEANUP is explicit:
//!
//! - A [`TestTransaction`] is a transaction the harness only knows how to `rollback`; there is no commit method, and
//!   sqlx rolls a dropped transaction back, so disposable rows do not survive the test.
//! - `TestDatabase::with_rollback` runs the production statement, reads the result back and rolls back, the
//!   technique `docs/rust-contributing.md` names for verifying a write without changing a row.
//! - An isolated schema wraps every object a test creates, and [`IsolatedSchema::cleanup`] drops it with `CASCADE`.
//!   `CASCADE` is not cleanup hygiene, it is the contract: the schema and everything a test built inside it goes
//!   together.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use db::{resolve_declared_target, Database, DbFailure, DbResult, DbTarget, DbTransaction};
use sqlx::PgConnection;

use crate::fixtures::FixtureFactory;

/// Counts `TestDatabase` instances in this process, so each gets its own isolated schema.
static TEST_DB_INSTANCE: AtomicU64 = AtomicU64::new(0);

/// A namespace unique to one [`TestDatabase`] instance.
///
/// Isolation is per-instance, not per-process: `cargo test` runs one binary's tests on parallel threads, so two
/// `TestDatabase`s in a single process that shared a namespace would share one schema — and whichever test finished
/// first would `drop schema ... cascade` the other's objects out from under it. The process id keeps two processes
/// apart; a monotonic counter and the wall clock keep two instances in one process apart. The result is safe as a SQL
/// identifier (hex digits, `-`, and no caller text), which is what lets it name a schema directly.
pub fn unique_namespace() -> String {
    let instance = TEST_DB_INSTANCE.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let seed = (u64::from(std::process::id()) << 32) ^ nanos ^ instance;
    FixtureFactory::new(seed).namespace().to_owned()
}

/// A harness database operation failed, or the harness refused to perform it.
#[derive(Debug, thiserror::Error)]
pub enum HarnessDbError {
    /// The declared or supplied target was PRODUCTION, which the harness never connects to.
    #[error("test-harness refuses the PRODUCTION database: {0}")]
    ProductionRefused(String),
    /// The environment did not declare a target, and the harness will not guess.
    #[error("database target is undeclared: {0}")]
    Undeclared(String),
    /// The underlying database failure.
    #[error(transparent)]
    Db(#[from] DbFailure),
}

/// Refuse a PRODUCTION target; allow DEV.
///
/// This is the guard the self-tests prove: it is pure, so it runs without a database, and every constructor in this
/// module is built on it.
pub fn guard_target(target: DbTarget) -> Result<(), HarnessDbError> {
    match target {
        DbTarget::Dev => Ok(()),
        DbTarget::Prod => Err(HarnessDbError::ProductionRefused(
            "the harness is for disposable DEV test databases".into(),
        )),
    }
}

/// Resolve the declared environment and refuse anything that resolves to PRODUCTION.
///
/// `VERCEL_ENV` wins over `APP_ENV`, exactly as production resolution does (`db::pool::resolve_declared_target`), so
/// the harness cannot be tricked into DEV by setting one variable while the other says production.
pub fn resolve_test_target(
    vercel_env: Option<&str>,
    app_env: Option<&str>,
) -> Result<DbTarget, HarnessDbError> {
    let target = resolve_declared_target(vercel_env, app_env)
        .map_err(|failure| HarnessDbError::Undeclared(failure.to_string()))?;
    guard_target(target)?;
    Ok(target)
}

/// The disposable test database: a non-production pool plus its cleanup contract.
#[derive(Clone)]
pub struct TestDatabase {
    inner: Database,
    namespace: String,
}

impl TestDatabase {
    /// Wrap an already-connected pool, refusing a PRODUCTION handle.
    pub fn from_database(inner: Database) -> Result<Self, HarnessDbError> {
        guard_target(inner.target())?;
        let namespace = unique_namespace();
        Ok(Self { inner, namespace })
    }

    /// Read the declared environment, refuse PRODUCTION, and connect to the resolved target.
    ///
    /// Unlike `Database::connect_from_env`, this never connects when the declared target is production.
    pub async fn connect_from_env() -> Result<Self, HarnessDbError> {
        let vercel_env = std::env::var("VERCEL_ENV").ok();
        let app_env = std::env::var("APP_ENV").ok();
        Self::connect_declared(vercel_env.as_deref(), app_env.as_deref()).await
    }

    /// Like [`TestDatabase::connect_from_env`], but with the declaration supplied by the caller.
    pub async fn connect_declared(
        vercel_env: Option<&str>,
        app_env: Option<&str>,
    ) -> Result<Self, HarnessDbError> {
        let target = resolve_test_target(vercel_env, app_env)?;
        let inner = Database::connect_target(target).await?;
        Self::from_database(inner)
    }

    /// The underlying production pool, for passing to production DAOs.
    pub fn database(&self) -> &Database {
        &self.inner
    }

    /// The database target, always DEV when this type exists.
    pub fn target(&self) -> DbTarget {
        self.inner.target()
    }

    /// This test database's unique namespace, safe to use as a schema prefix or a row-marker prefix.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Begin a transaction the harness can only roll back.
    pub async fn begin(&self) -> Result<TestTransaction, HarnessDbError> {
        Ok(TestTransaction {
            inner: Some(self.inner.begin("test-harness.transaction").await?),
        })
    }

    /// Run `body` inside a transaction, then **always roll back**, returning what `body` produced.
    ///
    /// This is how a test verifies a write path — run the real statement, read the result back, roll back — without
    /// changing a single canonical row (`docs/rust-contributing.md`). A failure inside `body` still rolls back
    /// before the failure is returned.
    ///
    /// The closure returns a boxed future because the future borrows the connection; callers write
    /// `|conn| Box::pin(async move { ... })`.
    pub async fn with_rollback<F, T>(&self, body: F) -> Result<T, HarnessDbError>
    where
        F: for<'a> FnOnce(
            &'a mut PgConnection,
        ) -> Pin<Box<dyn Future<Output = DbResult<T>> + 'a>>,
    {
        let mut transaction = self.begin().await?;
        let outcome = body(transaction.connection()).await;
        let rollback = transaction.rollback().await;
        match (outcome, rollback) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(error), _) => Err(error.into()),
        }
    }

    /// Create a schema that every object this test makes lives inside, and that one call drops with `CASCADE`.
    pub async fn create_isolated_schema(&self) -> Result<IsolatedSchema, HarnessDbError> {
        let name = schema_name(&self.namespace);
        sqlx::query(sqlx::AssertSqlSafe(format!("create schema if not exists \"{name}\"")))
            .execute(self.inner.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.create_schema", &error))?;
        Ok(IsolatedSchema {
            inner: self.inner.clone(),
            name,
        })
    }
}

fn schema_name(namespace: &str) -> String {
    // A schema name must be a valid identifier. The namespace is generated from a hex seed and a process id, both
    // `[0-9a-f]`/digits, so the result is `[a-z0-9_]` by construction — no caller text is interpolated.
    format!("tsth_{}", namespace.replace('-', "_"))
}

/// A transaction a test may read and write through, and that the harness only knows how to roll back.
pub struct TestTransaction {
    inner: Option<DbTransaction>,
}

impl TestTransaction {
    /// The live connection, for running the production statement under test.
    pub fn connection(&mut self) -> &mut PgConnection {
        self.inner
            .as_mut()
            .expect("test transaction already finalized")
            .connection()
    }

    /// Roll back explicitly. A [`TestTransaction`] dropped without this is rolled back by the driver; committing is
    /// not offered, because disposable test data must not survive the test.
    pub async fn rollback(mut self) -> DbResult<()> {
        self.inner
            .take()
            .expect("test transaction already finalized")
            .rollback()
            .await
    }
}

/// A schema created by a test, dropped by [`IsolatedSchema::cleanup`].
pub struct IsolatedSchema {
    inner: Database,
    name: String,
}

impl IsolatedSchema {
    /// The schema's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A statement setting the session's search path to this schema, so unqualified names resolve inside it.
    pub fn search_path_statement(&self) -> String {
        format!("set search_path to \"{}\"", self.name)
    }

    /// Drop the schema and everything a test created inside it.
    pub async fn cleanup(self) -> Result<(), HarnessDbError> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "drop schema if exists \"{}\" cascade",
            self.name
        )))
        .execute(self.inner.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.drop_schema", &error))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_production_target_is_refused() {
        let error = guard_target(DbTarget::Prod).expect_err("PROD must be refused");
        assert!(matches!(error, HarnessDbError::ProductionRefused(_)));
        assert!(error.to_string().contains("PRODUCTION"));
    }

    #[test]
    fn the_dev_target_is_allowed() {
        assert!(guard_target(DbTarget::Dev).is_ok());
    }

    #[test]
    fn a_declared_production_environment_is_refused_even_when_the_other_variable_says_dev() {
        // VERCEL_ENV wins, exactly as production resolution does: this is still PROD.
        assert!(resolve_test_target(Some("production"), Some("dev")).is_err());
        assert!(resolve_test_target(None, Some("production")).is_err());
        assert!(resolve_test_target(None, Some("prod")).is_err());
    }

    #[test]
    fn a_dev_or_test_environment_resolves_to_dev() {
        assert_eq!(
            resolve_test_target(Some("preview"), Some("prod")).unwrap(),
            DbTarget::Dev
        );
        assert_eq!(
            resolve_test_target(None, Some("test")).unwrap(),
            DbTarget::Dev
        );
        assert_eq!(
            resolve_test_target(None, Some("dev")).unwrap(),
            DbTarget::Dev
        );
    }

    #[test]
    fn silence_is_refused_rather_than_defaulted_to_a_database() {
        assert!(resolve_test_target(None, None).is_err());
    }

    #[test]
    fn every_test_database_gets_its_own_namespace() {
        // The namespace names the isolated schema, and `cleanup` drops it `CASCADE`. If two instances shared one,
        // one test's cleanup would delete another test's objects. Per-instance uniqueness is the isolation contract.
        let first = unique_namespace();
        let second = unique_namespace();
        assert_ne!(
            first, second,
            "two test databases in one process must not share a namespace/schema"
        );
        assert!(first.starts_with("tsth-"), "a namespace names a schema prefix safely");
    }
}

//! Disposable-database probes for the DB.MIGRATION contract suite.
//!
//! Three stories (001, 002, 007) need a database that starts EMPTY: the migration chain, the
//! apply order, and the boot gate can only be proven where no baseline hides a missing
//! dependency. The shared DEV database cannot provide that — it carries 240+ applied
//! migrations — so these tests create a real database, use it, and drop it.
//!
//! TWO ENDPOINTS, TWO JOBS. The pooled `DATABASE_URL_DEV` endpoint cannot own database
//! lifecycle: PgBouncer holds server sessions after the client goes away, and Postgres
//! refuses `DROP DATABASE` while any session touches the database (observed 2026-10-05:
//! a prototype database became undroppable through the pooler). So administration
//! (CREATE, session drain, DROP) goes through the DIRECT endpoint
//! (`DATABASE_URL_UNPOOLED`), while the replay itself runs through the production
//! [`Database`] type — the same boundary `db-tool apply` uses. Admin is scaffolding;
//! the replay is the subject.

use db::{Database, DbFailure, DbTarget};

use crate::database::HarnessDbError;

/// Build the direct-URL of a disposable database from the unpooled endpoint.
///
/// The pooler endpoint is refused on purpose (see module docs): lifecycle through PgBouncer
/// leaves sessions behind and `DROP DATABASE` fails. A missing `DATABASE_URL_UNPOOLED` is a
/// hard error, never a silent fallback to the pooler.
pub fn fresh_db_url(db_name: &str) -> Result<String, HarnessDbError> {
    let base = std::env::var("DATABASE_URL_UNPOOLED").map_err(|_| {
        HarnessDbError::Undeclared(
            "DATABASE_URL_UNPOOLED is required for empty-database migration proofs; \
             the pooled endpoint cannot drop a database"
                .to_string(),
        )
    })?;
    let (head, query) = match base.split_once('?') {
        Some((head, query)) => (head, Some(query)),
        None => (base.as_str(), None),
    };
    let head = head.trim_end_matches('/');
    let prefix = match head.rfind('/') {
        Some(index) => &head[..=index],
        None => "",
    };
    let url = match query {
        Some(query) => format!("{prefix}{db_name}?{query}"),
        None => format!("{prefix}{db_name}"),
    };
    Ok(url)
}

/// A database name derived from the harness namespace: lowercase alphanumerics only.
pub fn fresh_db_name(namespace: &str) -> String {
    format!("tstdb_{}", namespace.replace('-', "_"))
}

/// Create an empty disposable database on the DIRECT endpoint and return its name.
///
/// The caller owns the drop (`drop_fresh_db`). Creation goes through a one-connection
/// admin pool because `CREATE DATABASE` cannot run inside a transaction block.
pub async fn create_fresh_db(namespace: &str) -> Result<String, HarnessDbError> {
    let name = fresh_db_name(namespace);
    let admin_url = std::env::var("DATABASE_URL_UNPOOLED").map_err(|_| {
        HarnessDbError::Undeclared(
            "DATABASE_URL_UNPOOLED is required for empty-database migration proofs".to_string(),
        )
    })?;
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .map_err(|error| DbFailure::from_sqlx("migration_probe.admin_connect", &error))?;
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .map_err(|error| DbFailure::from_sqlx("migration_probe.create_database", &error))?;
    admin.close().await;
    Ok(name)
}

/// Connect the production [`Database`] type to a disposable database.
///
/// `Database` only connects by declared target, so the fresh URL is published as
/// `DATABASE_URL_DEV` for exactly one call and restored before this function returns —
/// the pool owns its connections by then, and no other thread in the test binary can
/// observe the swap (each canonical test file runs in its own binary).
pub async fn connect_fresh_db(db_name: &str) -> Result<Database, HarnessDbError> {
    let url = fresh_db_url(db_name)?;
    let previous = std::env::var("DATABASE_URL_DEV").ok();
    // NOTE: single-test binaries only. `std::env::set_var` is process-global; every
    // canonical DB.MIGRATION file holds exactly one test, so no sibling thread reads this.
    std::env::set_var("DATABASE_URL_DEV", &url);
    let connected = Database::connect_target(DbTarget::Dev).await;
    match previous {
        Some(value) => std::env::set_var("DATABASE_URL_DEV", value),
        None => std::env::remove_var("DATABASE_URL_DEV"),
    }
    let database = connected?;
    debug_assert_eq!(database.target(), DbTarget::Dev);
    Ok(database)
}

/// Drop a disposable database: wait for its backends to drain, then drop.
///
/// `Database` owns no close method and sqlx retires pooled connections in the background,
/// so the drop polls `pg_stat_activity` (DIRECT endpoint, which sees the real backends)
/// before issuing `DROP DATABASE`. A lingering backend is retried, not forced: the pooler
/// path is never used here, so a drain always terminates.
pub async fn drop_fresh_db(db_name: &str) -> Result<(), HarnessDbError> {
    let admin_url = std::env::var("DATABASE_URL_UNPOOLED").map_err(|_| {
        HarnessDbError::Undeclared(
            "DATABASE_URL_UNPOOLED is required for empty-database migration proofs".to_string(),
        )
    })?;
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .map_err(|error| DbFailure::from_sqlx("migration_probe.admin_connect", &error))?;
    let mut backends: i64 = 1;
    for _ in 0..60 {
        backends = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = $1 AND pid <> pg_backend_pid()",
        )
        .bind(db_name)
        .fetch_one(&admin)
        .await
        .map_err(|error| DbFailure::from_sqlx("migration_probe.drain_wait", &error))?;
        if backends == 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    if backends != 0 {
        admin.close().await;
        return Err(HarnessDbError::Db(DbFailure::configuration(
            "migration_probe.drop_database",
            format!("disposable database {db_name} still has {backends} backends after 30s; refusing to drop"),
        )));
    }
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{db_name}\"")))
        .execute(&admin)
        .await
        .map_err(|error| DbFailure::from_sqlx("migration_probe.drop_database", &error))?;
    admin.close().await;
    Ok(())
}

/// True when a table exists in the `public` schema of the given database.
pub async fn table_exists(database: &Database, table: &str) -> Result<bool, HarnessDbError> {
    let present: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.' || $1)::text")
        .bind(table)
        .fetch_one(database.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("migration_probe.table_exists", &error))?;
    Ok(present.is_some())
}

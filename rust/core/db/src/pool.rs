use crate::error::{DbFailure, DbResult};
use crate::transaction::DbTransaction;
use futures_util::TryStreamExt;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Connection;
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

        // PREPARED STATEMENTS STAY ON. This was briefly disabled, on the reasoning that Neon's `-pooler` endpoint is
        // PgBouncer in transaction mode and transaction mode does not honour named prepared statements - so a
        // statement prepared on one backend would not exist on the next. The reasoning is right about PgBouncer and
        // wrong about this code: measured against the real endpoint, a query on a held connection costs 79.6ms with
        // the cache and 160.0ms without it, which is one round trip versus two. sqlx re-prepares when a statement is
        // missing, and the pooler accepts what sqlx sends, so the cost of the caution was 80ms on every statement -
        // doubling the latency of every query in the application - for a failure that does not happen.
        //
        // If prepared statements ever do start failing on this endpoint, the symptom will be loud ("prepared statement
        // does not exist", SQLSTATE 26000) and this is the line to revisit. Until then the cache stays on.

        // Production serves the whole portal through one long-lived Rust service. Several screens issue
        // independent reads in parallel (Cockpit alone has ten projections), so the old max=5 caused real
        // requests to queue behind connection creation while readiness still looked healthy on its single
        // warm connection. Keep DEV conservative, but size PROD for the concurrency the service actually has.
        let default_max_connections = if target == DbTarget::Prod { 12 } else { 5 };
        let max_connections = positive_u32("FORGE_DB_POOL_MAX", default_max_connections);
        // KEEP ONE CONNECTION WARM. Without a floor the pool holds nothing when idle, so the next request pays a full
        // connect: measured at 498ms for the handshake plus authentication, against ~72ms for a round trip on a
        // connection that is already open. That is the difference between a page feeling instant and feeling slow, and
        // it hit the engine hardest because engine commands are far apart in time. `FORGE_DB_POOL_MIN=0` restores the
        // old hold-nothing behaviour, which is what tests want.
        let default_min_connections = if target == DbTarget::Prod { 5 } else { 1 };
        let min_connections = non_negative_u32("FORGE_DB_POOL_MIN", default_min_connections)
            .min(max_connections);
        // `idle_timeout` only reclaims connections ABOVE the floor. 10s was aggressive enough that a burst of activity
        // followed by a pause re-established everything; 60s keeps a working set without holding connections forever.
        // The floor is what guarantees warmth, this is only about not churning the rest.
        let idle_ms = positive_u64("FORGE_DB_POOL_IDLE_MS", 60_000);
        let connect_ms = positive_u64("FORGE_DB_POOL_CONNECT_MS", 10_000);

        // PROBE ON IDLE, NOT ON EVERY CHECKOUT.
        //
        // sqlx pings a connection before handing it out whenever `test_before_acquire` is set, and it is set by
        // default. For Postgres that ping is `write_sync` + `wait_until_ready` - not a query, but a full round trip -
        // and a round trip to the dev database measures 72ms. A page load takes several pool checkouts, so the default
        // silently adds a few hundred milliseconds of nothing to every request that is otherwise doing real work.
        //
        // The fix is sqlx's own documented pattern: turn the blanket ping off and probe only a connection that has sat
        // idle long enough for the pooler to have dropped it. A warm connection is used immediately; a stale one is
        // checked before it can hand a dead socket to a query. `FORGE_DB_IDLE_PROBE_MS=0` probes every time, which is
        // the old behaviour, and is how this was measured.
        let idle_probe = Duration::from_millis(non_negative_u64("FORGE_DB_IDLE_PROBE_MS", 30_000));

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .idle_timeout(Some(Duration::from_millis(idle_ms)))
            .acquire_timeout(Duration::from_millis(connect_ms))
            .test_before_acquire(false)
            .before_acquire(move |conn, meta| {
                crate::metrics::record_checkout();
                // `age` is the time since the connection was opened, so a connection being opened right now has an age
                // near zero. That is the difference between a reused connection and a handshake, which is the number
                // this workspace most needs to watch.
                if meta.age < Duration::from_millis(250) {
                    crate::metrics::record_connection_opened();
                }
                Box::pin(async move {
                    if meta.idle_for >= idle_probe {
                        crate::metrics::record_idle_probe();
                        if let Err(error) = conn.ping().await {
                            crate::metrics::record_probe_failed();
                            return Err(error);
                        }
                    }
                    Ok(true)
                })
            })
            .connect_with(options)
            .await
            .map_err(|error| DbFailure::from_sqlx("db.connect", &error))?;

        Ok(Self { pool, target })
    }

    pub const fn target(&self) -> DbTarget {
        self.target
    }

    /// What the pool has been doing. Counters are since process start plus the pool's current occupancy.
    pub fn metrics(&self) -> crate::metrics::Snapshot {
        use std::sync::atomic::Ordering;
        let counters = &crate::metrics::COUNTERS;
        crate::metrics::Snapshot {
            checkouts: counters.checkouts.load(Ordering::Relaxed),
            connections_opened: counters.connections_opened.load(Ordering::Relaxed),
            idle_probes: counters.idle_probes.load(Ordering::Relaxed),
            probes_failed: counters.probes_failed.load(Ordering::Relaxed),
            pool_size: self.pool.size(),
            pool_idle: self.pool.num_idle() as u32,
        }
    }

    /// Keep the pool warm so a suspended Neon branch never has to be woken by a user request.
    ///
    /// Neon suspends an idle database, and a suspended database makes the NEXT connect slow - which is exactly the
    /// cold-connect the retry policy exists to limp through. Prevention beats retrying around it: one cheap `select 1`
    /// every few minutes means the wake cost is never paid by a page load. The failure of a keepalive ping is not
    /// returned to anyone, but it IS announced like any other failure, so a database that has genuinely gone away
    /// still produces an `app_error` row instead of failing silently in a background task.
    ///
    /// OFF by default in tests and in anything with `FORGE_DB_KEEPALIVE_MS=0`, because a pool that pings forever is
    /// also a pool a test process cannot exit.
    pub fn spawn_keepalive(
        &self,
        runtime: &tokio::runtime::Handle,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let interval_ms = keepalive_interval_ms();
        if interval_ms == 0 {
            return None;
        }
        let pool = self.pool.clone();
        Some(runtime.spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // The first tick is immediate; skip it so a just-started pool is not pinged twice.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                if let Err(failure) = ping_pool(&pool).await {
                    // Announced from the DbFailure constructor, so this appears in app_error like any other failure.
                    let _ = failure;
                }
            }
        }))
    }

    pub async fn ping(&self) -> DbResult<()> {
        ping_pool(&self.pool).await
    }

    pub async fn begin(&self, operation: &'static str) -> DbResult<DbTransaction> {
        let transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| DbFailure::from_sqlx(operation, &error))?;
        Ok(DbTransaction::new(transaction, operation))
    }

    /// The shared pool, for crates that must run SQL this crate does not model.
    ///
    /// PUBLIC ON PURPOSE (2026-09-20). This was `pub(crate)`, which cannot be called from `integrations` or
    /// `core/workflow` — separate crates — so nine call sites in the BoldSign adapter did not compile. The
    /// alternative considered and rejected was an `UnsafeCell`-style workaround; the honest fix is to name the
    /// accessor and document what it is for.
    ///
    /// `Database` still owns the only pool in the Rust workspace (`Database::connect_from_env`); handing out a
    /// borrow does not create a second one. Callers should prefer the typed DAOs, and `run_text` below is the
    /// precedent for this kind of sanctioned escape hatch. A `with_conn`-style borrowed API is the intended
    /// follow-up so this accessor can eventually narrow again.
    pub fn pool(&self) -> &PgPool {
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
        let mut stream =
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned())).fetch_many(&mut *connection);
        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|error| DbFailure::from_sqlx("db.run_text", &error))?
        {
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

/// `FORGE_DB_KEEPALIVE_MS` — how often to ping.
///
/// 60 seconds by default. Neon suspends an idle branch, and waking one costs about two seconds, paid by whoever happens
/// to make the next request - which in development is the person looking at the screen wondering why the first click
/// after a pause is slow. A `select 1` a minute keeps the branch awake. It is a real query against a real database, so
/// it does keep Neon compute warm: `FORGE_DB_KEEPALIVE_MS=0` turns it off, which is reasonable in production where the
/// application's own traffic keeps the branch awake anyway.
fn keepalive_interval_ms() -> u64 {
    match env::var("FORGE_DB_KEEPALIVE_MS") {
        Ok(value) => value.trim().parse::<u64>().unwrap_or(0),
        Err(_) => 60_000,
    }
}

async fn ping_pool(pool: &PgPool) -> DbResult<()> {
    let _: i32 = sqlx::query_scalar("select 1::int")
        .fetch_one(pool)
        .await
        .map_err(|error| DbFailure::from_sqlx("db.ping", &error))?;
    Ok(())
}

/// Like `positive_u32`, but 0 is a meaningful value: for `FORGE_DB_POOL_MIN` it means "hold no connection open", which
/// is what a test process or a one-shot CLI wants.
fn non_negative_u32(name: &str, fallback: u32) -> u32 {
    match env::var(name) {
        Ok(value) => value.trim().parse::<u32>().unwrap_or(fallback),
        Err(_) => fallback,
    }
}

fn non_negative_u64(name: &str, fallback: u64) -> u64 {
    match env::var(name) {
        Ok(value) => value.trim().parse::<u64>().unwrap_or(fallback),
        Err(_) => fallback,
    }
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

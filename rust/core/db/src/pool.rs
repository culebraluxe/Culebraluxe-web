use crate::error::{DbFailure, DbResult};
use crate::transaction::DbTransaction;
use futures_util::TryStreamExt;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Connection;
use sqlx::PgPool;
use std::env;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Every statement on the pool is cancelled after this, unless the environment overrides it. See the comment in
/// `connect_target`: it is a ceiling against a stuck query, not a performance budget.
const DEFAULT_STATEMENT_TIMEOUT_MS: u64 = 30_000;

/// The ceiling, resolved once per process and settable to 0 by `disable_statement_timeout`.
static STATEMENT_TIMEOUT_MS: OnceLock<AtomicU64> = OnceLock::new();

/// Production keeps this many connections OPEN and idle, ready to be used: the pool's floor.
///
/// It is a floor and not a target — `idle_timeout` never reclaims a connection that would take the pool below it — so
/// this is the number of handshakes the system does not pay for. See `default_pool_bounds` for why it is 20.
const PROD_POOL_MIN: u32 = 20;

/// Production's ceiling on simultaneous connections. Thirty leaves the twenty-connection floor room to grow with load.
const PROD_POOL_MAX: u32 = 30;

/// Development keeps almost nothing warm: a developer's loop does not have the concurrency, and every warm connection
/// is a Neon connection somebody is paying for.
const DEV_POOL_MIN: u32 = 1;
const DEV_POOL_MAX: u32 = 5;

/// The moment the pool stops trusting connections it has not just verified, as millis since process start. 0 means
/// "nothing has failed"; see `note_connection_failure`.
static SUSPECT_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

/// The process's clock for the suspect window. Monotonic, so a wall-clock adjustment cannot close the window early
/// or hold it open past its end.
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

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
    pub(crate) identity: Arc<()>,
}

impl Database {
    pub async fn connect_from_env() -> DbResult<Self> {
        let target = resolve_declared_target(
            env::var("VERCEL_ENV").ok().as_deref(),
            env::var("APP_ENV").ok().as_deref(),
        )?;
        Self::connect_target(target).await
    }

    /// Connect the pool Forge is allowed to use: production, or nothing.
    ///
    /// The stories, the cockpit and the flight recorder live in the production database. It is the golden source,
    /// so a Forge process on any other target reads and writes a control plane nobody is working in and its
    /// verdicts are about rows nobody makes decisions from. `resolve_declared_target` still owns the declaration
    /// (`APP_ENV` / `VERCEL_ENV`) and still refuses silence; this adds Forge's ceiling on top of it, so a caller
    /// cannot flip the environment by exporting one variable.
    pub async fn connect_forge_from_env() -> DbResult<Self> {
        let target = resolve_forge_target(
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

        // A CEILING ON EVERY STATEMENT — APPLIED AS A STATEMENT, NOT AS A STARTUP PARAMETER.
        //
        // Without a ceiling, a query that goes wrong holds its connection until Postgres or the network gives up.
        // There are thirty connections in production, so a handful of stuck statements is the whole portal
        // waiting, and the symptom reads as "the site is slow" rather than "this statement never returned".
        // Thirty seconds is far above the slowest page (the worst statement measured here is 872ms) and far
        // below the patience of a person.
        //
        // THIS SETTING USED TO TRAVEL IN `PgConnectOptions::options(...)`, AND THAT COULD NOT CONNECT AT ALL.
        // libpq's `options` is part of the STARTUP PACKET, and both of this project's endpoints — DEV and PROD — are
        // Neon `-pooler` endpoints (PgBouncer). PgBouncer refuses a startup option it does not track, so every
        // connection died before a single query ran:
        //
        //   unsupported startup parameter in options: statement_timeout.
        //   Please use unpooled connection or remove this parameter from the startup package.
        //
        // It was found by starting the server against DEV (2026-09-28, `db.connect` on the boot line, SQLSTATE
        // 08P01) — the unit tests never open a socket, which is exactly the gap `docs/rust-contributing.md` warns
        // about. `SET` is an ordinary statement and is honoured on a pooled connection, so the ceiling is applied
        // once per connection, after it is established.
        //
        // `FORGE_DB_STATEMENT_TIMEOUT_MS=0` disables it; work that is deliberately long — a migration that builds an
        // index, a bulk load — calls `db::disable_statement_timeout()` instead, because waiting there is the
        // operator's own decision and not a request a browser is holding open. Connections opened after that call
        // get `0`; connections already open keep the ceiling until they are recycled, which is the same behaviour the
        // startup parameter had.
        //
        // The value is a `u64` formatted into the statement: a number cannot carry a quote, so this is not
        // string-built SQL, and `SET` does not accept a bind parameter.

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

        // PRODUCTION SIZING: 20 HOT, 30 TOTAL, PER PROCESS (captain, 2026-09-29).
        //
        // The engine and the application are SEPARATE PROCESSES with SEPARATE POOLS. They are never up at once on the
        // same pool, so "the engine borrows the app's connections" is not a thing that happens — and that is what made
        // the old numbers undersized rather than conservative: five hot connections had to cover Cockpit's ten
        // parallel projections, a burst of ordinary reads, and every engine step, and a checkout that waits for a
        // handshake against Neon (~498ms) looks to a person like the database being slow.
        //
        // The FLOOR is the number that matters. These connections are already open when work arrives, so a page load or
        // an engine step pays a round trip (~72ms) and never a handshake. `idle_timeout` only reclaims connections
        // ABOVE the floor, so the twenty are held deliberately; `FORGE_DB_POOL_MIN=0` restores the old hold-nothing
        // behaviour, which is what tests want.
        //
        // AND TWENTY WAS NOT REACHABLE UNTIL THE POOL STOPPED WAITING FOR THEM (2026-09-29, hours after this sizing
        // landed). `PgPoolOptions::connect_with` establishes `min_connections` before it returns, in a SERIAL
        // `while size() < min_connections` loop, inside a deadline taken from `acquire_timeout`
        // (sqlx-core 0.9.0, `pool/options.rs:541-550` and `pool/inner.rs:400`). A single cold connect to this project's
        // Neon `-pooler` endpoint measured 1.4s, and the loop's slope measured ~0.93s per connection (the bisect below),
        // so twenty of them is 19-28s against a 10s budget: the pool did not come up slowly, it did not come up at all,
        // and the error was
        //
        //   Timeout during db.connect: pool timed out while waiting for an open connection
        //
        // Measured live on PRODUCTION, with the engine's own floor already open on the same endpoint: `min=2` -> 2.3s,
        // `min=5` -> 4.8s, `min=10` -> 9.3s, `min=20` -> failed at 10.1s. Linear in the floor, because the loop is
        // serial. Serial on THIS SIDE only, though: four concurrent one-connection clients took 1.7s wall where four
        // serial ones took 5.7s, so the pooler serves logins in parallel. The engine survived the floor for one reason
        // and it was not the pool: it raises its own connect budget to 60s (`forge/src/engine/db_budget.rs`), which is
        // long enough to warm twenty serially. Nothing else in the workspace has that budget, which is why a CLI read
        // timed out at ten seconds while the engine was running.
        //
        // So the pool opens LAZILY (`connect_lazy_with`), the floor is warmed concurrently below, and the first query
        // pays one handshake instead of twenty. Re-measured against production after the change, same 10s budget:
        // `min=20` acquires in 3.0s where it previously failed at 10.1s, and the cost no longer tracks the floor
        // (`min=2` 2.2s, `min=10` 2.5s, `min=20` 3.0s).
        //
        // The ceiling does NOT fix a dead socket. The run that died at 19:25 on 2026-09-29 was killed by a connection
        // that broke while it was checked out, and no ceiling size can repair that — a bigger pool would only have had
        // more good connections to hand out around the broken one. The repairs for that are `max_lifetime`, the
        // post-failure verification window below, and the step retry in the workflow store; this is headroom.
        let (default_min_connections, default_max_connections) = default_pool_bounds(target);
        let max_connections = positive_u32("FORGE_DB_POOL_MAX", default_max_connections);
        let min_connections =
            non_negative_u32("FORGE_DB_POOL_MIN", default_min_connections).min(max_connections);
        // `idle_timeout` only reclaims connections ABOVE the floor. 10s was aggressive enough that a burst of activity
        // followed by a pause re-established everything; 60s keeps a working set without holding connections forever.
        // The floor is what guarantees warmth, this is only about not churning the rest.
        let idle_ms = positive_u64("FORGE_DB_POOL_IDLE_MS", 60_000);
        let connect_ms = positive_u64("FORGE_DB_POOL_CONNECT_MS", 10_000);

        // RETIRE A CONNECTION BY AGE. `FORGE_DB_POOL_MAX_LIFETIME_MS=0` keeps connections forever, which is what this
        // did until 2026-09-29.
        //
        // Both endpoints are Neon `-pooler` (PgBouncer) connections, and the pooler retires server connections on its
        // own schedule - for reasons that include idle time and its own rebalancing. A client socket that has been
        // around for a long time has survived many of those windows and is the most likely thing in the pool to be a
        // half-dead socket that has not been noticed yet. sqlx closes a connection past its lifetime when it is
        // RETURNED to the pool (`pool/connection.rs`) and the maintenance task closes the ones that are sitting idle
        // (`pool/inner.rs`), re-establishing the floor immediately, so the next checkout gets a fresh connection
        // instead of an old one. Thirty minutes is far longer than any single engine step and far shorter than a
        // Neon suspend window.
        let lifetime_ms = non_negative_u64("FORGE_DB_POOL_MAX_LIFETIME_MS", 1_800_000);

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

        // AND VERIFY EVERYTHING FOR A WHILE AFTER SOMETHING BROKE.
        //
        // The idle threshold is a guess about when the pooler drops a connection, and a wrong guess is handed straight
        // to a caller: a connection that broke while it was CHECKED OUT - mid-transaction, which is exactly what killed
        // the engine run at 19:25 on 2026-09-29 with `Broken pipe (os error 32)` - comes back to the pool looking
        // exactly like a good one and can be reissued before it has been idle long enough to be probed. Age does not
        // distinguish them either; both are seconds old.
        //
        // So the pool stops trusting its own contents when there is evidence: any connection-class `DbFailure` calls
        // `note_connection_failure`, and for `FORGE_DB_RECHECK_MS` (60s, 0 to disable) every checkout is verified
        // before it is used. A bad connection is then caught at the ping instead of by the caller's first statement,
        // and sqlx replaces it rather than handing it out. One extra round trip per checkout for a minute after a
        // failure is nothing next to losing a three-hour run, and the window closes by itself.
        let recheck_ms = recheck_window_ms();

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .idle_timeout(Some(Duration::from_millis(idle_ms)))
            .acquire_timeout(Duration::from_millis(connect_ms))
            .max_lifetime((lifetime_ms > 0).then(|| Duration::from_millis(lifetime_ms)))
            .test_before_acquire(false)
            // THE CEILING GOES ON HERE, NOT IN THE STARTUP PACKET (see the comment above `options`).
            // `set_config(..., false)` is the session-scoped form, and it takes the value as a BIND: sqlx 0.9's
            // `SqlSafeStr` refuses dynamically built SQL (`sqlx::query` only accepts `&'static str`), which is the
            // right fence to have — and `SET` cannot take a parameter, so the statement is static and the number
            // travels as a bind.
            .after_connect(move |conn, _meta| {
                Box::pin(async move {
                    let millis = format!("{}ms", statement_timeout_ms());
                    sqlx::query("SELECT set_config('statement_timeout', $1, false)")
                        .bind(millis)
                        .execute(conn)
                        .await
                        .map(|_| ())
                })
            })
            .before_acquire(move |conn, meta| {
                crate::metrics::record_checkout();
                // `age` is the time since the connection was opened, so a connection being opened right now has an age
                // near zero. That is the difference between a reused connection and a handshake, which is the number
                // this workspace most needs to watch.
                if meta.age < Duration::from_millis(250) {
                    crate::metrics::record_connection_opened();
                }
                let idle_due = meta.idle_for >= idle_probe;
                // The pool distrusts itself for a window after a connection-class failure. `FORGE_DB_RECHECK_MS=0`
                // turns the repair off, which is how the old one-probe behaviour is measured.
                let suspect = recheck_ms > 0 && suspect_window_open();
                let probe = probe_needed(meta.idle_for, idle_probe, suspect);
                // Which reason fired only matters for the counters: the ping is the same round trip either way.
                let recheck = probe && !idle_due;
                Box::pin(async move {
                    if probe {
                        if recheck {
                            crate::metrics::record_recheck_probe();
                        } else {
                            crate::metrics::record_idle_probe();
                        }
                        if let Err(error) = conn.ping().await {
                            crate::metrics::record_probe_failed();
                            return Err(error);
                        }
                    }
                    Ok(true)
                })
            })
            .connect_lazy_with(options);

        // ONE BOUNDED CHECKOUT, because a database that is not there must still be reported as `db.connect` when the
        // pool is built — not discovered later by whichever query happens to run first. This is exactly the second
        // half of sqlx's own `connect_with` (`pool/options.rs:552-555`); what is deliberately NOT copied is its first
        // half, which establishes the floor before returning (see the sizing comment above the builder).
        let checkout = pool
            .acquire()
            .await
            .map_err(|error| DbFailure::from_sqlx("db.connect", &error))?;
        drop(checkout);

        // WARM THE FLOOR CONCURRENTLY, because the pooler serves logins in parallel and sqlx's reaper does not.
        // Measured against production on 2026-09-29: four concurrent one-connection clients took 1.7s wall where
        // four serial ones took 5.7s, so nineteen parallel handshakes cost about what one does. Left to the reaper's
        // serial `try_min_connections` loop (`pool/inner.rs:400`) the floor would arrive in ~28s, and every request in
        // that window would pay the handshake the floor exists to remove. Failures here are ignored on purpose: the
        // reaper keeps trying, and a warm-up that cannot open a connection is not a reason to fail a `Db::connect`.
        let openers = warm_openers(min_connections, max_connections);
        if openers > 0 {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let warm_pool = pool.clone();
                handle.spawn(async move {
                    let mut warming = Vec::with_capacity(openers as usize);
                    for _ in 0..openers {
                        let pool = warm_pool.clone();
                        warming.push(tokio::spawn(async move {
                            if let Ok(connection) = pool.acquire().await {
                                drop(connection);
                            }
                        }));
                    }
                    for task in warming {
                        let _ = task.await;
                    }
                });
            }
        }

        Ok(Self {
            pool,
            target,
            identity: Arc::new(()),
        })
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
            recheck_probes: counters.recheck_probes.load(Ordering::Relaxed),
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

    /// Begin a transaction, retrying when the failure is a SESSION the server took away.
    ///
    /// WHY THE RETRY BELONGS HERE AND NOT AROUND THE STEP. `BEGIN` carries no work: if it fails, no transaction
    /// existed and no statement ran, so repeating it cannot double anything. That is not true of the statements
    /// *inside* the transaction, which is why the step itself is not retried here — whoever knows whether a step is
    /// idempotent owns that decision (`crate::retry` says the same thing at more length).
    ///
    /// Measured 2026-09-29 against PROD: a Forge run died 2m45s in, at a workflow-step boundary, on
    /// `sqlstate 25P03 — terminating connection due to idle-in-transaction timeout`. Nothing was wrong with the work.
    /// The engine had been handed a session the server had already terminated, the failure classified as `Unknown`
    /// (so nothing retried it), and an eight-minute story was lost at second three. The retry is what makes a lost
    /// session cost one round trip instead of a run.
    ///
    /// The retry does NOT reuse the connection: `PgPool::begin` acquires, and a connection that read a FATAL error is
    /// discarded by sqlx, so the second attempt is a different session. Both failures are announced (each
    /// `DbFailure` announces itself from its constructor), so a retry is visible in `app_error` rather than silent.
    pub async fn begin(&self, operation: &'static str) -> DbResult<DbTransaction> {
        let policy = crate::retry::policy();
        let attempts = policy.attempts.max(1);
        let mut attempt = 1;
        loop {
            match self.begin_single_attempt(operation).await {
                Ok(transaction) => return Ok(transaction),
                Err(failure) => {
                    // Only a failure the taxonomy calls retryable is repeated - a constraint violation or a schema
                    // mismatch is retried zero times, because waiting does not fix either.
                    if !failure.retryable || attempt >= attempts {
                        return Err(failure);
                    }
                    attempt += 1;
                    // A short pause, because the replacement connection has to be made: the pool is acquiring one
                    // while this sleeps, and a cold Neon branch's handshake is the cost being waited out.
                    tokio::time::sleep(policy.base_delay).await;
                }
            }
        }
    }

    /// Begin a transaction the *server* will not let write: `SET TRANSACTION READ ONLY` as its first statement.
    ///
    /// The difference matters. A read tool that merely promises not to write relies on its own guard being right;
    /// a read tool on a read-only transaction relies on Postgres, which refuses the write whatever the caller
    /// intended. `SET TRANSACTION` must be the transaction's first statement (Postgres rejects it once the
    /// transaction has done work), which is why it lives in the same place `BEGIN` does rather than at the call site.
    ///
    /// Used by `ForgeReadDao::read_only_rows` — the `forge sql` verb — so that an ad-hoc question about the
    /// control plane can be asked from a terminal without a throwaway script and without any way to write.
    pub async fn begin_read_only(&self, operation: &'static str) -> DbResult<DbTransaction> {
        let mut transaction = self.begin(operation).await?;
        sqlx::query("set transaction read only")
            .execute(transaction.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx(operation, &error))?;
        Ok(transaction)
    }

    async fn begin_single_attempt(&self, operation: &'static str) -> DbResult<DbTransaction> {
        if let Some(scope) = crate::unit_of_work::current(&self.identity) {
            let guard = scope.transaction.clone().lock_owned().await;
            if guard.is_none()
                || scope
                    .rollback_only
                    .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err(DbFailure::configuration(
                    operation,
                    "Mutation transaction has already been rolled back.",
                ));
            }
            return Ok(DbTransaction::scoped(guard, scope.rollback_only));
        }
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

    /// Which database this handle is on. Read by the operator tools so a read can say out loud whether it
    /// answered from DEV or PROD — a board that does not name its database is a board an operator has to
    /// guess about, and guessing is how a cleanup lands on the wrong environment.
    pub fn declared_target(&self) -> DbTarget {
        self.target
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

/// The database Forge is allowed to run against: PROD, and nothing else.
///
/// THE RULE (captain, 2026-09-29): **Forge runs against production only.** The stories, the cockpit and the flight
/// recorder are the golden source, and this is not ceremony — a Forge process on DEV reports on rows nobody is
/// working in, and the report is trusted because it is the only number on the screen (2026-09-16: a department
/// reading the wrong database produced verdicts nobody could trust).
///
/// `resolve_declared_target` owns the declaration: it reads `APP_ENV` / `VERCEL_ENV` and refuses silence.
/// This function owns Forge's ceiling on top of that declaration, so no caller — a wrapper script, a stale
/// `.env`, a hand-typed `cargo run` — can flip a Forge process to another environment by exporting one variable.
/// A refusal is a `DbFailure::configuration`, which is captured like every other database failure.
pub fn resolve_forge_target(vercel_env: Option<&str>, app_env: Option<&str>) -> DbResult<DbTarget> {
    let declared = resolve_declared_target(vercel_env, app_env)?;
    if declared != DbTarget::Prod {
        return Err(DbFailure::configuration(
            "db.resolve_forge_target",
            format!(
                "Forge runs against PRODUCTION only; the declared environment resolved to '{}'. \
                 Debug workflow and Forge in production",
                declared.as_str()
            ),
        ));
    }
    Ok(DbTarget::Prod)
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

/// The per-statement ceiling, resolved once per process.
///
/// Once, not per connection: `non_negative_u64` reads the environment, and a process that resolved it per connect
/// could give two connections in the same pool different ceilings — exactly the kind of difference nobody would
/// ever find.
fn statement_timeout_ms() -> u64 {
    STATEMENT_TIMEOUT_MS
        .get_or_init(|| {
            AtomicU64::new(non_negative_u64(
                "FORGE_DB_STATEMENT_TIMEOUT_MS",
                DEFAULT_STATEMENT_TIMEOUT_MS,
            ))
        })
        .load(Ordering::Relaxed)
}

/// Turn the per-statement ceiling OFF for this process.
///
/// For the operators' own long work: applying a migration that builds an index, or a bulk load, may legitimately
/// take minutes, and cancelling it halfway is worse than waiting. The request path holds no such statement, which
/// is why the default protects it and this exists for the tools that do.
pub fn disable_statement_timeout() {
    statement_timeout_ms();
    if let Some(ceiling) = STATEMENT_TIMEOUT_MS.get() {
        ceiling.store(0, Ordering::Relaxed);
    }
}

/// The pool bounds for a target: connections kept hot (`min`) and the ceiling (`max`).
///
/// A FUNCTION SO THE DECISION CAN BE TESTED (2026-09-29). These numbers used to be inline in `connect_target`, which
/// only runs with a real database URL, so nothing could assert them — and the sizing is a decision rather than a
/// detail: the floor is how many handshakes the system does not pay for, and the engine and the application each pay
/// their own, because they are separate processes.
///
/// PRODUCTION: 20 hot, 30 total. DEV: 1 hot, 5 total — a development loop has no concurrency and every warm connection
/// is one somebody is paying for. See `connect_target` for what the production numbers replaced and what they do NOT
/// fix.
const fn default_pool_bounds(target: DbTarget) -> (u32, u32) {
    match target {
        DbTarget::Prod => (PROD_POOL_MIN, PROD_POOL_MAX),
        DbTarget::Dev => (DEV_POOL_MIN, DEV_POOL_MAX),
    }
}
/// How many connections the background warm-up opens: the floor, less the one the bounded checkout already holds.
///
/// CLAMPED BY THE CEILING, and that clamp is the reason this is a function. The checkout and the warm-up are the two
/// things that grow the pool at start-up, so `1 + warm_openers` overshooting the ceiling would mean asking for
/// connections the pool cannot grant — a self-inflicted version of the starvation this file keeps repairing. A floor
/// above the ceiling is clamped by sqlx too (`min_connections` wins), so the warm-up has to clamp the same way or the
/// two disagree about what the pool size is.
///
/// Zero is the common case in tests and one-shot tools (`FORGE_DB_POOL_MIN=0`): nothing to warm, nothing spawned.
const fn warm_openers(min_connections: u32, max_connections: u32) -> u32 {
    let floor = min_connections.saturating_sub(1);
    let ceiling = max_connections.saturating_sub(1);
    if floor < ceiling {
        floor
    } else {
        ceiling
    }
}



/// Whether the pool must verify a connection before it is handed to a caller.
///
/// Two independent reasons. The first is the age of an idle connection: the pooler may have dropped it, and 72ms of
/// round trip is cheaper than a caller discovering it. The second is what this grew on 2026-09-29: while the pool is
/// suspect after a connection-class failure, EVERY connection is verified, because it cannot tell which socket broke -
/// a connection that died while it was checked out comes back into the pool looking exactly like a good one.
///
/// This is the single decision point, and it is the function the unit tests below exercise: the rule in force and the
/// rule under test cannot drift apart.
fn probe_needed(idle_for: Duration, idle_probe: Duration, suspect: bool) -> bool {
    idle_for >= idle_probe || suspect
}

/// Millis since this process started, from a monotonic clock.
fn now_ms() -> u64 {
    let elapsed = PROCESS_START.get_or_init(Instant::now).elapsed();
    elapsed.as_millis().min(u64::MAX as u128) as u64
}

/// How long the pool verifies every connection it hands out after a connection-class failure. 0 disables it.
///
/// Resolved once: this is a deployment setting, not a per-checkout value, and it is read on the hot path.
fn recheck_window_ms() -> u64 {
    static WINDOW: OnceLock<u64> = OnceLock::new();
    *WINDOW.get_or_init(|| non_negative_u64("FORGE_DB_RECHECK_MS", 60_000))
}

/// The end of the verification window opened at `now_ms`, saturating rather than wrapping.
fn suspect_until_ms(now_ms: u64, window_ms: u64) -> u64 {
    now_ms.saturating_add(window_ms)
}

/// Whether a connection-class failure has happened recently enough that the pool should not trust what it holds.
fn suspect_window_open() -> bool {
    SUSPECT_UNTIL_MS.load(Ordering::Relaxed) > now_ms()
}

/// Record that a failure belonged to the CONNECTION, not to the statement, so the pool verifies what it hands out.
///
/// Called from `DbFailure::from_sqlx` for every failure classified `DatabaseUnavailable`: a broken pipe, a socket
/// error, a session the server terminated (`25P03`), a pooler that closed the backend. Nothing else calls it. A
/// constraint violation or a schema mismatch says nothing about the health of the pool, and marking the pool suspect
/// for those would put a round trip on every checkout for no reason.
///
/// The window EXTENDS while failures keep arriving and closes by itself once they stop, so a database that is genuinely
/// flapping pays one ping per checkout and a database that hiccuped once pays it for a minute.
pub(crate) fn note_connection_failure() {
    let window_ms = recheck_window_ms();
    if window_ms == 0 {
        return;
    }
    // `fetch_max`: two failures a moment apart hold the window open to the later end, and a failure can never pull it
    // earlier than the one before it left it.
    SUSPECT_UNTIL_MS.fetch_max(suspect_until_ms(now_ms(), window_ms), Ordering::Relaxed);
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

    /// The two window tests below poke PROCESS-GLOBAL state, so they must not run at the same time: the harness runs
    /// tests on threads, and one test restoring the window would otherwise clear the window another test just opened.
    /// (Found by these tests failing, 2026-09-29.)
    static WINDOW_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    /// The rule this guards (AGENTS.md, "Never run Forge against DEV"): Forge's database is production, and a
    /// declared development environment is refused rather than honoured. Silence is refused too — by the
    /// declaration it builds on — so a Forge process cannot reach any target by saying nothing.
    #[test]
    fn forge_refuses_everything_but_production() {
        assert_eq!(
            resolve_forge_target(None, Some("production")).unwrap(),
            DbTarget::Prod
        );
        assert_eq!(
            resolve_forge_target(Some("production"), None).unwrap(),
            DbTarget::Prod
        );
        assert_eq!(
            resolve_forge_target(Some("production"), Some("prod")).unwrap(),
            DbTarget::Prod
        );
        assert!(resolve_forge_target(None, Some("development")).is_err());
        assert!(resolve_forge_target(None, Some("dev")).is_err());
        assert!(resolve_forge_target(Some("preview"), Some("prod")).is_err());
        assert!(resolve_forge_target(None, None).is_err());

        let message = resolve_forge_target(None, Some("dev"))
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("PRODUCTION only"),
            "the refusal must name the rule it enforces: {message}"
        );
    }

    #[test]
    fn ssl_modes_are_pinned_to_verify_full() {
        assert_eq!(
            normalize_ssl_mode("postgres://x/db?sslmode=require"),
            "postgres://x/db?sslmode=verify-full"
        );
    }

    /// THE SIZING IS A DECISION, SO IT IS PINNED (captain, 2026-09-29). Production holds twenty connections open and
    /// ready — the engine and the application each hold their own twenty, because they are separate processes with
    /// separate pools — inside a ceiling of thirty. DEV stays small on purpose.
    #[test]
    fn production_keeps_twenty_hot_connections_inside_a_ceiling_of_thirty() {
        assert_eq!(default_pool_bounds(DbTarget::Prod), (20, 30));
        assert_eq!(default_pool_bounds(DbTarget::Dev), (1, 5));

        // A floor above the ceiling is silently clamped by `min_connections`, so the two have to stay ordered or the
        // floor stops meaning what it says. This is the assertion that fails when someone raises one and forgets the
        // other.
        let (min, max) = default_pool_bounds(DbTarget::Prod);
        assert!(
            min <= max,
            "the floor ({min}) cannot exceed the ceiling ({max})"
        );
    }

    /// THE WARM-UP IS BOUNDED BY THE CEILING, AND THAT IS THE ASSERTION THAT MATTERS (2026-09-29).
    ///
    /// The pool is grown at start-up by exactly two things: the bounded checkout (one connection) and the background
    /// warm-up. If the warm-up asked for more than the ceiling holds, start-up would demand connections the pool cannot
    /// grant — and the failure would look like the starvation this file already spent the day repairing, not like a
    /// sizing mistake.
    #[test]
    fn the_background_warm_up_never_asks_for_more_than_the_ceiling_holds() {
        // The production decision, in full: one checkout plus nineteen openers is the twenty hot connections.
        assert_eq!(warm_openers(20, 30), 19);
        // Nothing to warm: the case every test process and one-shot CLI runs, and the reason `min=0` spawns nothing.
        assert_eq!(warm_openers(1, 5), 0);
        assert_eq!(warm_openers(0, 0), 0);
        // A full pool: the checkout holds one, so the warm-up may claim every remaining slot and no more.
        assert_eq!(warm_openers(30, 30), 29);
        // A floor above the ceiling is clamped by sqlx, so the warm-up clamps the same way rather than disagreeing
        // about the pool's size.
        assert_eq!(warm_openers(5, 2), 1);
    }

    /// window after any connection-class failure — and NOT otherwise, because a round trip on every checkout is the
    /// cost this design exists to avoid.
    #[test]
    fn a_connection_is_probed_when_it_is_idle_or_when_the_pool_is_suspect() {
        let idle_probe = Duration::from_millis(30_000);
        let warm = Duration::from_millis(5);
        let stale = Duration::from_millis(31_000);

        // Nothing has failed and this connection was just used: hand it over without a round trip.
        assert!(!probe_needed(warm, idle_probe, false));
        // Nothing has failed, but it has been sitting long enough for the pooler to have dropped it.
        assert!(probe_needed(stale, idle_probe, false));
        // A failure happened a moment ago: even a connection returned milliseconds ago is verified, because the pool
        // cannot tell which socket broke. When that window closes this goes back to false — see
        // `a_connection_failure_opens_the_verification_window`, which is where the window's end is asserted.
        assert!(probe_needed(warm, idle_probe, true));
    }

    /// The window arithmetic must not wrap. A saturating add on a monotonic clock is what stops a long-running
    /// process from computing an end time in the past and quietly turning the verification off.
    #[test]
    fn the_suspect_window_saturates_instead_of_wrapping() {
        assert_eq!(suspect_until_ms(1_000, 60_000), 61_000);
        assert_eq!(suspect_until_ms(u64::MAX, 60_000), u64::MAX);
    }

    /// A connection-class failure opens the window. This test pokes the static deliberately — a unit test has no pool
    /// to break — and restores it, so the two tests above stay pure.
    #[test]
    fn a_connection_failure_opens_the_verification_window() {
        if recheck_window_ms() == 0 {
            // `FORGE_DB_RECHECK_MS=0` disables the repair on purpose: there is nothing to assert.
            return;
        }
        let _guard = WINDOW_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = SUSPECT_UNTIL_MS.swap(0, Ordering::Relaxed);
        assert!(!suspect_window_open(), "the test needs the window closed");

        note_connection_failure();
        assert!(
            suspect_window_open(),
            "a broken socket must make the pool verify what it hands out next"
        );
        assert!(
            SUSPECT_UNTIL_MS.load(Ordering::Relaxed) <= now_ms().saturating_add(recheck_window_ms()),
            "the window must end when the configured interval passes, not later"
        );

        SUSPECT_UNTIL_MS.store(previous, Ordering::Relaxed);
    }

    /// THE WIRING, NOT JUST THE RULE. A driver failure the taxonomy calls a connection failure has to open the window,
    /// because that call is the only thing connecting "a socket broke" to "the pool verifies what it hands out". The
    /// error constructed here is the one that killed the run on 2026-09-29 — `sqlx::Error::Io` with `BrokenPipe`, whose
    /// Display is exactly `error communicating with database: Broken pipe (os error 32)`.
    #[test]
    fn a_broken_pipe_from_the_driver_opens_the_verification_window() {
        if recheck_window_ms() == 0 {
            return;
        }
        let _guard = WINDOW_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = SUSPECT_UNTIL_MS.swap(0, Ordering::Relaxed);

        let error = sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "Broken pipe (os error 32)",
        ));
        let failure = crate::DbFailure::from_sqlx("workflow.step", &error);
        assert_eq!(
            failure.kind,
            crate::DbFailureKind::DatabaseUnavailable,
            "a broken pipe is the connection's failure, not the statement's"
        );
        assert!(
            failure.retryable,
            "and it is retryable, which is what lets a step be repeated"
        );
        assert!(
            suspect_window_open(),
            "a real driver failure must make the pool verify the connections it hands out"
        );

        SUSPECT_UNTIL_MS.store(previous, Ordering::Relaxed);
    }
}

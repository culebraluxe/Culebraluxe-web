# Plan B: the other Rust Postgres stack

Recorded so it does not have to be re-derived. This is not a recommendation to switch. It is what the alternative
actually is, what it would cost, and what it would and would not buy us.

## The two stacks

```
what we run                      the alternative
--------------                   -------------------------
sqlx 0.9                         tokio-postgres 0.7
  driver + pool in one             driver
  sqlx::PgPool                     + deadpool 0.13
                                     pool
                                     + deadpool-postgres 0.14
                                       manager (wires the two)
```

They are alternatives, not layers. Deadpool is a pool that needs a manager to create and recycle connections, and
`deadpool-postgres` is that manager for `tokio-postgres` connections. It cannot sit behind `sqlx::PgPool`; sqlx manages
its own connections. **So "try deadpool" is not a pool experiment, it is a driver rewrite.**

The evidence for that, from this repository:

| what would have to change | count |
| --- | --- |
| `sqlx::query_as` / `query_scalar` call sites | **162** |
| files deriving `FromRow` for typed rows | **19** |
| files owning the pool policy (the swappable part) | **6** |
| uses of sqlx's migrator (nothing to lose) | 0 |
| uses of sqlx compile-time query macros (nothing to lose) | 0 |

The pool *policy* is centralized in six files. The *driver usage* is spread across 162 call sites and 19 row mappings.
That asymmetry is the whole cost question.

## Defaults, side by side

Both crates' defaults matter more than their features, because a default is what you run with before you know better.

| | sqlx `PgPool` | deadpool + tokio-postgres | which is right for us |
| --- | --- | --- | --- |
| Check on checkout | **pings: `write_sync` + `wait_until_ready`** - a full round trip | `Fast` - `is_closed()` only, **no round trip** | deadpool's default |
| Acquire timeout | 30s | **none** - a request can wait forever | sqlx's default |
| Queue order | idle-first (freshest connection) | **Fifo** - the longest-idle connection goes out first | sqlx's default |
| Max size | 10 | `cpu_core_count * 2` | neither; we set 5 deliberately for the pooler |

Sources: [sqlx `PoolOptions`](https://docs.rs/sqlx/latest/sqlx/pool/struct.PoolOptions.html),
[deadpool `PoolConfig`](https://docs.rs/deadpool/latest/deadpool/managed/struct.PoolConfig.html),
[deadpool-postgres `RecyclingMethod`](https://docs.rs/deadpool-postgres/latest/deadpool_postgres/enum.RecyclingMethod.html).

Note the pattern: **each crate defaults to the opposite of the other on the one setting that bit us.** sqlx verifies
every checkout; deadpool verifies none. Both defaults are wrong for a database across a network - sqlx pays a round trip
it does not need, deadpool hands out the stalest connection in the pool without looking at it, and Fifo makes that
worse. Neither crate ships "verify only if it has been idle a while", which is what we now do with
`test_before_acquire(false)` plus a `before_acquire` callback that probes only a connection idle beyond
`FORGE_DB_IDLE_PROBE_MS`.

## What deadpool genuinely offers

Worth knowing, and mostly things sqlx either has or we have already built:

- **`RecyclingMethod::Clean`** - on recycle, runs `CLOSE ALL; SET SESSION AUTHORIZATION DEFAULT; RESET ALL; UNLISTEN *;
  SELECT pg_advisory_unlock_all(); DISCARD TEMP; DISCARD SEQUENCES;`. It deliberately avoids `DEALLOCATE ALL` and
  `DISCARD PLAN` so as not to empty the statement cache. This is the most interesting thing in the crate: a session
  hygiene policy with a real opinion about prepared statements. sqlx has no equivalent.
- **`Metrics`** - `created`, `recycled`, `recycle_count`, `age()`, `last_used()`, handed to `Manager::recycle`. Built-in
  per-connection telemetry. Comparable to sqlx's `PoolConnectionMetadata { age, idle_for }`, which is what our counter
  distinguishing new from reused connections is built on - so this is nice, not decisive.
- **`QueueMode::Lifo`** - hands out the most recently used connection. That is what sqlx does implicitly; deadpool makes
  it explicit and configurable.
- **`Manager::statement_caches`** - statement caches managed across pooled clients.
- **Runtime-agnostic** (`deadpool-runtime`: tokio, async-std, smol). We are tokio throughout, so this buys nothing.

## What a swap would have to preserve

This is the part that matters, because these are the bugs fixed in this session. A different pool that reintroduces any
of them would be a regression dressed as progress.

1. **One pool per process.** Ours is `db::shared`, installed by the composition root. The engine once built a pool *per
   call*; nothing about deadpool prevents that mistake, it just moves where it lives.
2. **No verification round trip on every checkout.** `Fast` in deadpool, `test_before_acquire(false)` in sqlx. If
   verification is turned on, pair it with Lifo, not Fifo.
3. **A warm floor.** At least one connection kept open, or the next request pays 498ms for a handshake.
4. **No named prepared statements through the pooler.** Both `DATABASE_URL_DEV` and `DATABASE_URL` are `-pooler` hosts
   (PgBouncer, transaction mode). sqlx needs `statement_cache_capacity(0)`; deadpool needs its statement cache left
   empty. `Clean` is designed to preserve a statement cache, which is exactly the cache we cannot use.
5. **An acquire timeout.** deadpool's default is none.
6. **Failures must reach the capture hook.** `db::Database` is where the retry policy, the `app_error` sink and the
   telemetry hooks live. A swap re-wires all of that, and losing it means failures go back to being invisible.
7. **The counters.** `GET /v1/diagnostics/db`. Without them "the pool is cold" and "the database is slow" look identical
   from outside, which is how this took so long to find in the first place.

## Measured, both stacks, same machine

Rather than argue, a throwaway crate outside the repository (`/tmp/rustpool/bench`, sqlx and deadpool side by side) ran
the same statements through both, against the same database, in the same minute. The bench reads the connection string
from the environment and never prints it.

| | sqlx `PgPool` | tokio-postgres + deadpool |
| --- | --- | --- |
| `select 1` on a held connection | **79.6ms** | 176.6ms |
| `begin`/`select`/`commit` | **239ms** | 331ms |
| `deadpool pool.get()` alone | - | **625 nanoseconds** |
| pool size at the end | 3 | 2 |

Readings, in order of how much they matter:

1. **The pool is not the lever.** deadpool's checkout is 625ns, and its source confirms why: with the default
   `RecyclingMethod::Fast`, `recycle` runs `is_closed()` and nothing else - `config.recycling_method.query()` returns
   `None`, so no round trip. That is the same policy we now get from `test_before_acquire(false)`.
2. **The driver difference is prepared statements.** sqlx caches them, so a query is one round trip; tokio-postgres
   prepares on every call, so it is two. sqlx is therefore about twice as fast per statement here, and switching would
   make every query slower, not faster, in exchange for rewriting 162 call sites.
3. **The exercise paid for itself by finding our regression.** The benchmark measured the same statement with the
   statement cache on and off - 79.6ms versus 160.0ms - which is what revealed that disabling the cache was doubling
   the latency of every query in the application. That change is reverted.

**Verdict: keep sqlx.** Not because the alternative is bad, but because it is slower here, costs a driver rewrite, and
the one genuinely interesting thing in it (`RecyclingMethod::Clean`, session hygiene that preserves a statement cache)
is redundant for us: a transaction-mode pooler already resets session state between transactions, which is
PgBouncer's job, not ours.


Keep sqlx unless a benchmark shows **sqlx's driver**, not the pool, is the bottleneck. Given the measurements, that is
unlikely:

- The reclaim statement costs **0.058ms** server-side. The database is not doing the work.
- 25 checkouts resolve to **1** connection opened and **0** idle probes. The pool is not doing anything wrong.
- A page load is now **2** database round trips, and repeated requests are **0** identity round trips.

What is left is the wire and the number of round trips. **No pool implementation can remove a round trip.** If a page
feels slow while `connectionReuseRate` is high and `connectionsOpened` is flat, the remaining time is the link or the
number of requests - and the fix is fewer, larger requests (the aggregated workspace endpoint), not a different pool.

If the experiment gets run anyway, it should not be a rewrite. The cheap version: a scratch binary in this workspace
running the same N queries through `deadpool-postgres` and printing per-query latency, compared against the same N
through `PgPool` with the counters above. That answers "is the driver the bottleneck" in an afternoon, without touching
the 162 call sites.

# pool-bench (experiment, not production code)

Answers one question: **is the database driver the bottleneck, or the pool?**

It runs the same statements through `sqlx::PgPool` and through `tokio-postgres` + `deadpool-postgres`, against the same
database, on the same machine, in the same minute.

```
bash rust/experiments/pool-bench/run.sh
```

Reads `DATABASE_URL_DEV` from `.env.local` and never prints it.

## Why it is outside the workspace

It depends on `tokio-postgres` and `deadpool-postgres`, which production does not use and should not start using by
accident. `rust/Cargo.toml` excludes this directory, so `cargo check --workspace` and the test suite measure shipped code
only. It has its own target directory for the same reason.

## What it found (2026-09-21)

| | sqlx | tokio-postgres + deadpool |
| --- | --- | --- |
| `select 1`, warm connection | **79.6ms** | 176.6ms |
| `begin`/`select`/`commit` | **239ms** | 331ms |
| pool checkout alone | - | **625ns** |
| statement cache on / off (sqlx) | 79.6ms / **160.0ms** | - |

1. The **pool is not the lever** - a checkout is 625ns, and deadpool's `Fast` recycling only calls `is_closed()`.
2. sqlx is about **2x faster per statement** because it caches prepared statements, so a query is one round trip where
   tokio-postgres takes two.
3. The cache row is the one that mattered: it showed that disabling sqlx's statement cache (done for the transaction-mode
   pooler, on a correct-but-inapplicable theory) was **doubling every query's latency** in production. That change was
   reverted.

Full write-up and the invariants any future swap must preserve: `docs/rust-dbpool-plan-b.md`.

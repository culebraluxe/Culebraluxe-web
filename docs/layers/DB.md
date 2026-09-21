# Layer: DB

Crate: **`rust/core/db`** + migrations in **`legacy/db/migrations`**. **Owns:** the one pool, the DAOs, retry, the failure
taxonomy, and the writing of failures to `app_error`.

## One pool per process

`db::shared` holds it; the composition root installs it at boot (`server/src/bin/http.rs`). Everything else asks for it:
the server, the workflow engine's store, the Forge session helper. A `Database` is a cheap handle to that pool — **never
build one per call.** (An engine command once did: 2368ms each.)

Set by the environment, and it **fails closed**: `VERCEL_ENV=production` or `APP_ENV=production` selects PROD,
`preview`/`development` selects DEV, anything ambiguous refuses to start. The boot line and `GET /v1/diagnostics/db`
report the target. **Check it before believing a command is on dev.**

## Pool policy (all measured, all deliberate)

| setting | value | why |
| --- | --- | --- |
| `FORGE_DB_POOL_MIN` | 1 | a cold connect is 498ms; keep one warm |
| checkout ping | **off** | sqlx pings by default and that is a full round trip; probe only when idle ≥ `FORGE_DB_IDLE_PROBE_MS` (30s) |
| `FORGE_DB_POOL_IDLE_MS` | 60s | reclaims only connections above the floor |
| statement cache | **on** | turning it off doubles every query (80ms). It works through the pooler; do not disable it |
| retry | `fork` for retryable kinds only | exponential + jitter; **never** retry an unguarded write |
| keepalive | 60s | stops Neon suspending under a user; `0` disables |

## What lives here

- **DAOs**, one per area (`client.rs`, `project.rs`, …), with `FromRow` structs and `DbFailure::from_sqlx("area.op", …)`.
- **`retrying_read!`** — the macro that wraps read paths (a macro because the operation borrows the DAO mutably and the
  loop re-borrows it). Reads only; writes are not retried without a receipt.
- **`DbFailure`** — the taxonomy: `DatabaseUnavailable`, `SchemaMismatch`, `Constraint`, `Timeout`, each with
  `retryable` and an `incident_id`. Constructing one **announces** it, which is how every failure reaches `app_error`
  without the operation knowing.
- **`metrics`** — checkouts, connections opened, idle probes, reuse rate. Surfaced at `/v1/diagnostics/db`.

## Migrations

Numbered files in `legacy/db/migrations`, applied **explicitly per target** — never by the deploy. `pnpm db:migrations` shows
per-target state.

⚠ **Dev and prod schemas currently differ**: 34 migrations are applied to only one environment. Reconcile in DEV before a
release. Nothing in the Rust paths touches the prod-only tables today, so nothing is broken — but "works in dev" is not
proof for prod.

## Two things to get right

1. **Bind typing.** A quoted SQL literal is untyped and Postgres coerces it; a bind is typed `text`. A `uuid` column needs
   `$1::uuid`; a `text` column must **not** get one. Read the migration.
2. **Verify a write path inside a rolled-back transaction.** `BEGIN`, run the real statement with real binds, read the
   result back, `ROLLBACK`. It validates SQL, types and semantics without changing a row.

## Read the code

`rust/core/db/src/pool.rs` (policy + metrics), `retry.rs` (and its tests), `error.rs` (the taxonomy), `capture.rs` (the
announce path), `shared.rs` (the one pool), then any DAO. Live verification:
`scripts/rust-live-check/`.

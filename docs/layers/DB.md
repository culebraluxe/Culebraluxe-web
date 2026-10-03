# Layer: DB

Crate: **`db`** + migrations in **`db/migrations`**. **Owns:** the one pool, the DAOs, retry, the failure
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
| `FORGE_DB_POOL_MIN` | 20 | the floor: connections open and ready, so work pays a round trip (~72ms) and not a handshake (498ms). The engine and the app are separate processes with separate pools, so each holds its own twenty. **Opened in the background after the pool is built, never before it can be used**: sqlx's warm-up is serial, and twenty of them (measured 2.3s at min=2, 4.8s at min=5, 9.3s at min=10) exceed the 10s connect budget — so the pool opens lazily and warms concurrently (`core/db/src/pool.rs`) |
| `FORGE_DB_POOL_MAX` | 30 | the ceiling on simultaneous connections, leaving the floor room to grow with load |
| checkout ping | **off** | sqlx pings by default and that is a full round trip; probe when idle ≥ `FORGE_DB_IDLE_PROBE_MS` (30s), and for `FORGE_DB_RECHECK_MS` (60s) after any connection-class failure |
| `FORGE_DB_POOL_MAX_LIFETIME_MS` | 30min | retire a connection by age and replace it; the pooler retires server connections on its own schedule, and an old socket is the likeliest one to be half-dead |
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

Numbered files in `db/migrations`, applied **explicitly per target** — never by the deploy — with
`pnpm db:migrate <file> dev|prod` (`cli db-tool apply`, recorded in the `schema_migration` ledger). Run it from the repo
root: the CLI reads `.env.local` there.

Two gates, and they answer different questions. `pnpm db:migrations` reads the **ledger** (what was recorded where); it
reports many one-target rows because a DEV refresh from PROD replaces DEV's ledger, so read it as history, not drift.
`pnpm db:parity` compares the **live schemas** and is the release gate.

**2026-09-28: `PARITY OK`,** after PROD received `217_website_intake_notified_at`, `219_guest_sign_in_code` and
`223_command_receipt_runtime` — three DEV migrations that code on `main` already used (the website lead emails, guest
sign-in codes, command receipts). A migration that reached only DEV is the usual way a feature "works in dev". Re-run
`pnpm db:parity` before trusting this line.

## Two things to get right

1. **Bind typing.** A quoted SQL literal is untyped and Postgres coerces it; a bind is typed `text`. A `uuid` column needs
   `$1::uuid`; a `text` column must **not** get one. Read the migration.
2. **Verify a write path inside a rolled-back transaction.** `BEGIN`, run the real statement with real binds, read the
   result back, `ROLLBACK`. It validates SQL, types and semantics without changing a row.

## Read the code

`db/src/pool.rs` (policy + metrics), `retry.rs` (and its tests), `error.rs` (the taxonomy), `capture.rs` (the
announce path), `shared.rs` (the one pool), then any DAO. Live verification:
`scripts/rust-live-check/`.

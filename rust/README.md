# Rust workspace

This directory is the side-by-side Rust backend workspace for CulebraLuxe.

Nothing here is wired into Next.js, Vercel production routing, or the TypeScript
runtime yet. Rust is being introduced behind explicit parity boundaries.

## Structure

- `core/domain` — infrastructure-free domain types and rules.
- `core/db` — the one Rust PostgreSQL pool plus DAOs and DB failure normalization.
- `core/workflow` — workflow engine primitives.
- `core/auth` — server-side authorization/authentication boundary.
- `forge` — Forge runtime and SDLC roles.
- `server` — authoritative CulebraLuxe service/API layer.
- `integrations` — external-system adapters.
- `cli` — operational command-line entry point.

The existing repository-level `db/migrations/` remains the canonical SQL
migration history. Do not create a competing Rust migration tree.

Dependency direction is inward toward `core/domain`. Infrastructure and
integration concerns must not leak into domain code.

Forge role ownership is preserved structurally. QA verifies outcomes and does
not own Git/release authority; DEV_OPS/release owns release identity and
promotion concerns.

## Slice 1: DB + Project service

The first real vertical slice uses the existing Neon `project` table:

```text
ProjectService
    -> ProjectRepository
        -> ProjectDao
            -> Database / SQLx PgPool
                -> Neon PostgreSQL
```

`Database` owns the only Rust PostgreSQL pool. It follows the existing
application environment contract:

- `VERCEL_ENV=production` -> `DATABASE_URL_PROD`
- `VERCEL_ENV=preview|development` -> `DATABASE_URL_DEV`
- otherwise `APP_ENV=production|prod` -> PROD
- otherwise `APP_ENV=development|dev|test|testing` -> DEV
- silence is refused; there is no implicit environment fallback.

Connection URLs using `sslmode=prefer|require|verify-ca` are pinned to
`verify-full`, matching the current TypeScript ForgeDB intent. Query ceilings
remain database defaults; the Rust client does not inject startup `options`
that Neon's pooled endpoint rejects.

### Compile and test

From `rust/`:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo check --workspace --all-targets
```

### Read-only DEV smoke

With the existing DEV environment loaded:

```bash
APP_ENV=dev cargo run -p cli -- db-smoke
```

The smoke command connects only to the explicitly declared target, runs
`select 1`, then calls the Rust `ProjectService.list()` path. It prints only
the target, project count, and non-sensitive project identity/status metadata.
It performs no writes and never prints a database URL or credential.

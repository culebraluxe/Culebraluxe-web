# Skill: neon

- Canonical data lives in Postgres on Neon. Do not invent a parallel store.
- Migrations live in `legacy/db/migrations` and must be numbered.
- Repository modules own driver-value normalization.
- Prefer a Neon *branch* for dangerous schema experiments. Never reset PROD.
- WhatsApp is a channel, not an identity type. Phones are strict E.164.
- A story that touches schema is not done until DEV and PROD match the code.
- Install Neon vendor skills for CLI/branch syntax if needed; do not spawn a Neon-hosted agent as Smith.

## Anchored to
- `legacy/db/database-gateway.ts` — the one place DB access, normalization and failure capture happen.
- `rust/core/db/src/schema_parity.rs` — the DEV/PROD comparison this pack says a schema story owes,
  run as `pnpm db:parity`; its DB reader is `read_snapshot`, and the ledger side lives in
  `rust/core/db/src/schema_migration.rs`.
- `legacy/db/migrations/179_forge_kind_policy.sql` — a numbered migration in the shape this pack requires.


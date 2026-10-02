---
name: database-migration
description: Use when adding, editing or applying a SQL migration - numbering, the two migration roots, and the apply, status and scan gates
---

# Skill: database-migration

There are **two** migration roots. They are not the same series:

- `db/migrations/` — the control plane. Three digits, zero-padded, named for what changed:
  `259_forge_declared_work_type.sql`, `261_docsign_entitlements.sql`. This is what `pnpm db:migrate` applies.
- `workflow_engine/scripts/migrations/` — the engine's own numbered series (`001_...`, `002_...`). Never continue
  one series' numbering inside the other.

Commands:

- `pnpm db:migrate` → `cli db-tool apply`
- `pnpm db:migrations` → `cli db-tool status` — read it before and after; an unapplied file reads as clean
- `pnpm scan:migrations` → `scripts/scan-migrations.sh`

Conventions the existing files hold to:

- Wrap in `begin; ... commit;`.
- Catalog and seed inserts are idempotent: `insert ... on conflict (<key>) do update set ...` (`261` is the model).
- An applied migration is never renumbered or edited into a different meaning. A correction is a new file.
- A seed whose rows are mirrored in Rust (entitlements, catalogs) must land with the Rust change in the same
  commit. Two sources that disagree are worse than one that is late.

## Anchored to
- `db/migrations/261_docsign_entitlements.sql` — the idempotent catalog-seed pattern.
- `package.json` — `db:migrate`, `db:migrations`, `scan:migrations`.

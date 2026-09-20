# Rust workspace

This directory is the side-by-side Rust backend workspace for CulebraLuxe.

It is intentionally inert at introduction: nothing here is wired into Next.js,
Vercel, production routing, or database migrations yet.

## Structure

- `core/domain` — infrastructure-free domain types and rules.
- `core/db` — database boundary and repository infrastructure.
- `core/workflow` — workflow engine primitives.
- `core/auth` — server-side authorization/authentication boundary.
- `forge` — Forge runtime and SDLC roles.
- `server` — authoritative CulebraLuxe server/API.
- `integrations` — external-system adapters.
- `cli` — operational command-line entry point.

The existing repository-level `db/migrations/` remains the canonical SQL
migration history. Do not create a competing Rust migration tree.

Dependency direction is inward toward `core/domain`. Infrastructure and
integration concerns must not leak into domain code.

Forge role ownership is preserved structurally. QA verifies outcomes and does
not own Git/release authority; DEV_OPS/release owns release identity and
promotion concerns.
